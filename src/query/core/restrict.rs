//! Row-level access restrictions requested for `РАЗРЕШЕННЫЕ` statements.
//!
//! The application answers a [`RestrictionRequest`] with one
//! [`AccessRestriction`] per target it wants to filter; the condition is
//! SDBL text compiled by the library, never raw SQL.

use crate::metadata::ObjectId;
use crate::query::core::names::names_equal;

/// One table an `РАЗРЕШЕННЫЕ` statement reads: a metadata object, or one
/// of its tabular sections.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RestrictionTarget {
    /// The metadata object, or the owner of the tabular section.
    pub object: ObjectId,
    /// The tabular-section name as the metadata spells it, when the source
    /// is a tabular section (`Документ.Реализация.Товары`).
    pub table_part: Option<String>,
}

impl RestrictionTarget {
    /// Whether a restriction addresses this target; section names compare
    /// case-insensitively, like every 1C identifier.
    #[must_use]
    pub fn matches(&self, restriction: &AccessRestriction) -> bool {
        self.object == restriction.object
            && match (&self.table_part, &restriction.table_part) {
                (None, None) => true,
                (Some(target), Some(supplied)) => names_equal(target, supplied),
                _ => false,
            }
    }
}

/// Deduplicated batch callback request for application access policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestrictionRequest {
    /// Targets of every `РАЗРЕШЕННЫЕ` statement of the batch, in stable
    /// order; empty when no statement carries the keyword.
    pub targets: Vec<RestrictionTarget>,
}

/// A row filter for one target, written as an SDBL condition over the
/// fields of that target, in the style of 1C role-restriction templates.
///
/// ```
/// use open_sdbl::metadata::ObjectId;
/// use open_sdbl::query::AccessRestriction;
///
/// let object = ObjectId::from_bytes([0x11; 16]);
/// let restriction = AccessRestriction::new(object, "Организация В (&Организации)");
/// assert_eq!(restriction.condition(), "Организация В (&Организации)");
/// assert!(restriction.table_part_name().is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRestriction {
    object: ObjectId,
    table_part: Option<String>,
    condition: String,
}

impl AccessRestriction {
    /// Restricts rows of `object` to those satisfying `condition`.
    ///
    /// Fields are written unqualified (`Организация`), one-hop
    /// dereferences (`Владелец.Ответственный`), nested `В (ВЫБРАТЬ …)`, and
    /// `&Параметр` referring to a session parameter are allowed.
    #[must_use]
    pub fn new(object: ObjectId, condition: impl Into<String>) -> Self {
        Self {
            object,
            table_part: None,
            condition: condition.into(),
        }
    }

    /// Addresses a tabular section of the object instead of the object.
    #[must_use]
    pub fn table_part(mut self, name: impl Into<String>) -> Self {
        self.table_part = Some(name.into());
        self
    }

    /// The restricted metadata object, or the owner of the section.
    #[must_use]
    pub const fn object(&self) -> ObjectId {
        self.object
    }

    /// The tabular section the restriction targets, if any.
    #[must_use]
    pub fn section(&self) -> Option<&str> {
        self.table_part.as_deref()
    }

    /// The tabular-section name, when the restriction addresses one.
    #[must_use]
    pub fn table_part_name(&self) -> Option<&str> {
        self.table_part.as_deref()
    }

    /// The SDBL condition text.
    #[must_use]
    pub fn condition(&self) -> &str {
        &self.condition
    }
}

/// Which statements of a batch are filtered by access decisions.
///
/// The mode is chosen when a query is prepared and belongs to the
/// compilation, not to the query text: `РАЗРЕШЕННЫЕ` is what the text can
/// say, [`RestrictionMode::Restricted`] is what an application can
/// demand of any text at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RestrictionMode {
    /// Only statements carrying `РАЗРЕШЕННЫЕ` are filtered, and a target
    /// the application says nothing about is read unfiltered. This is what
    /// the compiler has always done.
    #[default]
    Statement,
    /// Every statement is filtered, whether or not it carries the keyword,
    /// and every target of the request needs an explicit
    /// [`AccessDecision`]. A read the compiler cannot filter is refused
    /// before any SQL is generated.
    Restricted,
}

impl RestrictionMode {
    /// Whether every statement of the batch is filtered.
    #[must_use]
    pub const fn is_restricted(self) -> bool {
        matches!(self, Self::Restricted)
    }
}

/// What the application decided about one target of a restriction request.
///
/// The absence of an [`AccessRestriction`] cannot say whether the
/// application allowed the table or simply did not answer, so
/// [`RestrictionMode::Restricted`] asks for a decision instead.
///
/// ```
/// use open_sdbl::metadata::ObjectId;
/// use open_sdbl::query::{AccessDecision, AccessRestriction, RestrictionTarget};
///
/// let object = ObjectId::from_bytes([0x11; 16]);
/// let target = RestrictionTarget { object, table_part: None };
/// let allowed = AccessDecision::unrestricted(target.clone());
/// let denied = AccessDecision::denied(target.clone());
/// let filtered = AccessDecision::restricted(AccessRestriction::new(object, "Проведен"));
///
/// assert!(allowed.matches(&target) && denied.matches(&target) && filtered.matches(&target));
/// assert!(denied.is_denied());
/// assert_eq!(filtered.restriction().map(AccessRestriction::condition), Some("Проведен"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessDecision {
    /// The target may be read in full, with no row filter.
    Unrestricted(RestrictionTarget),
    /// The target may be read where the condition holds.
    Restricted(AccessRestriction),
    /// The target may not be read: the source is filtered by a predicate
    /// no row satisfies, never left unfiltered.
    Denied(RestrictionTarget),
}

impl AccessDecision {
    /// Allows the target without a row filter.
    #[must_use]
    pub const fn unrestricted(target: RestrictionTarget) -> Self {
        Self::Unrestricted(target)
    }

    /// Refuses the target: its source admits no row.
    #[must_use]
    pub const fn denied(target: RestrictionTarget) -> Self {
        Self::Denied(target)
    }

    /// Allows the rows of the target the restriction's condition holds for.
    #[must_use]
    pub const fn restricted(restriction: AccessRestriction) -> Self {
        Self::Restricted(restriction)
    }

    /// The metadata object the decision addresses.
    #[must_use]
    pub const fn object(&self) -> ObjectId {
        match self {
            Self::Unrestricted(target) | Self::Denied(target) => target.object,
            Self::Restricted(restriction) => restriction.object(),
        }
    }

    /// The tabular section the decision addresses, if any.
    #[must_use]
    pub fn table_part_name(&self) -> Option<&str> {
        match self {
            Self::Unrestricted(target) | Self::Denied(target) => target.table_part.as_deref(),
            Self::Restricted(restriction) => restriction.table_part_name(),
        }
    }

    /// The condition, when the decision carries one.
    #[must_use]
    pub const fn restriction(&self) -> Option<&AccessRestriction> {
        match self {
            Self::Restricted(restriction) => Some(restriction),
            Self::Unrestricted(_) | Self::Denied(_) => None,
        }
    }

    /// Whether the decision refuses the target.
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        matches!(self, Self::Denied(_))
    }

    /// Whether the decision addresses `target`; section names compare
    /// case-insensitively, like every 1C identifier.
    #[must_use]
    pub fn matches(&self, target: &RestrictionTarget) -> bool {
        target.object == self.object()
            && match (&target.table_part, self.table_part_name()) {
                (None, None) => true,
                (Some(wanted), Some(supplied)) => names_equal(wanted, supplied),
                _ => false,
            }
    }
}
