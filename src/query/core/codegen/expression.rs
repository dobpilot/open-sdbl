use super::context::CompilationContext;
use super::sources::compile_metadata_value;
use crate::metadata::MetadataSnapshot;
use crate::query::core::ast::{AggregateArgument, AggregateKind, Expression};
use crate::query::core::dialect::compile_literal;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, QueryableColumn, QueryableField, kind_from_query_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

/// Derives the output kind of a compiled scalar expression. Must be called
/// only after [`compile_expression`] succeeded for the same expression, so
/// every field it names resolves.
pub(super) fn expression_kind(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<ColumnKind, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = single_column(resolved.field(), reference.last())?;
            Ok(column.kind.clone())
        }
        Expression::Unary { operator, value } => match operator.kind {
            TokenKind::Keyword(Keyword::Not) => Ok(ColumnKind::Boolean),
            _ => expression_kind(value, context),
        },
        _ => Ok(source_free_expression_kind(expression, context.snapshot)),
    }
}

/// Derives the output kind of an expression that names no source field.
pub(super) fn source_free_expression_kind(
    expression: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
) -> ColumnKind {
    match expression {
        Expression::Field(_) => ColumnKind::Unknown {
            data_type: String::new(),
        },
        Expression::Literal(token) => literal_kind(token),
        Expression::DateTime { .. } | Expression::BeginOfPeriod { .. } => ColumnKind::DateTime,
        Expression::MetadataValue { kind, object, .. } => ColumnKind::Reference {
            targets: kind_from_query_name(kind.lexeme)
                .and_then(|kind| snapshot.object_id(kind, object.lexeme).ok())
                .into_iter()
                .collect(),
            runtime_typed: false,
        },
        Expression::Unary { operator, value } => match operator.kind {
            TokenKind::Keyword(Keyword::Not) => ColumnKind::Boolean,
            _ => source_free_expression_kind(value, snapshot),
        },
        Expression::Binary { operator, .. } => match operator.kind {
            TokenKind::Keyword(_) => ColumnKind::Boolean,
            _ if matches!(operator.lexeme, "+" | "-" | "*" | "/") => ColumnKind::Number {
                precision: None,
                scale: None,
            },
            _ => ColumnKind::Boolean,
        },
        Expression::InList { .. } | Expression::IsNull { .. } => ColumnKind::Boolean,
    }
}

fn literal_kind(token: &Token<'_>) -> ColumnKind {
    match token.kind {
        TokenKind::Number => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        TokenKind::String => ColumnKind::String { length: None },
        TokenKind::Binary => ColumnKind::Binary { length: None },
        TokenKind::Keyword(Keyword::True | Keyword::False) => ColumnKind::Boolean,
        TokenKind::Keyword(Keyword::Null) => ColumnKind::Null,
        _ => ColumnKind::Unknown {
            data_type: token.lexeme.to_owned(),
        },
    }
}

pub(super) fn compile_expression(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = single_column(resolved.field(), reference.last())?;
            Ok(context
                .dialect
                .qualified_column(Some(&resolved.sql_alias), &column.physical_name))
        }
        Expression::Literal(token) => compile_literal(token, context.dialect),
        Expression::DateTime { token, value } => {
            context.dialect.datetime_expression(*value, true, token)
        }
        Expression::BeginOfPeriod {
            token,
            value,
            period,
        } => {
            let value = compile_date_operand(value, context, token)?;
            Ok(context.dialect.begin_of_period(&value, *period))
        }
        Expression::MetadataValue {
            token,
            kind,
            object,
            value,
        } => compile_metadata_value(
            token,
            kind,
            object,
            value,
            context.snapshot,
            context.dialect,
        ),
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
                compile_expression(value, context)?
            ))
        }
        Expression::Binary {
            left: _,
            operator: _,
            right: _,
        } => compile_binary_expression(expression, context),
        Expression::InList { value, items } => {
            let value_sql = compile_expression(value, context)?;
            let item_sql = items
                .iter()
                .map(|item| compile_expression_operand(item, value, context))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({value_sql} IN ({}))", item_sql.join(", ")))
        }
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_expression(value, context)?,
            if *negated { "NOT " } else { "" }
        )),
    }
}

fn compile_binary_expression(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let (left, terms) = left_binary_spine(expression);
    let mut sql = "(".repeat(terms.len());
    let Some((_, first_right)) = terms.first() else {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            "binary expression compiler received a non-binary expression",
        ));
    };
    sql.push_str(&compile_expression_operand(left, first_right, context)?);
    for (index, (operator, right)) in terms.into_iter().enumerate() {
        sql.push(' ');
        sql.push_str(binary_operator_sql(operator)?);
        sql.push(' ');
        if index == 0 {
            sql.push_str(&compile_expression_operand(right, left, context)?);
        } else {
            sql.push_str(&compile_expression(right, context)?);
        }
        sql.push(')');
    }
    Ok(sql)
}

fn compile_expression_operand(
    expression: &Expression<'_, '_>,
    other: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    if let (Expression::Literal(token), Expression::Field(reference)) = (expression, other) {
        let resolved = context.resolve(reference)?;
        let column = single_column(resolved.field(), reference.last())?;
        return context.dialect.literal_for_type(token, &column.data_type);
    }
    compile_expression(expression, context)
}

fn compile_date_operand(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    if let Expression::Field(reference) = expression {
        let resolved = context.resolve(reference)?;
        let column = single_column(resolved.field(), reference.last())?;
        if !is_date_sql_type(&column.data_type) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "BEGINOFPERIOD first argument must resolve to a date field",
            ));
        }
        return Ok(context
            .dialect
            .qualified_column(Some(&resolved.sql_alias), &column.physical_name));
    }
    if expression.is_date() {
        return compile_expression(expression, context);
    }
    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(token),
        "BEGINOFPERIOD first argument must be a date expression",
    ))
}

fn is_date_sql_type(data_type: &str) -> bool {
    ColumnKind::from_catalog_type(data_type) == ColumnKind::DateTime
}

/// Compiles one aggregate projection and reports its output kind: `COUNT`
/// and `SUM` are numbers, `MIN`/`MAX` keep the kind of their argument.
pub(super) fn compile_aggregate(
    context: &mut CompilationContext<'_, '_>,
    kind: AggregateKind,
    distinct: bool,
    argument: &AggregateArgument<'_, '_>,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let number = ColumnKind::Number {
        precision: None,
        scale: None,
    };
    let (argument, argument_kind) = match argument {
        AggregateArgument::All => ("*".to_owned(), number.clone()),
        AggregateArgument::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = countable_column(resolved.field(), reference.last())?;
            let column_kind = match &column.kind {
                // An aggregate over the RRRef member alone returns 16 bytes.
                ColumnKind::Reference { targets, .. } => ColumnKind::Reference {
                    targets: targets.clone(),
                    runtime_typed: false,
                },
                other => other.clone(),
            };
            (context.sql_column(&resolved, column), column_kind)
        }
    };
    let output_kind = match kind {
        AggregateKind::Count | AggregateKind::Sum => number,
        _ => argument_kind,
    };
    Ok((
        format!(
            "{}({}{argument})",
            kind.sql_name(),
            if distinct { "DISTINCT " } else { "" }
        ),
        output_kind,
    ))
}

fn countable_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ if !field.reference_targets.is_empty() => reference_column(field, token),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "compound field {:?} cannot be used as an aggregate argument",
                field.name
            ),
        )),
    }
}

pub(super) fn reference_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    let candidates = field
        .columns
        .iter()
        .filter(|column| {
            let lower = column.physical_name.to_ascii_lowercase();
            lower.ends_with("rref") && !lower.ends_with("rtref")
        })
        .collect::<Vec<_>>();
    match (field.columns.as_slice(), candidates.as_slice()) {
        ([column], _) => Ok(column),
        (_, [column]) => Ok(*column),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!(
                "reference field {:?} has no unique RRef physical member",
                field.name
            ),
        )),
    }
}

pub(super) fn reference_type_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    let candidates = field
        .columns
        .iter()
        .filter(|column| column.physical_name.to_ascii_lowercase().ends_with("rtref"))
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [column] => Ok(*column),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!(
                "multi-target reference field {:?} has no unique RTRef physical member",
                field.name
            ),
        )),
    }
}

pub(super) fn resolve_named_field<'field>(
    fields: &'field [QueryableField],
    name: &Token<'_>,
) -> Result<(usize, &'field QueryableField), QueryDiagnostic> {
    let matches = matching_fields(fields, name);
    match matches.as_slice() {
        [field] => Ok(*field),
        [] => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownField,
            Some(name),
            format!("field {:?} was not found", name.lexeme),
        )),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::AmbiguousField,
            Some(name),
            format!("field {:?} is ambiguous", name.lexeme),
        )),
    }
}

pub(super) fn matching_fields<'field>(
    fields: &'field [QueryableField],
    name: &Token<'_>,
) -> Vec<(usize, &'field QueryableField)> {
    fields
        .iter()
        .enumerate()
        .filter(|(_, field)| {
            field
                .aliases
                .iter()
                .any(|alias| names_equal(alias, name.lexeme))
        })
        .collect()
}

pub(super) fn single_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    single_column_at(field, Some(token))
}

pub(super) fn single_column_at<'field>(
    field: &'field QueryableField,
    token: Option<&Token<'_>>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ => Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            token,
            format!(
                "compound field {:?} can be projected but not used in expressions",
                field.name
            ),
        )),
    }
}

type BinaryTerm<'expression, 'tokens, 'source> = (
    &'tokens Token<'source>,
    &'expression Expression<'tokens, 'source>,
);

pub(super) fn left_binary_spine<'expression, 'tokens, 'source>(
    expression: &'expression Expression<'tokens, 'source>,
) -> (
    &'expression Expression<'tokens, 'source>,
    Vec<BinaryTerm<'expression, 'tokens, 'source>>,
) {
    let mut leftmost = expression;
    let mut terms = Vec::new();
    while let Expression::Binary {
        left,
        operator,
        right,
    } = leftmost
    {
        terms.push((*operator, right.as_ref()));
        leftmost = left.as_ref();
    }
    terms.reverse();
    (leftmost, terms)
}

pub(super) fn binary_operator_sql<'source>(
    operator: &Token<'source>,
) -> Result<&'source str, QueryDiagnostic> {
    match operator.kind {
        TokenKind::Keyword(Keyword::And) => Ok("AND"),
        TokenKind::Keyword(Keyword::Or) => Ok("OR"),
        _ if matches!(
            operator.lexeme,
            "=" | "<>" | "<" | ">" | "<=" | ">=" | "+" | "-" | "*" | "/"
        ) =>
        {
            Ok(operator.lexeme)
        }
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(operator),
            "unsupported binary operator",
        )),
    }
}
