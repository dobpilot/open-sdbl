//! Dependency-free decoding and resolution of 1C metadata resources.
//!
//! Resolution returns [`crate::metadata::ResolvedMetadata`], which keeps the
//! query-ready [`crate::metadata::MetadataSnapshot`] together with a structured
//! [`crate::metadata::ResolutionReport`]. The report describes recoverable
//! source mismatches; decoding and validation failures use
//! [`crate::metadata::MetadataErrorKind`] and document their positional unit
//! through [`crate::metadata::MetadataError::offset_unit`].

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
    ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue, ParsedConfigResource, Synonym,
    parse_config_descriptor, parse_config_descriptors, parse_config_predefined_values,
    parse_config_resource_bounded,
};
pub use db_names::{DbNameEntry, DbNames, MetadataKind, parse_db_names};
pub use deflate::{DEFAULT_OUTPUT_LIMIT, inflate_raw_deflate, inflate_raw_deflate_bounded};
pub use guid::Guid;
pub use identity::{AttributeId, FieldId, LookupError, ObjectId, StandardFieldId};
pub(crate) use normalize::normalize_standard_field_name;
pub use normalize::{
    LogicalField, collapse_logical_fields, normalize_index_key, recase_postgres_identifier,
};
pub use queries::{MsSqlMetadataQueries, PostgresMetadataQueries};
pub(crate) use resolve::SnapshotFingerprint;
pub use resolve::{
    AllowedLength, IndexComparison, LiveColumn, LiveIndex, LiveTable, MetadataField,
    MetadataObject, MetadataSnapshot, MetadataValue, ResolutionFinding, ResolutionReport,
    ResolvedMetadata, resolve_metadata, resolve_metadata_with_predefined_values,
};
pub use schema::{
    ColumnType, SchemaAnomaly, SchemaColumn, SchemaIndex, SchemaStorage, SchemaTable,
    parse_schema_storage,
};
pub use value::{Value, parse_serialized};

use std::fmt;

const SERIALIZED_NESTING_LIMIT: usize = 512;

fn ensure_serialized_depth(depth: usize, offset: usize) -> Result<(), MetadataError> {
    if depth >= SERIALIZED_NESTING_LIMIT {
        Err(MetadataError::serialization(
            offset,
            format!("metadata nesting depth exceeds limit of {SERIALIZED_NESTING_LIMIT}"),
        ))
    } else {
        Ok(())
    }
}

/// An error raised while decoding or resolving platform metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataError {
    kind: MetadataErrorKind,
    message: String,
    offset: Option<usize>,
}

/// Machine-readable category of a metadata decoding or validation failure.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataErrorKind {
    /// Raw-DEFLATE decoding failed; offsets are measured in bits.
    Deflate,
    /// Decoded metadata is not valid UTF-8; offsets are measured in bytes.
    Utf8,
    /// Brace-serialized metadata parsing failed; offsets are measured in bytes.
    Serialization,
    /// A decoded GUID is malformed; no source offset is available.
    Guid,
    /// A metadata resource has no recognized structural declaration.
    Schema,
    /// Resolved metadata is internally inconsistent.
    Resolution,
}

/// Unit used by a positional [`MetadataError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataOffsetUnit {
    /// A zero-based byte offset in decoded metadata.
    Byte,
    /// A zero-based bit offset in a raw-DEFLATE stream.
    Bit,
}

impl MetadataError {
    pub(crate) fn new(kind: MetadataErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: None,
        }
    }

    pub(crate) fn at(kind: MetadataErrorKind, offset: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: Some(offset),
        }
    }

    pub(crate) fn deflate(offset: usize, message: impl Into<String>) -> Self {
        Self::at(MetadataErrorKind::Deflate, offset, message)
    }

    pub(crate) fn serialization(offset: usize, message: impl Into<String>) -> Self {
        Self::at(MetadataErrorKind::Serialization, offset, message)
    }

    pub(crate) fn utf8(offset: usize, message: impl Into<String>) -> Self {
        Self::at(MetadataErrorKind::Utf8, offset, message)
    }

    /// Returns the machine-readable error category.
    #[must_use]
    pub const fn kind(&self) -> MetadataErrorKind {
        self.kind
    }

    /// Returns the unit of [`Self::offset`], when this category is positional.
    #[must_use]
    pub const fn offset_unit(&self) -> Option<MetadataOffsetUnit> {
        match self.kind {
            MetadataErrorKind::Deflate => Some(MetadataOffsetUnit::Bit),
            MetadataErrorKind::Utf8 | MetadataErrorKind::Serialization => {
                Some(MetadataOffsetUnit::Byte)
            }
            MetadataErrorKind::Guid | MetadataErrorKind::Schema | MetadataErrorKind::Resolution => {
                None
            }
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

#[cfg(test)]
mod tests {
    use super::{
        MetadataErrorKind, MetadataOffsetUnit, inflate_raw_deflate_bounded, parse_serialized,
    };

    #[test]
    fn categorizes_metadata_offsets_by_decoder_domain() {
        let deflate = inflate_raw_deflate_bounded(&[0xff], 1024).unwrap_err();
        assert_eq!(deflate.kind(), MetadataErrorKind::Deflate);
        assert_eq!(deflate.offset_unit(), Some(MetadataOffsetUnit::Bit));
        assert!(deflate.offset().is_some());

        let serialized = parse_serialized(b"{").unwrap_err();
        assert_eq!(serialized.kind(), MetadataErrorKind::Serialization);
        assert_eq!(serialized.offset_unit(), Some(MetadataOffsetUnit::Byte));
        assert!(serialized.offset().is_some());
    }
}
