//! Batch compilation: temporary tables assembled as common table expressions.

use std::collections::BTreeSet;

use super::orchestrate::{PresentationCompilation, compile_query_ast};
use super::select::render_parameter_table;
use crate::Token;
use crate::metadata::MetadataSnapshot;
use crate::query::core::MaterializedBatchResult;
use crate::query::core::ast::{
    BatchAst, Expression, IndexAst, IntoAst, Projection, QueryAst, StatementAst,
};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::params::{ParameterColumn, ParameterValue};
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

/// Executes each placement selection through the host before compiling the
/// next statement. The manager is committed only after the whole batch succeeds.
pub(super) fn compile_materialized_batch_ast<E, F>(
    ast: &BatchAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    presentations: &mut PresentationCompilation<'_>,
    manager: &mut TempTablesManager,
    on_placement: &mut F,
) -> Result<MaterializedBatchResult, E>
where
    E: From<QueryDiagnostic>,
    F: FnMut(&CompiledQuery) -> Result<Vec<Vec<ParameterValue>>, E>,
{
    let dialect = presentations.dialect;
    let fingerprint = snapshot.fingerprint();
    manager.check_binding(dialect, fingerprint)?;
    let mut scratch = manager.clone();
    let mut result = MaterializedBatchResult::Dropped;
    let last = ast.statements.len().saturating_sub(1);
    for (position, statement) in ast.statements.iter().enumerate() {
        let final_statement = position == last;
        match statement {
            StatementAst::Drop { name } => {
                scratch.drop_table(name)?;
                if final_statement {
                    result = MaterializedBatchResult::Dropped;
                }
            }
            StatementAst::Query(query) => {
                let compiled = compile_statement(
                    query,
                    snapshot,
                    presentations,
                    &scratch,
                    query.into.as_ref(),
                )?;
                let Some(into) = &query.into else {
                    if final_statement {
                        let CompiledStatement {
                            query,
                            dependencies,
                            ..
                        } = compiled;
                        check_materialized_dependencies(&scratch, &dependencies, None)?;
                        result = MaterializedBatchResult::Query(CompiledQuery {
                            sql: join_with_prefix(
                                with_prefix(&scratch, &dependencies, dialect),
                                &query.sql,
                            ),
                            ..query
                        });
                    }
                    continue;
                };
                let CompiledStatement {
                    query: selection,
                    dependencies,
                    hierarchy,
                } = compiled;
                check_materialized_dependencies(&scratch, &dependencies, Some(into.token))?;
                if !into.append && scratch.contains(into.name.lexeme) {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::TemporaryTable,
                        Some(into.name),
                        format!("temporary table {:?} already exists", into.name.lexeme),
                    )
                    .into());
                }
                let previous = if into.append {
                    let previous = scratch.source(into.name)?;
                    check_append_structure(&previous.columns, &selection.columns, into.token)?;
                    let entry = scratch
                        .entry(previous.id)
                        .expect("resolved temporary table exists");
                    if !entry.materialized {
                        return Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::TemporaryTable,
                            Some(into.token),
                            "cannot append materialized rows to a CTE-only temporary table",
                        )
                        .into());
                    }
                    Some(previous)
                } else {
                    None
                };
                let local = hierarchy
                    .iter()
                    .map(|(name, body)| format!("{} AS ({body})", dialect.quote_identifier(name)))
                    .collect::<Vec<_>>();
                let capture_sql =
                    logical_capture_dates(&selection.sql, &selection.columns, dialect);
                let local_sql = if local.is_empty() {
                    capture_sql.clone()
                } else {
                    let keyword = if dialect == SqlDialect::Postgres {
                        "WITH RECURSIVE"
                    } else {
                        "WITH"
                    };
                    format!("{keyword} {} {capture_sql}", local.join(", "))
                };
                let selection_sql =
                    join_with_prefix(with_prefix(&scratch, &dependencies, dialect), &local_sql);
                let selection = CompiledQuery {
                    sql: selection_sql,
                    ..selection
                };
                let rows = on_placement(&selection)?;
                let count = rows.len();
                let literal = materialized_relation(
                    &selection.columns,
                    &rows,
                    into.token,
                    snapshot,
                    dialect,
                )?;
                let restricted = presentations.mode.is_restricted() || query.allowed.is_some();
                let id = match previous {
                    Some(previous) => {
                        let previous_id = previous.id;
                        let columns = previous.columns;
                        let body = format!(
                            "SELECT {} FROM {} UNION ALL {literal}",
                            columns
                                .iter()
                                .map(|column| dialect.quote_identifier(&column.label))
                                .collect::<Vec<_>>()
                                .join(", "),
                            dialect.quote_identifier(&cte_name(previous_id)),
                        );
                        scratch.append(
                            into.name,
                            body,
                            columns,
                            BTreeSet::from([previous_id]),
                            Vec::new(),
                            dialect,
                            fingerprint,
                            restricted,
                        )?
                    }
                    None => scratch.define(
                        into.name,
                        literal,
                        selection.columns,
                        BTreeSet::new(),
                        Vec::new(),
                        dialect,
                        fingerprint,
                        restricted,
                    )?,
                };
                scratch
                    .entry_mut(id)
                    .expect("new temporary table exists")
                    .materialized = true;
                scratch.check_materialized_size(into.token)?;
                if final_statement {
                    result = MaterializedBatchResult::Placed(count);
                }
            }
        }
    }
    *manager = scratch;
    Ok(result)
}

/// Placement compiles in the storage date domain. The host callback sees
/// logical dates, and the literal renderer shifts them back on retention.
fn logical_capture_dates(sql: &str, columns: &[CompiledColumn], dialect: SqlDialect) -> String {
    if !dialect.is_mssql()
        || !columns
            .iter()
            .any(|column| matches!(column.kind, ColumnKind::DateTime))
    {
        return sql.to_owned();
    }
    let alias = dialect.quote_identifier("__capture");
    let projection = columns
        .iter()
        .map(|column| {
            let label = dialect.quote_identifier(&column.label);
            let value = dialect.qualified_column(Some("__capture"), &column.label);
            let value = if matches!(column.kind, ColumnKind::DateTime) {
                dialect.date_scalar(&value)
            } else {
                value
            };
            format!("{value} AS {label}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("SELECT {projection} FROM ({sql}) AS {alias}")
}

/// A CTE-only table would silently reread its source through the new path.
fn check_materialized_dependencies(
    manager: &TempTablesManager,
    dependencies: &BTreeSet<u32>,
    token: Option<&Token<'_>>,
) -> Result<(), QueryDiagnostic> {
    if dependencies
        .iter()
        .any(|id| manager.entry(*id).is_some_and(|entry| !entry.materialized))
    {
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::TemporaryTable,
            token,
            "materialized batch cannot read a CTE-only temporary table",
        ));
    }
    Ok(())
}

/// Reuses the validated value-table renderer for rows captured by the host.
fn materialized_relation(
    columns: &[CompiledColumn],
    rows: &[Vec<ParameterValue>],
    token: &Token<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    let parameter_columns = columns
        .iter()
        .map(|column| ParameterColumn::new(column.label.clone(), column.kind.clone()))
        .collect::<Vec<_>>();
    let (_, body) = render_parameter_table(&parameter_columns, rows, token, snapshot, dialect)
        .map_err(|diagnostic| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(token),
                format!("materialized rows do not match projected columns: {diagnostic}"),
            )
        })?;
    let names = columns
        .iter()
        .map(|column| dialect.quote_identifier(&column.label))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "SELECT {names} FROM ({body}) AS {}",
        dialect.quote_identifier("__materialized")
    ))
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
            forbidden_row_column: query.forbidden_row_column,
            sql: join_with_prefix(with_prefix(manager, &dependencies, dialect), &query.sql),
            logical_projection: query.logical_projection,
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
        forbidden_row_column: None,
        logical_projection: None,
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
