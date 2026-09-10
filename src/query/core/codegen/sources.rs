use std::collections::BTreeSet;
use std::sync::Arc;

use super::context::{CompilationContext, CompiledBranch, SourceScope};
use super::expression::{
    Operand, binary_operator_sql, check_like_operand, compile_case, compile_expression,
    left_binary_spine, operand_token, reference_column, reference_type_column, render_coalesce,
    render_like, single_column, source_free_expression_kind, widen_reference,
};
use super::params::render_scalar_parameter;
use super::virtual_tables::{
    compile_accumulation_relation, compile_constant_date_expression, compile_date_parameter,
};
use crate::metadata::{
    ConfigFieldPurpose, LiveTable, MetadataKind, MetadataObject, MetadataSnapshot, ObjectId,
};
use crate::query::core::ast::{
    AggregateArgument, AggregateKind, CastTarget, Expression, OrderTerm, PresentationArgument,
    PresentationOperation, Projection, SelectAst, SourceAst,
};
use crate::query::core::dialect::{OutputLabelAllocator, SqlDialect, compile_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::Parameters;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableField, kind_from_query_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

pub(super) fn compile_source_free_branch(
    ast: &SelectAst<'_, '_>,
    order_terms: &[OrderTerm<'_, '_>],
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    widen: &BTreeSet<usize>,
    parameters: Parameters<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    if ast.join.is_some() {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            "JOIN requires FROM",
        ));
    }
    if ast.filter.is_some() {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            "WHERE requires FROM",
        ));
    }
    if let Some(key) = ast.group.first() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(key.token),
            "GROUP BY requires FROM",
        ));
    }
    if ast.having.is_some() {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            "HAVING requires FROM",
        ));
    }
    if !order_terms.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(order_terms[0].field.last()),
            "ORDER BY requires FROM for a source-free SELECT",
        ));
    }

    let mut projections = Vec::with_capacity(ast.projection.len());
    let mut columns = Vec::with_capacity(ast.projection.len());
    let mut labels = OutputLabelAllocator::new(dialect);
    for (index, projection) in ast.projection.iter().enumerate() {
        let (sql, default_label, kind) = match &projection.expression {
            Projection::Aggregate {
                token,
                kind: AggregateKind::Count,
                distinct: false,
                argument: AggregateArgument::All,
            } => (
                "COUNT(*)".to_owned(),
                token.lexeme.to_owned(),
                ColumnKind::Number {
                    precision: None,
                    scale: None,
                },
            ),
            Projection::Aggregate { token, .. } => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "aggregate field argument requires FROM",
                ));
            }
            Projection::Scalar(expression) => {
                let sql =
                    compile_source_free_expression(expression, snapshot, dialect, parameters)?;
                (
                    sql,
                    format!("column{}", index + 1),
                    source_free_expression_kind(expression, snapshot, parameters),
                )
            }
            Projection::Presentation {
                token,
                operation: PresentationOperation::Reference | PresentationOperation::String,
                argument: PresentationArgument::Literal(literal),
            } => (
                dialect.scalar_text(&compile_literal(literal, dialect)?),
                token.lexeme.to_owned(),
                ColumnKind::String { length: None },
            ),
            Projection::Presentation { token, .. } => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "reference presentation field requires FROM",
                ));
            }
            Projection::Field(reference) => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(reference.last()),
                    "field projection requires FROM",
                ));
            }
            Projection::All => {
                return Err(QueryDiagnostic::unpositioned(
                    QueryDiagnosticKind::UnsupportedFeature,
                    "wildcard projection requires FROM",
                ));
            }
        };
        let requested_label = projection
            .alias
            .map_or(default_label, |alias| alias.lexeme.to_owned());
        let (sql, kind) = if widen.contains(&columns.len()) {
            widen_reference(&sql, &kind, None, snapshot, dialect)?
        } else {
            (sql, kind)
        };
        let label = labels.allocate(&requested_label);
        projections.push(format!("{sql} AS {}", dialect.quote_identifier(&label)));
        columns.push(CompiledColumn::new(label, kind));
    }
    let mut sql = dialect.select_prefix(ast.distinct, ast.top);
    sql.push_str(&projections.join(", "));
    dialect.append_limit(&mut sql, ast.top);
    Ok(CompiledBranch {
        sql,
        logical_width: columns.len(),
        columns,
        deferred_presentations: Vec::new(),
        order: Vec::new(),
    })
}

fn compile_source_free_expression(
    expression: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    parameters: Parameters<'_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(reference.last()),
            "field expression requires FROM",
        )),
        Expression::Literal(token) => compile_literal(token, dialect),
        Expression::DateTime { token, value } => dialect.datetime_expression(*value, false, token),
        Expression::BeginOfPeriod {
            token,
            value,
            period,
        } => {
            if source_free_expression_kind(value, snapshot, parameters) != ColumnKind::DateTime {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(token),
                    "BEGINOFPERIOD first argument must be a date expression",
                ));
            }
            let value = compile_source_free_expression(value, snapshot, dialect, parameters)?;
            Ok(dialect.begin_of_period(&value, *period))
        }
        Expression::MetadataValue {
            token,
            kind,
            object,
            value,
        } => compile_metadata_value(token, kind, object, value, snapshot, dialect),
        Expression::Uuid { token, .. } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "UUID requires FROM",
        )),
        Expression::Cast {
            token,
            target: CastTarget::Reference { .. },
            ..
        } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "CAST to a metadata type requires FROM",
        )),
        Expression::Cast {
            argument, target, ..
        } => {
            let inner = compile_source_free_expression(argument, snapshot, dialect, parameters)?;
            Ok(dialect.cast_scalar(&inner, *target))
        }
        Expression::Unary { operator, value } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::Not) => "NOT ",
                _ if operator.lexeme == "+" => "+",
                _ if operator.lexeme == "-" => "-",
                _ => {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(operator),
                        "unsupported unary operator",
                    ));
                }
            };
            Ok(format!(
                "({operator}{})",
                compile_source_free_expression(value, snapshot, dialect, parameters)?
            ))
        }
        Expression::Binary {
            left: _,
            operator: _,
            right: _,
        } => compile_source_free_binary_expression(expression, snapshot, dialect, parameters),
        Expression::InList { value, items } => {
            let value = compile_source_free_expression(value, snapshot, dialect, parameters)?;
            let items = items
                .iter()
                .map(|item| compile_source_free_expression(item, snapshot, dialect, parameters))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({value} IN ({}))", items.join(", ")))
        }
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_source_free_expression(value, snapshot, dialect, parameters)?,
            if *negated { "NOT " } else { "" }
        )),
        Expression::Parameter(token) => match parameters.lookup(token)? {
            Some(value) => render_scalar_parameter(value, token, dialect, false),
            None => Ok("NULL".to_owned()),
        },
        Expression::Case {
            branches,
            otherwise,
            ..
        } => compile_case(
            branches,
            otherwise.as_deref(),
            snapshot,
            dialect,
            |expression, predicate| {
                if predicate {
                    Ok((
                        compile_source_free_predicate(expression, snapshot, dialect, parameters)?,
                        ColumnKind::Boolean,
                    ))
                } else {
                    Ok((
                        compile_source_free_expression(expression, snapshot, dialect, parameters)?,
                        source_free_expression_kind(expression, snapshot, parameters),
                    ))
                }
            },
        )
        .map(|(sql, _)| sql),
        Expression::IsNullFunction {
            token,
            value,
            fallback,
        } => {
            let mut operands = [
                Operand {
                    token,
                    sql: compile_source_free_expression(value, snapshot, dialect, parameters)?,
                    kind: source_free_expression_kind(value, snapshot, parameters),
                },
                Operand {
                    token,
                    sql: compile_source_free_expression(fallback, snapshot, dialect, parameters)?,
                    kind: source_free_expression_kind(fallback, snapshot, parameters),
                },
            ];
            render_coalesce(&mut operands, snapshot, dialect).map(|(sql, _)| sql)
        }
        Expression::Like { token, .. } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "LIKE is supported only in predicate positions",
        )),
        Expression::Aggregate { token, .. } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "aggregate field argument requires FROM",
        )),
    }
}

/// The source-free counterpart of `compile_predicate`: boolean literals and
/// boolean-valued expressions become comparisons on MSSQL.
fn compile_source_free_predicate(
    expression: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    parameters: Parameters<'_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Literal(token) => match token.kind {
            TokenKind::Keyword(Keyword::True) => Ok(dialect.boolean_literal_predicate(true)),
            TokenKind::Keyword(Keyword::False) => Ok(dialect.boolean_literal_predicate(false)),
            _ => compile_source_free_expression(expression, snapshot, dialect, parameters),
        },
        Expression::Cast {
            target: CastTarget::Boolean,
            ..
        }
        | Expression::Case { .. }
        | Expression::IsNullFunction { .. }
        | Expression::Parameter(_) => {
            let sql = compile_source_free_expression(expression, snapshot, dialect, parameters)?;
            Ok(
                if source_free_expression_kind(expression, snapshot, parameters)
                    == ColumnKind::Boolean
                {
                    dialect.boolean_predicate(&sql)
                } else {
                    sql
                },
            )
        }
        Expression::Like {
            token,
            value,
            pattern,
            escape,
            negated,
        } => {
            for operand in [value.as_ref(), pattern.as_ref()]
                .into_iter()
                .chain(escape.as_deref())
            {
                check_like_operand(
                    token,
                    &source_free_expression_kind(operand, snapshot, parameters),
                )?;
            }
            let escape = escape
                .as_ref()
                .map(|escape| compile_source_free_expression(escape, snapshot, dialect, parameters))
                .transpose()?;
            Ok(render_like(
                &compile_source_free_expression(value, snapshot, dialect, parameters)?,
                &compile_source_free_expression(pattern, snapshot, dialect, parameters)?,
                escape.as_deref(),
                *negated,
            ))
        }
        Expression::Unary { operator, value }
            if operator.kind == TokenKind::Keyword(Keyword::Not) =>
        {
            Ok(format!(
                "(NOT {})",
                compile_source_free_predicate(value, snapshot, dialect, parameters)?
            ))
        }
        Expression::Binary { operator, .. }
            if matches!(
                operator.kind,
                TokenKind::Keyword(Keyword::And | Keyword::Or)
            ) =>
        {
            let (left, terms) = left_binary_spine(expression);
            let mut sql = "(".repeat(terms.len());
            sql.push_str(&compile_source_free_predicate(
                left, snapshot, dialect, parameters,
            )?);
            for (operator, right) in terms {
                sql.push(' ');
                sql.push_str(binary_operator_sql(operator)?);
                sql.push(' ');
                sql.push_str(&compile_source_free_predicate(
                    right, snapshot, dialect, parameters,
                )?);
                sql.push(')');
            }
            Ok(sql)
        }
        _ => compile_source_free_expression(expression, snapshot, dialect, parameters),
    }
}

fn compile_source_free_binary_expression(
    expression: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    parameters: Parameters<'_>,
) -> Result<String, QueryDiagnostic> {
    let (left, terms) = left_binary_spine(expression);
    let mut sql = "(".repeat(terms.len());
    sql.push_str(&compile_source_free_expression(
        left, snapshot, dialect, parameters,
    )?);
    for (operator, right) in terms {
        sql.push(' ');
        sql.push_str(binary_operator_sql(operator)?);
        sql.push(' ');
        sql.push_str(&compile_source_free_expression(
            right, snapshot, dialect, parameters,
        )?);
        sql.push(')');
    }
    Ok(sql)
}

/// `ПустаяСсылка` / `EmptyRef`: the 16-byte zero reference of an object.
pub(super) fn is_empty_reference_name(name: &str) -> bool {
    names_equal(name, "ПустаяСсылка") || names_equal(name, "EmptyRef")
}

/// Metadata kinds whose objects have a reference table.
pub(super) fn has_reference_table(kind: MetadataKind) -> bool {
    matches!(
        kind,
        MetadataKind::Catalog
            | MetadataKind::Document
            | MetadataKind::Enumeration
            | MetadataKind::ChartOfCharacteristicTypes
            | MetadataKind::ChartOfAccounts
            | MetadataKind::ChartOfCalculationTypes
            | MetadataKind::ExchangePlan
            | MetadataKind::BusinessProcess
            | MetadataKind::Task
    )
}

pub(super) fn compile_metadata_value(
    token: &Token<'_>,
    kind_token: &Token<'_>,
    object_token: &Token<'_>,
    value_token: &Token<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    let kind = kind_from_query_name(kind_token.lexeme).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(kind_token),
            format!("unknown VALUE metadata kind {:?}", kind_token.lexeme),
        )
    })?;
    if is_empty_reference_name(value_token.lexeme) {
        if !has_reference_table(kind) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(kind_token),
                format!("{} has no empty reference", kind.as_str()),
            ));
        }
        snapshot
            .object_id(kind, object_token.lexeme)
            .map_err(|error| {
                QueryDiagnostic::lookup(
                    object_token,
                    error.clone(),
                    format!(
                        "VALUE object {}.{:?} could not be resolved: {error}",
                        kind.as_str(),
                        object_token.lexeme
                    ),
                )
            })?;
        return Ok(dialect.binary_literal(&[0; 16]));
    }
    if !matches!(kind, MetadataKind::Catalog | MetadataKind::Enumeration) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(kind_token),
            "VALUE currently supports only catalogs and enumerations",
        ));
    }
    let object_id = snapshot
        .object_id(kind, object_token.lexeme)
        .map_err(|error| {
            QueryDiagnostic::lookup(
                object_token,
                error.clone(),
                format!(
                    "VALUE object {}.{:?} could not be resolved: {error}",
                    kind.as_str(),
                    object_token.lexeme
                ),
            )
        })?;
    let value = snapshot
        .predefined_value(object_id, value_token.lexeme)
        .map_err(|error| {
            QueryDiagnostic::lookup(
                value_token,
                error.clone(),
                format!(
                    "VALUE {:?}.{:?}.{:?} could not be resolved: {error}",
                    kind_token.lexeme, object_token.lexeme, value_token.lexeme
                ),
            )
        })?;
    let literal = dialect.binary_literal(&value.guid.to_1c_bytes());
    if kind == MetadataKind::Enumeration {
        return Ok(literal);
    }

    let object = snapshot.object_by_id(object_id).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            "VALUE catalog object disappeared from metadata index",
        )
    })?;
    let physical_table = object.physical_table.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(object_token),
            "VALUE catalog has no physical table",
        )
    })?;
    let table = snapshot.live_table(physical_table).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::NotLive,
            Some(object_token),
            "VALUE catalog table is not live",
        )
    })?;
    let id = table
        .columns
        .iter()
        .find(|column| names_equal(&column.name, "_IDRRef"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(object_token),
                "VALUE catalog has no _IDRRef column",
            )
        })?;
    let predefined = table
        .columns
        .iter()
        .find(|column| names_equal(&column.name, "_PredefinedID"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(object_token),
                "VALUE catalog has no _PredefinedID column",
            )
        })?;
    let alias = "__open_sdbl_value";
    Ok(format!(
        "(SELECT {} FROM {} AS {} WHERE ({} = {literal}))",
        dialect.qualified_column(Some(alias), &id.name),
        dialect.quote_identifier(&table.name),
        dialect.quote_identifier(alias),
        dialect.qualified_column(Some(alias), &predefined.name),
    ))
}

/// Whether a projection yields an aggregated value: an aggregate call or a
/// scalar expression containing one.
pub(super) fn projection_is_aggregated(projection: &Projection<'_, '_>) -> bool {
    match projection {
        Projection::Aggregate { .. } => true,
        Projection::Scalar(expression) => contains_aggregate(expression),
        Projection::All | Projection::Field(_) | Projection::Presentation { .. } => false,
    }
}

/// Whether an expression contains an aggregate call anywhere.
pub(super) fn contains_aggregate(expression: &Expression<'_, '_>) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Expression::Aggregate { .. } => return true,
            Expression::Field(_)
            | Expression::Literal(_)
            | Expression::Parameter(_)
            | Expression::DateTime { .. }
            | Expression::MetadataValue { .. }
            | Expression::Uuid { .. } => {}
            Expression::BeginOfPeriod { value, .. }
            | Expression::Unary { value, .. }
            | Expression::IsNull { value, .. }
            | Expression::Cast {
                argument: value, ..
            } => pending.push(value),
            Expression::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            Expression::InList { value, items } => {
                pending.push(value);
                pending.extend(items.iter());
            }
            Expression::Case {
                branches,
                otherwise,
                ..
            } => {
                pending.extend(otherwise.as_deref());
                for branch in branches {
                    pending.push(&branch.when);
                    pending.push(&branch.then);
                }
            }
            Expression::IsNullFunction {
                value, fallback, ..
            } => {
                pending.push(value);
                pending.push(fallback);
            }
            Expression::Like {
                value,
                pattern,
                escape,
                ..
            } => {
                pending.push(value);
                pending.push(pattern);
                pending.extend(escape.as_deref());
            }
        }
    }
    false
}

/// Without `GROUP BY`, a branch projects either only aggregated values or
/// none; grouped branches are validated against their keys later.
pub(super) fn validate_aggregate_projection(
    ast: &SelectAst<'_, '_>,
) -> Result<(), QueryDiagnostic> {
    if !ast.group.is_empty() {
        if let Some(projection) = ast
            .projection
            .iter()
            .find(|projection| matches!(projection.expression, Projection::All))
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                projection.alias,
                "'*' cannot be combined with GROUP BY",
            ));
        }
        return Ok(());
    }
    let aggregated = ast
        .projection
        .iter()
        .find(|projection| projection_is_aggregated(&projection.expression));
    if let Some(aggregated) = aggregated
        && let Some(plain) = ast
            .projection
            .iter()
            .find(|projection| !projection_is_aggregated(&projection.expression))
    {
        let token = projection_token(&plain.expression)
            .or_else(|| projection_token(&aggregated.expression));
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            token,
            "aggregates cannot be mixed with non-aggregate projections without GROUP BY",
        ));
    }
    Ok(())
}

/// A token positioning diagnostics about a projection.
pub(super) fn projection_token<'tokens, 'source>(
    projection: &Projection<'tokens, 'source>,
) -> Option<&'tokens Token<'source>> {
    match projection {
        Projection::All => None,
        Projection::Field(reference) => Some(reference.last()),
        Projection::Scalar(expression) => operand_token(expression),
        Projection::Aggregate { token, .. } | Projection::Presentation { token, .. } => Some(token),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReferencePresentationTargets {
    Scalar,
    Static(Vec<(ObjectId, u32)>),
    Deferred,
}

pub(super) fn presentation_targets(
    snapshot: &MetadataSnapshot,
    owner: ObjectId,
    field: &QueryableField,
    token: &Token<'_>,
) -> Result<ReferencePresentationTargets, QueryDiagnostic> {
    if names_equal(&field.schema_name, "ID") {
        let object = snapshot.object_by_id(owner).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(token),
                "reference owner was not resolved",
            )
        })?;
        return Ok(ReferencePresentationTargets::Static(vec![(
            owner,
            object.number.unwrap_or_default(),
        )]));
    }
    if field.reference_targets.is_empty() {
        return Ok(ReferencePresentationTargets::Scalar);
    }
    if field.reference_targets.iter().any(String::is_empty) {
        let _ = reference_column(field, token)?;
        let _ = reference_type_column(field, token)?;
        return Ok(ReferencePresentationTargets::Deferred);
    }
    let mut targets = Vec::new();
    for target in &field.reference_targets {
        let physical = format!("_{}", target.strip_prefix('_').unwrap_or(target));
        let matches = snapshot
            .objects()
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| names_equal(table, &physical))
            })
            .collect::<Vec<_>>();
        let object = match matches.as_slice() {
            [object] => *object,
            [] => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnknownObject,
                    Some(token),
                    format!("reference target {physical:?} was not resolved"),
                ));
            }
            _ => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::AmbiguousObject,
                    Some(token),
                    format!("reference target {physical:?} is ambiguous"),
                ));
            }
        };
        let id = ObjectId::from(&object.guid);
        if !targets.iter().any(|(candidate, _)| *candidate == id) {
            targets.push((id, object.number.unwrap_or_default()));
        }
    }
    Ok(ReferencePresentationTargets::Static(targets))
}

pub(super) fn compile_deferred_reference_presentation(
    source_alias: &str,
    field: &QueryableField,
    token: &Token<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    let reference = reference_column(field, token)?;
    let type_column = reference_type_column(field, token)?;
    Ok(dialect.reference_payload(
        &dialect.qualified_column(Some(source_alias), &type_column.physical_name),
        &dialect.qualified_column(Some(source_alias), &reference.physical_name),
    ))
}

pub(super) fn wrap_reference_presentation(
    source_alias: &str,
    reference_column: &str,
    type_column: Option<&str>,
    variants: &[(u32, String)],
    dialect: SqlDialect,
) -> String {
    let reference = dialect.qualified_column(Some(source_alias), reference_column);
    if variants.len() == 1 {
        return format!(
            "CASE WHEN {reference} IS NULL THEN {} ELSE {} END",
            dialect.string_literal(""),
            variants[0].1
        );
    }
    let type_column = type_column.expect("multiple reference targets have RTRef");
    let type_value = dialect.qualified_column(Some(source_alias), type_column);
    let empty = dialect.string_literal("");
    let mut sql = format!("CASE WHEN {reference} IS NULL THEN {empty}");
    for (number, expression) in variants {
        use std::fmt::Write as _;
        write!(
            sql,
            " WHEN {type_value} = {} THEN {expression}",
            dialect.binary_u32(*number)
        )
        .expect("writing to String cannot fail");
    }
    sql.push_str(&format!(" ELSE {empty} END"));
    sql
}

pub(super) struct CompiledSourceRelation {
    pub(super) sql: String,
    pub(super) fields: Arc<[QueryableField]>,
}

pub(super) fn compile_live_relation(
    snapshot: &MetadataSnapshot,
    canonical: &LiveTable,
    fields: &[QueryableField],
    dialect: SqlDialect,
) -> String {
    let canonical_name = extension_table_base(&canonical.name).unwrap_or(&canonical.name);
    let mut tables = snapshot
        .live_table(canonical_name)
        .into_iter()
        .chain(snapshot.extension_live_tables(canonical_name))
        .collect::<Vec<_>>();
    tables.sort_by_key(|table| table.name.to_ascii_lowercase());
    if tables.len() == 1 {
        return dialect.quote_identifier(&tables[0].name);
    }

    let mut seen = BTreeSet::new();
    let columns = fields
        .iter()
        .flat_map(|field| &field.columns)
        .filter(|column| seen.insert(column.physical_name.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let branches = tables
        .into_iter()
        .map(|table| {
            let projection = columns
                .iter()
                .map(|column| {
                    let value = table
                        .columns
                        .iter()
                        .find(|candidate| names_equal(&candidate.name, &column.physical_name))
                        .map_or_else(
                            || "NULL".to_owned(),
                            |candidate| dialect.quote_identifier(&candidate.name),
                        );
                    format!(
                        "{value} AS {}",
                        dialect.quote_identifier(&column.physical_name)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "SELECT {projection} FROM {}",
                dialect.quote_identifier(&table.name)
            )
        })
        .collect::<Vec<_>>();
    format!("({})", branches.join(" UNION ALL "))
}

fn extension_table_base(candidate: &str) -> Option<&str> {
    let position = candidate.rfind(['X', 'x'])?;
    let (prefix, suffix) = candidate.split_at(position);
    let number = &suffix[1..];
    prefix
        .as_bytes()
        .last()
        .is_some_and(u8::is_ascii_digit)
        .then_some(())
        .filter(|()| number.bytes().all(|digit| digit.is_ascii_digit()))
        .map(|()| prefix)
}

pub(super) fn compile_source_relation(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    live_table: &LiveTable,
    fields: &[QueryableField],
    dialect: SqlDialect,
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    if let Some(accumulation) = &source.accumulation {
        return compile_accumulation_relation(
            source,
            accumulation,
            snapshot,
            catalog,
            object,
            live_table,
            fields,
            dialect,
        );
    }
    let Some(slice) = &source.slice else {
        return Ok(CompiledSourceRelation {
            sql: compile_live_relation(snapshot, live_table, fields, dialect),
            fields: fields.to_vec().into(),
        });
    };
    if object.kind != Some(MetadataKind::InformationRegister) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(slice.token),
            format!(
                "{} is supported only for information registers",
                slice.kind.name()
            ),
        ));
    }
    let period = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "Period"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(slice.token),
                format!("{} requires a live Period field", slice.kind.name()),
            )
        })?;
    let period_column = single_column(period, slice.token)?;

    let physical_table = object
        .physical_table
        .as_deref()
        .expect("a live information register has a physical table");
    let owned_fields = snapshot
        .fields()
        .iter()
        .filter(|field| {
            field
                .owner_tables
                .iter()
                .any(|owner| names_equal(owner, physical_table))
        })
        .collect::<Vec<_>>();
    if !owned_fields.is_empty() && owned_fields.iter().all(|field| field.purpose.is_none()) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(slice.token),
            format!(
                "{} dimension roles are unavailable in Config metadata",
                slice.kind.name()
            ),
        ));
    }

    let mut partition_names = BTreeSet::new();
    let mut partition_columns = Vec::new();
    for metadata_field in owned_fields.iter().filter(|field| {
        field.data_separator
            || field.purpose == Some(ConfigFieldPurpose::InformationRegisterDimension)
    }) {
        let schema_name = format!("Fld{}", metadata_field.number);
        let field = fields
            .iter()
            .find(|field| names_equal(&field.schema_name, &schema_name))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::NotLive,
                    Some(slice.token),
                    format!(
                        "{} dimension {:?} has no live physical representation",
                        slice.kind.name(),
                        metadata_field.name.as_deref().unwrap_or(&schema_name)
                    ),
                )
            })?;
        for column in &field.columns {
            if partition_names.insert(column.physical_name.to_lowercase()) {
                partition_columns
                    .push(dialect.qualified_column(Some("__slice_base"), &column.physical_name));
            }
        }
    }

    let period_bound = slice
        .period
        .as_ref()
        .map(|expression| match expression {
            Expression::Literal(token) => compile_literal(token, dialect),
            Expression::DateTime { .. } | Expression::BeginOfPeriod { .. } => {
                compile_constant_date_expression(expression, dialect)
            }
            Expression::Parameter(token) => {
                compile_date_parameter(token, catalog.parameters, dialect)
            }
            _ => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(slice.token),
                format!("{} period must be a scalar literal", slice.kind.name()),
            )),
        })
        .transpose()?;
    let virtual_condition = if let Some(condition) = &slice.condition {
        let mut condition_context = CompilationContext {
            snapshot,
            catalog,
            sources: vec![SourceScope {
                object: ObjectId::from(&object.guid),
                fields: fields.to_vec().into(),
                relation: String::new(),
                sql_alias: "__slice_base".to_owned(),
                object_name: source.object.lexeme.to_owned(),
                source_alias: Some("__slice_base".to_owned()),
                identity_is_base: true,
                reference_joins: Vec::new(),
            }],
            dialect,
            aggregates_allowed: false,
        };
        let sql = compile_expression(condition, &mut condition_context)?;
        if !condition_context.sources[0].reference_joins.is_empty() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(slice.token),
                format!(
                    "{} condition supports direct fields only",
                    slice.kind.name()
                ),
            ));
        }
        Some(sql)
    } else {
        None
    };

    let qualified_period =
        dialect.qualified_column(Some("__slice_base"), &period_column.physical_name);
    let mut predicates = Vec::new();
    if let Some(period_bound) = period_bound {
        predicates.push(format!(
            "({qualified_period} {} {period_bound})",
            slice.kind.period_operator()
        ));
    }
    if let Some(condition) = virtual_condition {
        predicates.push(condition);
    }
    let partition = if partition_columns.is_empty() {
        String::new()
    } else {
        format!("PARTITION BY {} ", partition_columns.join(", "))
    };
    let slice_base = dialect.quote_identifier("__slice_base");
    let slice_ranked = dialect.quote_identifier("__slice_ranked");
    let slice_rank = dialect.quote_identifier("__open_sdbl_slice_rank");
    let qualified_rank = dialect.qualified_column(Some("__slice_ranked"), "__open_sdbl_slice_rank");
    let mut relation = format!(
        "(SELECT {slice_ranked}.* FROM (SELECT {slice_base}.*, DENSE_RANK() OVER ({partition}ORDER BY {qualified_period} {}) AS {slice_rank} FROM {} AS {slice_base}",
        slice.kind.order(),
        dialect.quote_identifier(&live_table.name),
    );
    if !predicates.is_empty() {
        relation.push_str(" WHERE ");
        relation.push_str(&predicates.join(" AND "));
    }
    relation.push_str(&format!(") AS {slice_ranked} WHERE {qualified_rank} = 1)"));
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: fields.to_vec().into(),
    })
}
