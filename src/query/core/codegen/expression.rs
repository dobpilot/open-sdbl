use super::context::{CompilationContext, ResolvedPath};
use super::nested::{SectionValue, section_column};
use super::orchestrate::{
    PresentationCompilation, compile_query_ast, compile_query_ast_with_outer,
};
use super::params::{
    ReferenceConstant, list_elements, object_type_number, parameter_kind,
    reference_constant_of_bytes, reference_constant_of_value, render_scalar_parameter,
};
use super::select::derived_data_type;
use super::sources::compile_metadata_value;
use super::totals::hierarchical_catalog_of;
use crate::metadata::MetadataSnapshot;
use crate::metadata::ObjectId;
use crate::query::core::ast::{
    AggregateArgument, AggregateKind, CaseBranch, CastTarget, Expression, FieldReference,
    PrimitiveType, ScalarFunction, TypeName,
};
use crate::query::core::dialect::{SqlDialect, compile_literal, decode_binary_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::{ParameterValue, Parameters};
use crate::query::core::resolve::{
    ColumnKind, CompiledColumn, CompiledQuery, QueryableColumn, QueryableField,
    kind_from_query_name,
};
use crate::query::core::types::TypeValue;
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
            if let Some(sql) = reference_pair_payload(&resolved, context) {
                return Ok(sql);
            }
            // A value dereferenced across reference targets is spread over
            // composite members; in an expression it stands for its value
            // member, the first one rendered.
            if !resolved.member_expressions.is_empty()
                && let Some(expression) = &resolved.expression
            {
                return Ok(expression.clone());
            }
            let column = scalar_column(&resolved, reference.last())?;
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
        _ => compile_logical_or_value(expression, context),
    }
}

/// Whether an expression is a logical one. The platform lets such an
/// expression stand as a value, and a dialect without boolean values needs
/// it wrapped there, while a predicate position takes it as it is.
fn is_logical(expression: &Expression<'_, '_>) -> bool {
    match expression {
        Expression::Refs { .. }
        | Expression::Between { .. }
        | Expression::IsNull { .. }
        | Expression::Like { .. }
        | Expression::InQuery { .. }
        | Expression::InList { .. } => true,
        Expression::Unary { operator, .. } => operator.kind == TokenKind::Keyword(Keyword::Not),
        Expression::Binary { operator, .. } => {
            matches!(
                operator.kind,
                TokenKind::Keyword(Keyword::And | Keyword::Or)
            ) || COMPARISON_LEXEMES.contains(&operator.lexeme)
        }
        _ => false,
    }
}

/// Compiles a `ВЫБОР` compared with a value of a known kind: an
/// alternative of another kind — `ИНАЧЕ ЛОЖЬ` beside a reference — never
/// equals that value on the platform, so it compares as `NULL` here.
/// Answers `None` for any other operand, which compiles as usual.
fn compile_case_toward(
    expression: &Expression<'_, '_>,
    other: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let Expression::Case {
        subject,
        branches,
        otherwise,
        ..
    } = expression
    else {
        return Ok(None);
    };
    let Ok(target) = expression_kind(other, context) else {
        return Ok(None);
    };
    // Compared with `NULL` or a parameter without a value, the
    // alternatives are measured against the first typed one: the
    // comparison answers NULL whichever kind the value has.
    let target = if target.is_wildcard() {
        let mut first = None;
        for value in branches
            .iter()
            .map(|branch| &branch.then)
            .chain(otherwise.as_deref())
        {
            let kind = value_operand_kind(value, context)?;
            if !kind.is_wildcard() {
                first = Some(kind);
                break;
            }
        }
        let Some(first) = first else {
            return Ok(None);
        };
        first
    } else {
        target
    };
    context.catalog.charge(branches.len(), None)?;
    let snapshot = context.snapshot;
    let dialect = context.dialect;
    compile_case(
        subject.as_deref(),
        branches,
        otherwise.as_deref(),
        snapshot,
        dialect,
        |part| match part {
            CasePart::Condition {
                subject: Some(subject),
                when,
                token,
            } => Ok((
                compile_case_match(subject, when, token, context)?,
                ColumnKind::Boolean,
            )),
            CasePart::Condition { when, .. } => {
                Ok((compile_predicate(when, context)?, ColumnKind::Boolean))
            }
            CasePart::Value(value) => {
                let (sql, kind) = value_operand(value, context)?;
                if kind.is_wildcard() || kind.is_compatible_with(&target) {
                    Ok((sql, kind))
                } else {
                    Ok(("NULL".to_owned(), ColumnKind::Null))
                }
            }
        },
    )
    .map(|(sql, _)| Some(sql))
}

/// The comparison spellings, which produce a boolean like the other
/// logical forms.
const COMPARISON_LEXEMES: [&str; 6] = ["=", "<>", "<", "<=", ">", ">="];

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
    // `ВЫРАЗИТЬ` also narrows an expression: the platform accepts it when
    // the value can hold the named type and reports incompatible types
    // otherwise, measured on the probe base.
    let Expression::Field(reference) = argument else {
        if path.is_some() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "a field of a narrowed expression is read through a narrowed field",
            ));
        }
        let (sql, value_kind) = value_operand(argument, context)?;
        let narrowed = ColumnKind::Reference {
            targets: vec![target_id],
            runtime_typed: false,
        };
        return match &value_kind {
            ColumnKind::Reference {
                targets,
                runtime_typed: false,
            } if targets.as_slice() == [target_id] => Ok(NarrowedReference {
                sql,
                kind: value_kind,
            }),
            ColumnKind::Reference {
                runtime_typed: true,
                ..
            } => {
                let number = object_type_number(target_id, token, context.snapshot)?;
                Ok(NarrowedReference {
                    sql: format!(
                        "CASE WHEN {} = {} THEN {} END",
                        context.dialect.payload_type(&sql),
                        context.dialect.binary_u32(number),
                        context.dialect.payload_reference(&sql),
                    ),
                    kind: narrowed,
                })
            }
            // A value that is `NULL` whatever its type narrows to `NULL`
            // of the named type; an unbound parameter is such a value.
            ColumnKind::Null | ColumnKind::Undefined | ColumnKind::Unknown { .. } => {
                Ok(NarrowedReference {
                    sql,
                    kind: narrowed,
                })
            }
            other => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                format!(
                    "CAST argument of kind {other:?} cannot hold {}.{}",
                    kind.as_str(),
                    target.object.lexeme
                ),
            )),
        };
    };
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

/// The `_TYPE` member of a composite field: the discriminator byte the
/// platform writes next to the value members.
fn composite_type_member(field: &QueryableField) -> Option<&QueryableColumn> {
    field
        .columns
        .iter()
        .find(|column| column.physical_name.to_ascii_lowercase().ends_with("_type"))
}

/// The single reference target of a composite field's value member, when
/// the field admits exactly one reference type.
fn single_reference_target(field: &QueryableField) -> Option<ObjectId> {
    match &field
        .columns
        .iter()
        .find(|column| column.is_reference_value_member())?
        .kind
    {
        ColumnKind::Reference { targets, .. } => match targets.as_slice() {
            [target] => Some(*target),
            _ => None,
        },
        _ => None,
    }
}

/// Resolves the argument of `ТИП(…)` to the type it names.
pub(super) fn type_literal_value(
    name: &TypeName<'_, '_>,
    snapshot: &MetadataSnapshot,
    token: &Token<'_>,
) -> Result<TypeValue, QueryDiagnostic> {
    match name {
        TypeName::Primitive(PrimitiveType::String) => Ok(TypeValue::String),
        TypeName::Primitive(PrimitiveType::Number) => Ok(TypeValue::Number),
        TypeName::Primitive(PrimitiveType::Date) => Ok(TypeValue::Date),
        TypeName::Primitive(PrimitiveType::Boolean) => Ok(TypeValue::Boolean),
        TypeName::Object { kind, object } => {
            let metadata_kind = kind_from_query_name(kind.lexeme).ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::UnknownObject,
                    Some(kind),
                    format!("unknown TYPE metadata kind {:?}", kind.lexeme),
                )
            })?;
            let target = snapshot
                .object_id(metadata_kind, object.lexeme)
                .map_err(|error| {
                    QueryDiagnostic::lookup(
                        object,
                        error.clone(),
                        format!(
                            "TYPE target {}.{:?} could not be resolved: {error}",
                            metadata_kind.as_str(),
                            object.lexeme
                        ),
                    )
                })?;
            Ok(TypeValue::Reference(object_type_number(
                target, token, snapshot,
            )?))
        }
    }
}

/// The five-byte SQL constant of a type value.
pub(super) fn type_constant(value: TypeValue, dialect: SqlDialect) -> String {
    dialect.binary_literal(&value.encode())
}

/// The type of a value already compiled to `sql` with `kind`, as SQL that
/// is never `NULL`: a `NULL` value has the `NULL` type, as on the
/// platform. `nullable` asks for the `IS NULL` guard, which literals and
/// bound parameters do not need.
pub(super) fn value_type_sql(
    sql: &str,
    kind: &ColumnKind,
    nullable: bool,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    let null_type = type_constant(TypeValue::Null, dialect);
    // A runtime-typed payload carries its table number in the first four
    // bytes, so the type is read from the value itself.
    if let ColumnKind::Reference {
        runtime_typed: true,
        ..
    } = kind
    {
        let payload = dialect.reference_payload(
            &dialect.binary_literal(&[TypeValue::TAG_REFERENCE]),
            &dialect.payload_type(sql),
        );
        return Ok(format!("COALESCE({payload}, {null_type})"));
    }
    let constant = match kind {
        ColumnKind::Reference { targets, .. } => match targets.as_slice() {
            [target] => TypeValue::Reference(object_type_number(*target, token, snapshot)?),
            _ => return Err(value_type_diagnostic(token, kind)),
        },
        ColumnKind::String { .. } => TypeValue::String,
        ColumnKind::Number { .. } => TypeValue::Number,
        ColumnKind::Boolean => TypeValue::Boolean,
        ColumnKind::DateTime => TypeValue::Date,
        ColumnKind::Null => TypeValue::Null,
        ColumnKind::Undefined => TypeValue::Undefined,
        ColumnKind::Binary { .. }
        | ColumnKind::Uuid
        | ColumnKind::Type
        | ColumnKind::Unknown { .. } => return Err(value_type_diagnostic(token, kind)),
    };
    let constant = type_constant(constant, dialect);
    if !nullable || constant == null_type {
        return Ok(constant);
    }
    Ok(format!(
        "CASE WHEN {sql} IS NULL THEN {null_type} ELSE {constant} END"
    ))
}

/// The member that stands for a compound field in `ЕСТЬ NULL`: the
/// `_TYPE` discriminator, or the reference value member. Both are `NULL`
/// exactly when the row is missing, which is what the platform reports.
/// `None` leaves the expression to the ordinary compiler.
fn compound_null_member(
    value: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let Expression::Field(reference) = value else {
        return Ok(None);
    };
    let resolved = context.resolve(reference)?;
    if resolved.field().columns.len() < 2 {
        return Ok(None);
    }
    let member = composite_type_member(resolved.field()).or_else(|| {
        resolved
            .field()
            .columns
            .iter()
            .find(|column| column.is_reference_value_member())
    });
    Ok(member.map(|member| context.sql_column(&resolved, member)))
}

/// The result kind of a scalar function.
pub(super) fn scalar_function_kind(function: ScalarFunction) -> ColumnKind {
    if function.returns_string() {
        ColumnKind::String { length: None }
    } else {
        ColumnKind::Number {
            precision: None,
            scale: None,
        }
    }
}

/// Whether the argument at `index` is a string; the others are numbers.
pub(super) fn scalar_argument_is_string(function: ScalarFunction, index: usize) -> bool {
    match function {
        ScalarFunction::StrFind | ScalarFunction::StrReplace => true,
        ScalarFunction::Substring | ScalarFunction::Left | ScalarFunction::Right => index == 0,
        other => other.takes_strings() && index == 0,
    }
}

/// Checks one argument of a scalar function against the kind the platform
/// expects there. `NULL` and unclassified values pass, as elsewhere.
pub(super) fn check_scalar_argument(
    function: ScalarFunction,
    index: usize,
    token: &Token<'_>,
    kind: &ColumnKind,
) -> Result<(), QueryDiagnostic> {
    let wildcard = matches!(
        kind,
        ColumnKind::Null | ColumnKind::Undefined | ColumnKind::Unknown { .. }
    );
    let expected_string = scalar_argument_is_string(function, index);
    let matches = if expected_string {
        matches!(kind, ColumnKind::String { .. })
    } else {
        matches!(kind, ColumnKind::Number { .. })
    };
    if wildcard || matches {
        return Ok(());
    }
    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(token),
        format!(
            "{} argument {} must be {}, found {kind:?}",
            function.name(),
            index + 1,
            if expected_string {
                "a string"
            } else {
                "a number"
            }
        ),
    ))
}

/// Compiles one call of the scalar string and arithmetic library. A
/// character column of the PostgreSQL 1C extension types is cast to
/// `text` first, because the functions are not defined on them.
fn compile_scalar_function(
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    function: ScalarFunction,
    arguments: &[Expression<'_, '_>],
) -> Result<String, QueryDiagnostic> {
    let mut compiled = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().enumerate() {
        let kind = expression_kind(argument, context)?;
        check_scalar_argument(
            function,
            index,
            operand_token(argument).unwrap_or(token),
            &kind,
        )?;
        let sql = match argument {
            Expression::Field(reference) => {
                let resolved = context.resolve(reference)?;
                let column = scalar_column(&resolved, reference.last())?;
                context.dialect.column_projection(
                    &context.sql_column(&resolved, column),
                    &column.kind,
                    &column.data_type,
                )
            }
            other => compile_expression(other, context)?,
        };
        compiled.push(sql);
    }
    Ok(context.dialect.scalar_function(function, &compiled))
}

/// Whether `ТИПЗНАЧЕНИЯ` can name the type of a value of this kind.
fn is_classifiable_kind(kind: &ColumnKind) -> bool {
    match kind {
        ColumnKind::Reference {
            targets,
            runtime_typed,
        } => *runtime_typed || targets.len() == 1,
        ColumnKind::String { .. }
        | ColumnKind::Number { .. }
        | ColumnKind::Boolean
        | ColumnKind::DateTime
        | ColumnKind::Null
        | ColumnKind::Undefined => true,
        ColumnKind::Binary { .. }
        | ColumnKind::Uuid
        | ColumnKind::Type
        | ColumnKind::Unknown { .. } => false,
    }
}

/// Whether the argument is a parameter whose value the current pass does
/// not know: it compiles to `NULL`, so its type is the `NULL` type, and
/// the bound pass then sees the real kind.
pub(super) fn is_unbound_parameter(argument: &Expression<'_, '_>, kind: &ColumnKind) -> bool {
    matches!(argument, Expression::Parameter(_)) && matches!(kind, ColumnKind::Unknown { .. })
}

fn value_type_diagnostic(token: &Token<'_>, kind: &ColumnKind) -> QueryDiagnostic {
    QueryDiagnostic::at(
        QueryDiagnosticKind::UnsupportedFeature,
        Some(token),
        format!("VALUETYPE does not classify an expression of kind {kind:?}"),
    )
}

/// Compiles `ТИПЗНАЧЕНИЯ(<выражение>)`. A composite field reads its
/// `_TYPE` member, and its `_RTRef` member when the tag says the value is
/// a reference; every other expression reports the type of its kind.
fn compile_value_type(
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    argument: &Expression<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let dialect = context.dialect;
    if let Expression::Field(reference) = argument {
        let resolved = context.resolve(reference)?;
        if let Some(type_member) = composite_type_member(resolved.field()) {
            let tag = context.sql_column(&resolved, type_member);
            let zero = dialect.binary_literal(&[0; 4]);
            // With several reference alternatives the table number lives in
            // the `_RTRef` member; with one it is fixed by the field.
            let table = match resolved
                .field()
                .columns
                .iter()
                .find(|column| column.is_reference_type_member())
            {
                Some(table_member) => Some(context.sql_column(&resolved, table_member)),
                None => single_reference_target(resolved.field())
                    .map(|target| object_type_number(target, token, context.snapshot))
                    .transpose()?
                    .map(|number| dialect.binary_u32(number)),
            };
            let value = match table {
                Some(table) => format!(
                    "CASE WHEN {tag} = {} THEN {} ELSE {} END",
                    dialect.binary_literal(&[TypeValue::TAG_REFERENCE]),
                    dialect.reference_payload(&tag, &table),
                    dialect.reference_payload(&tag, &zero)
                ),
                None => dialect.reference_payload(&tag, &zero),
            };
            return Ok(format!(
                "COALESCE({value}, {})",
                type_constant(TypeValue::Null, dialect)
            ));
        }
    }
    // A reference pair carries no `_TYPE` member because it is always a
    // reference: its type is the tag beside the table number the `RTRef`
    // member holds. Measured on 8.3.27.
    if let Expression::Field(reference) = argument {
        let resolved = context.resolve(reference)?;
        if composite_type_member(resolved.field()).is_none()
            && let Some((type_member, _)) = reference_pair(resolved.field())
        {
            return Ok(dialect.reference_payload(
                &dialect.binary_literal(&[TypeValue::TAG_REFERENCE]),
                &context.sql_column(&resolved, type_member),
            ));
        }
    }
    if let Expression::Field(reference) = argument
        && let Some(sql) = compile_derived_value_type(context, reference)?
    {
        return Ok(sql);
    }
    let (sql, kind) = value_operand(argument, context)?;
    if is_unbound_parameter(argument, &kind) {
        return Ok(type_constant(TypeValue::Null, dialect));
    }
    let nullable = !matches!(
        argument,
        Expression::Literal(_) | Expression::Parameter(_) | Expression::TypeLiteral { .. }
    );
    value_type_sql(&sql, &kind, nullable, context.snapshot, dialect, token)
}

/// Reads the type of a composite field that reached this query through a
/// derived source, which projects the `_TYPE` member as a separate
/// column next to the reference payload. `None` when the field has no
/// such companion column.
fn compile_derived_value_type(
    context: &mut CompilationContext<'_, '_>,
    reference: &FieldReference<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let resolved = context.resolve(reference)?;
    let ColumnKind::Reference {
        runtime_typed: true,
        ..
    } = scalar_column(&resolved, reference.last())?.kind
    else {
        return Ok(None);
    };
    let Some(tag_path) = context.companion_field(&resolved, "_TYPE") else {
        return Ok(None);
    };
    let tag_column = single_column(tag_path.field(), reference.last())?;
    if !matches!(tag_column.kind, ColumnKind::Binary { .. }) {
        return Ok(None);
    }
    let dialect = context.dialect;
    let tag = context.sql_column(&tag_path, tag_column);
    let payload = context.sql_column(&resolved, scalar_column(&resolved, reference.last())?);
    let value = format!(
        "CASE WHEN {tag} = {} THEN {} ELSE {} END",
        dialect.binary_literal(&[TypeValue::TAG_REFERENCE]),
        dialect.reference_payload(&tag, &dialect.payload_type(&payload)),
        dialect.reference_payload(&tag, &dialect.binary_literal(&[0; 4]))
    );
    Ok(Some(format!(
        "COALESCE({value}, {})",
        type_constant(TypeValue::Null, dialect)
    )))
}

/// Whether an expression is the `НЕОПРЕДЕЛЕНО` literal.
fn is_undefined_literal(expression: &Expression<'_, '_>) -> bool {
    matches!(
        expression,
        Expression::Literal(token) if token.kind == TokenKind::Keyword(Keyword::Undefined)
    )
}

/// Renders `<выражение> =|<> НЕОПРЕДЕЛЕНО` as a comparison of the value's
/// type with the undefined type: a composite field holding the undefined
/// value matches, every other value does not, and no diagnostic is
/// raised for a field that cannot hold it, as on the platform.
fn compile_undefined_comparison(
    left: &Expression<'_, '_>,
    right: &Expression<'_, '_>,
    operator: &Token<'_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let other = match (is_undefined_literal(left), is_undefined_literal(right)) {
        (_, true) => left,
        (true, false) => right,
        (false, false) => return Ok(None),
    };
    // A value that cannot hold `Неопределено` never equals it, which is
    // what the platform answers instead of raising a type error.
    let composite = match other {
        Expression::Field(reference) => {
            composite_type_member(context.resolve(reference)?.field()).is_some()
        }
        _ => false,
    };
    if !composite {
        let kind = value_operand_kind(other, context)?;
        if !is_classifiable_kind(&kind) && !is_unbound_parameter(other, &kind) {
            return Ok(Some(
                context
                    .dialect
                    .boolean_literal_predicate(operator.lexeme == "<>"),
            ));
        }
    }
    let value = compile_value_type(context, operator, other)?;
    Ok(Some(format!(
        "({value} {} {})",
        operator.lexeme,
        type_constant(TypeValue::Undefined, context.dialect)
    )))
}

/// Compiles `<field> ССЫЛКА <Kind>.<Object>`: a composite field compares
/// its type member with the target's database type number, a runtime-typed
/// derived column compares the payload prefix, and a fixed-target field of
/// the named table is always true (the platform treats the empty reference
/// as a reference of the field's type).
fn compile_refs(
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    value: &Expression<'_, '_>,
    kind_token: &Token<'_>,
    object_token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    let reference = match value {
        Expression::Field(reference) => reference,
        // `ВЫРАЗИТЬ(Поле КАК Документ.X) ССЫЛКА Документ.X` asks whether
        // the field holds a reference of that type: the cast keeps such a
        // value and turns any other into NULL, so the test is the field's.
        Expression::Cast {
            argument,
            target: CastTarget::Reference { kind, object },
            path: None,
            ..
        } if names_equal(kind.lexeme, kind_token.lexeme)
            && names_equal(object.lexeme, object_token.lexeme)
            && matches!(argument.as_ref(), Expression::Field(_)) =>
        {
            let Expression::Field(reference) = argument.as_ref() else {
                unreachable!("matched a field argument")
            };
            reference
        }
        _ => {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "REFS argument must be a reference field",
            ));
        }
    };
    let kind = kind_from_query_name(kind_token.lexeme).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(kind_token),
            format!("unknown REFS metadata kind {:?}", kind_token.lexeme),
        )
    })?;
    let target_id = context
        .snapshot
        .object_id(kind, object_token.lexeme)
        .map_err(|error| {
            QueryDiagnostic::lookup(
                object_token,
                error.clone(),
                format!(
                    "REFS target {}.{:?} could not be resolved: {error}",
                    kind.as_str(),
                    object_token.lexeme
                ),
            )
        })?;
    let target_object = context.snapshot.object_by_id(target_id).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(object_token),
            "REFS target disappeared from the metadata index",
        )
    })?;
    let number = object_type_number(target_id, object_token, context.snapshot)?;
    let resolved = context.resolve(reference)?;
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
                "REFS argument {:?} must be a reference field",
                reference.last().lexeme
            ),
        ));
    }
    let dialect = context.dialect;
    if let Some(type_member) = field
        .columns
        .iter()
        .find(|column| column.is_reference_type_member())
    {
        return Ok(format!(
            "({} = {})",
            context.sql_column(&resolved, type_member),
            dialect.binary_u32(number)
        ));
    }
    let universal = field.reference_targets.iter().any(String::is_empty);
    if universal {
        let payload = reference_column(field, reference.last())?;
        return Ok(format!(
            "({} = {})",
            dialect.payload_type(&context.sql_column(&resolved, payload)),
            dialect.binary_u32(number)
        ));
    }
    let admissible = target_object
        .physical_table
        .as_deref()
        .is_some_and(|table| {
            field.reference_targets.iter().any(|candidate| {
                names_equal(
                    table.strip_prefix('_').unwrap_or(table),
                    candidate.strip_prefix('_').unwrap_or(candidate),
                )
            })
        });
    if !admissible {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(object_token),
            format!(
                "field {:?} cannot hold {}.{}",
                field.name,
                kind.as_str(),
                object_token.lexeme
            ),
        ));
    }
    Ok(dialect.boolean_literal_predicate(true))
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
            if let Some((_, value_member)) = reference_pair(resolved.field()) {
                return Ok(payload_kind(&value_member.kind));
            }
            if !resolved.member_expressions.is_empty()
                && let Some(column) = resolved.field().columns.first()
            {
                return Ok(column.kind.clone());
            }
            let column = scalar_column(&resolved, reference.last())?;
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
        Expression::Between { .. } => Ok(ColumnKind::Boolean),
        Expression::ScalarFunction { function, .. } => Ok(scalar_function_kind(*function)),
        Expression::TypeLiteral { .. } | Expression::ValueType { .. } => Ok(ColumnKind::Type),
        Expression::Parameter(token) => Ok(context
            .catalog
            .parameters()
            .lookup(token)?
            .map_or_else(unknown_kind, parameter_kind)),
        Expression::Aggregate { kind, argument, .. } => match (kind, argument) {
            (AggregateKind::Count | AggregateKind::Sum | AggregateKind::Avg, _)
            | (_, AggregateArgument::All) => Ok(ColumnKind::Number {
                precision: None,
                scale: None,
            }),
            (_, AggregateArgument::Expression(argument)) => match argument.as_ref() {
                Expression::Field(reference) => {
                    let resolved = context.resolve(reference)?;
                    if let Some((_, value_member)) = reference_pair(resolved.field()) {
                        return Ok(payload_kind(&value_member.kind));
                    }
                    let column = countable_column(resolved.field(), reference.last())?;
                    Ok(column.kind.clone())
                }
                other => expression_kind(other, context),
            },
        },
        // `+` concatenates when any operand is a string, so the value it
        // answers is a string and not the number a sum would be.
        Expression::Binary { operator, .. } if operator.lexeme == "+" => {
            let (left, terms) = left_binary_spine(expression);
            if terms.iter().all(|(operator, _)| operator.lexeme == "+") {
                let mut string = matches!(
                    value_operand_kind(left, context)?,
                    ColumnKind::String { .. }
                );
                for (_, right) in &terms {
                    string |= matches!(
                        value_operand_kind(right, context)?,
                        ColumnKind::String { .. }
                    );
                }
                if string {
                    return Ok(ColumnKind::String { length: None });
                }
            }
            Ok(source_free_expression_kind(
                expression,
                context.snapshot,
                context.catalog.parameters(),
            ))
        }
        _ => Ok(source_free_expression_kind(
            expression,
            context.snapshot,
            context.catalog.parameters(),
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
        | Expression::EndOfPeriod { token, .. }
        | Expression::DateAdd { token, .. }
        | Expression::DateDiff { token, .. }
        | Expression::DatePart { token, .. }
        | Expression::Refs { token, .. }
        | Expression::MetadataValue { token, .. }
        | Expression::SystemValue { token, .. }
        | Expression::Tuple { token, .. }
        | Expression::Uuid { token, .. }
        | Expression::Between { token, .. }
        | Expression::ScalarFunction { token, .. }
        | Expression::TypeLiteral { token, .. }
        | Expression::ValueType { token, .. }
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

/// The `RTRef ‖ RRRef` payload of a field stored as a reference pair, or
/// `None` when the field is not one.
fn reference_pair_payload(
    resolved: &ResolvedPath,
    context: &CompilationContext<'_, '_>,
) -> Option<String> {
    let (type_member, value_member) = reference_pair(resolved.field())?;
    Some(context.dialect.reference_payload(
        &context.sql_column(resolved, type_member),
        &context.sql_column(resolved, value_member),
    ))
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
        // Only wildcards: `НЕОПРЕДЕЛЕНО` names the column kind when it is
        // the only literal kind present, otherwise `NULL` does.
        let undefined = operands
            .iter()
            .any(|operand| operand.kind == ColumnKind::Undefined);
        return Ok(CommonKind {
            kind: if undefined {
                ColumnKind::Undefined
            } else {
                ColumnKind::Null
            },
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

/// Renders a projected `ВЫБОР` or `ЕСТЬNULL` whose branches differ in
/// type as the members of a composite value. Returns `None` for every
/// other expression and whenever the branches agree on one kind.
pub(super) fn compile_composite_projection(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<Vec<CompositeMember>>, QueryDiagnostic> {
    let snapshot = context.snapshot;
    let dialect = context.dialect;
    match expression {
        Expression::Case {
            subject,
            branches,
            otherwise,
            ..
        } => {
            let mut conditions = Vec::with_capacity(branches.len());
            let mut values = Vec::with_capacity(branches.len() + 1);
            for branch in branches {
                conditions.push(match subject.as_deref() {
                    Some(subject) => {
                        compile_case_match(subject, &branch.when, branch.token, context)?
                    }
                    None => compile_predicate(&branch.when, context)?,
                });
                let (sql, kind) = value_operand(&branch.then, context)?;
                values.push(Operand {
                    token: branch.token,
                    sql,
                    kind,
                });
            }
            if let Some(otherwise) = otherwise.as_deref() {
                let (sql, kind) = value_operand(otherwise, context)?;
                values.push(Operand {
                    token: operand_token(otherwise).unwrap_or(branches[0].token),
                    sql,
                    kind,
                });
            }
            compile_composite_alternatives(
                &conditions,
                &mut values,
                otherwise.is_some(),
                snapshot,
                dialect,
            )
        }
        Expression::IsNullFunction {
            token,
            value,
            fallback,
        } => {
            let (value_sql, value_kind) = value_operand(value, context)?;
            let (fallback_sql, fallback_kind) = value_operand(fallback, context)?;
            let condition = format!("({value_sql} IS NOT NULL)");
            let mut values = vec![
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
            compile_composite_alternatives(
                std::slice::from_ref(&condition),
                &mut values,
                true,
                snapshot,
                dialect,
            )
        }
        _ => Ok(None),
    }
}

/// One physical member of a composite result: the suffix its output label
/// carries, the rendered value and the kind of that column.
pub(super) struct CompositeMember {
    pub(super) suffix: &'static str,
    pub(super) sql: String,
    pub(super) kind: ColumnKind,
}

/// The member a value of this kind occupies in a composite result, with
/// the value every other branch writes there. Mirrors how the platform
/// stores a value of several types and how a composite field is projected.
pub(super) fn composite_member_of(kind: &ColumnKind) -> Option<(&'static str, TypeValue)> {
    match kind {
        ColumnKind::String { .. } => Some(("_S", TypeValue::String)),
        ColumnKind::Number { .. } => Some(("_N", TypeValue::Number)),
        ColumnKind::DateTime => Some(("_T", TypeValue::Date)),
        ColumnKind::Boolean => Some(("_L", TypeValue::Boolean)),
        ColumnKind::Reference { .. } => Some(("", TypeValue::Reference(0))),
        ColumnKind::Undefined => Some(("", TypeValue::Undefined)),
        ColumnKind::Null => Some(("", TypeValue::Null)),
        // Neither a unique identifier nor raw bytes have a member in what
        // 1C stores, because such a value never reaches a table; the
        // platform answers both as binary data whose ТИПЗНАЧЕНИЯ is Null.
        // They keep members of their own so that a value carrying both
        // stays typed on each side.
        ColumnKind::Uuid => Some(("_U", TypeValue::Null)),
        ColumnKind::Binary { .. } => Some(("_B", TypeValue::Null)),
        _ => None,
    }
}

/// The value a branch of another type writes into this member: the zero of
/// its type, as the platform writes it.
fn composite_member_zero(suffix: &str, dialect: SqlDialect) -> String {
    match suffix {
        "_S" => dialect.string_literal(""),
        "_N" => "0".to_owned(),
        "_T" => dialect.zero_datetime(),
        "_L" => dialect.boolean_literal(false).to_owned(),
        "_U" => dialect.zero_uuid().to_owned(),
        "_B" => dialect.binary_literal(&[]),
        _ => dialect.binary_literal(&[0; 20]),
    }
}

/// The kind of a composite member column.
fn composite_member_kind(suffix: &str) -> ColumnKind {
    match suffix {
        "_S" => ColumnKind::String { length: None },
        "_N" => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        "_T" => ColumnKind::DateTime,
        "_L" => ColumnKind::Boolean,
        "_U" => ColumnKind::Uuid,
        "_B" => ColumnKind::Binary { length: None },
        _ => ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        },
    }
}

/// Renders an alternative-valued expression whose branches differ in type
/// as the members the platform spreads such a value over: the `_TYPE`
/// discriminator, the member of every type present, and the reference
/// payload. Returns `None` when the branches agree on one kind, which the
/// ordinary path renders as a single column.
pub(super) fn compile_composite_alternatives(
    conditions: &[String],
    values: &mut [Operand<'_, '_>],
    has_else: bool,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<Option<Vec<CompositeMember>>, QueryDiagnostic> {
    if common_kind(values).is_ok() {
        return Ok(None);
    }
    let mut tags = Vec::with_capacity(values.len());
    for operand in values.iter_mut() {
        let Some((suffix, tag)) = composite_member_of(&operand.kind) else {
            return Ok(None);
        };
        if matches!(operand.kind, ColumnKind::Reference { .. }) {
            let (sql, kind) = widen_reference(
                &operand.sql,
                &operand.kind,
                Some(operand.token),
                snapshot,
                dialect,
            )?;
            operand.sql = sql;
            let tag = match kind {
                ColumnKind::Reference { .. } => TypeValue::Reference(0),
                _ => tag,
            };
            operand.kind = kind;
            tags.push((suffix, tag));
        } else {
            tags.push((suffix, tag));
        }
    }
    let mut suffixes = Vec::new();
    for (suffix, _) in &tags {
        if !suffixes.contains(suffix) {
            suffixes.push(*suffix);
        }
    }
    // The payload member comes first, the way a composite field projects
    // one; it is present only when a branch carries a reference.
    suffixes.sort_by_key(|suffix| if suffix.is_empty() { 0 } else { 1 });
    // The string member mixes stored strings with literals, which are
    // different SQL types on PostgreSQL, so every branch of that member is
    // rendered as text.
    let member_value = |member: &str, sql: &str| -> String {
        if member == "_S" {
            dialect.scalar_text(sql)
        } else {
            sql.to_owned()
        }
    };
    let render = |member: &str| -> String {
        let mut sql = String::from("CASE");
        for (condition, (operand, (suffix, _))) in
            conditions.iter().zip(values.iter().zip(tags.iter()))
        {
            sql.push_str(" WHEN ");
            sql.push_str(condition);
            sql.push_str(" THEN ");
            if *suffix == member {
                sql.push_str(&member_value(member, &operand.sql));
            } else {
                sql.push_str(&member_value(
                    member,
                    &composite_member_zero(member, dialect),
                ));
            }
        }
        if has_else {
            let last = values.len() - 1;
            sql.push_str(" ELSE ");
            if tags[last].0 == member {
                sql.push_str(&member_value(member, &values[last].sql));
            } else {
                sql.push_str(&member_value(
                    member,
                    &composite_member_zero(member, dialect),
                ));
            }
        }
        sql.push_str(" END");
        sql
    };
    let mut members = suffixes
        .iter()
        .map(|suffix| CompositeMember {
            suffix,
            sql: render(suffix),
            kind: composite_member_kind(suffix),
        })
        .collect::<Vec<_>>();
    // The discriminator says which member holds the value of a row.
    let mut tag_sql = String::from("CASE");
    for (condition, (_, tag)) in conditions.iter().zip(tags.iter()) {
        tag_sql.push_str(" WHEN ");
        tag_sql.push_str(condition);
        tag_sql.push_str(" THEN ");
        tag_sql.push_str(&dialect.binary_literal(&[tag.tag()]));
    }
    if has_else {
        tag_sql.push_str(" ELSE ");
        tag_sql.push_str(&dialect.binary_literal(&[tags[values.len() - 1].1.tag()]));
    }
    tag_sql.push_str(" END");
    members.push(CompositeMember {
        suffix: "_TYPE",
        sql: tag_sql,
        kind: ColumnKind::Binary { length: Some(1) },
    });
    Ok(Some(members))
}

/// Renders `CASE WHEN … THEN … [ELSE …] END` from compiled alternatives.
/// `values` holds one operand per `WHEN` followed by the `ELSE` operand when
/// `has_else` is set.
/// Gives an untyped `NULL` branch the type of the alternative. A branch
/// that is nothing but `NULL` — a nested `ВЫБОР` of unbound parameters,
/// say — has no type of its own, and PostgreSQL reads it as `text`, which
/// has no common type with the reference payload the other branches
/// carry; the server then refuses the whole `CASE`.
fn type_null_operands(kind: &ColumnKind, values: &mut [Operand<'_, '_>], dialect: SqlDialect) {
    if !matches!(
        kind,
        ColumnKind::Reference { .. } | ColumnKind::Binary { .. }
    ) {
        return;
    }
    let sql_type = dialect.reference_identifier_type();
    for value in values.iter_mut() {
        if value.kind == ColumnKind::Null {
            value.sql = dialect.typed_null(sql_type);
        }
    }
}

pub(super) fn render_case(
    whens: &[String],
    values: &mut [Operand<'_, '_>],
    has_else: bool,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let kind = unify_operands(values, snapshot, dialect)?;
    unify_string_operands(&kind, values, dialect);
    type_null_operands(&kind, values, dialect);
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

/// Renders every operand of a string-valued alternative as text. A stored
/// string and a literal are different SQL types on PostgreSQL, which has no
/// common type for them, so an alternative mixing the two would be refused
/// by the server.
fn unify_string_operands(kind: &ColumnKind, values: &mut [Operand<'_, '_>], dialect: SqlDialect) {
    if !matches!(kind, ColumnKind::String { .. }) || dialect != SqlDialect::Postgres {
        return;
    }
    // A literal beside a stored string is coerced by the server; only an
    // alternative already rendered as text has no common type with one, so
    // the whole alternative becomes text just there.
    if !values.iter().any(|value| value.sql.contains("::text")) {
        return;
    }
    for value in values.iter_mut() {
        value.sql = dialect.scalar_text(&value.sql);
    }
}

/// Renders `COALESCE(value, fallback)` after unifying the operand kinds.
pub(super) fn render_coalesce(
    values: &mut [Operand<'_, '_>; 2],
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, ColumnKind), QueryDiagnostic> {
    let kind = unify_operands(values, snapshot, dialect)?;
    unify_string_operands(&kind, values, dialect);
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
/// Renders one alternative of the simple form `ВЫБОР <выражение> КОГДА
/// <значение> ТОГДА …` as the comparison of the subject with the value.
/// Measured on the platform: the alternatives compare with `=`, so a
/// `NULL` subject matches no alternative, not even a `NULL` one.
pub(super) fn compile_case_match(
    subject: &Expression<'_, '_>,
    value: &Expression<'_, '_>,
    when: &Token<'_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let operator = Token {
        kind: TokenKind::Operator,
        lexeme: "=",
        span: when.span,
    };
    if let Some(sql) = compile_undefined_comparison(subject, value, &operator, context)? {
        return Ok(sql);
    }
    if let Some(sql) = compile_reference_pair_comparison(subject, value, &operator, context)? {
        return Ok(sql);
    }
    if let Some(sql) = compile_composite_comparison(subject, value, &operator, context)? {
        return Ok(sql);
    }
    Ok(format!(
        "({} = {})",
        compile_expression_operand(subject, value, context)?,
        compile_expression_operand(value, subject, context)?,
    ))
}

/// One operand a `ВЫБОР` renderer asks its caller to compile: the
/// condition of a branch, which the simple form compares with the subject,
/// or a resulting value.
pub(super) enum CasePart<'part, 'tokens, 'source> {
    Condition {
        /// The subject of the simple form `ВЫБОР <выражение> КОГДА …`.
        subject: Option<&'part Expression<'tokens, 'source>>,
        when: &'part Expression<'tokens, 'source>,
        token: &'tokens Token<'source>,
    },
    Value(&'part Expression<'tokens, 'source>),
}

pub(super) fn compile_case<'tokens, 'source, E>(
    subject: Option<&Expression<'tokens, 'source>>,
    branches: &[CaseBranch<'tokens, 'source>],
    otherwise: Option<&Expression<'tokens, 'source>>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    mut compile: E,
) -> Result<(String, ColumnKind), QueryDiagnostic>
where
    E: FnMut(CasePart<'_, 'tokens, 'source>) -> Result<(String, ColumnKind), QueryDiagnostic>,
{
    let mut whens = Vec::with_capacity(branches.len());
    let mut values = Vec::with_capacity(branches.len() + 1);
    for branch in branches {
        whens.push(
            compile(CasePart::Condition {
                subject,
                when: &branch.when,
                token: branch.token,
            })?
            .0,
        );
        let (sql, kind) = compile(CasePart::Value(&branch.then))?;
        values.push(Operand {
            token: branch.token,
            sql,
            kind,
        });
    }
    if let Some(otherwise) = otherwise {
        let (sql, kind) = compile(CasePart::Value(otherwise))?;
        values.push(Operand {
            token: operand_token(otherwise).unwrap_or(branches[0].token),
            sql,
            kind,
        });
    }
    render_case(&whens, &mut values, otherwise.is_some(), snapshot, dialect)
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
        Expression::DateTime { .. }
        | Expression::BeginOfPeriod { .. }
        | Expression::EndOfPeriod { .. }
        | Expression::DateAdd { .. } => ColumnKind::DateTime,
        Expression::DateDiff { .. } | Expression::DatePart { .. } => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        Expression::Uuid { .. } => ColumnKind::Uuid,
        Expression::Cast { target, .. } => scalar_cast_kind(*target),
        Expression::MetadataValue { kind, object, .. } => ColumnKind::Reference {
            targets: kind_from_query_name(kind.lexeme)
                .and_then(|kind| snapshot.object_id(kind, object.lexeme).ok())
                .into_iter()
                .collect(),
            runtime_typed: false,
        },
        Expression::SystemValue { .. } => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        Expression::Tuple { .. } => ColumnKind::Unknown {
            data_type: "tuple".to_owned(),
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
        Expression::Between { .. } => ColumnKind::Boolean,
        Expression::ScalarFunction { function, .. } => scalar_function_kind(*function),
        Expression::TypeLiteral { .. } | Expression::ValueType { .. } => ColumnKind::Type,
        Expression::InList { .. }
        | Expression::InQuery { .. }
        | Expression::IsNull { .. }
        | Expression::Refs { .. }
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
            (AggregateKind::Count | AggregateKind::Sum | AggregateKind::Avg, _)
            | (_, AggregateArgument::All) => ColumnKind::Number {
                precision: None,
                scale: None,
            },
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
        TokenKind::Keyword(Keyword::Undefined) => ColumnKind::Undefined,
        _ => ColumnKind::Unknown {
            data_type: token.lexeme.to_owned(),
        },
    }
}

/// Compiles an expression in a value position. A logical expression is a
/// value in SDBL, so it is wrapped in the boolean value form of the
/// dialect; every other expression renders as it is.
pub(super) fn compile_expression(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let sql = compile_logical_or_value(expression, context)?;
    if is_logical(expression) {
        return Ok(context.dialect.boolean_scalar(&sql));
    }
    Ok(sql)
}

/// Renders an expression without the boolean value wrapping: a logical
/// expression comes out as the plain predicate a filter takes.
fn compile_logical_or_value(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve(reference)?;
            // A value stored as an `RTRef`/`RRRef` pair is always a
            // reference, so it is one value: its payload.
            if let Some(sql) = reference_pair_payload(&resolved, context) {
                return Ok(sql);
            }
            let column = scalar_column(&resolved, reference.last())?;
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
            let value = compile_date_operand(value, context, token, "first")?;
            Ok(context.dialect.begin_of_period(&value, *period))
        }
        Expression::EndOfPeriod {
            token,
            value,
            period,
        } => {
            let value = compile_date_operand(value, context, token, "first")?;
            Ok(context.dialect.end_of_period(&value, *period))
        }
        Expression::DateAdd {
            token,
            value,
            period,
            count,
        } => {
            let value = compile_date_operand(value, context, token, "first")?;
            let count = compile_count_operand(count, context, token)?;
            Ok(context.dialect.date_add(&value, *period, &count))
        }
        Expression::DateDiff {
            token,
            from,
            to,
            period,
        } => {
            let from = compile_date_operand(from, context, token, "first")?;
            let to = compile_date_operand(to, context, token, "second")?;
            Ok(context.dialect.date_diff(&from, &to, *period, true))
        }
        Expression::DatePart { token, part, value } => {
            let value = compile_date_operand(value, context, token, "first")?;
            Ok(context.dialect.date_part(*part, &value, true))
        }
        Expression::Refs {
            token,
            value,
            kind,
            object,
        } => compile_refs(context, token, value, kind, object),
        Expression::Between {
            value,
            low,
            high,
            negated,
            ..
        } => {
            let sql = format!(
                "({} BETWEEN {} AND {})",
                compile_expression_operand(value, low, context)?,
                compile_expression_operand(low, value, context)?,
                compile_expression_operand(high, value, context)?
            );
            Ok(if *negated {
                format!("(NOT {sql})")
            } else {
                sql
            })
        }
        Expression::ScalarFunction {
            token,
            function,
            arguments,
        } => compile_scalar_function(context, token, *function, arguments),
        Expression::TypeLiteral { token, name } => Ok(type_constant(
            type_literal_value(name, context.snapshot, token)?,
            context.dialect,
        )),
        Expression::ValueType { token, argument } => compile_value_type(context, token, argument),
        Expression::SystemValue { code, .. } => Ok(code.to_string()),
        Expression::Tuple { token, .. } => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "a tuple is accepted only as the left side of В (ВЫБРАТЬ …)",
        )),
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
            // `НЕ` negates a predicate, so its operand keeps the plain
            // predicate form here.
            let value = if operator == "NOT " {
                compile_predicate(value, context)?
            } else {
                compile_expression(value, context)?
            };
            Ok(format!("({operator}{value})"))
        }
        // `И`/`ИЛИ` join predicates, so the spine keeps the plain
        // predicate form of its operands here.
        Expression::Binary { operator, .. }
            if matches!(
                operator.kind,
                TokenKind::Keyword(Keyword::And | Keyword::Or)
            ) =>
        {
            compile_predicate(expression, context)
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
            hierarchy,
        } => {
            if *hierarchy {
                let (seeds, target) = compile_hierarchy_query_seeds(token, query, context)?;
                return compile_in_hierarchy(token, value, seeds, target, *negated, context);
            }
            compile_in_query(token, value, query, *negated, context)
        }
        Expression::InList {
            token,
            value,
            items,
            negated,
            hierarchy,
        } => {
            if *hierarchy {
                let (seeds, target) = compile_hierarchy_list_seeds(token, items, context)?;
                return compile_in_hierarchy(token, value, seeds, target, *negated, context);
            }
            if let Some(sql) = compile_reference_pair_in_list(value, items, *negated, context)? {
                return Ok(sql);
            }
            if let Some(sql) = compile_composite_in_list(value, items, *negated, context)? {
                return Ok(sql);
            }
            let value_sql = compile_expression(value, context)?;
            let mut item_sql = Vec::with_capacity(items.len());
            for item in items {
                if let Expression::Parameter(token) = item
                    && let Some(value) = context.catalog.parameters().lookup(token)?
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
        Expression::Parameter(token) => match context.catalog.parameters().lookup(token)? {
            Some(value) => render_scalar_parameter(value, token, context.dialect, true),
            None => Ok("NULL".to_owned()),
        },
        Expression::IsNull { value, negated } => {
            let sql = match compound_null_member(value, context)? {
                Some(sql) => sql,
                None => compile_expression(value, context)?,
            };
            Ok(format!(
                "({sql} IS {}NULL)",
                if *negated { "NOT " } else { "" }
            ))
        }
        Expression::Case {
            subject,
            branches,
            otherwise,
            ..
        } => {
            context.catalog.charge(branches.len(), None)?;
            let snapshot = context.snapshot;
            let dialect = context.dialect;
            compile_case(
                subject.as_deref(),
                branches,
                otherwise.as_deref(),
                snapshot,
                dialect,
                |part| match part {
                    CasePart::Condition {
                        subject: Some(subject),
                        when,
                        token,
                    } => Ok((
                        compile_case_match(subject, when, token, context)?,
                        ColumnKind::Boolean,
                    )),
                    CasePart::Condition { when, .. } => {
                        Ok((compile_predicate(when, context)?, ColumnKind::Boolean))
                    }
                    CasePart::Value(expression) => value_operand(expression, context),
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
    // `Д.Товары.Количество > 2` asks whether any row of the section
    // satisfies the comparison; the platform answers the owner once even
    // when several rows match, measured on 8.3.27.
    if let [(operator, right)] = terms.as_slice()
        && COMPARISON_LEXEMES.contains(&operator.lexeme)
        && let Some(sql) = compile_tabular_section_comparison(left, right, operator, context)?
    {
        return Ok(sql);
    }
    if let [(operator, right)] = terms.as_slice()
        && matches!(operator.lexeme, "=" | "<>")
        && let Some(sql) = compile_undefined_comparison(left, right, operator, context)?
    {
        return Ok(sql);
    }
    if let [(operator, right)] = terms.as_slice()
        && matches!(operator.lexeme, "=" | "<>")
        && let Some(sql) = compile_reference_pair_comparison(left, right, operator, context)?
    {
        return Ok(sql);
    }
    if let [(operator, right)] = terms.as_slice()
        && matches!(operator.lexeme, "=" | "<>")
        && let Some(sql) = compile_composite_comparison(left, right, operator, context)?
    {
        return Ok(sql);
    }
    // The platform concatenates strings with `+` and refuses to mix a
    // string with another type there, measured on the probe base.
    if !terms.is_empty() && terms.iter().all(|(operator, _)| operator.lexeme == "+") {
        let mut operands = vec![left];
        operands.extend(terms.iter().map(|(_, right)| *right));
        let mut kinds = Vec::with_capacity(operands.len());
        for operand in &operands {
            kinds.push(value_operand_kind(operand, context)?);
        }
        if kinds
            .iter()
            .any(|kind| matches!(kind, ColumnKind::String { .. }))
        {
            let mut parts = Vec::with_capacity(operands.len());
            for (operand, kind) in operands.iter().zip(kinds.iter()) {
                if !matches!(
                    kind,
                    ColumnKind::String { .. }
                        | ColumnKind::Null
                        | ColumnKind::Undefined
                        | ColumnKind::Unknown { .. }
                ) {
                    return Err(QueryDiagnostic::at_or_unpositioned(
                        QueryDiagnosticKind::UnsupportedFeature,
                        operand_token(operand),
                        format!(
                            "operands of + are a string and {kind:?}; the platform concatenates strings only"
                        ),
                    ));
                }
                let sql = compile_expression(operand, context)?;
                parts.push(context.dialect.scalar_text(&sql));
            }
            return Ok(context.dialect.concatenate(&parts));
        }
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
        Expression::Parameter(token) => match context.catalog.parameters().lookup(token)? {
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
            && let Some(value) = context.catalog.parameters().lookup(token)?
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

/// One value described the way a composite field stores it: the tag the
/// platform writes into the `_TYPE` member, the SQL of the value itself,
/// and the `RTRef` table number of a reference. The shape is taken from
/// the SQL the platform generates for the same comparison.
struct CompositeOperand {
    tag: TypeValue,
    /// Compared with the member that carries a value of this tag's type.
    /// `None` for `Неопределено`, which occupies no member, and for an
    /// unbound parameter, which compares with `NULL`.
    value_sql: Option<String>,
    /// The `RTRef` bytes of a reference value, when known.
    type_sql: Option<String>,
}

/// The member of a composite field that carries a value of `tag`.
fn composite_value_member(field: &QueryableField, tag: TypeValue) -> Option<&QueryableColumn> {
    let suffix = match tag {
        TypeValue::Boolean => "_l",
        TypeValue::Number => "_n",
        TypeValue::Date => "_t",
        TypeValue::String => "_s",
        TypeValue::Reference(_) => {
            return field
                .columns
                .iter()
                .find(|column| column.is_reference_value_member());
        }
        TypeValue::Undefined | TypeValue::Null => return None,
    };
    field
        .columns
        .iter()
        .find(|column| column.physical_name.to_ascii_lowercase().ends_with(suffix))
}

/// The tag a value of this kind carries in the `_TYPE` member.
fn composite_tag_of_kind(kind: &ColumnKind) -> Option<TypeValue> {
    match kind {
        ColumnKind::Boolean => Some(TypeValue::Boolean),
        ColumnKind::Number { .. } => Some(TypeValue::Number),
        ColumnKind::DateTime => Some(TypeValue::Date),
        ColumnKind::String { .. } => Some(TypeValue::String),
        _ => None,
    }
}

/// Describes one operand of a comparison with a composite field.
fn composite_operand(
    other: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<CompositeOperand>, QueryDiagnostic> {
    if is_undefined_literal(other) {
        return Ok(Some(CompositeOperand {
            tag: TypeValue::Undefined,
            value_sql: None,
            type_sql: None,
        }));
    }
    if let Some(constant) = reference_constant(other, context)? {
        return Ok(Some(CompositeOperand {
            tag: TypeValue::Reference(0),
            value_sql: Some(constant.id_sql),
            type_sql: constant.type_sql,
        }));
    }
    let kind = value_operand_kind(other, context)?;
    if matches!(kind, ColumnKind::Null) || is_unbound_parameter(other, &kind) {
        return Ok(Some(CompositeOperand {
            tag: TypeValue::Null,
            value_sql: None,
            type_sql: None,
        }));
    }
    if let ColumnKind::Reference { targets, .. } = &kind {
        let [target] = targets.as_slice() else {
            return Ok(None);
        };
        let token = operand_token(other).ok_or_else(|| {
            QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::Metadata,
                "reference operand without a token",
            )
        })?;
        let number = object_type_number(*target, token, context.snapshot)?;
        let type_sql = context.dialect.binary_u32(number);
        return Ok(Some(CompositeOperand {
            tag: TypeValue::Reference(number),
            value_sql: Some(compile_expression(other, context)?),
            type_sql: Some(type_sql),
        }));
    }
    let Some(tag) = composite_tag_of_kind(&kind) else {
        return Ok(None);
    };
    Ok(Some(CompositeOperand {
        tag,
        value_sql: Some(compile_expression(other, context)?),
        type_sql: None,
    }))
}

/// Describes a bound parameter value the same way, for the elements of a
/// list parameter.
fn composite_operand_of_value(
    value: &ParameterValue,
    token: &Token<'_>,
    context: &CompilationContext<'_, '_>,
) -> Result<Option<CompositeOperand>, QueryDiagnostic> {
    if let Some(constant) =
        reference_constant_of_value(value, token, context.snapshot, context.dialect)?
    {
        return Ok(Some(CompositeOperand {
            tag: TypeValue::Reference(0),
            value_sql: Some(constant.id_sql),
            type_sql: constant.type_sql,
        }));
    }
    let tag = match value {
        ParameterValue::Boolean(_) => TypeValue::Boolean,
        ParameterValue::Number { .. } => TypeValue::Number,
        ParameterValue::String(_) => TypeValue::String,
        ParameterValue::Date(_) => TypeValue::Date,
        ParameterValue::Null => TypeValue::Null,
        ParameterValue::Reference { .. }
        | ParameterValue::Binary(_)
        | ParameterValue::List(_)
        | ParameterValue::Table { .. } => {
            return Ok(None);
        }
    };
    let value_sql = match tag {
        TypeValue::Null => None,
        _ => Some(render_scalar_parameter(
            value,
            token,
            context.dialect,
            true,
        )?),
    };
    Ok(Some(CompositeOperand {
        tag,
        value_sql,
        type_sql: None,
    }))
}

/// Renders the equality of a composite field with one described value: the
/// `_TYPE` member carries the tag, the member of that type carries the
/// value, and a reference also compares its `RTRef`. A field admitting a
/// single reference type stores no `RTRef` member, so the comparison
/// synthesizes it from the discriminator, as the platform does.
fn composite_member_equality(
    resolved: &ResolvedPath,
    operand: &CompositeOperand,
    token: &Token<'_>,
    context: &CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let field = resolved.field();
    let Some(type_member) = composite_type_member(field) else {
        return Ok(None);
    };
    let discriminator = context.sql_column(resolved, type_member);
    if operand.tag == TypeValue::Null {
        return Ok(Some(format!("({discriminator} = NULL)")));
    }
    let tag_sql = context.dialect.binary_literal(&[operand.tag.tag()]);
    let mut parts = vec![format!("({discriminator} = {tag_sql})")];
    if matches!(operand.tag, TypeValue::Reference(_))
        && let Some(type_sql) = &operand.type_sql
    {
        if let Some(member) = field
            .columns
            .iter()
            .find(|column| column.is_reference_type_member())
        {
            parts.push(format!(
                "({} = {type_sql})",
                context.sql_column(resolved, member)
            ));
        } else if let Some(target) = single_reference_target(field) {
            let number = object_type_number(target, token, context.snapshot)?;
            parts.push(format!(
                "(CASE WHEN {discriminator} = {tag_sql} THEN {} WHEN {discriminator} <> {tag_sql} THEN {} END = {type_sql})",
                context.dialect.binary_u32(number),
                context.dialect.binary_u32(0),
            ));
        }
    }
    if let Some(value_sql) = &operand.value_sql
        && let Some(member) = composite_value_member(field, operand.tag)
    {
        parts.push(format!(
            "({} = {value_sql})",
            context.sql_column(resolved, member)
        ));
    }
    Ok(Some(if parts.len() == 1 {
        parts.pop().expect("one part")
    } else {
        format!("({})", parts.join(" AND "))
    }))
}

/// Compares a composite field with one value. Returns `None` when the
/// operands are not such a pair, leaving the generic path to report what
/// it cannot render.
/// Compiles a comparison naming `<источник>.<Состав>.<Поле>` as an
/// `EXISTS` over the section, correlated with the owner row. Returns
/// `None` when neither side names a section, leaving the ordinary path to
/// compile — or to report — the expression.
fn compile_tabular_section_comparison(
    left: &Expression<'_, '_>,
    right: &Expression<'_, '_>,
    operator: &Token<'_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let (path, other, section_on_left) = match (left, right) {
        (Expression::Field(path), other) if context.section_path(path).is_some() => {
            (path, other, true)
        }
        (other, Expression::Field(path)) if context.section_path(path).is_some() => {
            (path, other, false)
        }
        _ => return Ok(None),
    };
    let (scope, section, field) = context
        .section_path(path)
        .expect("the guard resolved this path");
    let Some(column) = section_column(context, scope, section, field) else {
        return Ok(None);
    };
    let alias = context.next_section_alias();
    let dialect = context.dialect;
    let quoted = dialect.quote_identifier(&alias);
    // A reference pair of the section compares as one payload, so the
    // other side is widened to a payload the same way an ordinary
    // comparison of a composite reference widens it.
    let (section_sql, other_sql) = match &column.column {
        SectionValue::Single(name) => (
            format!("{quoted}.{}", dialect.quote_identifier(name)),
            compile_expression(other, context)?,
        ),
        SectionValue::ReferencePair { type_member, value } => {
            let (sql, kind) = value_operand(other, context)?;
            let (payload, _) =
                widen_reference(&sql, &kind, operand_token(other), context.snapshot, dialect)?;
            (
                dialect.reference_payload(
                    &format!("{quoted}.{}", dialect.quote_identifier(type_member)),
                    &format!("{quoted}.{}", dialect.quote_identifier(value)),
                ),
                payload,
            )
        }
    };
    let (left_sql, right_sql) = if section_on_left {
        (section_sql, other_sql)
    } else {
        (other_sql, section_sql)
    };
    let owner_sql = context.identity_sql(scope)?;
    Ok(Some(format!(
        "EXISTS (SELECT 1 FROM {} AS {quoted} WHERE {quoted}.{} = {owner_sql} AND ({left_sql} {} {right_sql}))",
        dialect.quote_identifier(&column.table),
        dialect.quote_identifier(&column.owner_column),
        binary_operator_sql(operator)?,
    )))
}

fn compile_composite_comparison(
    left: &Expression<'_, '_>,
    right: &Expression<'_, '_>,
    operator: &Token<'_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let (field, other) = match (left, right) {
        (Expression::Field(field), other) | (other, Expression::Field(field)) => (field, other),
        _ => return Ok(None),
    };
    let resolved = context.resolve(field)?;
    if resolved.field().columns.len() < 2 {
        return Ok(None);
    }
    let Some(operand) = composite_operand(other, context)? else {
        return Ok(None);
    };
    let token = operand_token(other).unwrap_or(field.last());
    let Some(sql) = composite_member_equality(&resolved, &operand, token, context)? else {
        return Ok(None);
    };
    Ok(Some(if operator.lexeme == "<>" {
        format!("(NOT {sql})")
    } else {
        sql
    }))
}

/// Compiles `<составное поле> [НЕ] В (…)` as the disjunction of the member
/// comparisons of the listed values, the way the platform groups its own
/// list by type.
fn compile_composite_in_list(
    value: &Expression<'_, '_>,
    items: &[Expression<'_, '_>],
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<String>, QueryDiagnostic> {
    let Expression::Field(field) = value else {
        return Ok(None);
    };
    let resolved = context.resolve(field)?;
    if resolved.field().columns.len() < 2 || composite_type_member(resolved.field()).is_none() {
        return Ok(None);
    }
    let mut operands = Vec::with_capacity(items.len());
    for item in items {
        if let Expression::Parameter(token) = item
            && let Some(value) = context.catalog.parameters().lookup(token)?
            && let Some(elements) = list_elements(value, token)?
        {
            for element in elements {
                let Some(operand) = composite_operand_of_value(element, token, context)? else {
                    return Ok(None);
                };
                operands.push((operand, *token));
            }
            continue;
        }
        let Some(operand) = composite_operand(item, context)? else {
            return Ok(None);
        };
        operands.push((operand, operand_token(item).unwrap_or(field.last())));
    }
    if operands.is_empty() {
        return Ok(Some(context.dialect.boolean_literal_predicate(negated)));
    }
    let mut parts = Vec::with_capacity(operands.len());
    for (operand, token) in &operands {
        let Some(sql) = composite_member_equality(&resolved, operand, token, context)? else {
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
/// The column the seed relation of `В ИЕРАРХИИ` exposes.
const HIERARCHY_NODE: &str = "__node";

/// Compiles the seed list of `В ИЕРАРХИИ (…)` into a relation of one
/// column. The seeds live in a CTE, which cannot read the outer row, so
/// only constants are accepted there.
fn compile_hierarchy_list_seeds(
    token: &Token<'_>,
    items: &[Expression<'_, '_>],
    context: &mut CompilationContext<'_, '_>,
) -> Result<(String, Option<ObjectId>), QueryDiagnostic> {
    let node = context.dialect.quote_identifier(HIERARCHY_NODE);
    let mut parts = Vec::with_capacity(items.len());
    // The catalog every seed belongs to, when they agree on one.
    let mut target: Option<Option<ObjectId>> = None;
    for item in items {
        if let Ok(ColumnKind::Reference { targets, .. }) = expression_kind(item, context) {
            let seed = match targets.as_slice() {
                [only] => Some(*only),
                _ => None,
            };
            target = Some(match target {
                Some(previous) if previous == seed => seed,
                Some(_) => None,
                None => seed,
            });
        }
        if matches!(item, Expression::Field(_)) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(operand_token(item).unwrap_or(token)),
                "IN HIERARCHY accepts constants and a nested query, not a field",
            ));
        }
        if let Expression::Parameter(parameter) = item
            && let Some(value) = context.catalog.parameters().lookup(parameter)?
            && let Some(elements) = list_elements(value, parameter)?
        {
            for element in elements {
                let sql = render_scalar_parameter(element, parameter, context.dialect, true)?;
                parts.push(format!("SELECT {sql} AS {node}"));
            }
            continue;
        }
        let mut sql = compile_expression(item, context)?;
        // The seed relation is the anchor of a recursive CTE, and an
        // untyped `NULL` there leaves the column without a type to join the
        // catalog's reference against.
        if expression_kind(item, context)? == ColumnKind::Null {
            sql = context
                .dialect
                .typed_null(context.dialect.reference_identifier_type());
        }
        parts.push(format!("SELECT {sql} AS {node}"));
    }
    if parts.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(token),
            "IN HIERARCHY list must contain at least one expression",
        ));
    }
    Ok((parts.join(" UNION ALL "), target.flatten()))
}

/// Compiles the nested query of `В ИЕРАРХИИ (ВЫБРАТЬ …)` into a relation
/// of one column.
fn compile_hierarchy_query_seeds(
    token: &Token<'_>,
    query: &crate::query::core::ast::QueryAst<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
) -> Result<(String, Option<ObjectId>), QueryDiagnostic> {
    let dialect = context.dialect;
    let mut presentations =
        PresentationCompilation::strict(&[], context.catalog.parameters(), dialect);
    let inner = compile_query_ast(
        query,
        context.snapshot,
        context.catalog,
        &mut presentations,
        Some(token),
    )?;
    let [column] = inner.columns.as_slice() else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "IN HIERARCHY subquery must project exactly one column, found {}",
                inner.columns.len()
            ),
        ));
    };
    // The catalog the seeds belong to, which a composite value must match
    // by type to be under any of them.
    let target = match &column.kind {
        ColumnKind::Reference { targets, .. } => match targets.as_slice() {
            [target] => Some(*target),
            _ => None,
        },
        _ => None,
    };
    let alias = dialect.quote_identifier("__seeds");
    Ok((
        format!(
            "SELECT {alias}.{} AS {} FROM ({}) AS {alias}",
            dialect.quote_identifier(&column.label),
            dialect.quote_identifier(HIERARCHY_NODE),
            inner.sql
        ),
        target,
    ))
}

/// Compiles `<поле> [НЕ] В ИЕРАРХИИ (<seeds>)`: the value matches a seed
/// or any of its descendants. The descent is a recursive CTE over the
/// catalog's parent column, defined once per predicate at statement
/// level; a catalog without a parent column degenerates to plain
/// membership, as on the platform.
fn compile_in_hierarchy(
    token: &Token<'_>,
    value: &Expression<'_, '_>,
    seeds: String,
    seeds_target: Option<ObjectId>,
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let Expression::Field(reference) = value else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "IN HIERARCHY tests a reference field",
        ));
    };
    let resolved = context.resolve(reference)?;
    let dialect = context.dialect;
    let node = dialect.quote_identifier(HIERARCHY_NODE);
    // A composite reference is tested member by member: its identifier is
    // compared with the seeds, and its type must be the seeds' own, since
    // a value of another type is under no seed — measured on 8.3.27.
    let (target, value_sql, type_guard) = match reference_pair(resolved.field()) {
        Some((type_member, value_member)) => {
            // Without a known catalog behind the seeds — a parameter bound
            // to NULL, say — there is no hierarchy to descend and no type
            // to guard; the identifiers alone decide, and they are unique.
            let guard = seeds_target
                .map(|target| {
                    object_type_number(target, reference.last(), context.snapshot).map(|number| {
                        format!(
                            "{} = {}",
                            context.sql_column(&resolved, type_member),
                            dialect.binary_literal(&number.to_be_bytes())
                        )
                    })
                })
                .transpose()?;
            (
                seeds_target,
                context.sql_column(&resolved, value_member),
                guard,
            )
        }
        None => {
            let column = scalar_column(&resolved, reference.last())?;
            let ColumnKind::Reference {
                targets,
                runtime_typed: false,
            } = &column.kind
            else {
                return Err(hierarchy_target_diagnostic(reference.last()));
            };
            let [target] = targets.as_slice() else {
                return Err(hierarchy_target_diagnostic(reference.last()));
            };
            (Some(*target), context.sql_column(&resolved, column), None)
        }
    };
    let relation = match target
        .and_then(|target| hierarchical_catalog_of(target, context.snapshot, dialect))
    {
        Some((table, id, parent)) => {
            // The descent reads the parent chain of the catalog with no
            // filter of its own; a restricted compilation refuses it
            // rather than pretending the walk is contained.
            context
                .catalog
                .refuse_unfiltered_read(Some(token), "a hierarchy descent (В ИЕРАРХИИ)")?;
            let name = context.catalog.next_hierarchy_name();
            let quoted = dialect.quote_identifier(&name);
            let source = dialect.quote_identifier("__catalog");
            context.catalog.push_hierarchy_cte(
                name,
                format!(
                    "{seeds} UNION ALL SELECT {source}.{id} FROM {table} AS {source} JOIN {quoted} ON {source}.{parent} = {quoted}.{node}"
                ),
            );
            quoted
        }
        // A catalog without a parent column has no hierarchy, so the
        // predicate is plain membership.
        None => format!("({seeds}) AS {}", dialect.quote_identifier("__seeds_flat")),
    };
    let membership = format!("EXISTS (SELECT 1 FROM {relation} WHERE {node} = {value_sql})");
    let tested = match type_guard {
        Some(guard) => format!("({guard} AND {membership})"),
        None => membership,
    };
    Ok(format!("{}{tested}", if negated { "NOT " } else { "" }))
}

fn hierarchy_target_diagnostic(token: &Token<'_>) -> QueryDiagnostic {
    QueryDiagnostic::at(
        QueryDiagnosticKind::UnsupportedFeature,
        Some(token),
        format!(
            "IN HIERARCHY needs the field {:?} to reference one catalog",
            token.lexeme
        ),
    )
}

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
        PresentationCompilation::strict(&[], context.catalog.parameters(), dialect);
    // A subquery of a predicate may read the row of the enclosing
    // statement, which is how the platform answers a correlated `В (…)`.
    let outer = context.outer_scopes();
    let inner = compile_query_ast_with_outer(
        query,
        snapshot,
        context.catalog,
        &mut presentations,
        Some(token),
        &outer,
    )?;
    if let Expression::Tuple { items, .. } = value {
        return compile_in_tuple_query(token, items, &inner, negated, context);
    }
    // A subquery whose value is composite projects one column per member,
    // and the platform compares the members side by side, measured on
    // 8.3.27: `(T1._Fld70_TYPE, T1._Fld70_S, …) IN (SELECT …)`. Columns
    // that are not the members of one value stay an error.
    if let Some(members) = composite_result_members(&inner.columns) {
        return compile_in_composite_query(token, value, &inner, &members, negated, context);
    }
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
        // The subquery projects its string as text, and PostgreSQL has no
        // operator between text and the mvarchar the outer field is.
        (ColumnKind::String { .. }, ColumnKind::String { .. }) => {
            (dialect.scalar_text(&outer_sql), inner.sql)
        }
        _ => (outer_sql, inner.sql),
    };
    let sql = format!("({outer_sql} IN ({inner_sql}))");
    Ok(if negated { format!("(NOT {sql})") } else { sql })
}

/// `(А, Б) [НЕ] В (ВЫБРАТЬ X, Y …)`: one projection of the subquery per
/// tuple item, compared side by side. A composite projection spreads
/// over several columns (`Поле_TYPE`, `Поле_S`, `Поле_RRRef`, …), and the
/// item is spread over the same members the way a composite `В (…)`
/// does. Rendered as `EXISTS` over the subquery with one equality per
/// column, so both dialects read it the same way.
fn compile_in_tuple_query(
    token: &Token<'_>,
    items: &[Expression<'_, '_>],
    inner: &CompiledQuery,
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let groups = projection_groups(&inner.columns);
    if groups.len() != items.len() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "IN subquery must project {} columns to match the tuple, found {}",
                items.len(),
                groups.len()
            ),
        ));
    }
    let dialect = context.dialect;
    let wrapper = "__in";
    let mut equalities = Vec::new();
    for (item, group) in items.iter().zip(&groups) {
        let column_sql =
            |column: &CompiledColumn| dialect.qualified_column(Some(wrapper), &column.label);
        if let [(column, "")] = group.as_slice() {
            let (outer_sql, outer_kind) = value_operand(item, context)?;
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
            let runtime_typed = |kind: &ColumnKind| {
                matches!(
                    kind,
                    ColumnKind::Reference {
                        runtime_typed: true,
                        ..
                    }
                )
            };
            // A reference of several types is its RTRef ‖ RRRef payload on
            // both sides; a fixed one is widened to it, as `В (ВЫБРАТЬ …)`
            // widens a single value.
            let (inner_sql, outer_sql) =
                match (runtime_typed(&outer_kind), runtime_typed(&column.kind)) {
                    (true, false) => (
                        widen_reference(
                            &column_sql(column),
                            &column.kind,
                            Some(token),
                            context.snapshot,
                            dialect,
                        )?
                        .0,
                        outer_sql,
                    ),
                    (false, true) => (
                        column_sql(column),
                        widen_reference(
                            &outer_sql,
                            &outer_kind,
                            operand_token(item),
                            context.snapshot,
                            dialect,
                        )?
                        .0,
                    ),
                    _ => (
                        column_sql(column),
                        match (&outer_kind, &column.kind) {
                            (ColumnKind::String { .. }, ColumnKind::String { .. }) => {
                                dialect.scalar_text(&outer_sql)
                            }
                            _ => outer_sql,
                        },
                    ),
                };
            equalities.push(format!("{inner_sql} = {outer_sql}"));
            continue;
        }
        // A composite field on the outer side answers with its own
        // members, the reference of several types included.
        let members = group.iter().map(|(_, member)| *member).collect::<Vec<_>>();
        if let Some(rendered) = composite_field_members(item, &members, context)? {
            for ((column, _), member_sql) in group.iter().zip(rendered) {
                equalities.push(format!("{} = {member_sql}", column_sql(column)));
            }
            continue;
        }
        // Otherwise the item is spread over the members the way a
        // composite `В (…)` spreads its value — its own member carries
        // it, the discriminator its tag, the others their zero, all of
        // them `NULL` while the item is.
        let (outer_sql, outer_kind) = value_operand(item, context)?;
        let Some((own, tag)) = composite_member_of(&outer_kind) else {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!("value of kind {outer_kind:?} has no place in a composite result"),
            ));
        };
        if group.iter().any(|(_, member)| *member == "_RTRef")
            || matches!(
                outer_kind,
                ColumnKind::Reference {
                    runtime_typed: true,
                    ..
                }
            )
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "a tuple membership test does not accept a reference of several types",
            ));
        }
        let guarded = |value: String| format!("CASE WHEN {outer_sql} IS NOT NULL THEN {value} END");
        for (column, member) in group {
            let member_sql = match *member {
                "_TYPE" => guarded(dialect.binary_literal(&[tag.tag()])),
                "_S" if own == "_S" => dialect.scalar_text(&outer_sql),
                member if member == own => outer_sql.clone(),
                // The reference member of a projected field is its 16-byte
                // identifier, not the widened payload of a result.
                "" => guarded(dialect.binary_literal(&[0; 16])),
                other => guarded(composite_member_zero(other, dialect)),
            };
            equalities.push(format!("{} = {member_sql}", column_sql(column)));
        }
    }
    let sql = format!(
        "EXISTS (SELECT 1 FROM ({}) AS {} WHERE {})",
        inner.sql,
        dialect.quote_identifier(wrapper),
        equalities.join(" AND ")
    );
    Ok(if negated { format!("(NOT {sql})") } else { sql })
}

/// Groups the columns of a result by the projection they come from: a
/// composite projection labels its members `<name>_TYPE`, `<name>_S`, …,
/// with the reference member as `<name>_RRRef`; every other column is a
/// projection of its own. Each column is paired with its member name in
/// the spelling [`spread_over_members`] takes (`""` for the reference).
#[allow(clippy::type_complexity)]
fn projection_groups(columns: &[CompiledColumn]) -> Vec<Vec<(&CompiledColumn, &'static str)>> {
    const SUFFIXES: [(&str, &str); 9] = [
        ("_TYPE", "_TYPE"),
        ("_RTRef", "_RTRef"),
        ("_RRRef", ""),
        ("_S", "_S"),
        ("_N", "_N"),
        ("_T", "_T"),
        ("_L", "_L"),
        ("_U", "_U"),
        ("_B", "_B"),
    ];
    // A member column, by the projection it labels and its member name;
    // `None` for a plain column.
    let member_of = |label: &str| -> Option<(String, &'static str)> {
        SUFFIXES.iter().find_map(|(suffix, member)| {
            label
                .strip_suffix(suffix)
                .filter(|base| !base.is_empty())
                .map(|base| (base.to_owned(), *member))
        })
    };
    let mut groups: Vec<(Option<String>, Vec<(&CompiledColumn, &'static str)>)> = Vec::new();
    for column in columns {
        match member_of(&column.label) {
            Some((base, member)) => match groups.last_mut() {
                Some((Some(open), group)) if *open == base => group.push((column, member)),
                _ => groups.push((Some(base), vec![(column, member)])),
            },
            // The reference payload of a projected composite field carries
            // the bare name, after the `_TYPE` member.
            None => match groups.last_mut() {
                Some((Some(open), group)) if *open == column.label => group.push((column, "")),
                _ => groups.push((None, vec![(column, "")])),
            },
        }
    }
    groups.into_iter().map(|(_, group)| group).collect()
}

/// The member suffixes of a result that is one composite value: every
/// column labels the same value with its member suffix, one of them is the
/// discriminator and one carries the value itself. `None` for a result
/// whose columns are separate values.
fn composite_result_members(columns: &[CompiledColumn]) -> Option<Vec<&'static str>> {
    if columns.len() < 2 {
        return None;
    }
    let mut members = Vec::with_capacity(columns.len());
    let mut base = None::<&str>;
    for column in columns {
        let suffix = ["_TYPE", "_S", "_N", "_T", "_L"]
            .into_iter()
            .find(|suffix| column.label.ends_with(suffix))
            .unwrap_or("");
        let label = column
            .label
            .strip_suffix(suffix)
            .expect("the suffix was found in the label");
        match base {
            Some(base) if base != label => return None,
            Some(_) => {}
            None => base = Some(label),
        }
        members.push(suffix);
    }
    if !members.contains(&"_TYPE") || !members.contains(&"") {
        return None;
    }
    Some(members)
}

/// Spreads one value over the members of a composite result: its own
/// member carries the value, the discriminator carries its tag, every other
/// member carries the zero of its type — and all of them stay `NULL` while
/// the value is `NULL`, exactly as the platform writes them.
pub(super) fn spread_over_members(
    sql: &str,
    kind: &ColumnKind,
    members: &[&'static str],
    token: Option<&Token<'_>>,
    context: &CompilationContext<'_, '_>,
) -> Result<Vec<String>, QueryDiagnostic> {
    let dialect = context.dialect;
    let (own, tag) = composite_member_of(kind).ok_or_else(|| {
        QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnsupportedFeature,
            token,
            format!("value of kind {kind:?} has no place in a composite result"),
        )
    })?;
    let (sql, _) = widen_reference(sql, kind, token, context.snapshot, dialect)?;
    let guarded = |value: &str| format!("CASE WHEN {sql} IS NOT NULL THEN {value} END");
    members
        .iter()
        .map(|member| {
            Ok(match *member {
                "_TYPE" => guarded(&dialect.binary_literal(&[tag.tag()])),
                // The string member of a composite result is projected as
                // text, so the value compared with it is text as well.
                "_S" if *member == own => dialect.scalar_text(&sql),
                other if other == own => sql.clone(),
                other => guarded(&composite_member_zero(other, dialect)),
            })
        })
        .collect()
}

/// The members a composite field itself provides, in the requested order.
/// `None` when the value is not a composite field, and a diagnostic when it
/// is one whose members do not answer the request.
fn composite_field_members(
    value: &Expression<'_, '_>,
    members: &[&'static str],
    context: &mut CompilationContext<'_, '_>,
) -> Result<Option<Vec<String>>, QueryDiagnostic> {
    let Expression::Field(reference) = value else {
        return Ok(None);
    };
    let resolved = context.resolve(reference)?;
    if resolved.expression.is_some() || composite_type_member(resolved.field()).is_none() {
        return Ok(None);
    }
    let dialect = context.dialect;
    let member_of = |column: &QueryableColumn| -> &'static str {
        let lower = column.physical_name.to_ascii_lowercase();
        for (suffix, member) in [
            ("_type", "_TYPE"),
            ("_s", "_S"),
            ("_n", "_N"),
            ("_t", "_T"),
            ("_l", "_L"),
        ] {
            if lower.ends_with(suffix) {
                return member;
            }
        }
        ""
    };
    let mut rendered = Vec::with_capacity(members.len());
    for member in members {
        let sql = if member.is_empty() {
            let value_member = resolved
                .field()
                .columns
                .iter()
                .find(|column| column.is_reference_value_member());
            let type_member = resolved
                .field()
                .columns
                .iter()
                .find(|column| column.is_reference_type_member());
            match (type_member, value_member) {
                (Some(type_member), Some(value_member)) => dialect.reference_payload(
                    &context.sql_column(&resolved, type_member),
                    &context.sql_column(&resolved, value_member),
                ),
                (None, Some(value_member)) => context.sql_column(&resolved, value_member),
                _ => return Ok(None),
            }
        } else if *member == "_RTRef" {
            let Some(column) = resolved
                .field()
                .columns
                .iter()
                .find(|column| column.is_reference_type_member())
            else {
                return Ok(None);
            };
            context.sql_column(&resolved, column)
        } else {
            let column =
                resolved.field().columns.iter().find(|column| {
                    !column.is_reference_value_member() && member_of(column) == *member
                });
            let Some(column) = column else {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(reference.last()),
                    format!(
                        "composite field {:?} has no {member} member to compare with the subquery",
                        resolved.field().name
                    ),
                ));
            };
            let sql = context.sql_column(&resolved, column);
            if *member == "_S" {
                dialect.scalar_text(&sql)
            } else {
                sql
            }
        };
        rendered.push(sql);
    }
    Ok(Some(rendered))
}

/// Compiles `<значение> В (<подзапрос с составным значением>)`.
fn compile_in_composite_query(
    token: &Token<'_>,
    value: &Expression<'_, '_>,
    inner: &CompiledQuery,
    members: &[&'static str],
    negated: bool,
    context: &mut CompilationContext<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    if context.dialect != SqlDialect::Postgres {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "a composite subquery of В (…) needs a row comparison, which T-SQL has not",
        ));
    }
    // A composite value on the outer side answers with its own members.
    let spread = match composite_field_members(value, members, context)? {
        Some(rendered) => rendered,
        None => {
            let (outer_sql, outer_kind) = value_operand(value, context)?;
            spread_over_members(&outer_sql, &outer_kind, members, Some(token), context)?
        }
    };
    let sql = format!("(({}) IN ({}))", spread.join(", "), inner.sql);
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
    // A value dereferenced across reference targets carries its reference
    // member as the `RTRef ‖ RRRef` payload of each target, so a typed
    // constant is widened to its payload; a constant of unknown type — a
    // parameter bound to `NULL` — compares as it is.
    if !resolved.member_expressions.is_empty()
        && let Some(column) = resolved
            .field()
            .columns
            .iter()
            .find(|column| matches!(column.kind, ColumnKind::Reference { .. }))
    {
        let value = match &constant.type_sql {
            Some(type_sql) => context
                .dialect
                .reference_payload(type_sql, &constant.id_sql),
            None => constant.id_sql.clone(),
        };
        return Ok(Some(format!(
            "({} = {value})",
            context.sql_column(resolved, column)
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
    if let Some(sql) = compile_case_toward(expression, other, context)? {
        return Ok(sql);
    }
    if let (Expression::Literal(token), Expression::Field(reference)) = (expression, other) {
        let resolved = context.resolve(reference)?;
        let column = scalar_column(&resolved, reference.last())?;
        return context.dialect.literal_for_type(token, &column.data_type);
    }
    let sql = compile_expression(expression, context)?;
    // A string value stands beside a stored string of the provider's own
    // type, which has no common type with the text an expression answers,
    // so the value takes the type of the field it is compared with. The
    // field itself is left alone, or an index over it could not be used.
    if let Expression::Field(reference) = other
        && matches!(
            expression_kind(expression, context)?,
            ColumnKind::String { .. }
        )
    {
        let resolved = context.resolve(reference)?;
        let column = scalar_column(&resolved, reference.last())?;
        let own = match expression {
            Expression::Field(own) => {
                let resolved = context.resolve(own)?;
                single_column(resolved.field(), own.last())?
                    .data_type
                    .clone()
            }
            _ => String::new(),
        };
        let dialect = context.dialect;
        if dialect.is_provider_string_type(&column.data_type)
            && !dialect.is_provider_string_type(&own)
        {
            return Ok(format!(
                "CAST({sql} AS {})",
                column.data_type.trim().to_ascii_lowercase()
            ));
        }
    }
    Ok(sql)
}

fn compile_date_operand(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    position: &str,
) -> Result<String, QueryDiagnostic> {
    let name = function_name(token);
    if let Expression::Field(reference) = expression {
        let resolved = context.resolve(reference)?;
        let column = scalar_column(&resolved, reference.last())?;
        if column.kind != ColumnKind::DateTime && !is_date_sql_type(&column.data_type) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                format!("{name} {position} argument must resolve to a date field"),
            ));
        }
        return Ok(context.sql_column(&resolved, column));
    }
    let sql = compile_expression(expression, context)?;
    let kind = expression_kind(expression, context)?;
    if is_date_operand_kind(&kind) {
        // A value that is `NULL` states the type it stands for, or
        // PostgreSQL has none to resolve the date arithmetic against.
        if kind == ColumnKind::Null {
            return Ok(context
                .dialect
                .typed_null(&derived_data_type(&ColumnKind::DateTime, context.dialect)));
        }
        return Ok(sql);
    }
    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(token),
        format!("{name} {position} argument must be a date expression"),
    ))
}

/// Compiles the count of `ДОБАВИТЬКДАТЕ`, which must be numeric: a number
/// kind, a parameter of unknown value, or a `NULL`.
fn compile_count_operand(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    let sql = compile_expression(expression, context)?;
    let kind = expression_kind(expression, context)?;
    if is_count_kind(&kind) {
        return Ok(sql);
    }
    Err(count_diagnostic(token))
}

/// Whether a kind may serve as the date argument of a date function: a
/// date, or a value of unknown kind such as an unbound parameter.
pub(super) fn is_date_operand_kind(kind: &ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::DateTime | ColumnKind::Null | ColumnKind::Unknown { .. }
    )
}

/// Whether a kind may serve as the count of `ДОБАВИТЬКДАТЕ`.
pub(super) fn is_count_kind(kind: &ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::Number { .. } | ColumnKind::Null | ColumnKind::Unknown { .. }
    )
}

pub(super) fn count_diagnostic(token: &Token<'_>) -> QueryDiagnostic {
    QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(token),
        format!("{} count must be a number", function_name(token)),
    )
}

/// The stable English name of a function keyword token, for diagnostics.
pub(super) fn function_name(token: &Token<'_>) -> &'static str {
    match token.kind {
        TokenKind::Keyword(keyword) => keyword.as_str(),
        _ => "function",
    }
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
            // An aggregate over a reference pair takes the payload, the
            // way the platform aggregates the concatenated members.
            Expression::Field(reference) => context.resolve(reference).and_then(|resolved| {
                if let Some(sql) = reference_pair_payload(&resolved, context)
                    && let Some((_, value_member)) = reference_pair(resolved.field())
                {
                    return Ok((sql, payload_kind(&value_member.kind)));
                }
                // A member aggregated alone keeps its kind: the `RRRef`
                // member of a reference stays a fixed reference of 16
                // bytes, a payload column stays runtime-typed.
                let column = countable_column(resolved.field(), reference.last())?;
                Ok((context.sql_column(&resolved, column), column.kind.clone()))
            }),
            other => compile_expression(other, context)
                .and_then(|sql| Ok((sql, expression_kind(other, context)?))),
        },
    };
    context.aggregates_allowed = outer_allowed;
    let (argument, argument_kind) = compiled?;
    let output_kind = match kind {
        AggregateKind::Count | AggregateKind::Sum | AggregateKind::Avg => number,
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

/// The one column an expression reads from a field: a value dereferenced
/// across reference targets is spread over composite members and stands
/// for its first member, the value itself; any other compound field is
/// refused.
fn scalar_column<'field>(
    resolved: &'field ResolvedPath,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    if !resolved.member_expressions.is_empty()
        && let Some(column) = resolved.field().columns.first()
    {
        return Ok(column);
    }
    single_column(resolved.field(), token)
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
