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
