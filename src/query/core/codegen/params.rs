//! Named parameter values available during one compilation.

use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::DateTimeValue;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::params::ParameterValue;
use crate::query::core::resolve::ColumnKind;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// The column kind a parameter value contributes to an expression.
pub(super) fn parameter_kind(value: &ParameterValue) -> ColumnKind {
    match value {
        ParameterValue::Null => ColumnKind::Null,
        ParameterValue::Boolean(_) => ColumnKind::Boolean,
        ParameterValue::Number { scale, .. } => ColumnKind::Number {
            precision: None,
            scale: Some(*scale),
        },
        ParameterValue::String(_) => ColumnKind::String { length: None },
        ParameterValue::Date(_) => ColumnKind::DateTime,
        ParameterValue::Reference { object, .. } => ColumnKind::Reference {
            targets: vec![*object],
            runtime_typed: false,
        },
        ParameterValue::Binary(bytes) => ColumnKind::Binary {
            length: u32::try_from(bytes.len()).ok(),
        },
        ParameterValue::List(items) => items
            .iter()
            .map(parameter_kind)
            .find(|kind| !kind.is_wildcard())
            .unwrap_or(ColumnKind::Null),
    }
}

/// Renders a scalar parameter as a literal of the dialect. `storage_domain`
/// selects the physical (offset) date domain of sourced branches.
pub(super) fn render_scalar_parameter(
    value: &ParameterValue,
    token: &Token<'_>,
    dialect: SqlDialect,
    storage_domain: bool,
) -> Result<String, QueryDiagnostic> {
    match value {
        ParameterValue::Null => Ok("NULL".to_owned()),
        ParameterValue::Boolean(value) => Ok(dialect.boolean_literal(*value).to_owned()),
        ParameterValue::Number { unscaled, scale } => Ok(decimal_text(*unscaled, *scale)),
        ParameterValue::String(value) => Ok(dialect.string_literal(value)),
        ParameterValue::Date(date) => dialect.datetime_expression(
            DateTimeValue {
                year: date.year(),
                month: date.month(),
                day: date.day(),
                hour: date.hour(),
                minute: date.minute(),
                second: date.second(),
            },
            storage_domain,
            token,
        ),
        ParameterValue::Reference { id, .. } => Ok(dialect.binary_literal(id)),
        ParameterValue::Binary(bytes) => Ok(dialect.binary_literal(bytes)),
        ParameterValue::List(_) => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Parameter,
            Some(token),
            format!(
                "list parameter {:?} is allowed only as the operand of IN",
                token.lexeme
            ),
        )),
    }
}

/// The elements of a list parameter, rejecting nested lists.
pub(super) fn list_elements<'value>(
    value: &'value ParameterValue,
    token: &Token<'_>,
) -> Result<Option<&'value [ParameterValue]>, QueryDiagnostic> {
    let ParameterValue::List(items) = value else {
        return Ok(None);
    };
    if items
        .iter()
        .any(|item| matches!(item, ParameterValue::List(_)))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Parameter,
            Some(token),
            format!("list parameter {:?} must not contain lists", token.lexeme),
        ));
    }
    Ok(Some(items))
}

/// Renders `unscaled × 10^-scale` as a decimal literal.
pub(super) fn decimal_text(unscaled: i128, scale: u8) -> String {
    let negative = unscaled < 0;
    let mut digits = unscaled.unsigned_abs().to_string();
    let scale = usize::from(scale);
    if scale > 0 {
        while digits.len() <= scale {
            digits.insert(0, '0');
        }
        digits.insert(digits.len() - scale, '.');
    }
    if negative {
        digits.insert(0, '-');
    }
    digits
}

/// A constant that can stand for a reference in a comparison: an optional
/// 4-byte type discriminator expression and a 16-byte identifier expression.
pub(super) struct ReferenceConstant {
    /// The `RTRef` bytes, when the constant knows its type.
    pub(super) type_sql: Option<String>,
    /// The `RRRef` bytes or an expression yielding them.
    pub(super) id_sql: String,
    /// The target object, when known.
    pub(super) target: Option<ObjectId>,
}

/// Builds the reference constant of a parameter value, if it is one.
pub(super) fn reference_constant_of_value(
    value: &ParameterValue,
    token: &Token<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<Option<ReferenceConstant>, QueryDiagnostic> {
    match value {
        ParameterValue::Reference { object, id } => Ok(Some(ReferenceConstant {
            type_sql: Some(dialect.binary_u32(object_type_number(*object, token, snapshot)?)),
            id_sql: dialect.binary_literal(id),
            target: Some(*object),
        })),
        ParameterValue::Binary(bytes) => Ok(reference_constant_of_bytes(bytes, dialect)),
        _ => Ok(None),
    }
}

/// A 16-byte constant is a bare `RRRef`; a 20-byte constant is the console
/// output format `RTRef ‖ RRRef` of a runtime-typed field.
pub(super) fn reference_constant_of_bytes(
    bytes: &[u8],
    dialect: SqlDialect,
) -> Option<ReferenceConstant> {
    match bytes.len() {
        16 => Some(ReferenceConstant {
            type_sql: None,
            id_sql: dialect.binary_literal(bytes),
            target: None,
        }),
        20 => Some(ReferenceConstant {
            type_sql: Some(dialect.binary_literal(&bytes[..4])),
            id_sql: dialect.binary_literal(&bytes[4..]),
            target: None,
        }),
        _ => None,
    }
}

/// The database type number of a metadata object, used as its `RTRef`.
pub(super) fn object_type_number(
    object: ObjectId,
    token: &Token<'_>,
    snapshot: &MetadataSnapshot,
) -> Result<u32, QueryDiagnostic> {
    snapshot
        .object_by_id(object)
        .and_then(|object| object.number)
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(token),
                "reference target has no database type number",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::decimal_text;

    #[test]
    fn renders_decimals_from_unscaled_digits() {
        assert_eq!(decimal_text(1550, 2), "15.50");
        assert_eq!(decimal_text(-5, 3), "-0.005");
        assert_eq!(decimal_text(0, 0), "0");
        assert_eq!(decimal_text(42, 0), "42");
        assert_eq!(decimal_text(-1, 0), "-1");
    }
}
