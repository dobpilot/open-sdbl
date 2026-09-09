use super::context::CompilationContext;
use super::sources::compile_metadata_value;
use crate::metadata::MetadataSnapshot;
use crate::query::core::ast::{AggregateArgument, AggregateKind, CastTarget, Expression};
use crate::query::core::dialect::compile_literal;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, QueryableColumn, QueryableField, kind_from_query_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

/// Compiles an expression in a predicate position (`WHERE`, `ON`, operands
/// of `AND`/`OR`/`NOT`). SQL Server has no boolean expressions, so boolean
/// fields, literals, and casts become explicit comparisons there; other
/// dialects and expressions compile unchanged.
pub(super) fn compile_predicate(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = single_column(resolved.field(), reference.last())?;
            let sql = context
                .dialect
                .qualified_column(Some(&resolved.sql_alias), &column.physical_name);
            Ok(if column.kind == ColumnKind::Boolean {
                context.dialect.boolean_predicate(&sql)
            } else {
                sql
            })
        }
        Expression::Literal(token) => match token.kind {
            TokenKind::Keyword(Keyword::True) => {
                Ok(context.dialect.boolean_literal_predicate(true))
            }
            TokenKind::Keyword(Keyword::False) => {
                Ok(context.dialect.boolean_literal_predicate(false))
            }
            _ => compile_expression(expression, context),
        },
        Expression::Cast {
            target: CastTarget::Boolean,
            ..
        } => {
            let sql = compile_expression(expression, context)?;
            Ok(context.dialect.boolean_predicate(&sql))
        }
        Expression::Unary { operator, value }
            if operator.kind == TokenKind::Keyword(Keyword::Not) =>
        {
            Ok(format!("(NOT {})", compile_predicate(value, context)?))
        }
        Expression::Binary { operator, .. }
            if matches!(
                operator.kind,
                TokenKind::Keyword(Keyword::And | Keyword::Or)
            ) =>
        {
            // Walk the left-associative AND/OR spine iteratively so that very
            // long conjunctions do not recurse once per operator.
            let mut leftmost = expression;
            let mut terms = Vec::new();
            while let Expression::Binary {
                left,
                operator,
                right,
            } = leftmost
                && matches!(
                    operator.kind,
                    TokenKind::Keyword(Keyword::And | Keyword::Or)
                )
            {
                terms.push((*operator, right.as_ref()));
                leftmost = left.as_ref();
            }
            terms.reverse();
            let mut sql = "(".repeat(terms.len());
            sql.push_str(&compile_predicate(leftmost, context)?);
            for (operator, right) in terms {
                sql.push(' ');
                sql.push_str(binary_operator_sql(operator)?);
                sql.push(' ');
                sql.push_str(&compile_predicate(right, context)?);
                sql.push(')');
            }
            Ok(sql)
        }
        _ => compile_expression(expression, context),
    }
}

/// A `<Kind>.<Object>` cast target before resolution.
#[derive(Clone, Copy)]
struct NarrowingTarget<'tokens, 'source> {
    kind: &'tokens Token<'source>,
    object: &'tokens Token<'source>,
}

struct NarrowedReference {
    sql: String,
    kind: ColumnKind,
}

/// Compiles `ВЫРАЗИТЬ(<field> КАК <Kind>.<Object>)[.<Field>]`.
fn compile_narrowed_reference(
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    argument: &Expression<'_, '_>,
    target: NarrowingTarget<'_, '_>,
    path: Option<&Token<'_>>,
) -> Result<NarrowedReference, QueryDiagnostic> {
    let Expression::Field(reference) = argument else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(token),
            "CAST to a metadata type expects a reference field",
        ));
    };
    let kind = kind_from_query_name(target.kind.lexeme).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(target.kind),
            format!("unknown CAST metadata kind {:?}", target.kind.lexeme),
        )
    })?;
    let target_id = context
        .snapshot
        .object_id(kind, target.object.lexeme)
        .map_err(|error| {
            QueryDiagnostic::lookup(
                target.object,
                error.clone(),
                format!(
                    "CAST target {}.{:?} could not be resolved: {error}",
                    kind.as_str(),
                    target.object.lexeme
                ),
            )
        })?;
    let target_object = context.snapshot.object_by_id(target_id).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(target.object),
            "CAST target disappeared from the metadata index",
        )
    })?;
    let resolved = context.resolve_direct(reference)?;
    let field = resolved.field();
    let is_reference = !field.reference_targets.is_empty()
        || field
            .columns
            .iter()
            .any(QueryableColumn::is_reference_value_member);
    let reference_member = if is_reference {
        reference_column(field, reference.last()).ok()
    } else {
        None
    };
    let Some(reference_member) = reference_member else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(token),
            format!(
                "CAST argument {:?} must be a reference field",
                reference.last().lexeme
            ),
        ));
    };
    let type_member = field
        .columns
        .iter()
        .find(|column| column.is_reference_type_member());
    let fixed_targets = field
        .reference_targets
        .iter()
        .filter(|target| !target.is_empty())
        .collect::<Vec<_>>();
    let universal = field.reference_targets.iter().any(String::is_empty);
    if !universal && !fixed_targets.is_empty() {
        let admissible = target_object
            .physical_table
            .as_deref()
            .is_some_and(|table| {
                fixed_targets.iter().any(|candidate| {
                    names_equal(
                        table.strip_prefix('_').unwrap_or(table),
                        candidate.strip_prefix('_').unwrap_or(candidate),
                    )
                })
            });
        if !admissible {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(target.object),
                format!(
                    "field {:?} cannot hold {}.{}",
                    field.name,
                    kind.as_str(),
                    target.object.lexeme
                ),
            ));
        }
    }
    let scope = resolved.scope;
    let source_alias = resolved.sql_alias.clone();
    let reference_sql = context
        .dialect
        .qualified_column(Some(&source_alias), &reference_member.physical_name);

    let Some(path) = path else {
        let sql = match type_member {
            Some(type_member) => {
                let number = target_object.number.ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Metadata,
                        Some(target.object),
                        "CAST target has no database type number",
                    )
                })?;
                context.dialect.narrowed_reference(
                    &context
                        .dialect
                        .qualified_column(Some(&source_alias), &type_member.physical_name),
                    number,
                    &reference_sql,
                )
            }
            None => reference_sql,
        };
        return Ok(NarrowedReference {
            sql,
            kind: ColumnKind::Reference {
                targets: vec![target_id],
                runtime_typed: false,
            },
        });
    };

    let field = field.clone();
    let alias = context.ensure_presentation_join(
        scope,
        &source_alias,
        &field,
        target_id,
        type_member.is_some(),
        token,
    )?;
    let target_fields = context.catalog.fields(target_object, Some(token))?;
    let (_, target_field) = resolve_named_field(&target_fields, path)?;
    let column = single_column(target_field, path)?;
    Ok(NarrowedReference {
        sql: context.dialect.column_projection(
            &context
                .dialect
                .qualified_column(Some(&alias), &column.physical_name),
            &column.kind,
            &column.data_type,
        ),
        kind: column.kind.clone(),
    })
}

fn scalar_cast_kind(target: CastTarget<'_, '_>) -> ColumnKind {
    match target {
        CastTarget::String { length } => ColumnKind::String { length },
        CastTarget::Number { precision, scale } => ColumnKind::Number { precision, scale },
        CastTarget::Boolean => ColumnKind::Boolean,
        CastTarget::Date => ColumnKind::DateTime,
        CastTarget::Reference { .. } => ColumnKind::Unknown {
            data_type: String::new(),
        },
    }
}

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
        Expression::Cast {
            token,
            argument,
            target: CastTarget::Reference { kind, object },
            path,
        } => Ok(compile_narrowed_reference(
            context,
            token,
            argument,
            NarrowingTarget { kind, object },
            *path,
        )?
        .kind),
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
        Expression::Uuid { .. } => ColumnKind::Uuid,
        Expression::Cast { target, .. } => scalar_cast_kind(*target),
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
        Expression::Cast {
            token,
            argument,
            target,
            path,
        } => match target {
            CastTarget::Reference { kind, object } => Ok(compile_narrowed_reference(
                context,
                token,
                argument,
                NarrowingTarget { kind, object },
                *path,
            )?
            .sql),
            scalar => {
                let inner = compile_expression(argument, context)?;
                Ok(context.dialect.cast_scalar(&inner, *scalar))
            }
        },
        Expression::Uuid { token, argument } => {
            let resolved = context.resolve(argument)?;
            let field = resolved.field();
            let is_reference = !field.reference_targets.is_empty()
                || field
                    .columns
                    .iter()
                    .any(QueryableColumn::is_reference_value_member);
            if !is_reference {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(token),
                    format!(
                        "UUID argument {:?} must be a reference field",
                        argument.last().lexeme
                    ),
                ));
            }
            let column = reference_column(field, argument.last())?;
            Ok(context
                .dialect
                .reference_uuid(&context.sql_column(&resolved, column)))
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
