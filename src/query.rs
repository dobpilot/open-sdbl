//! Functional facade for compiling bounded 1C queries into database SQL.
//!
//! [`crate::query::QueryCompiler`] combines an immutable metadata snapshot with
//! a backend value. PostgreSQL and MSSQL specialize the same generic compiler
//! without dynamic dispatch, hidden state, or database I/O.
//! [`crate::query::MsSqlBackend::new`] validates the physical 1C year offset
//! before a compiler can be constructed. Failures are classified by
//! [`crate::query::QueryDiagnosticKind`], so callers do not need to parse
//! diagnostic messages.
//!
//! The v2 API intentionally has no database-named free-function entry points:
//!
//! ```compile_fail
//! use open_sdbl::query::compile_postgres_query;
//! ```

mod core;
mod mssql;
mod postgres;

use crate::metadata::{MetadataSnapshot, SnapshotFingerprint};

pub use core::{
    AccessRestriction, ColumnKind, CompileOptions, CompiledColumn, CompiledQuery,
    InvalidParameterDate, ParameterDate, ParameterValue, PresentationExpression, PresentationPlan,
    PresentationRequest, PresentationTarget, QueryDiagnostic, QueryDiagnosticKind, QueryParameter,
    QueryableColumn, QueryableField, QueryableFieldCatalog, RestrictionRequest, RestrictionTarget,
    SessionParameters, TempTable, TempTablesManager, find_metadata_object, queryable_field_catalog,
    queryable_fields,
};
pub use mssql::{InvalidMsSqlYearOffset, MsSqlBackend, MsSqlDialectLevel};
pub use postgres::PostgresBackend;

mod sealed {
    use super::core::SqlDialect;

    pub trait Sealed: Copy {
        fn dialect(self) -> SqlDialect;
    }
}

/// Supported database backend for query compilation.
///
/// This trait is sealed: only backends supplied by `open-sdbl` can implement
/// it, which keeps SQL generation constrained to audited dialects.
///
/// ```compile_fail
/// #[derive(Clone, Copy)]
/// struct CustomBackend;
///
/// impl open_sdbl::query::Backend for CustomBackend {}
/// ```
pub trait Backend: sealed::Sealed + Copy {}

impl<T> Backend for T where T: sealed::Sealed + Copy {}

/// Pure SQL query compiler parameterized by an immutable backend value.
#[derive(Debug, Clone, Copy)]
pub struct QueryCompiler<'snapshot, Backend> {
    pub(super) snapshot: &'snapshot MetadataSnapshot,
    pub(super) backend: Backend,
}

impl<B: Backend> QueryCompiler<'_, B> {
    /// Compiles a bounded 1C SELECT query for this backend.
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when parsing, metadata resolution, or
    /// SQL generation fails.
    pub fn compile(&self, source: &str) -> Result<CompiledQuery, QueryDiagnostic> {
        self.compile_with(source, &CompileOptions::new())
    }

    /// Compiles a query with presentation plans and named parameter values.
    ///
    /// Parameters are inlined as typed literals; every `&Имя` in the source
    /// must have a value and every supplied value must be referenced.
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when parsing, metadata resolution,
    /// parameter binding, or SQL generation fails.
    pub fn compile_with(
        &self,
        source: &str,
        options: &CompileOptions<'_>,
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        core::compile_query(source, self.snapshot, options, self.backend.dialect())
    }

    /// Compiles a batch of `;`-separated statements, updating `manager`.
    ///
    /// Temporary tables placed by earlier statements or by earlier batches
    /// are emulated with common table expressions. `Ok(None)` means the
    /// batch ends with `УНИЧТОЖИТЬ` and produces no rows, exactly as
    /// `Запрос.Выполнить()` yields `Неопределено` there. The manager is
    /// updated only when the whole batch compiles.
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when parsing, metadata resolution,
    /// parameter binding, temporary-table use, or SQL generation fails.
    pub fn compile_batch(
        &self,
        source: &str,
        options: &CompileOptions<'_>,
        manager: &mut TempTablesManager,
    ) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
        core::compile_batch(
            source,
            self.snapshot,
            options,
            self.backend.dialect(),
            manager,
        )
    }

    /// Resolves a query and collects its presentation and restriction
    /// requests.
    ///
    /// [`Prepared::compile`] recompiles the source after the application has
    /// supplied presentation plans because the parser AST borrows its tokens.
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when the query cannot be safely
    /// resolved.
    pub fn prepare(&self, source: &str) -> Result<Prepared<B>, QueryDiagnostic> {
        let requests = core::prepare_query(source, self.snapshot, self.backend.dialect())?;
        Ok(Prepared {
            source: source.to_owned(),
            backend: self.backend,
            request: requests.presentations,
            restrictions: requests.restrictions,
            snapshot_fingerprint: self.snapshot.fingerprint(),
        })
    }

    /// Resolves a batch with the manager's temporary tables visible.
    ///
    /// The manager is not modified: definitions of the batch are applied to
    /// a private copy so that the request can be collected before the
    /// application supplies presentation plans.
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when the batch cannot be safely
    /// resolved.
    pub fn prepare_with(
        &self,
        source: &str,
        manager: &TempTablesManager,
    ) -> Result<Prepared<B>, QueryDiagnostic> {
        let requests =
            core::prepare_query_with(source, self.snapshot, self.backend.dialect(), manager)?;
        Ok(Prepared {
            source: source.to_owned(),
            backend: self.backend,
            request: requests.presentations,
            restrictions: requests.restrictions,
            snapshot_fingerprint: self.snapshot.fingerprint(),
        })
    }

    /// Compiles a query with validated application presentation plans.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the query or a plan is invalid.
    pub fn compile_with_presentations(
        &self,
        source: &str,
        plans: &[PresentationPlan],
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        self.compile_with(source, &CompileOptions::new().presentations(plans))
    }

    /// Compiles a safe batch lookup for deferred reference presentations.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the plan, target, or reference batch is
    /// invalid.
    pub fn compile_presentation_lookup(
        &self,
        plan: &PresentationPlan,
        references: &[[u8; 16]],
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        core::compile_presentation_lookup(self.snapshot, plan, references, self.backend.dialect())
    }
}

/// Query resolved far enough to request application presentation plans
/// and access restrictions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared<B: Backend> {
    source: String,
    backend: B,
    request: PresentationRequest,
    restrictions: RestrictionRequest,
    snapshot_fingerprint: SnapshotFingerprint,
}

impl<B: Backend> Prepared<B> {
    /// Returns the batch callback request. It is empty when the query uses no
    /// reference presentation.
    #[must_use]
    pub fn presentation_request(&self) -> &PresentationRequest {
        &self.request
    }

    /// Returns the tables that `РАЗРЕШЕННЫЕ` statements of the batch read,
    /// so that the application can answer with
    /// [`CompileOptions::restrictions`]. It is empty when no statement
    /// carries the keyword.
    #[must_use]
    pub fn restriction_request(&self) -> &RestrictionRequest {
        &self.restrictions
    }

    /// Recompiles the source with application-provided presentation plans.
    ///
    /// Preparation caches the request, not the borrowed parser AST, so this
    /// method deliberately repeats parsing and metadata resolution.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic for an invalid query or presentation plan.
    pub fn compile(
        &self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        self.compile_with(snapshot, &CompileOptions::new().presentations(plans))
    }

    /// Recompiles the source with presentation plans and parameter values.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic for an invalid query, plan, parameter binding,
    /// or a snapshot other than the one the query was prepared against.
    pub fn compile_with(
        &self,
        snapshot: &MetadataSnapshot,
        options: &CompileOptions<'_>,
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        if snapshot.fingerprint() != self.snapshot_fingerprint {
            return Err(QueryDiagnostic::snapshot_mismatch());
        }
        core::compile_query(&self.source, snapshot, options, self.backend.dialect())
    }

    /// Recompiles the prepared batch, updating `manager`.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic for an invalid batch, plan, parameter binding,
    /// temporary-table use, or a snapshot other than the one the batch was
    /// prepared against.
    pub fn compile_batch(
        &self,
        snapshot: &MetadataSnapshot,
        options: &CompileOptions<'_>,
        manager: &mut TempTablesManager,
    ) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
        if snapshot.fingerprint() != self.snapshot_fingerprint {
            return Err(QueryDiagnostic::snapshot_mismatch());
        }
        core::compile_batch(
            &self.source,
            snapshot,
            options,
            self.backend.dialect(),
            manager,
        )
    }
}

/// Compatibility alias for a prepared PostgreSQL query.
#[deprecated(since = "0.2.0", note = "use Prepared<PostgresBackend>")]
pub type PreparedPostgresQuery = Prepared<PostgresBackend>;

/// Compatibility alias for a prepared MSSQL query.
#[deprecated(since = "0.2.0", note = "use Prepared<MsSqlBackend>")]
pub type PreparedMsSqlQuery = Prepared<MsSqlBackend>;

impl<'snapshot, Backend> QueryCompiler<'snapshot, Backend> {
    /// Binds one metadata snapshot and backend value to the compiler.
    #[must_use]
    pub const fn new(snapshot: &'snapshot MetadataSnapshot, backend: Backend) -> Self {
        Self { snapshot, backend }
    }

    /// Returns the immutable backend configuration.
    #[must_use]
    pub const fn backend(&self) -> &Backend {
        &self.backend
    }

    /// Returns the immutable metadata snapshot used for compilation.
    #[must_use]
    pub const fn snapshot(&self) -> &'snapshot MetadataSnapshot {
        self.snapshot
    }
}
