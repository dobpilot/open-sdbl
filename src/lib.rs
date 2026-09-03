#![warn(missing_docs, rustdoc::all)]

//! Dependency-free foundations for the 1C query language and metadata.
//!
//! The crate generates query text and decodes caller-provided data. It does
//! not perform process, filesystem, environment, terminal, or network I/O.
//!
//! # Metadata resolution
//!
//! [`metadata::resolve_metadata`] and
//! [`metadata::resolve_metadata_with_predefined_values`] return
//! [`metadata::ResolvedMetadata`]. Applications compile against its immutable
//! [`metadata::ResolvedMetadata::snapshot`] and inspect
//! [`metadata::ResolvedMetadata::report`] after every refresh. The structured
//! [`metadata::ResolutionFinding`] values distinguish recoverable mismatches
//! between DBNames, Config, SchemaStorage, and the observed database catalog.
//!
//! # Backend-neutral compilation
//!
//! [`query::QueryCompiler`] is generic over the sealed [`query::Backend`]
//! trait. The supplied [`query::PostgresBackend`] and [`query::MsSqlBackend`]
//! therefore share one compiler API without exposing an unaudited dialect
//! extension point. [`query::MsSqlBackend::new`] is fallible and accepts only
//! physical 1C year offsets in `0..=10_000`; use
//! [`query::MsSqlBackend::default`] for offset zero.
//!
//! # Machine-readable diagnostics
//!
//! Query failures expose [`query::QueryDiagnosticKind`] plus byte offset,
//! line, and column; wrapped lexer and metadata-lookup failures remain
//! available through [`std::error::Error::source`]. Metadata decoding failures
//! expose [`metadata::MetadataErrorKind`] and, when positional,
//! [`metadata::MetadataOffsetUnit`]. These public enums are non-exhaustive, so
//! callers must retain a fallback match arm.

mod lexer;

#[cfg(test)]
#[path = "../tests/support/hex.rs"]
mod hex_test_support;

pub use lexer::*;

/// Reading and resolving metadata stored by the 1C platform.
pub mod metadata;
/// Bounded query parsing and database SQL generation through resolved metadata.
pub mod query;
