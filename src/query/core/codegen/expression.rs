use super::context::{CompilationContext, ResolvedPath};
use super::orchestrate::{PresentationCompilation, compile_query_ast};
use super::params::{
    ReferenceConstant, list_elements, object_type_number, parameter_kind,
    reference_constant_of_bytes, reference_constant_of_value, render_scalar_parameter,
};
use super::sources::compile_metadata_value;
use crate::metadata::MetadataSnapshot;
use crate::metadata::ObjectId;
use crate::query::core::ast::{
    AggregateArgument, AggregateKind, CaseBranch, CastTarget, Expression, FieldReference,
};
use crate::query::core::dialect::{SqlDialect, compile_literal, decode_binary_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::Parameters;
use crate::query::core::resolve::{
    ColumnKind, QueryableColumn, QueryableField, kind_from_query_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};
use std::collections::BTreeSet;

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
            let sql = context.sql_column(&resolved, column);
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
        Expression::Case { .. } | Expression::IsNullFunction { .. } | Expression::Parameter(_) => {
            let sql = compile_expression(expression, context)?;
            Ok(
                if expression_kind(expression, context)? == ColumnKind::Boolean {
                    context.dialect.boolean_predicate(&sql)
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
            let value_sql = compile_expression(value, context)?;
            let pattern_sql = compile_expression(pattern, context)?;
            let escape_sql = escape
                .as_ref()
                .map(|escape| compile_expression(escape, context))
                .transpose()?;
            for operand in [value.as_ref(), pattern.as_ref()]
                .into_iter()
                .chain(escape.as_deref())
            {
                let kind = expression_kind(operand, context)?;
                check_like_operand(token, &kind)?;
            }
            Ok(render_like(
                &value_sql,
                &pattern_sql,
                escape_sql.as_deref(),
                *negated,
            ))
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
        Expression::Case {
            branches,
            otherwise,
            ..
        } => {
            let mut operands = Vec::with_capacity(branches.len() + 1);
            for branch in branches {
                operands.push(Operand {
                    token: branch.token,
                    sql: String::new(),
                    kind: value_operand_kind(&branch.then, context)?,
                });
            }
            if let Some(otherwise) = otherwise {
                operands.push(Operand {
                    token: operand_token(otherwise).unwrap_or(branches[0].token),
                    sql: String::new(),
                    kind: value_operand_kind(otherwise, context)?,
                });
            }
            Ok(common_kind(&operands)?.kind)
        }
        Expression::IsNullFunction {
            token,
            value,
            fallback,
        } => {
            let operands = [
                Operand {
                    token,
                    sql: String::new(),
                    kind: value_operand_kind(value, context)?,
                },
                Operand {
                    token,
                    sql: String::new(),
                    kind: value_operand_kind(fallback, context)?,
                },
            ];
            Ok(common_kind(&operands)?.kind)
        }
        Expression::Like { .. } => Ok(ColumnKind::Boolean),
        Expression::Parameter(token) => Ok(context
            .catalog
            .parameters
            .lookup(token)?
            .map_or_else(unknown_kind, parameter_kind)),
        Expression::Aggregate { kind, argument, .. } => match (kind, argument) {
            (AggregateKind::Count | AggregateKind::Sum, _) | (_, AggregateArgument::All) => {
                Ok(ColumnKind::Number {
                    precision: None,
                    scale: None,
                })
            }
            (_, AggregateArgument::Expression(argument)) => match argument.as_ref() {
                Expression::Field(reference) => {
                    let resolved = context.resolve(reference)?;
                    let column = countable_column(resolved.field(), reference.last())?;
                    Ok(aggregated_field_kind(&column.kind))
                }
                other => expression_kind(other, context),
            },
        },
        _ => Ok(source_free_expression_kind(
            expression,
            context.snapshot,
            context.catalog.parameters,
        )),
    }
}

fn unknown_kind() -> ColumnKind {
    ColumnKind::Unknown {
        data_type: String::new(),
    }
}

/// The token that positions diagnostics about an operand expression.
pub(super) fn operand_token<'tokens, 'source>(
    expression: &Expression<'tokens, 'source>,
) -> Option<&'tokens Token<'source>> {
    match expression {
        Expression::Field(reference) => Some(reference.last()),
        Expression::Literal(token)
        | Expression::DateTime { token, .. }
        | Expression::BeginOfPeriod { token, .. }
        | Expression::MetadataValue { token, .. }
        | Expression::Uuid { token, .. }
        | Expression::Cast { token, .. }
        | Expression::Case { token, .. }
        | Expression::IsNullFunction { token, .. }
        | Expression::Like { token, .. }
        | Expression::Parameter(token)
        | Expression::Aggregate { token, .. } => Some(token),
        Expression::Unary { operator, .. } | Expression::Binary { operator, .. } => Some(operator),
        Expression::InList { value, .. } | Expression::IsNull { value, .. } => operand_token(value),
        Expression::InQuery { token, .. } => Some(token),
    }
}

/// The physical reference pair of a field, when it has one.
fn reference_pair(field: &QueryableField) -> Option<(&QueryableColumn, &QueryableColumn)> {
    let type_member = field
        .columns
        .iter()
        .find(|column| column.is_reference_type_member())?;
    let value_member = field
        .columns
        .iter()
        .find(|column| column.is_reference_value_member())?;
    Some((type_member, value_member))
}

/// Compiles an operand that contributes a value to a `ВЫБОР`/`ЕСТЬNULL`
/// result: a runtime-typed reference field becomes its `RTRef ‖ RRRef`
/// payload, every other expression compiles as usual.
pub(super) fn value_operand(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    if let Expression::Field(reference) = expression {
        let resolved = context.resolve(reference)?;
        if let Some((type_member, value_member)) = reference_pair(resolved.field()) {
            let sql = context.dialect.reference_payload(
                &context.sql_column(&resolved, type_member),
                &context.sql_column(&resolved, value_member),
            );
            return Ok((sql, payload_kind(&value_member.kind)));
        }
    }
    let sql = compile_expression(expression, context)?;
    let kind = expression_kind(expression, context)?;
    Ok((sql, kind))
}

/// The kind reported by [`value_operand`] without compiling the SQL.
pub(super) fn value_operand_kind(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<ColumnKind, QueryDiagnostic> {
    if let Expression::Field(reference) = expression {
        let resolved = context.resolve(reference)?;
        if let Some((_, value_member)) = reference_pair(resolved.field()) {
            return Ok(payload_kind(&value_member.kind));
        }
    }
    expression_kind(expression, context)
}

fn payload_kind(kind: &ColumnKind) -> ColumnKind {
    match kind {
        ColumnKind::Reference { targets, .. } => ColumnKind::Reference {
            targets: targets.clone(),
            runtime_typed: true,
        },
        other => other.clone(),
    }
}

/// One operand of an expression whose operands must agree on a kind
/// (`ВЫБОР` alternatives, `ЕСТЬNULL` arguments, `ОБЪЕДИНИТЬ` columns).
pub(super) struct Operand<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) sql: String,
    pub(super) kind: ColumnKind,
}

/// The kind shared by several operands and whether reference operands must
/// be widened to one runtime-typed payload.
pub(super) struct CommonKind {
    pub(super) kind: ColumnKind,
    pub(super) widen: bool,
}

/// Computes the common kind of operands: the first non-wildcard kind, with
/// which every other operand must be compatible. Reference operands that
/// differ in target or width are widened to one runtime-typed payload whose
/// targets are the union of the operand targets.
pub(super) fn common_kind(operands: &[Operand<'_, '_>]) -> Result<CommonKind, QueryDiagnostic> {
    let Some(first) = operands
        .iter()
        .map(|operand| &operand.kind)
        .find(|kind| !kind.is_wildcard())
    else {
        return Ok(CommonKind {
            kind: ColumnKind::Null,
            widen: false,
        });
    };
    for operand in operands {
        if !operand.kind.is_compatible_with(first) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(operand.token),
                format!(
                    "expression kinds differ: {:?} where {:?} was expected",
                    operand.kind, first
                ),
            ));
        }
    }
    let ColumnKind::Reference { .. } = first else {
        return Ok(CommonKind {
            kind: first.clone(),
            widen: false,
        });
    };
    let mut targets = BTreeSet::new();
    let mut fixed_target: Option<Option<ObjectId>> = None;
    let mut uniform = true;
    for operand in operands {
        let ColumnKind::Reference {
            targets: operand_targets,
            runtime_typed,
        } = &operand.kind
        else {
            continue;
        };
        targets.extend(operand_targets.iter().copied());
        let single = if *runtime_typed {
            None
        } else {
            match operand_targets.as_slice() {
                [target] => Some(*target),
                _ => None,
            }
        };
        if *runtime_typed || single.is_none() {
            uniform = false;
        }
        match fixed_target {
            None => fixed_target = Some(single),
            Some(previous) if previous != single => uniform = false,
            Some(_) => {}
        }
    }
    if uniform {
        return Ok(CommonKind {
            kind: first.clone(),
            widen: false,
        });
    }
    Ok(CommonKind {
        kind: ColumnKind::Reference {
            targets: targets.into_iter().collect(),
            runtime_typed: true,
        },
        widen: true,
    })
}

/// Rewrites a fixed reference expression into the `RTRef ‖ RRRef` payload
/// of its single target so it can share a column with runtime-typed
/// references. Payload expressions pass through unchanged.
pub(super) fn widen_reference(
    sql: &str,
    kind: &ColumnKind,
    token: Option<&Token<'_>>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let ColumnKind::Reference {
        targets,
        runtime_typed: false,
    } = kind
    else {
        return Ok((sql.to_owned(), kind.clone()));
    };
    let [target] = targets.as_slice() else {
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            token,
            "reference expression without a single fixed target cannot be widened",
        ));
    };
    let number = snapshot
        .object_by_id(*target)
        .and_then(|object| object.number)
        .ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::Metadata,
                token,
                "reference target has no database type number",
            )
        })?;
    Ok((
        dialect.reference_payload(&dialect.binary_u32(number), sql),
        ColumnKind::Reference {
            targets: targets.clone(),
            runtime_typed: true,
        },
    ))
}

/// Unifies operand kinds and widens references in place, returning the
/// common kind.
pub(super) fn unify_operands(
    operands: &mut [Operand<'_, '_>],
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<ColumnKind, QueryDiagnostic> {
    let common = common_kind(operands)?;
    if common.widen {
        for operand in operands.iter_mut() {
            let (sql, kind) = widen_reference(
                &operand.sql,
                &operand.kind,
                Some(operand.token),
                snapshot,
                dialect,
            )?;
            operand.sql = sql;
            operand.kind = kind;
        }
    }
    Ok(common.kind)
}

/// Renders `CASE WHEN … THEN … [ELSE …] END` from compiled alternatives.
/// `values` holds one operand per `WHEN` followed by the `ELSE` operand when
/// `has_else` is set.
pub(super) fn render_case(
    whens: &[String],
    values: &mut [Operand<'_, '_>],
    has_else: bool,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let kind = unify_operands(values, snapshot, dialect)?;
    let mut sql = String::from("CASE");
    for (when, value) in whens.iter().zip(values.iter()) {
        sql.push_str(" WHEN ");
        sql.push_str(when);
        sql.push_str(" THEN ");
        sql.push_str(&value.sql);
    }
    if has_else {
        sql.push_str(" ELSE ");
        sql.push_str(&values[values.len() - 1].sql);
    }
    sql.push_str(" END");
    Ok((sql, kind))
}

/// Renders `COALESCE(value, fallback)` after unifying the operand kinds.
pub(super) fn render_coalesce(
    values: &mut [Operand<'_, '_>; 2],
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let kind = unify_operands(values, snapshot, dialect)?;
    Ok((
        format!("COALESCE({}, {})", values[0].sql, values[1].sql),
        kind,
    ))
}

/// Renders `[NOT] (value LIKE pattern [ESCAPE escape])`.
pub(super) fn render_like(
    value: &str,
    pattern: &str,
    escape: Option<&str>,
    negated: bool,
) -> String {
    let mut sql = format!("({value} LIKE {pattern}");
    if let Some(escape) = escape {
        sql.push_str(" ESCAPE ");
        sql.push_str(escape);
    }
    sql.push(')');
    if negated { format!("(NOT {sql})") } else { sql }
}

/// `ПОДОБНО` operands must be strings; `NULL` and unknown types pass.
pub(super) fn check_like_operand(
    token: &Token<'_>,
    kind: &ColumnKind,
) -> Result<(), QueryDiagnostic> {
    if matches!(
        kind,
        ColumnKind::String { .. } | ColumnKind::Null | ColumnKind::Unknown { .. }
    ) {
        Ok(())
    } else {
        Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!("LIKE operands must be strings, found {kind:?}"),
        ))
    }
}

/// Compiles the alternatives of a `ВЫБОР` expression with the given operand
/// compilers and renders the `CASE`.
pub(super) fn compile_case<'tokens, 'source, E>(
    branches: &[CaseBranch<'tokens, 'source>],
    otherwise: Option<&Expression<'tokens, 'source>>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    mut compile: E,
) -> Result<(String, ColumnKind), QueryDiagnostic>
where
    E: FnMut(&Expression<'tokens, 'source>, bool) -> Result<(String, ColumnKind), QueryDiagnostic>,
{
    let mut whens = Vec::with_capacity(branches.len());
    let mut values = Vec::with_capacity(branches.len() + 1);
    for branch in branches {
        whens.push(compile(&branch.when, true)?.0);
        let (sql, kind) = compile(&branch.then, false)?;
        values.push(Operand {
            token: branch.token,
            sql,
            kind,
        });
    }
    if let Some(otherwise) = otherwise {
        let (sql, kind) = compile(otherwise, false)?;
        values.push(Operand {
            token: operand_token(otherwise).unwrap_or(branches[0].token),
            sql,
            kind,
        });
    }
    render_case(&whens, &mut values, otherwise.is_some(), snapshot, dialect)
}

/// The kind of an aggregate over a field member: aggregating the `RRRef`
/// member of a reference alone returns 16 bytes.
fn aggregated_field_kind(kind: &ColumnKind) -> ColumnKind {
    match kind {
        ColumnKind::Reference { targets, .. } => ColumnKind::Reference {
            targets: targets.clone(),
            runtime_typed: false,
        },
        other => other.clone(),
    }
}

/// Derives the output kind of an expression that names no source field.
pub(super) fn source_free_expression_kind(
    expression: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    parameters: Parameters<'_>,
) -> ColumnKind {
    match expression {
        Expression::Parameter(token) => parameters
            .lookup(token)
            .ok()
            .flatten()
            .map_or_else(unknown_kind, parameter_kind),
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
            _ => source_free_expression_kind(value, snapshot, parameters),
        },
        Expression::Binary { operator, .. } => match operator.kind {
            TokenKind::Keyword(_) => ColumnKind::Boolean,
            _ if matches!(operator.lexeme, "+" | "-" | "*" | "/") => ColumnKind::Number {
                precision: None,
                scale: None,
            },
            _ => ColumnKind::Boolean,
        },
        Expression::InList { .. }
        | Expression::InQuery { .. }
        | Expression::IsNull { .. }
        | Expression::Like { .. } => ColumnKind::Boolean,
        Expression::Case {
            branches,
            otherwise,
            ..
        } => {
            let mut operands = branches
                .iter()
                .map(|branch| Operand {
                    token: branch.token,
                    sql: String::new(),
                    kind: source_free_expression_kind(&branch.then, snapshot, parameters),
                })
                .collect::<Vec<_>>();
            if let Some(otherwise) = otherwise {
                operands.push(Operand {
                    token: operand_token(otherwise).unwrap_or(branches[0].token),
                    sql: String::new(),
                    kind: source_free_expression_kind(otherwise, snapshot, parameters),
                });
            }
            common_kind(&operands).map_or_else(
                |_| ColumnKind::Unknown {
                    data_type: String::new(),
                },
                |common| common.kind,
            )
        }
        Expression::IsNullFunction {
            token,
            value,
            fallback,
        } => {
            let operands = [
                Operand {
                    token,
                    sql: String::new(),
                    kind: source_free_expression_kind(value, snapshot, parameters),
                },
                Operand {
                    token,
                    sql: String::new(),
                    kind: source_free_expression_kind(fallback, snapshot, parameters),
                },
            ];
            common_kind(&operands).map_or_else(
                |_| ColumnKind::Unknown {
                    data_type: String::new(),
                },
                |common| common.kind,
            )
        }
        Expression::Aggregate { kind, argument, .. } => match (kind, argument) {
            (AggregateKind::Count | AggregateKind::Sum, _) | (_, AggregateArgument::All) => {
                ColumnKind::Number {
                    precision: None,
                    scale: None,
                }
            }
            (_, AggregateArgument::Expression(argument)) => {
                source_free_expression_kind(argument, snapshot, parameters)
            }
        },
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
            Ok(context.sql_column(&resolved, column))
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
        Expression::InQuery {
            token,
            value,
            query,
            negated,
        } => compile_in_query(token, value, query, *negated, context),
        Expression::InList {
            value,
            items,
            negated,
        } => {
            if let Some(sql) = compile_reference_pair_in_list(value, items, *negated, context)? {
                return Ok(sql);
            }
            let value_sql = compile_expression(value, context)?;
            let mut item_sql = Vec::with_capacity(items.len());
            for item in items {
                if let Expression::Parameter(token) = item
                    && let Some(value) = context.catalog.parameters.lookup(token)?
                    && let Some(elements) = list_elements(value, token)?
                {
                    for element in elements {
                        item_sql.push(render_scalar_parameter(
                            element,
                            token,
                            context.dialect,
                            true,
                        )?);
                    }
                    continue;
                }
                item_sql.push(compile_expression_operand(item, value, context)?);
            }
            if item_sql.is_empty() {
                return Ok(context.dialect.boolean_literal_predicate(*negated));
            }
            let sql = format!("({value_sql} IN ({}))", item_sql.join(", "));
            Ok(if *negated {
                format!("(NOT {sql})")
            } else {
                sql
            })
        }
        Expression::Parameter(token) => match context.catalog.parameters.lookup(token)? {
            Some(value) => render_scalar_parameter(value, token, context.dialect, true),
            None => Ok("NULL".to_owned()),
        },
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_expression(value, context)?,
            if *negated { "NOT " } else { "" }
        )),
        Expression::Case {
            branches,
            otherwise,
            ..
        } => {
            context.catalog.charge(branches.len(), None)?;
            let snapshot = context.snapshot;
            let dialect = context.dialect;
            compile_case(
                branches,
                otherwise.as_deref(),
                snapshot,
                dialect,
                |expression, predicate| {
                    if predicate {
                        Ok((compile_predicate(expression, context)?, ColumnKind::Boolean))
                    } else {
                        value_operand(expression, context)
                    }
                },
            )
            .map(|(sql, _)| sql)
        }
        Expression::IsNullFunction {
            token,
            value,
            fallback,
        } => {
            context.catalog.charge(1, None)?;
            let (value_sql, value_kind) = value_operand(value, context)?;
            let (fallback_sql, fallback_kind) = value_operand(fallback, context)?;
            let mut operands = [
                Operand {
                    token,
                    sql: value_sql,
                    kind: value_kind,
                },
                Operand {
                    token,
                    sql: fallback_sql,
                    kind: fallback_kind,
                },
            ];
            render_coalesce(&mut operands, context.snapshot, context.dialect).map(|(sql, _)| sql)
        }
        Expression::Like { token, .. } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "LIKE is supported only in predicate positions",
        )),
        Expression::Aggregate {
            token,
            kind,
            distinct,
            argument,
        } => {
            if !context.aggregates_allowed {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "aggregate functions are supported only as projections of a grouped branch",
                ));
            }
            compile_aggregate(context, *kind, *distinct, argument).map(|(sql, _)| sql)
        }
    }
}

fn compile_binary_expression(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let (left, terms) = left_binary_spine(expression);
    if let [(operator, right)] = terms.as_slice()
        && matches!(operator.lexeme, "=" | "<>")
        && let Some(sql) = compile_reference_pair_comparison(left, right, operator, context)?
    {
        return Ok(sql);
    }
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

/// The reference constant an expression stands for: a typed parameter, a
/// `0x…` literal in the console output format (16 or 20 bytes), or a
/// `ЗНАЧЕНИЕ` of a reference type.
fn reference_constant(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<ReferenceConstant>, QueryDiagnostic> {
    match expression {
        Expression::Parameter(token) => match context.catalog.parameters.lookup(token)? {
            Some(value) => {
                reference_constant_of_value(value, token, context.snapshot, context.dialect)
            }
            None => Ok(None),
        },
        Expression::Literal(token) if token.kind == TokenKind::Binary => {
            let bytes = decode_binary_literal(token)?;
            Ok(reference_constant_of_bytes(&bytes, context.dialect))
        }
        Expression::MetadataValue {
            token,
            kind,
            object,
            value,
        } => {
            let id_sql = compile_metadata_value(
                token,
                kind,
                object,
                value,
                context.snapshot,
                context.dialect,
            )?;
            let target = kind_from_query_name(kind.lexeme)
                .and_then(|kind| context.snapshot.object_id(kind, object.lexeme).ok());
            let type_sql = target
                .map(|target| object_type_number(target, token, context.snapshot))
                .transpose()?
                .map(|number| context.dialect.binary_u32(number));
            Ok(Some(ReferenceConstant {
                type_sql,
                id_sql,
                target,
            }))
        }
        _ => Ok(None),
    }
}

/// Renders the comparison of a reference field with a reference constant
/// by physical member: a runtime-typed field compares `RTRef` and `RRRef`,
/// a single-member field compares its `RRRef` only. Returns `None` when the
/// operands are not such a pair, leaving the generic path to handle them.
fn compile_reference_pair_comparison(
    left: &Expression<'_, '_>,
    right: &Expression<'_, '_>,
    operator: &Token<'_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let (field, other) = match (left, right) {
        (Expression::Field(field), other) | (other, Expression::Field(field)) => (field, other),
        _ => return Ok(None),
    };
    let Some(constant) = reference_constant(other, context)? else {
        return Ok(None);
    };
    let resolved = context.resolve(field)?;
    let Some(sql) = reference_member_equality(&resolved, field, &constant, other, context)? else {
        return Ok(None);
    };
    Ok(Some(if operator.lexeme == "<>" {
        format!("(NOT {sql})")
    } else {
        sql
    }))
}

/// `field IN (constants)` over reference members.
fn compile_reference_pair_in_list(
    value: &Expression<'_, '_>,
    items: &[Expression<'_, '_>],
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let Expression::Field(field) = value else {
        return Ok(None);
    };
    let resolved = context.resolve(field)?;
    if reference_pair(resolved.field()).is_none() {
        return Ok(None);
    }
    let mut constants = Vec::new();
    for item in items {
        if let Expression::Parameter(token) = item
            && let Some(value) = context.catalog.parameters.lookup(token)?
            && let Some(elements) = list_elements(value, token)?
        {
            for element in elements {
                let Some(constant) =
                    reference_constant_of_value(element, token, context.snapshot, context.dialect)?
                else {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::Parameter,
                        Some(token),
                        "list elements compared with a reference field must be references",
                    ));
                };
                constants.push((constant, item));
            }
            continue;
        }
        let Some(constant) = reference_constant(item, context)? else {
            return Ok(None);
        };
        constants.push((constant, item));
    }
    if constants.is_empty() {
        return Ok(Some(context.dialect.boolean_literal_predicate(negated)));
    }
    let mut parts = Vec::with_capacity(constants.len());
    for (constant, item) in &constants {
        let Some(sql) = reference_member_equality(&resolved, field, constant, item, context)?
        else {
            return Ok(None);
        };
        parts.push(sql);
    }
    let sql = if parts.len() == 1 {
        parts.pop().expect("one part")
    } else {
        format!("({})", parts.join(" OR "))
    };
    Ok(Some(if negated { format!("(NOT {sql})") } else { sql }))
}

/// Compiles `<value> [НЕ] В (<query>)`. The nested statement must project
/// exactly one column of a compatible kind; reference operands are brought
/// to the same width by widening the fixed side to an `RTRef ‖ RRRef`
/// payload.
fn compile_in_query(
    token: &Token<'_>,
    value: &Expression<'_, '_>,
    query: &crate::query::core::ast::QueryAst<'_, '_>,
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let snapshot = context.snapshot;
    let dialect = context.dialect;
    let mut presentations =
        PresentationCompilation::strict(&[], context.catalog.parameters, dialect);
    let inner = compile_query_ast(
        query,
        snapshot,
        context.catalog,
        &mut presentations,
        Some(token),
    )?;
    let [column] = inner.columns.as_slice() else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "IN subquery must project exactly one column, found {}",
                inner.columns.len()
            ),
        ));
    };
    let (outer_sql, outer_kind) = value_operand(value, context)?;
    if !column.kind.is_compatible_with(&outer_kind) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "IN subquery column kind {:?} is not compatible with {:?}",
                column.kind, outer_kind
            ),
        ));
    }
    let (outer_sql, inner_sql) = match (&outer_kind, &column.kind) {
        (
            ColumnKind::Reference {
                runtime_typed: true,
                ..
            },
            ColumnKind::Reference {
                runtime_typed: false,
                ..
            },
        ) => {
            // Widen the inner side through a wrapping statement so the
            // nested SQL stays untouched.
            let wrapper = "__in";
            let inner_column = dialect.qualified_column(Some(wrapper), &column.label);
            let (widened, _) =
                widen_reference(&inner_column, &column.kind, Some(token), snapshot, dialect)?;
            (
                outer_sql,
                format!(
                    "SELECT {widened} FROM ({}) AS {}",
                    inner.sql,
                    dialect.quote_identifier(wrapper)
                ),
            )
        }
        (
            ColumnKind::Reference {
                runtime_typed: false,
                ..
            },
            ColumnKind::Reference {
                runtime_typed: true,
                ..
            },
        ) => {
            let (widened, _) = widen_reference(
                &outer_sql,
                &outer_kind,
                operand_token(value),
                snapshot,
                dialect,
            )?;
            (widened, inner.sql)
        }
        _ => (outer_sql, inner.sql),
    };
    let sql = format!("({outer_sql} IN ({inner_sql}))");
    Ok(if negated { format!("(NOT {sql})") } else { sql })
}

fn reference_member_equality(
    resolved: &ResolvedPath,
    field: &FieldReference<'_, '_>,
    constant: &ReferenceConstant,
    other: &Expression<'_, '_>,
    context: &CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let token = operand_token(other).unwrap_or(field.last());
    if let Some((type_member, value_member)) = reference_pair(resolved.field()) {
        let Some(type_sql) = &constant.type_sql else {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!(
                    "runtime-typed reference {:?} must be compared with a typed reference or a 20-byte RTRef ‖ RRRef value",
                    resolved.field().name
                ),
            ));
        };
        return Ok(Some(format!(
            "(({} = {type_sql}) AND ({} = {}))",
            context.sql_column(resolved, type_member),
            context.sql_column(resolved, value_member),
            constant.id_sql
        )));
    }
    let [column] = resolved.field().columns.as_slice() else {
        return Ok(None);
    };
    if !matches!(column.kind, ColumnKind::Reference { .. }) {
        return Ok(None);
    }
    if constant.type_sql.is_some() && constant.target.is_none() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "reference {:?} expects a 16-byte value, not a 20-byte RTRef ‖ RRRef payload",
                resolved.field().name
            ),
        ));
    }
    Ok(Some(format!(
        "({} = {})",
        context.sql_column(resolved, column),
        constant.id_sql
    )))
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
        return Ok(context.sql_column(&resolved, column));
    }
    let sql = compile_expression(expression, context)?;
    if expression_kind(expression, context)? == ColumnKind::DateTime {
        return Ok(sql);
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
    // The argument of an aggregate cannot contain another aggregate.
    let outer_allowed = std::mem::replace(&mut context.aggregates_allowed, false);
    let compiled = match argument {
        AggregateArgument::All => Ok(("*".to_owned(), number.clone())),
        AggregateArgument::Expression(expression) => match expression.as_ref() {
            Expression::Field(reference) => context.resolve(reference).and_then(|resolved| {
                let column = countable_column(resolved.field(), reference.last())?;
                Ok((
                    context.sql_column(&resolved, column),
                    aggregated_field_kind(&column.kind),
                ))
            }),
            other => compile_expression(other, context)
                .and_then(|sql| Ok((sql, expression_kind(other, context)?))),
        },
    };
    context.aggregates_allowed = outer_allowed;
    let (argument, argument_kind) = compiled?;
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
