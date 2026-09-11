//! Batch compilation: temporary tables assembled as common table expressions.

use std::collections::BTreeSet;

use super::orchestrate::{PresentationCompilation, compile_query_ast};
use crate::Token;
use crate::metadata::MetadataSnapshot;
use crate::query::core::ast::{BatchAst, IndexAst, IntoAst, QueryAst, StatementAst};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery};
use crate::query::core::temp_tables::{TempTablesManager, cte_name};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// The result of one compiled statement before the `WITH` prefix is added.
struct CompiledStatement {
    query: CompiledQuery,
    dependencies: BTreeSet<u32>,
}

/// Compiles a batch against `manager`, which is updated only on success.
///
/// Returns `None` when the batch ends with `УНИЧТОЖИТЬ`, which produces no
/// rows, exactly as `Запрос.Выполнить()` returns `Неопределено` there.
pub(super) fn compile_batch_ast(
    ast: &BatchAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    presentations: &mut PresentationCompilation<'_>,
    manager: &mut TempTablesManager,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let dialect = presentations.dialect;
    let fingerprint = snapshot.fingerprint();
    manager.check_binding(dialect, fingerprint)?;
    let mut scratch = manager.clone();
    let mut result = None;
    let last = ast.statements.len().saturating_sub(1);
    for (position, statement) in ast.statements.iter().enumerate() {
        let final_statement = position == last;
        match statement {
            StatementAst::Drop { name } => {
                scratch.drop_table(name)?;
                result = None;
            }
            StatementAst::Query(query) => {
                let compiled = compile_statement(
                    query,
                    snapshot,
                    presentations,
                    &scratch,
                    query.into.as_ref(),
                )?;
                result = place_statement(
                    query,
                    compiled,
                    &mut scratch,
                    dialect,
                    final_statement,
                    snapshot,
                )?;
            }
        }
    }
    *manager = scratch;
    Ok(result)
}

/// Compiles one statement with the temporary tables of `manager` visible.
/// A statement that defines a table follows the nested-query rules so that
/// its dates stay in the storage domain and its labels stay explicit.
fn compile_statement(
    query: &QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    presentations: &mut PresentationCompilation<'_>,
    manager: &TempTablesManager,
    into: Option<&IntoAst<'_, '_>>,
) -> Result<CompiledStatement, QueryDiagnostic> {
    let mut catalog =
        CompilationCatalog::with_temporary(snapshot, presentations.parameters, manager);
    catalog.set_restrictions(presentations.restrictions, query.allowed.is_some());
    let compiled = compile_query_ast(
        query,
        snapshot,
        &catalog,
        presentations,
        into.map(|into| into.token),
    )?;
    presentations
        .restriction_targets
        .extend(catalog.restriction_targets());
    presentations
        .used_restrictions
        .extend(catalog.used_restrictions());
    if let Some(index) = &query.index {
        check_index_fields(index, &compiled.columns)?;
    }
    Ok(CompiledStatement {
        query: compiled,
        dependencies: catalog.used_temporary(),
    })
}

/// Stores a definition, or renders the final statement of the batch.
fn place_statement(
    query: &QueryAst<'_, '_>,
    compiled: CompiledStatement,
    manager: &mut TempTablesManager,
    dialect: SqlDialect,
    final_statement: bool,
    snapshot: &MetadataSnapshot,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let fingerprint = snapshot.fingerprint();
    let Some(into) = &query.into else {
        if !final_statement {
            return Ok(None);
        }
        let CompiledStatement {
            query,
            dependencies,
        } = compiled;
        return Ok(Some(CompiledQuery {
            sql: with_prefix(manager, &dependencies, dialect) + &query.sql,
            columns: query.columns,
            deferred_presentations: query.deferred_presentations,
        }));
    };
    let CompiledStatement {
        query: statement,
        dependencies,
    } = compiled;
    if into.append {
        let previous = manager.source(into.name)?;
        check_append_structure(&previous.columns, &statement.columns, into.token)?;
        let mut definition = dependencies.clone();
        definition.insert(previous.id);
        definition.extend(previous.dependencies.iter().copied());
        let body = format!(
            "SELECT {} FROM {} UNION ALL {}",
            previous
                .columns
                .iter()
                .map(|column| dialect.quote_identifier(&column.label))
                .collect::<Vec<_>>()
                .join(", "),
            dialect.quote_identifier(&cte_name(previous.id)),
            statement.sql,
        );
        manager.append(
            into.name,
            body,
            previous.columns,
            definition,
            dialect,
            fingerprint,
        )?;
        // Only the appended rows are counted, so the base table is not read.
        return Ok(final_statement.then(|| {
            count_of_relation(
                format!("({})", statement.sql),
                &dependencies,
                manager,
                dialect,
            )
        }));
    }
    let id = manager.define(
        into.name,
        statement.sql,
        statement.columns,
        dependencies,
        dialect,
        fingerprint,
    )?;
    if !final_statement {
        return Ok(None);
    }
    let dependencies = manager
        .entry(id)
        .map(|entry| {
            let mut all = entry.dependencies.clone();
            all.insert(entry.id);
            all
        })
        .unwrap_or_default();
    Ok(Some(count_of_relation(
        dialect.quote_identifier(&cte_name(id)),
        &dependencies,
        manager,
        dialect,
    )))
}

/// The one-row result of a placement statement, as 1C reports it.
fn count_of_relation(
    relation: String,
    dependencies: &BTreeSet<u32>,
    manager: &TempTablesManager,
    dialect: SqlDialect,
) -> CompiledQuery {
    let label = dialect.quote_identifier("Количество");
    CompiledQuery {
        sql: format!(
            "{}SELECT COUNT(*) AS {label} FROM {relation} AS {}",
            with_prefix(manager, dependencies, dialect),
            dialect.quote_identifier("__placed"),
        ),
        columns: vec![CompiledColumn::new(
            "Количество".to_owned(),
            ColumnKind::Number {
                precision: None,
                scale: None,
            },
        )],
        deferred_presentations: Vec::new(),
    }
}

/// Renders the `WITH` list of the CTEs a statement reaches, in definition
/// order so that every CTE precedes the ones reading it.
fn with_prefix(
    manager: &TempTablesManager,
    dependencies: &BTreeSet<u32>,
    dialect: SqlDialect,
) -> String {
    let definitions = dependencies
        .iter()
        .filter_map(|id| manager.entry(*id))
        .map(|entry| {
            format!(
                "{} AS ({})",
                dialect.quote_identifier(&cte_name(entry.id)),
                entry.sql
            )
        })
        .collect::<Vec<_>>();
    if definitions.is_empty() {
        return String::new();
    }
    format!("WITH {} ", definitions.join(", "))
}

/// `ДОБАВИТЬ` accepts only rows the table can already hold: 1C reports a
/// type error instead of widening the column, so no widening happens here.
fn check_append_structure(
    target: &[CompiledColumn],
    appended: &[CompiledColumn],
    token: &Token<'_>,
) -> Result<(), QueryDiagnostic> {
    if target.len() != appended.len() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::TemporaryTable,
            Some(token),
            format!(
                "appended statement projects {} columns; the temporary table has {}",
                appended.len(),
                target.len()
            ),
        ));
    }
    for (position, (target, appended)) in target.iter().zip(appended).enumerate() {
        let compatible = target.kind.is_compatible_with(&appended.kind)
            && references_match(&target.kind, &appended.kind);
        if !compatible {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(token),
                format!(
                    "appended column {} is {:?}; the temporary table holds {:?}",
                    position + 1,
                    appended.kind,
                    target.kind
                ),
            ));
        }
    }
    Ok(())
}

/// Reference columns must also agree on their targets and width, which the
/// variant-only compatibility check does not cover.
fn references_match(target: &ColumnKind, appended: &ColumnKind) -> bool {
    match (target, appended) {
        (
            ColumnKind::Reference {
                targets,
                runtime_typed,
            },
            ColumnKind::Reference {
                targets: appended_targets,
                runtime_typed: appended_runtime_typed,
            },
        ) => targets == appended_targets && runtime_typed == appended_runtime_typed,
        _ => true,
    }
}

/// Index fields name columns of the statement; no index is generated
/// because a common table expression cannot carry one.
fn check_index_fields(
    index: &IndexAst<'_, '_>,
    columns: &[CompiledColumn],
) -> Result<(), QueryDiagnostic> {
    for field in index.sets.iter().flatten() {
        if !columns
            .iter()
            .any(|column| names_equal(&column.label, field.lexeme))
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(field),
                format!(
                    "index field {:?} is not in the selection list",
                    field.lexeme
                ),
            ));
        }
    }
    Ok(())
}
