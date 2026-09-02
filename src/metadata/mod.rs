//! Dependency-free decoding and resolution of 1C metadata resources.

mod config;
mod db_names;
mod deflate;
mod guid;
mod identity;
mod normalize;
mod queries;
mod resolve;
mod schema;
mod value;

pub use config::{
    ConfigDescriptor, ConfigFieldPurpose, Synonym, parse_config_descriptor,
    parse_config_descriptors,
};
pub use db_names::{DbNameEntry, DbNames, MetadataKind, parse_db_names};
pub use deflate::{DEFAULT_OUTPUT_LIMIT, inflate_raw_deflate, inflate_raw_deflate_bounded};
pub use guid::Guid;
pub use identity::{AttributeId, FieldId, LookupError, ObjectId, StandardFieldId};
pub use normalize::{
    LogicalField, collapse_logical_fields, normalize_index_key, recase_postgres_identifier,
};
pub use queries::{MsSqlMetadataQueries, PostgresMetadataQueries};
pub use resolve::{
    AllowedLength, IndexComparison, LiveColumn, LiveIndex, LiveTable, MetadataField,
    MetadataObject, MetadataSnapshot, resolve_metadata,
};
pub use schema::{
    ColumnType, SchemaColumn, SchemaIndex, SchemaStorage, SchemaTable, parse_schema_storage,
};
pub use value::{Value, parse_serialized};

use std::fmt;

/// An error raised while decoding or resolving platform metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataError {
    message: String,
    offset: Option<usize>,
}

impl MetadataError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            offset: None,
        }
    }

    pub(crate) fn at(offset: usize, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            offset: Some(offset),
        }
    }

    /// Returns the byte or bit offset associated with the failure, if known.
    #[must_use]
    pub const fn offset(&self) -> Option<usize> {
        self.offset
    }

    /// Returns the diagnostic message without its positional prefix.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(offset) = self.offset {
            write!(formatter, "metadata offset {offset}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for MetadataError {}
