//! Scalar semantic decisions shared by sourced and source-free compilation.

use crate::query::ColumnKind;
use crate::query::core::ast::{Expression, expression_children};
use crate::query::core::dialect::SqlDialect;
use crate::{Keyword, TokenKind};

/// Identify direct comparison operands without resolving names or generating SQL.
/// Contextual CASE selection belongs to the existing expression compiler.
pub(super) fn postgres_string_equality_operands<'e, 't, 's>(
    expression: &'e Expression<'t, 's>,
    dialect: SqlDialect,
) -> Option<(&'e Expression<'t, 's>, &'e Expression<'t, 's>)> {
    if dialect == SqlDialect::Postgres
        && let Expression::Binary {
            left,
            operator,
            right,
        } = expression
        && operator.lexeme == "="
        && !matches!(left.as_ref(), Expression::Case { .. })
        && !matches!(right.as_ref(), Expression::Case { .. })
    {
        Some((left, right))
    } else {
        None
    }
}

/// The measured PostgreSQL equality policy, shared by both source contexts.
pub(super) enum ScalarStringEquality {
    Strings,
    Incompatible,
}

impl ScalarStringEquality {
    pub(super) fn for_kinds(left: &ColumnKind, right: &ColumnKind) -> Option<Self> {
        use ColumnKind::{Boolean, DateTime, Number, String};
        match (left, right) {
            (String { .. }, String { .. }) => Some(Self::Strings),
            (String { .. }, Number { .. } | Boolean | DateTime)
            | (Number { .. } | Boolean | DateTime, String { .. }) => Some(Self::Incompatible),
            _ => None,
        }
    }
}

/// A scalar recipe is exact only if SQL did not erase a possible Undefined.
/// Wildcard kinds retain their existing standalone representation.
pub(super) fn has_exact_scalar_projection(
    expression: &Expression<'_, '_>,
    kind: &ColumnKind,
) -> bool {
    kind.is_wildcard() || !contains_undefined(expression)
}

fn contains_undefined(expression: &Expression<'_, '_>) -> bool {
    matches!(expression, Expression::Literal(token)
        if token.kind == TokenKind::Keyword(Keyword::Undefined))
        || expression_children(expression)
            .into_iter()
            .any(contains_undefined)
}
