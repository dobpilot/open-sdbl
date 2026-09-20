//! Which fields a query reads, and in what role.
//!
//! Hiding a value in the result hides nothing on its own: a statement that
//! filters on an attribute learns it from which rows come back, and
//! ordering, grouping and aggregating leak it the same way. An application
//! that must refuse those needs to know not only which attributes a query
//! reads but where it reads them, which only the compiler can say.

use crate::metadata::{FieldId, ObjectId};

/// The role a field is read in.
///
/// A field read in several roles is reported in each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FieldUsage {
    /// Projected into the result on its own.
    Projection,
    /// Read by the `ГДЕ` predicate of a statement.
    Filter,
    /// Read by the `ПО` condition of a join.
    JoinCondition,
    /// Read by the `ИМЕЮЩИЕ` predicate.
    Having,
    /// Read as a `СГРУППИРОВАТЬ ПО` key.
    Grouping,
    /// Read as an `УПОРЯДОЧИТЬ ПО` key.
    Ordering,
    /// Read as the argument of an aggregate function.
    Aggregate,
    /// Read as part of a computed expression rather than projected alone.
    Expression,
}

/// One field a batch reads, in one role.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldUse {
    /// The metadata object the field belongs to; the owner of the section
    /// when the source is a tabular section.
    pub object: ObjectId,
    /// The tabular-section name, when the source is a section.
    pub table_part: Option<String>,
    /// The field.
    pub field: FieldId,
    /// What the statement does with it.
    pub usage: FieldUsage,
}

/// Every field a prepared batch reads, with its role.
///
/// One field read twice in one role appears once; read in two roles it
/// appears twice. A dereferenced field is reported against the object the
/// path ended on, so `Т.Контрагент.ИНН` names the counterparty catalog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldUsageRequest {
    /// The reads, in a stable order.
    pub fields: Vec<FieldUse>,
}
