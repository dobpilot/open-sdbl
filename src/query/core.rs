//! Bounded compilation of 1C SELECT queries through resolved metadata.

mod ast;
mod codegen;
mod common_settings;
mod diag;
mod dialect;
mod discovery;
pub use discovery::{
    QueryConstantReferences, QueryMetadataReference, QueryMetadataReferences,
    query_metadata_references,
};
mod names;
mod params;
mod parser;
mod resolve;
mod restrict;
mod temp_tables;
mod types;
mod usage;

pub(super) use crate::metadata::settings_storage::SettingsStorage;
pub use common_settings::CommonSettingsSeparation;
pub(super) use common_settings::compile as compile_settings_load;

pub use diag::{QueryDiagnostic, QueryDiagnosticKind};
pub use params::{
    BoundaryKind, CompileOptions, InvalidParameterDate, ParameterBoundary, ParameterBoundaryValue,
    ParameterColumn, ParameterDate, ParameterPointInTime, ParameterReference, ParameterValue,
    QueryParameter, SessionParameters,
};
pub use resolve::{
    ColumnKind, ColumnOrigin, CompiledColumn, CompiledQuery, CompositeResultMember,
    LogicalResultColumn, LogicalResultValue, PresentationExpression, PresentationPlan,
    PresentationRequest, PresentationTarget, QueryableColumn, QueryableField,
    QueryableFieldCatalog, constants_table_fields, find_metadata_object, object_query_name,
    queryable_field_catalog, queryable_fields,
};
pub use restrict::{
    AccessDecision, AccessRestriction, RestrictionMode, RestrictionRequest, RestrictionTarget,
};
pub use temp_tables::{TempTable, TempTablesManager};

/// Result of sequentially materializing a query batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializedBatchResult {
    /// SQL of the final selection for the host to execute.
    Query(CompiledQuery),
    /// Number of rows captured by the final placement or append.
    Placed(usize),
    /// The final statement dropped a temporary table and has no result.
    Dropped,
}
pub use types::TypeValue;
pub use usage::{FieldUsage, FieldUsageRequest, FieldUse};

pub(super) use codegen::{
    compile_batch, compile_materialized_batch, compile_presentation_lookup, compile_query,
    prepare_query_with,
};
pub(super) use dialect::SqlDialect;
