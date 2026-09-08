//! Bounded compilation of 1C SELECT queries through resolved metadata.

mod ast;
mod codegen;
mod diag;
mod dialect;
mod names;
mod parser;
mod resolve;

pub use diag::{QueryDiagnostic, QueryDiagnosticKind};
pub use resolve::{
    ColumnKind, CompiledColumn, CompiledQuery, PresentationExpression, PresentationPlan,
    PresentationRequest, PresentationTarget, QueryableColumn, QueryableField,
    QueryableFieldCatalog, find_metadata_object, queryable_field_catalog, queryable_fields,
};

pub(super) use codegen::{compile_presentation_lookup, compile_query, prepare_query};
pub(super) use dialect::SqlDialect;
