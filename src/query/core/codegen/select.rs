use super::context::{
    CompilationContext, CompiledBranch, JoinPlan, ProjectedMember, ResolvedPath, ScopeId,
    SelectedProjection, SourceScope, compile_presentation, projected_members,
};
use super::expression::{
    compile_aggregate, compile_expression, expression_kind, reference_column,
    reference_type_column, single_column,
};
use super::orchestrate::PresentationCompilation;
use super::sources::{
    compile_source_free_branch, compile_source_relation, validate_aggregate_projection,
};
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::{
    Expression, JoinAst, JoinKind, OrderTerm, Projection, ProjectionItem, SelectAst, SourceAst,
};
use crate::query::core::dialect::{OutputLabelAllocator, SqlDialect};
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableField, resolve_source_metadata,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

pub(super) fn compile_branch(
    ast: &SelectAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    order_terms: &[OrderTerm<'_, '_>],
    union_order: bool,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    let dialect = presentations.dialect;
    validate_aggregate_projection(ast)?;
    let Some(source) = ast.source.as_ref() else {
        return compile_source_free_branch(ast, order_terms, snapshot, dialect);
    };
    let join = ast.join.as_ref();
    validate_join_projection(ast, join)?;
    let mut context = compile_branch_context(source, join, snapshot, catalog, dialect)?;
    let selected = compile_branch_projections(ast, source, join, &mut context, presentations)?;

    let RenderedProjections {
        columns,
        sql: projections,
        deferred_presentations,
    } = render_selected_projections(&selected, &context)?;
    if projections.is_empty() {
        return Err(empty_projection_diagnostic(source, join));
    }

    let condition = join
        .map(|join| compile_full_join_condition(&join.condition, &mut context, join.token))
        .transpose()?;
    let filter = ast
        .filter
        .as_ref()
        .map(|filter| compile_expression(filter, &mut context))
        .transpose()?;
    let order = compile_order_terms(
        order_terms,
        &selected,
        &mut context,
        union_order || join.is_some(),
        if join.is_some() {
            "JOIN ORDER BY field must occur in the projection"
        } else {
            "UNION ORDER BY field must occur in the first branch projection"
        },
    )?;

    let mut sql = compile_branch_sql(
        ast,
        join,
        &context,
        &projections,
        condition.as_ref(),
        filter.as_deref(),
    );
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
    join: Option<&JoinAst<'_, '_>>,
) -> Result<(), QueryDiagnostic> {
    let Some(join) = join else {
        return Ok(());
    };
    if join.kind == JoinKind::Full
        && ast
            .projection
            .iter()
            .any(|projection| matches!(projection.expression, Projection::Aggregate { .. }))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(join.token),
            "aggregates over a transposed FULL JOIN are not supported",
        ));
    }
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection.expression, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(join.token),
            "wildcard projection in JOIN is not supported",
        ));
    }
    Ok(())
}

fn compile_branch_context<'snapshot, 'catalog>(
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
    snapshot: &'snapshot MetadataSnapshot,
    catalog: &'catalog CompilationCatalog<'snapshot>,
    dialect: SqlDialect,
) -> Result<CompilationContext<'snapshot, 'catalog>, QueryDiagnostic> {
    if let Some(join) = join {
        let left = resolve_join_source(source, snapshot, catalog, "__left", dialect)?;
        let right = resolve_join_source(&join.source, snapshot, catalog, "__right", dialect)?;
        if names_equal(&left.sql_alias, &right.sql_alias) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(join.token),
                format!(
                    "JOIN sources must have distinct aliases; both resolve to {:?}",
                    left.sql_alias
                ),
            ));
        }
        return Ok(CompilationContext {
            snapshot,
            catalog,
            sources: vec![left, right],
            dialect,
        });
    }
    let resolved = resolve_source_metadata(source, snapshot, catalog)?;
    let relation = compile_source_relation(
        source,
        snapshot,
        catalog,
        resolved.object,
        resolved.live_table,
        &resolved.fields,
        dialect,
    )?;
    Ok(CompilationContext {
        snapshot,
        catalog,
        sources: vec![SourceScope {
            object: ObjectId::from(&resolved.object.guid),
            fields: relation.fields,
            relation: relation.sql,
            sql_alias: source
                .alias
                .map_or_else(|| "__src".to_owned(), |token| token.lexeme.to_owned()),
            object_name: resolved.qualifier_name,
            source_alias: source.alias.map(|token| token.lexeme.to_owned()),
            identity_is_base: resolved.identity_is_base,
            reference_joins: Vec::new(),
        }],
        dialect,
    })
}

fn compile_branch_projections(
    ast: &SelectAst<'_, '_>,
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
    context: &mut CompilationContext<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
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
    compile_selected_projections(ast, context, presentations)
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
                    sql: if expression.is_date() {
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

fn render_selected_projections(
    selected: &[SelectedProjection],
    context: &CompilationContext<'_, '_>,
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
                            context.dialect.column_projection(
                                &context.sql_column(resolved, column),
                                &column.kind,
                                &column.data_type,
                            ),
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
                let output_label = labels.allocate(label);
                sql.push(format!(
                    "{expression} AS {}",
                    context.dialect.quote_identifier(&output_label)
                ));
                if *deferred {
                    deferred_presentations.push(columns.len());
                }
                columns.push(CompiledColumn::new(output_label, kind.clone()));
            }
        }
    }
    Ok(RenderedProjections {
        columns,
        sql,
        deferred_presentations,
    })
}

fn compile_order_terms(
    order_terms: &[OrderTerm<'_, '_>],
    selected: &[SelectedProjection],
    context: &mut CompilationContext<'_, '_>,
    positional: bool,
    missing_message: &'static str,
) -> Result<Vec<String>, QueryDiagnostic> {
    order_terms
        .iter()
        .map(|term| {
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

fn compile_branch_sql(
    ast: &SelectAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    condition: Option<&FullJoinCondition>,
    filter: Option<&str>,
) -> String {
    let Some(join) = join else {
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
    let condition = condition.expect("a JOIN branch always compiles its condition");
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
        compile_native_join(ast, join.kind, context, projections, &condition.sql, filter)
    }
}

fn resolve_join_source(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    default_alias: &str,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
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

fn compile_full_join_condition(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
) -> Result<FullJoinCondition, QueryDiagnostic> {
    let mut parts = Vec::new();
    let mut left_marker = None;
    compile_join_condition_parts(expression, context, &mut parts, &mut left_marker)?;
    let left_marker = left_marker.ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "JOIN condition requires at least one top-level cross-source field equality combined by AND",
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

        if let Some((equality, marker)) = compile_cross_source_join_equality(expression, context)? {
            if left_marker.is_none() {
                *left_marker = Some(marker);
            }
            parts.push(equality);
            continue;
        }

        validate_direct_join_condition_fields(expression, context)?;
        parts.push(compile_expression(expression, context)?);
    }
    Ok(())
}

fn compile_cross_source_join_equality(
    expression: &Expression<'_, '_>,
    context: &CompilationContext<'_, '_>,
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
    if left_field.scope == right_field.scope {
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

fn validate_direct_join_condition_fields(
    expression: &Expression<'_, '_>,
    context: &CompilationContext<'_, '_>,
) -> Result<(), QueryDiagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Expression::Field(reference) => {
                context.resolve_direct(reference)?;
            }
            Expression::BeginOfPeriod { value, .. }
            | Expression::Unary { value, .. }
            | Expression::IsNull { value, .. } => pending.push(value),
            Expression::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            Expression::InList { value, items } => {
                pending.extend(items.iter().rev());
                pending.push(value);
            }
            Expression::Literal(_)
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
    kind: JoinKind,
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    condition: &str,
    filter: Option<&str>,
) -> String {
    let operator = match kind {
        JoinKind::Inner => "INNER JOIN",
        JoinKind::Left => "LEFT JOIN",
        JoinKind::Right => "RIGHT JOIN",
        JoinKind::Full => unreachable!("FULL JOIN is transposed separately"),
    };
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
    sql.push(' ');
    sql.push_str(operator);
    sql.push(' ');
    sql.push_str(&context.sources[1].relation);
    sql.push_str(" AS ");
    sql.push_str(
        &context
            .dialect
            .quote_identifier(&context.sources[1].sql_alias),
    );
    sql.push_str(" ON ");
    sql.push_str(condition);
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
