//! Batch compilation: temporary tables assembled as common table expressions.

use std::collections::BTreeSet;

use super::orchestrate::{PresentationCompilation, compile_query_ast};
use crate::Token;
use crate::metadata::MetadataSnapshot;
use crate::query::core::ast::{
    BatchAst, Expression, IndexAst, IntoAst, Projection, QueryAst, StatementAst,
};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery, RestrictionState,
};
use crate::query::core::temp_tables::{TempTablesManager, cte_name};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// The result of one compiled statement before the `WITH` prefix is added.
struct CompiledStatement {
    query: CompiledQuery,
    dependencies: BTreeSet<u32>,
    /// The recursive CTEs of `В ИЕРАРХИИ` a defining statement leaves to
    /// the manager; a final statement attaches its own.
    hierarchy: Vec<(String, String)>,
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
                    presentations.mode.is_restricted() || query.allowed.is_some(),
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
    catalog.set_restrictions(RestrictionState {
        restrictions: presentations.restrictions,
        decisions: presentations.decisions,
        mode: presentations.mode,
        keyword: query.allowed.is_some(),
        collecting: presentations.collecting,
    });
    let compiled = compile_query_ast(
        query,
        snapshot,
        &catalog,
        presentations,
        into.map(|into| into.token),
    )?;
    // A defining statement cannot open a `WITH` of its own inside the
    // table's CTE, so its hierarchy CTEs travel with the definition.
    let hierarchy = if into.is_some() {
        catalog.take_hierarchy_ctes()
    } else {
        Vec::new()
    };
    presentations
        .restriction_targets
        .extend(catalog.restriction_targets());
    presentations
        .used_restrictions
        .extend(catalog.used_restrictions());
    presentations
        .used_decisions
        .extend(catalog.used_decisions());
    for used in catalog.field_usage() {
        if !presentations.field_usage.contains(&used) {
            presentations.field_usage.push(used);
        }
    }
    if let Some(index) = &query.index {
        check_index_fields(index, query, &compiled.columns)?;
    }
    Ok(CompiledStatement {
        query: compiled,
        dependencies: catalog.used_temporary(),
        hierarchy,
    })
}

/// Stores a definition, or renders the final statement of the batch.
#[allow(clippy::too_many_arguments)]
fn place_statement(
    query: &QueryAst<'_, '_>,
    compiled: CompiledStatement,
    manager: &mut TempTablesManager,
    dialect: SqlDialect,
    final_statement: bool,
    snapshot: &MetadataSnapshot,
    restricted: bool,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let fingerprint = snapshot.fingerprint();
    let Some(into) = &query.into else {
        if !final_statement {
            return Ok(None);
        }
        let CompiledStatement {
            query,
            dependencies,
            hierarchy: _,
        } = compiled;
        return Ok(Some(CompiledQuery {
            sql: join_with_prefix(with_prefix(manager, &dependencies, dialect), &query.sql),
            columns: query.columns,
            deferred_presentations: query.deferred_presentations,
            nested: query.nested,
            service_columns: query.service_columns,
        }));
    };
    let CompiledStatement {
        query: statement,
        dependencies,
        hierarchy,
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
            hierarchy,
            dialect,
            fingerprint,
            restricted,
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
        hierarchy,
        dialect,
        fingerprint,
        restricted,
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
        nested: Vec::new(),
        service_columns: Vec::new(),
    }
}

/// Renders the `WITH` list of the CTEs a statement reaches, in definition
/// order so that every CTE precedes the ones reading it.
fn with_prefix(
    manager: &TempTablesManager,
    dependencies: &BTreeSet<u32>,
    dialect: SqlDialect,
) -> String {
    let mut recursive = false;
    let definitions = dependencies
        .iter()
        .filter_map(|id| manager.entry(*id))
        .flat_map(|entry| {
            recursive |= !entry.ctes.is_empty();
            entry
                .ctes
                .iter()
                .map(|(name, sql)| format!("{} AS ({sql})", dialect.quote_identifier(name)))
                .chain(std::iter::once(format!(
                    "{} AS ({})",
                    dialect.quote_identifier(&cte_name(entry.id)),
                    entry.sql
                )))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if definitions.is_empty() {
        return String::new();
    }
    let keyword = if recursive && dialect == SqlDialect::Postgres {
        "WITH RECURSIVE"
    } else {
        "WITH"
    };
    format!("{keyword} {} ", definitions.join(", "))
}

/// Prepends the temporary-table `WITH` list to a statement, merging the
/// lists when the statement (totals) brings its own CTE.
fn join_with_prefix(prefix: String, sql: &str) -> String {
    if prefix.is_empty() {
        return sql.to_owned();
    }
    let prefix = prefix.trim_end();
    let (definitions, recursive) = match prefix.strip_prefix("WITH RECURSIVE ") {
        Some(definitions) => (definitions, true),
        None => (
            prefix
                .strip_prefix("WITH ")
                .expect("the temporary-table prefix starts with WITH"),
            false,
        ),
    };
    if let Some(rest) = sql.strip_prefix("WITH RECURSIVE ") {
        return format!("WITH RECURSIVE {definitions}, {rest}");
    }
    let keyword = if recursive { "WITH RECURSIVE" } else { "WITH" };
    match sql.strip_prefix("WITH ") {
        Some(rest) => format!("{keyword} {definitions}, {rest}"),
        None => format!("{keyword} {definitions} {sql}"),
    }
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
/// An index field names a selection-list column: by its label, by the
/// alias the text gave it when the label had to be truncated, or — when
/// qualified — by the projected field path itself, as the platform
/// accepts `ИНДЕКСИРОВАТЬ ПО Т.Поле` for `Т.Поле КАК Иное`.
fn check_index_fields(
    index: &IndexAst<'_, '_>,
    query: &QueryAst<'_, '_>,
    columns: &[CompiledColumn],
) -> Result<(), QueryDiagnostic> {
    let projected_paths = query
        .branches
        .first()
        .map(|branch| {
            branch
                .projection
                .iter()
                .filter_map(|item| match &item.expression {
                    Projection::Field(reference)
                    | Projection::Scalar(Expression::Field(reference)) => Some(&reference.segments),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for field in index.sets.iter().flatten() {
        let label = field.label();
        let named = columns.iter().any(|column| {
            names_equal(&column.label, label.lexeme) || names_equal(&column.name, label.lexeme)
        });
        let projected = field.segments.len() > 1
            && projected_paths.iter().any(|segments| {
                segments.len() == field.segments.len()
                    && segments
                        .iter()
                        .zip(&field.segments)
                        .all(|(projected, named)| names_equal(projected.lexeme, named.lexeme))
            });
        if !named && !projected {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(label),
                format!(
                    "index field {:?} is not in the selection list",
                    label.lexeme
                ),
            ));
        }
    }
    Ok(())
}
