//! Bounded compilation of 1C SELECT queries through resolved metadata.

mod ast;
mod codegen;
mod diag;
mod dialect;
mod names;
mod params;
mod parser;
mod resolve;
mod restrict;
mod temp_tables;
mod types;

pub use diag::{QueryDiagnostic, QueryDiagnosticKind};
pub use params::{
    CompileOptions, InvalidParameterDate, ParameterColumn, ParameterDate, ParameterValue,
    QueryParameter, SessionParameters,
};
pub use resolve::{
    ColumnKind, CompiledColumn, CompiledQuery, PresentationExpression, PresentationPlan,
    PresentationRequest, PresentationTarget, QueryableColumn, QueryableField,
    QueryableFieldCatalog, constants_table_fields, find_metadata_object, object_query_name,
    queryable_field_catalog, queryable_fields,
};
pub use restrict::{AccessRestriction, RestrictionRequest, RestrictionTarget};
pub use temp_tables::{TempTable, TempTablesManager};
pub use types::TypeValue;

pub(super) use codegen::{
    compile_batch, compile_presentation_lookup, compile_query, prepare_query, prepare_query_with,
};
pub(super) use dialect::SqlDialect;
