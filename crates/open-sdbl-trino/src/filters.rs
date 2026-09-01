use serde::{Deserialize, Serialize};

/// A Trino column domain that the Java connector has accepted for pushdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FilterDomain {
    pub column: String,
    /// Whether the value set contains every non-null value.
    #[serde(default)]
    pub all: bool,
    /// Whether SQL NULL belongs to the domain.
    #[serde(default)]
    pub null_allowed: bool,
    /// Union of ordered ranges. An empty list with `all = false` is no values.
    #[serde(default)]
    pub ranges: Vec<FilterRange>,
}

/// One continuous range. Missing endpoints are unbounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FilterRange {
    pub low: Option<FilterBound>,
    pub high: Option<FilterBound>,
}

/// A text-encoded typed endpoint; column metadata determines its type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FilterBound {
    pub value: String,
    pub inclusive: bool,
}
