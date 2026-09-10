use super::context::{
    CompilationContext, CompiledBranch, JoinPlan, ProjectedMember, ResolvedPath, ScopeId,
    SelectedProjection, SourceScope, compile_presentation, projected_members,
};
use super::expression::{
    compile_aggregate, compile_expression, compile_predicate, expression_kind, reference_column,
    reference_type_column, single_column, widen_reference,
};
use super::orchestrate::{PresentationCompilation, compile_query_ast};
use super::sources::{
    compile_source_free_branch, compile_source_relation, contains_aggregate,
    projection_is_aggregated, projection_token, validate_aggregate_projection,
};
use crate::metadata::{Guid, MetadataSnapshot, ObjectId};
use crate::query::core::ast::{
    AggregateArgument, CastTarget, Expression, FieldReference, JoinAst, JoinKind, OrderTerm,
    PresentationArgument, Projection, ProjectionItem, SelectAst, SourceAst,
};
use crate::query::core::dialect::{OutputLabelAllocator, SqlDialect};
use crate::query::core::names::names_equal;
use crate::query::core::temp_tables::cte_name;
use std::str::FromStr;

use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableColumn, QueryableField,
    resolve_source_metadata,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};
use std::collections::BTreeSet;

/// How a branch is rendered within its statement.
#[derive(Clone, Copy)]
pub(super) struct BranchMode<'a> {
    /// Output positions whose fixed references must be widened to payloads.
    pub(super) widen: &'a BTreeSet<usize>,
    /// Whether the statement is nested and keeps values in the storage
    /// domain (no MSSQL year-offset correction on projections).
    pub(super) storage_domain: bool,
}

pub(super) fn compile_branch(
    ast: &SelectAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    order_terms: &[OrderTerm<'_, '_>],
    union_order: bool,
    presentations: &mut PresentationCompilation<'_>,
    mode: BranchMode<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    let BranchMode {
        widen,
        storage_domain,
    } = mode;
    let dialect = presentations.dialect;
    validate_aggregate_projection(ast)?;
    let Some(source) = ast.source.as_ref() else {
        return compile_source_free_branch(
            ast,
            order_terms,
            snapshot,
            dialect,
            widen,
            catalog.parameters,
        );
    };
    let joins = ast.joins.as_slice();
    validate_join_projection(ast, joins)?;
    let grouped = !ast.group.is_empty() || ast.having.is_some();
    if grouped && let Some(join) = joins.iter().find(|join| join.kind == JoinKind::Full) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(join.token),
            "GROUP BY and HAVING are not supported together with FULL JOIN",
        ));
    }
    let mut context =
        compile_branch_context(source, joins, snapshot, catalog, dialect, presentations)?;
    context.aggregates_allowed = grouped
        || ast
            .projection
            .iter()
            .any(|projection| projection_is_aggregated(&projection.expression));
    let selected = compile_branch_projections(
        ast,
        source,
        joins.first(),
        &mut context,
        presentations,
        storage_domain,
    )?;
    let group_by = compile_group_keys(ast, &selected, &mut context)?;
    context.aggregates_allowed = grouped;
    let having = ast
        .having
        .as_ref()
        .map(|having| compile_predicate(having, &mut context))
        .transpose()?;
    context.aggregates_allowed = false;

    let RenderedProjections {
        columns,
        sql: projections,
        deferred_presentations,
    } = render_selected_projections(&selected, &context, widen, storage_domain)?;
    if projections.is_empty() {
        return Err(empty_projection_diagnostic(source, joins.first()));
    }

    let conditions = joins
        .iter()
        .enumerate()
        .map(|(index, join)| {
            compile_join_condition(
                &join.condition,
                &mut context,
                join.token,
                ScopeId(index + 1),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let filter = ast
        .filter
        .as_ref()
        .map(|filter| compile_predicate(filter, &mut context))
        .transpose()?;
    let order = compile_order_terms(
        order_terms,
        ast,
        &selected,
        &mut context,
        union_order || !joins.is_empty() || grouped,
        if grouped {
            "GROUP BY ORDER BY field must be a key or a projection alias"
        } else if !joins.is_empty() {
            "JOIN ORDER BY field must occur in the projection"
        } else {
            "UNION ORDER BY field must occur in the first branch projection"
        },
    )?;

    let mut sql = compile_branch_sql(
        ast,
        joins,
        &context,
        &projections,
        &conditions,
        filter.as_deref(),
    );
    if !group_by.is_empty() {
        sql.push_str(" GROUP BY ");
        sql.push_str(&group_by.join(", "));
    }
    if let Some(having) = having {
        sql.push_str(" HAVING ");
        sql.push_str(&having);
    }
    if !order.is_empty() && !union_order {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order.join(", "));
    }
    dialect.append_limit(&mut sql, ast.top);
    Ok(CompiledBranch {
        sql,
        columns,
        deferred_presentations,
        logical_width: selected.len(),
        order,
    })
}

fn validate_join_projection(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
) -> Result<(), QueryDiagnostic> {
    let Some(first) = joins.first() else {
        return Ok(());
    };
    if let Some(full) = joins.iter().find(|join| join.kind == JoinKind::Full) {
        if joins.len() > 1 {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(full.token),
                "FULL JOIN must be the only join of a branch",
            ));
        }
        if ast
            .projection
            .iter()
            .any(|projection| projection_is_aggregated(&projection.expression))
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(full.token),
                "aggregates over a transposed FULL JOIN are not supported",
            ));
        }
    }
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection.expression, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(first.token),
            "wildcard projection in JOIN is not supported",
        ));
    }
    Ok(())
}

fn compile_branch_context<'snapshot, 'catalog>(
    source: &SourceAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    snapshot: &'snapshot MetadataSnapshot,
    catalog: &'catalog CompilationCatalog<'snapshot>,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompilationContext<'snapshot, 'catalog>, QueryDiagnostic> {
    if !joins.is_empty() {
        let mut sources = vec![resolve_join_source(
            source,
            snapshot,
            catalog,
            "__left",
            dialect,
            presentations,
        )?];
        for (index, join) in joins.iter().enumerate() {
            // Sources are numbered from one: the base is `__left`, the first
            // joined source `__right`, later ones `__join3`, `__join4`, …
            let default_alias = if index == 0 {
                "__right".to_owned()
            } else {
                format!("__join{}", index + 2)
            };
            let scope = resolve_join_source(
                &join.source,
                snapshot,
                catalog,
                &default_alias,
                dialect,
                presentations,
            )?;
            if let Some(previous) = sources
                .iter()
                .find(|previous: &&SourceScope| names_equal(&previous.sql_alias, &scope.sql_alias))
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(join.token),
                    format!(
                        "JOIN sources must have distinct aliases; both resolve to {:?}",
                        previous.sql_alias
                    ),
                ));
            }
            sources.push(scope);
        }
        return Ok(CompilationContext {
            snapshot,
            catalog,
            sources,
            dialect,
            aggregates_allowed: false,
        });
    }
    let scope = resolve_join_source(source, snapshot, catalog, "__src", dialect, presentations)?;
    Ok(CompilationContext {
        snapshot,
        catalog,
        sources: vec![scope],
        dialect,
        aggregates_allowed: false,
    })
}

/// Builds the scope of a `(ВЫБРАТЬ …) КАК alias` source: the nested
/// statement becomes the relation and its columns become the fields.
fn derived_source_scope(
    source: &SourceAst<'_, '_>,
    nested: &crate::query::core::ast::QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<SourceScope, QueryDiagnostic> {
    let compiled = compile_query_ast(
        nested,
        snapshot,
        catalog,
        presentations,
        Some(source.object),
    )?;
    let alias = source
        .alias
        .expect("the parser requires an alias on a nested source")
        .lexeme
        .to_owned();
    let fields = compiled
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| derived_field(index, &column.label, &column.kind, snapshot, dialect))
        .collect::<Vec<_>>();
    Ok(SourceScope {
        object: derived_owner(),
        fields: fields.into(),
        relation: format!("({})", compiled.sql),
        sql_alias: alias.clone(),
        object_name: alias.clone(),
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
    })
}

/// Builds the scope of a temporary-table source: the stored CTE becomes the
/// relation and the stored columns become the fields.
fn temporary_source_scope(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
    let table = catalog.temporary_source(source.object)?;
    let alias = source
        .alias
        .map_or_else(|| table.name.clone(), |token| token.lexeme.to_owned());
    let fields = table
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| derived_field(index, &column.label, &column.kind, snapshot, dialect))
        .collect::<Vec<_>>();
    Ok(SourceScope {
        object: derived_owner(),
        fields: fields.into(),
        relation: dialect.quote_identifier(&cte_name(table.id)),
        sql_alias: alias.clone(),
        object_name: table.name,
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
    })
}

/// The placeholder owner of derived-source fields; no metadata object has
/// the nil GUID.
pub(super) fn derived_owner() -> ObjectId {
    ObjectId::from(&Guid::from_str(Guid::NIL).expect("the nil GUID is well formed"))
}

/// One field of a derived source, addressed by the nested column label.
fn derived_field(
    index: usize,
    label: &str,
    kind: &ColumnKind,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> QueryableField {
    let physical_table = |id: &ObjectId| {
        snapshot
            .object_by_id(*id)
            .and_then(|object| object.physical_table.clone())
    };
    let (reference_target, reference_targets) = match kind {
        ColumnKind::Reference {
            targets,
            runtime_typed: false,
        } => {
            let tables = targets
                .iter()
                .filter_map(physical_table)
                .collect::<Vec<_>>();
            let single = (tables.len() == 1 && targets.len() == 1).then(|| tables[0].clone());
            (single, tables)
        }
        // A payload column already carries the RTRef: it is presented the
        // way universal references are, deferred to the application.
        ColumnKind::Reference { .. } => (None, vec![String::new()]),
        _ => (None, Vec::new()),
    };
    QueryableField {
        name: label.to_owned(),
        schema_name: format!("__derived{}", index + 1),
        aliases: vec![label.to_owned()],
        columns: vec![QueryableColumn {
            physical_name: label.to_owned(),
            data_type: derived_data_type(kind, dialect),
            output_label: label.to_owned(),
            kind: kind.clone(),
        }],
        reference_target,
        reference_targets,
    }
}

/// A catalog type name that renders literals correctly for a derived
/// column; kinds carry the truth.
fn derived_data_type(kind: &ColumnKind, dialect: SqlDialect) -> String {
    let postgres = dialect == SqlDialect::Postgres;
    match kind {
        ColumnKind::String { .. } => {
            if postgres {
                "text"
            } else {
                "nvarchar(max)"
            }
        }
        ColumnKind::Number { .. } => "numeric",
        ColumnKind::Boolean => {
            if postgres {
                "boolean"
            } else {
                "bit"
            }
        }
        ColumnKind::DateTime => {
            if postgres {
                "timestamp"
            } else {
                "datetime2"
            }
        }
        ColumnKind::Reference { .. } | ColumnKind::Binary { .. } => {
            if postgres {
                "bytea"
            } else {
                "varbinary"
            }
        }
        ColumnKind::Uuid => {
            if postgres {
                "uuid"
            } else {
                "uniqueidentifier"
            }
        }
        ColumnKind::Null => "",
        ColumnKind::Unknown { data_type } => data_type.as_str(),
    }
    .to_owned()
}

fn compile_branch_projections(
    ast: &SelectAst<'_, '_>,
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
    context: &mut CompilationContext<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
    storage_domain: bool,
) -> Result<Vec<SelectedProjection>, QueryDiagnostic> {
    if join.is_none()
        && matches!(
            ast.projection.as_slice(),
            [ProjectionItem {
                expression: Projection::All,
                ..
            }]
        )
    {
        let scope = context.source(ScopeId(0));
        return Ok(scope
            .fields
            .iter()
            .enumerate()
            .map(|field| ResolvedPath::from_source(ScopeId(0), scope, field))
            .map(SelectedProjection::Field)
            .collect());
    }
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection.expression, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(source.object),
            "'*' cannot be combined with named fields",
        ));
    }
    compile_selected_projections(ast, context, presentations, storage_domain)
}

fn empty_projection_diagnostic(
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
) -> QueryDiagnostic {
    join.map_or_else(
        || {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(source.object),
                "metadata table has no queryable live columns",
            )
        },
        |join| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(join.token),
                "JOIN has no queryable projected columns",
            )
        },
    )
}

struct FullJoinCondition {
    sql: String,
    left_marker: String,
}

struct RenderedProjections {
    columns: Vec<CompiledColumn>,
    sql: Vec<String>,
    deferred_presentations: Vec<usize>,
}

fn compile_selected_projections(
    ast: &SelectAst<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
    storage_domain: bool,
) -> Result<Vec<SelectedProjection>, QueryDiagnostic> {
    let mut selected = Vec::with_capacity(ast.projection.len());
    for projection in &ast.projection {
        match &projection.expression {
            Projection::Field(reference) => {
                let mut resolved = context.resolve(reference)?;
                if let Some(alias) = projection.alias {
                    resolved.path_label = Some(alias.lexeme.to_owned());
                }
                selected.push(SelectedProjection::Field(resolved));
            }
            Projection::Presentation {
                token,
                operation,
                argument,
            } => {
                let (sql, label, deferred) =
                    compile_presentation(context, token, *operation, argument, presentations)?;
                selected.push(SelectedProjection::Generated {
                    sql,
                    label: projection
                        .alias
                        .map_or(label, |alias| alias.lexeme.to_owned()),
                    deferred,
                    kind: if deferred {
                        ColumnKind::Reference {
                            targets: Vec::new(),
                            runtime_typed: true,
                        }
                    } else {
                        ColumnKind::String { length: None }
                    },
                });
            }
            Projection::Scalar(expression) => {
                let number = selected.len() + 1;
                let sql = compile_expression(expression, context)?;
                let kind = expression_kind(expression, context)?;
                selected.push(SelectedProjection::Generated {
                    sql: if kind == ColumnKind::DateTime && !storage_domain {
                        context.dialect.date_scalar(&sql)
                    } else {
                        sql
                    },
                    label: projection.alias.map_or_else(
                        || format!("column{number}"),
                        |alias| alias.lexeme.to_owned(),
                    ),
                    deferred: false,
                    kind,
                });
            }
            Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            } => {
                let (sql, output_kind) = compile_aggregate(context, *kind, *distinct, argument)?;
                selected.push(SelectedProjection::Generated {
                    sql,
                    label: projection
                        .alias
                        .map_or_else(|| token.lexeme.to_owned(), |alias| alias.lexeme.to_owned()),
                    deferred: false,
                    kind: output_kind,
                });
            }
            Projection::All => unreachable!("wildcard projections are handled by the caller"),
        }
    }
    Ok(selected)
}

/// Renders the selected projections as `expr AS label` pairs. Columns whose
/// position is listed in `widen` are fixed references that another UNION
/// branch projects as a runtime-typed payload; they are widened here so
/// every branch emits the same width.
fn render_selected_projections(
    selected: &[SelectedProjection],
    context: &CompilationContext<'_, '_>,
    widen: &BTreeSet<usize>,
    storage_domain: bool,
) -> Result<RenderedProjections, QueryDiagnostic> {
    let mut columns = Vec::new();
    let mut sql = Vec::new();
    let mut deferred_presentations = Vec::new();
    let mut labels = OutputLabelAllocator::new(context.dialect);
    for selected in selected {
        match selected {
            SelectedProjection::Field(resolved) => {
                for member in projected_members(resolved.field()) {
                    context.catalog.charge(1, None)?;
                    let (expression, requested_label, kind) = match member {
                        ProjectedMember::Single(column) => (
                            if storage_domain {
                                context.dialect.storage_column_projection(
                                    &context.sql_column(resolved, column),
                                    &column.kind,
                                    &column.data_type,
                                )
                            } else {
                                context.dialect.column_projection(
                                    &context.sql_column(resolved, column),
                                    &column.kind,
                                    &column.data_type,
                                )
                            },
                            resolved.output_label(column),
                            column.kind.clone(),
                        ),
                        ProjectedMember::Reference {
                            type_member,
                            value_member,
                        } => (
                            context.dialect.reference_payload(
                                &context.sql_column(resolved, type_member),
                                &context.sql_column(resolved, value_member),
                            ),
                            resolved.field_label(),
                            value_member.kind.clone(),
                        ),
                    };
                    let (expression, kind) = if widen.contains(&columns.len()) {
                        widen_reference(
                            &expression,
                            &kind,
                            None,
                            context.snapshot,
                            context.dialect,
                        )?
                    } else {
                        (expression, kind)
                    };
                    let output_label = labels.allocate(&requested_label);
                    sql.push(format!(
                        "{expression} AS {}",
                        context.dialect.quote_identifier(&output_label)
                    ));
                    columns.push(CompiledColumn::new(output_label, kind));
                }
            }
            SelectedProjection::Generated {
                sql: expression,
                label,
                deferred,
                kind,
            } => {
                context.catalog.charge(1, None)?;
                let (expression, kind) = if widen.contains(&columns.len()) {
                    widen_reference(expression, kind, None, context.snapshot, context.dialect)?
                } else {
                    (expression.clone(), kind.clone())
                };
                let output_label = labels.allocate(label);
                sql.push(format!(
                    "{expression} AS {}",
                    context.dialect.quote_identifier(&output_label)
                ));
                if *deferred {
                    deferred_presentations.push(columns.len());
                }
                columns.push(CompiledColumn::new(output_label, kind));
            }
        }
    }
    Ok(RenderedProjections {
        columns,
        sql,
        deferred_presentations,
    })
}

/// The output position (1-based) of the first column of projection `index`.
fn projection_position(selected: &[SelectedProjection], index: usize) -> usize {
    selected[..index]
        .iter()
        .map(|selected| match selected {
            SelectedProjection::Field(resolved) => projected_members(resolved.field()).len(),
            SelectedProjection::Generated { .. } => 1,
        })
        .sum::<usize>()
        + 1
}

/// The projection whose alias equals a single-segment field reference.
fn aliased_projection(ast: &SelectAst<'_, '_>, field: &FieldReference<'_, '_>) -> Option<usize> {
    let [segment] = field.segments.as_slice() else {
        return None;
    };
    ast.projection.iter().position(|projection| {
        projection
            .alias
            .is_some_and(|alias| names_equal(alias.lexeme, segment.lexeme))
    })
}

fn compile_order_terms(
    order_terms: &[OrderTerm<'_, '_>],
    ast: &SelectAst<'_, '_>,
    selected: &[SelectedProjection],
    context: &mut CompilationContext<'_, '_>,
    positional: bool,
    missing_message: &'static str,
) -> Result<Vec<String>, QueryDiagnostic> {
    order_terms
        .iter()
        .map(|term| {
            if positional && let Some(index) = aliased_projection(ast, &term.field) {
                return Ok(format!(
                    "{}{}",
                    projection_position(selected, index),
                    if term.descending { " DESC" } else { " ASC" }
                ));
            }
            let resolved = context.resolve(&term.field)?;
            let column = single_column(resolved.field(), term.field.last())?;
            let expression = if positional {
                selected_column_position(selected, &resolved, &column.physical_name)
                    .ok_or_else(|| {
                        QueryDiagnostic::at(
                            QueryDiagnosticKind::UnsupportedFeature,
                            Some(term.field.last()),
                            missing_message,
                        )
                    })?
                    .to_string()
            } else {
                context.sql_column(&resolved, column)
            };
            Ok(format!(
                "{expression}{}",
                if term.descending { " DESC" } else { " ASC" }
            ))
        })
        .collect()
}

/// One resolved `GROUP BY` key.
enum GroupKeyTarget {
    Path(ResolvedPath),
    Alias(usize),
    Scalar(String),
}

/// Resolves the `GROUP BY` keys, checks that every non-aggregated
/// projection is a key, and renders the physical `GROUP BY` list.
fn compile_group_keys(
    ast: &SelectAst<'_, '_>,
    selected: &[SelectedProjection],
    context: &mut CompilationContext<'_, '_>,
) -> Result<Vec<String>, QueryDiagnostic> {
    if ast.group.is_empty() {
        return Ok(Vec::new());
    }
    let mut keys = Vec::with_capacity(ast.group.len());
    let mut sql = Vec::new();
    let mut push = |expression: String| {
        if !sql.contains(&expression) {
            sql.push(expression);
        }
    };
    for key in &ast.group {
        context.catalog.charge(1, None)?;
        let target = match &key.expression {
            Expression::Field(reference) => match context.resolve(reference) {
                Ok(resolved) => GroupKeyTarget::Path(resolved),
                Err(error) => match aliased_projection(ast, reference) {
                    Some(index) if error.kind() == QueryDiagnosticKind::UnknownField => {
                        GroupKeyTarget::Alias(index)
                    }
                    _ => return Err(error),
                },
            },
            expression => {
                if contains_aggregate(expression) {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(key.token),
                        "GROUP BY key cannot contain aggregate functions",
                    ));
                }
                let compiled = compile_expression(expression, context)?;
                push(compiled);
                GroupKeyTarget::Scalar(expression_fingerprint(expression))
            }
        };
        match &target {
            GroupKeyTarget::Path(resolved) => {
                for column in &resolved.field().columns {
                    push(context.sql_column(resolved, column));
                }
            }
            GroupKeyTarget::Alias(index) => match &selected[*index] {
                SelectedProjection::Field(resolved) => {
                    for column in &resolved.field().columns {
                        push(context.sql_column(resolved, column));
                    }
                }
                SelectedProjection::Generated {
                    sql: expression, ..
                } => {
                    if projection_is_aggregated(&ast.projection[*index].expression) {
                        return Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::UnsupportedFeature,
                            Some(key.token),
                            "GROUP BY key cannot be an aggregate projection",
                        ));
                    }
                    push(expression.clone());
                }
            },
            GroupKeyTarget::Scalar(_) => {}
        }
        keys.push(target);
    }

    for (index, (item, projection)) in ast.projection.iter().zip(selected).enumerate() {
        if projection_is_aggregated(&item.expression) {
            continue;
        }
        let matched = keys
            .iter()
            .any(|key| match (key, projection, &item.expression) {
                (GroupKeyTarget::Alias(alias), _, _) => *alias == index,
                (GroupKeyTarget::Path(key), SelectedProjection::Field(resolved), _) => {
                    key.same_path(resolved)
                }
                (
                    GroupKeyTarget::Path(key),
                    SelectedProjection::Generated { .. },
                    Projection::Presentation {
                        argument: PresentationArgument::Field(reference),
                        ..
                    },
                ) => context
                    .resolve(reference)
                    .is_ok_and(|resolved| key.same_path(&resolved)),
                (GroupKeyTarget::Scalar(fingerprint), _, Projection::Scalar(expression)) => {
                    *fingerprint == expression_fingerprint(expression)
                }
                _ => false,
            });
        if !matched {
            return Err(QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::UnsupportedFeature,
                projection_token(&item.expression),
                format!(
                    "field {:?} must be grouped or aggregated",
                    projection_label(item, projection)
                ),
            ));
        }
        // An inline presentation of a key reads joined columns that must be
        // grouped as well; grouping by the rendered expression covers them.
        if let (
            SelectedProjection::Generated {
                sql: expression, ..
            },
            Projection::Presentation { .. },
        ) = (projection, &item.expression)
        {
            push(expression.clone());
        }
    }
    Ok(sql)
}

fn projection_label(item: &ProjectionItem<'_, '_>, projection: &SelectedProjection) -> String {
    item.alias.map_or_else(
        || match projection {
            SelectedProjection::Field(resolved) => resolved.field_label(),
            SelectedProjection::Generated { label, .. } => label.clone(),
        },
        |alias| alias.lexeme.to_owned(),
    )
}

/// A case- and whitespace-insensitive structural rendering used to match
/// `GROUP BY` expressions with projected expressions.
fn expression_fingerprint(expression: &Expression<'_, '_>) -> String {
    let mut output = String::new();
    fingerprint_into(expression, &mut output);
    output
}

fn fingerprint_into(expression: &Expression<'_, '_>, output: &mut String) {
    let upper = |token: &Token<'_>| token.lexeme.to_uppercase();
    match expression {
        Expression::Field(reference) => {
            output.push_str("F(");
            for segment in &reference.segments {
                output.push_str(&upper(segment));
                output.push('.');
            }
            output.push(')');
        }
        Expression::Literal(token) | Expression::Parameter(token) => {
            output.push_str("L(");
            output.push_str(&upper(token));
            output.push(')');
        }
        Expression::DateTime { value, .. } => {
            output.push_str(&format!("D({value:?})"));
        }
        Expression::BeginOfPeriod { value, period, .. } => {
            output.push_str(&format!("BOP({period:?},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::MetadataValue {
            kind,
            object,
            value,
            ..
        } => {
            output.push_str(&format!(
                "V({}.{}.{})",
                upper(kind),
                upper(object),
                upper(value)
            ));
        }
        Expression::Uuid { argument, .. } => {
            output.push_str("UUID(");
            fingerprint_into(&Expression::Field(argument.clone()), output);
            output.push(')');
        }
        Expression::Cast {
            argument,
            target,
            path,
            ..
        } => {
            output.push_str("CAST(");
            fingerprint_into(argument, output);
            output.push_str(&match target {
                CastTarget::String { length } => format!(",S{length:?}"),
                CastTarget::Number { precision, scale } => format!(",N{precision:?}{scale:?}"),
                CastTarget::Boolean => ",B".to_owned(),
                CastTarget::Date => ",D".to_owned(),
                CastTarget::Reference { kind, object } => {
                    format!(",R{}.{}", upper(kind), upper(object))
                }
            });
            if let Some(path) = path {
                output.push('.');
                output.push_str(&upper(path));
            }
            output.push(')');
        }
        Expression::Unary { operator, value } => {
            output.push_str(&format!("U({},", upper(operator)));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            output.push_str(&format!("B({},", upper(operator)));
            fingerprint_into(left, output);
            output.push(',');
            fingerprint_into(right, output);
            output.push(')');
        }
        Expression::InList {
            value,
            items,
            negated,
        } => {
            output.push_str(&format!("IN({negated},"));
            fingerprint_into(value, output);
            for item in items {
                output.push(',');
                fingerprint_into(item, output);
            }
            output.push(')');
        }
        Expression::InQuery {
            token,
            value,
            negated,
            ..
        } => {
            output.push_str(&format!("INQ({negated},{},", token.span.start));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::IsNull { value, negated } => {
            output.push_str(&format!("ISNULL({negated},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::Case {
            branches,
            otherwise,
            ..
        } => {
            output.push_str("CASE(");
            for branch in branches {
                fingerprint_into(&branch.when, output);
                output.push(':');
                fingerprint_into(&branch.then, output);
                output.push(',');
            }
            if let Some(otherwise) = otherwise {
                output.push_str("ELSE:");
                fingerprint_into(otherwise, output);
            }
            output.push(')');
        }
        Expression::IsNullFunction {
            value, fallback, ..
        } => {
            output.push_str("COALESCE(");
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(fallback, output);
            output.push(')');
        }
        Expression::Like {
            value,
            pattern,
            escape,
            negated,
            ..
        } => {
            output.push_str(&format!("LIKE({negated},"));
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(pattern, output);
            if let Some(escape) = escape {
                output.push(',');
                fingerprint_into(escape, output);
            }
            output.push(')');
        }
        Expression::Aggregate {
            kind,
            distinct,
            argument,
            ..
        } => {
            output.push_str(&format!("AGG({kind:?},{distinct},"));
            match argument {
                AggregateArgument::All => output.push('*'),
                AggregateArgument::Expression(expression) => fingerprint_into(expression, output),
            }
            output.push(')');
        }
    }
}

fn compile_branch_sql(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    conditions: &[FullJoinCondition],
    filter: Option<&str>,
) -> String {
    let Some(join) = joins.first() else {
        let dialect = context.dialect;
        let mut sql = dialect.select_prefix(ast.distinct, ast.top);
        sql.push_str(&projections.join(", "));
        sql.push_str(" FROM ");
        sql.push_str(&context.sources[0].relation);
        sql.push_str(" AS ");
        sql.push_str(&dialect.quote_identifier(context.base_alias()));
        for reference_join in &context.sources[0].reference_joins {
            sql.push_str(" LEFT JOIN ");
            sql.push_str(&reference_join.target_relation);
            sql.push_str(" AS ");
            sql.push_str(&dialect.quote_identifier(&reference_join.alias));
            sql.push_str(" ON ");
            sql.push_str(&dialect.qualified_column(
                Some(&reference_join.source_alias),
                &reference_join.source_column,
            ));
            sql.push_str(" = ");
            sql.push_str(&dialect.qualified_column(
                Some(&reference_join.alias),
                &reference_join.target_id_column,
            ));
            append_type_guard(
                &mut sql,
                &reference_join.source_alias,
                reference_join,
                dialect,
            );
        }
        if let Some(filter) = filter {
            sql.push_str(" WHERE ");
            sql.push_str(filter);
        }
        return sql;
    };
    let condition = conditions
        .first()
        .expect("a JOIN branch always compiles its conditions");
    let dialect = context.dialect;
    if join.kind == JoinKind::Full {
        let first = compile_directional_full_join(
            context,
            &context.sources[0],
            &context.sources[1],
            projections,
            &condition.sql,
            filter,
            None,
        );
        let second = compile_directional_full_join(
            context,
            &context.sources[1],
            &context.sources[0],
            projections,
            &condition.sql,
            filter,
            Some(&condition.left_marker),
        );
        let mut sql = dialect.select_prefix(ast.distinct, ast.top);
        sql.push_str("* FROM (");
        if dialect == SqlDialect::Postgres {
            sql.push('(');
        }
        sql.push_str(&first);
        sql.push_str(if dialect == SqlDialect::Postgres {
            ") UNION ALL ("
        } else {
            " UNION ALL "
        });
        sql.push_str(&second);
        if dialect == SqlDialect::Postgres {
            sql.push(')');
        }
        sql.push_str(") AS ");
        sql.push_str(&dialect.quote_identifier("__full"));
        sql
    } else {
        compile_native_join(ast, joins, context, projections, conditions, filter)
    }
}

fn resolve_join_source(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    default_alias: &str,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<SourceScope, QueryDiagnostic> {
    if let Some(nested) = &source.nested {
        return derived_source_scope(source, nested, snapshot, catalog, dialect, presentations);
    }
    if source.temporary {
        return temporary_source_scope(source, snapshot, catalog, dialect);
    }
    let resolved = resolve_source_metadata(source, snapshot, catalog)?;
    let compiled_source = compile_source_relation(
        source,
        snapshot,
        catalog,
        resolved.object,
        resolved.live_table,
        &resolved.fields,
        dialect,
    )?;
    Ok(SourceScope {
        object: ObjectId::from(&resolved.object.guid),
        fields: compiled_source.fields,
        relation: compiled_source.sql,
        sql_alias: source
            .alias
            .map_or_else(|| default_alias.to_owned(), |token| token.lexeme.to_owned()),
        object_name: resolved.qualifier_name,
        source_alias: source.alias.map(|token| token.lexeme.to_owned()),
        identity_is_base: resolved.identity_is_base,
        reference_joins: Vec::new(),
    })
}

/// Compiles the `ON` condition of the join that introduces scope `joined`.
/// The condition must contain a top-level direct-field equality between the
/// joined source and an earlier one and may not reference later sources.
fn compile_join_condition(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    joined: ScopeId,
) -> Result<FullJoinCondition, QueryDiagnostic> {
    let mut parts = Vec::new();
    let mut left_marker = None;
    compile_join_condition_parts(expression, context, &mut parts, &mut left_marker, joined)?;
    let left_marker = left_marker.ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "JOIN condition requires at least one top-level direct-field equality between the joined source and an earlier source combined by AND",
        )
    })?;
    Ok(FullJoinCondition {
        sql: parts.join(" AND "),
        left_marker,
    })
}

fn compile_join_condition_parts(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    parts: &mut Vec<String>,
    left_marker: &mut Option<String>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let Expression::Binary {
            left,
            operator,
            right,
        } = expression
            && operator.kind == TokenKind::Keyword(Keyword::And)
        {
            pending.push(right);
            pending.push(left);
            continue;
        }

        if let Some((equality, marker)) =
            compile_cross_source_join_equality(expression, context, joined)?
        {
            if left_marker.is_none() {
                *left_marker = Some(marker);
            }
            parts.push(equality);
            continue;
        }

        validate_direct_join_condition_fields(expression, context, joined)?;
        parts.push(compile_predicate(expression, context)?);
    }
    Ok(())
}

fn compile_cross_source_join_equality(
    expression: &Expression<'_, '_>,
    context: &CompilationContext<'_, '_>,
    joined: ScopeId,
) -> Result<Option<(String, String)>, QueryDiagnostic> {
    let Expression::Binary {
        left,
        operator,
        right,
    } = expression
    else {
        return Ok(None);
    };
    if operator.lexeme != "=" {
        return Ok(None);
    }
    let (Expression::Field(left_reference), Expression::Field(right_reference)) =
        (left.as_ref(), right.as_ref())
    else {
        return Ok(None);
    };
    let left_field = context.resolve_direct(left_reference)?;
    let right_field = context.resolve_direct(right_reference)?;
    check_join_scope(left_field.scope, left_reference.last(), joined)?;
    check_join_scope(right_field.scope, right_reference.last(), joined)?;
    // Only an equality that binds the joined source to an earlier one is the
    // anchor; equalities between earlier sources are ordinary predicates.
    if left_field.scope == right_field.scope
        || (left_field.scope != joined && right_field.scope != joined)
    {
        return Ok(None);
    }
    let equality = compile_join_field_equality(
        context,
        &left_field,
        left_reference.last(),
        &right_field,
        right_reference.last(),
    )?;
    let marker = if left_field.scope == ScopeId(0) {
        equality.left_marker
    } else {
        equality.right_marker
    };
    Ok(Some((equality.sql, marker)))
}

/// A join condition may reference the joined source and earlier ones only.
fn check_join_scope(
    scope: ScopeId,
    token: &Token<'_>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    if scope.0 > joined.0 {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "JOIN condition cannot reference {:?}, which is joined later",
                token.lexeme
            ),
        ));
    }
    Ok(())
}

fn validate_direct_join_condition_fields(
    expression: &Expression<'_, '_>,
    context: &CompilationContext<'_, '_>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Expression::Field(reference) => {
                let resolved = context.resolve_direct(reference)?;
                check_join_scope(resolved.scope, reference.last(), joined)?;
            }
            Expression::Uuid { argument, .. } => {
                let resolved = context.resolve_direct(argument)?;
                check_join_scope(resolved.scope, argument.last(), joined)?;
            }
            Expression::Cast {
                argument,
                path: None,
                ..
            } => pending.push(argument),
            Expression::Cast {
                token,
                path: Some(_),
                ..
            } => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "JOIN condition supports direct fields only",
                ));
            }
            Expression::BeginOfPeriod { value, .. }
            | Expression::Unary { value, .. }
            | Expression::IsNull { value, .. } => pending.push(value),
            Expression::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            Expression::InList { value, items, .. } => {
                pending.extend(items.iter().rev());
                pending.push(value);
            }
            Expression::InQuery { value, .. } => pending.push(value),
            Expression::Case {
                branches,
                otherwise,
                ..
            } => {
                if let Some(otherwise) = otherwise {
                    pending.push(otherwise);
                }
                for branch in branches.iter().rev() {
                    pending.push(&branch.then);
                    pending.push(&branch.when);
                }
            }
            Expression::IsNullFunction {
                value, fallback, ..
            } => {
                pending.push(fallback);
                pending.push(value);
            }
            Expression::Like {
                value,
                pattern,
                escape,
                ..
            } => {
                if let Some(escape) = escape {
                    pending.push(escape);
                }
                pending.push(pattern);
                pending.push(value);
            }
            Expression::Aggregate { token, .. } => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "JOIN condition cannot contain aggregate functions",
                ));
            }
            Expression::Literal(_)
            | Expression::Parameter(_)
            | Expression::DateTime { .. }
            | Expression::MetadataValue { .. } => {}
        }
    }
    Ok(())
}

struct JoinedFieldEquality {
    sql: String,
    left_marker: String,
    right_marker: String,
}

fn compile_join_field_equality(
    context: &CompilationContext<'_, '_>,
    left: &ResolvedPath,
    left_token: &Token<'_>,
    right: &ResolvedPath,
    right_token: &Token<'_>,
) -> Result<JoinedFieldEquality, QueryDiagnostic> {
    if let ([left_column], [right_column]) = (
        left.field().columns.as_slice(),
        right.field().columns.as_slice(),
    ) {
        let left_sql = context.sql_column(left, left_column);
        let right_sql = context.sql_column(right, right_column);
        return Ok(JoinedFieldEquality {
            sql: format!("{left_sql} = {right_sql}"),
            left_marker: left_sql,
            right_marker: right_sql,
        });
    }

    if left.field().columns.len() > 1 && right.field().columns.len() == 1 {
        return compile_compound_fixed_reference_equality(
            context,
            left,
            left_token,
            right,
            right_token,
            false,
        );
    }
    if right.field().columns.len() > 1 && left.field().columns.len() == 1 {
        return compile_compound_fixed_reference_equality(
            context,
            right,
            right_token,
            left,
            left_token,
            true,
        );
    }

    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::UnsupportedFeature,
        Some(left_token),
        "JOIN equality does not support these compound field shapes",
    ))
}

fn compile_compound_fixed_reference_equality(
    context: &CompilationContext<'_, '_>,
    compound: &ResolvedPath,
    compound_token: &Token<'_>,
    fixed: &ResolvedPath,
    fixed_token: &Token<'_>,
    fixed_is_left: bool,
) -> Result<JoinedFieldEquality, QueryDiagnostic> {
    let compound_reference = reference_column(compound.field(), compound_token)?;
    let compound_type = reference_type_column(compound.field(), compound_token)?;
    let fixed_reference = reference_column(fixed.field(), fixed_token)?;
    let database_type =
        fixed_reference_database_type(context.snapshot, fixed.field(), fixed_token)?;
    let compound_reference_sql = context.sql_column(compound, compound_reference);
    let compound_type_sql = context.sql_column(compound, compound_type);
    let fixed_reference_sql = context.sql_column(fixed, fixed_reference);
    let sql = format!(
        "({compound_reference_sql} = {fixed_reference_sql} AND {compound_type_sql} = {})",
        context.dialect.binary_u32(database_type)
    );
    let (left_marker, right_marker) = if fixed_is_left {
        (fixed_reference_sql, compound_reference_sql)
    } else {
        (compound_reference_sql, fixed_reference_sql)
    };
    Ok(JoinedFieldEquality {
        sql,
        left_marker,
        right_marker,
    })
}

fn fixed_reference_database_type(
    snapshot: &MetadataSnapshot,
    field: &QueryableField,
    token: &Token<'_>,
) -> Result<u32, QueryDiagnostic> {
    let target = field.reference_target.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!(
                "fixed reference field {:?} has no unique SchemaStorage target",
                field.name
            ),
        )
    })?;
    let matches = snapshot
        .schema()
        .tables
        .iter()
        .filter(|table| names_equal(&table.name, target))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [table] => Ok(table.number),
        [] => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!("reference target {target:?} has no SchemaStorage database type"),
        )),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::AmbiguousObject,
            Some(token),
            format!("reference target {target:?} has an ambiguous database type"),
        )),
    }
}

fn compile_directional_full_join(
    context: &CompilationContext<'_, '_>,
    base: &SourceScope,
    joined: &SourceScope,
    projections: &[String],
    condition: &str,
    filter: Option<&str>,
    anti_match: Option<&str>,
) -> String {
    let mut sql = format!(
        "SELECT {} FROM {} AS {} LEFT JOIN {} AS {} ON {}",
        projections.join(", "),
        base.relation,
        context.dialect.quote_identifier(&base.sql_alias),
        joined.relation,
        context.dialect.quote_identifier(&joined.sql_alias),
        condition,
    );
    append_joined_reference_joins(&mut sql, context);
    let mut predicates = Vec::new();
    if let Some(anti_match) = anti_match {
        predicates.push(format!("({anti_match} IS NULL)"));
    }
    if let Some(filter) = filter {
        predicates.push(filter.to_owned());
    }
    if !predicates.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&predicates.join(" AND "));
    }
    sql
}

fn compile_native_join(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    conditions: &[FullJoinCondition],
    filter: Option<&str>,
) -> String {
    let mut sql = context.dialect.select_prefix(ast.distinct, ast.top);
    sql.push_str(&projections.join(", "));
    sql.push_str(" FROM ");
    sql.push_str(&context.sources[0].relation);
    sql.push_str(" AS ");
    sql.push_str(
        &context
            .dialect
            .quote_identifier(&context.sources[0].sql_alias),
    );
    for ((join, condition), source) in joins.iter().zip(conditions).zip(&context.sources[1..]) {
        let operator = match join.kind {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Full => unreachable!("FULL JOIN is transposed separately"),
        };
        sql.push(' ');
        sql.push_str(operator);
        sql.push(' ');
        sql.push_str(&source.relation);
        sql.push_str(" AS ");
        sql.push_str(&context.dialect.quote_identifier(&source.sql_alias));
        sql.push_str(" ON ");
        sql.push_str(&condition.sql);
    }
    append_joined_reference_joins(&mut sql, context);
    if let Some(filter) = filter {
        sql.push_str(" WHERE ");
        sql.push_str(filter);
    }
    sql
}

fn append_joined_reference_joins(sql: &mut String, context: &CompilationContext<'_, '_>) {
    for source in &context.sources {
        for join in &source.reference_joins {
            sql.push_str(" LEFT JOIN ");
            sql.push_str(&join.target_relation);
            sql.push_str(" AS ");
            sql.push_str(&context.dialect.quote_identifier(&join.alias));
            sql.push_str(" ON ");
            sql.push_str(
                &context
                    .dialect
                    .qualified_column(Some(&join.source_alias), &join.source_column),
            );
            sql.push_str(" = ");
            sql.push_str(
                &context
                    .dialect
                    .qualified_column(Some(&join.alias), &join.target_id_column),
            );
            append_type_guard(sql, &join.source_alias, join, context.dialect);
        }
    }
}

fn append_type_guard(sql: &mut String, source_alias: &str, join: &JoinPlan, dialect: SqlDialect) {
    if let (Some(column), Some(number)) = (&join.source_type_column, join.database_type) {
        sql.push_str(" AND ");
        sql.push_str(&dialect.qualified_column(Some(source_alias), column));
        sql.push_str(" = ");
        sql.push_str(&dialect.binary_u32(number));
    }
}

fn selected_column_position(
    selected: &[SelectedProjection],
    ordered: &ResolvedPath,
    ordered_column: &str,
) -> Option<usize> {
    let mut position = 1;
    for selected in selected {
        match selected {
            SelectedProjection::Field(resolved) => {
                for member in projected_members(resolved.field()) {
                    let column = match member {
                        ProjectedMember::Single(column) => column,
                        ProjectedMember::Reference { value_member, .. } => value_member,
                    };
                    if resolved.scope == ordered.scope
                        && names_equal(&resolved.sql_alias, &ordered.sql_alias)
                        && names_equal(&column.physical_name, ordered_column)
                    {
                        return Some(position);
                    }
                    position += 1;
                }
            }
            SelectedProjection::Generated { .. } => position += 1,
        }
    }
    None
}
