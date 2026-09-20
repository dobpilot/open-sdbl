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
    AccessDecision, AccessRestriction, ColumnKind, ColumnOrigin, CompileOptions, CompiledColumn,
    CompiledQuery, FieldUsage, FieldUsageRequest, FieldUse, InvalidParameterDate, ParameterColumn,
    ParameterDate, ParameterValue, PresentationExpression, PresentationPlan, PresentationRequest,
    PresentationTarget, QueryDiagnostic, QueryDiagnosticKind, QueryParameter, QueryableColumn,
    QueryableField, QueryableFieldCatalog, RestrictionMode, RestrictionRequest, RestrictionTarget,
    SessionParameters, TempTable, TempTablesManager, TypeValue, constants_table_fields,
    find_metadata_object, object_query_name, queryable_field_catalog, queryable_fields,
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
        core::compile_query(
            source,
            self.snapshot,
            options,
            self.backend.dialect(),
            RestrictionMode::Statement,
        )
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
            RestrictionMode::Statement,
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
        self.prepare_with_options(source, &PrepareOptions::new())
    }

    /// Resolves a query under explicit preparation options.
    ///
    /// The restriction mode is fixed here, not at compile time: a query
    /// prepared in [`RestrictionMode::Restricted`] stays restricted for
    /// every compilation of the resulting [`Prepared`].
    ///
    /// # Errors
    ///
    /// Returns a positional diagnostic when the query cannot be safely
    /// resolved.
    pub fn prepare_with_options(
        &self,
        source: &str,
        options: &PrepareOptions<'_>,
    ) -> Result<Prepared<B>, QueryDiagnostic> {
        let requests = core::prepare_query_with(
            source,
            self.snapshot,
            self.backend.dialect(),
            options.temporary_tables_in_effect(),
            options.restriction_mode_in_effect(),
        )?;
        Ok(Prepared {
            source: source.to_owned(),
            backend: self.backend,
            request: requests.presentations,
            restrictions: requests.restrictions,
            field_usage: requests.field_usage,
            mode: options.restriction_mode_in_effect(),
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
        self.prepare_with_options(source, &PrepareOptions::new().temporary_tables(manager))
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
    /// The target table is read **unfiltered**: this entry point takes no
    /// access decisions. An application under
    /// [`RestrictionMode::Restricted`] resolves its deferred
    /// presentations with [`Prepared::compile_presentation_lookup`],
    /// which carries the mode and the decisions of the query the
    /// references came from.
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
        core::compile_presentation_lookup(
            self.snapshot,
            plan,
            references,
            self.backend.dialect(),
            &CompileOptions::new(),
            RestrictionMode::Statement,
        )
    }
}

/// Inputs a preparation may need beyond the source text.
///
/// ```
/// use open_sdbl::query::{PrepareOptions, RestrictionMode};
///
/// let options = PrepareOptions::new().restricted();
/// assert_eq!(options.restriction_mode_in_effect(), RestrictionMode::Restricted);
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PrepareOptions<'a> {
    temporary: &'a TempTablesManager,
    mode: RestrictionMode,
}

impl Default for PrepareOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> PrepareOptions<'a> {
    /// Options with no temporary tables visible, in the default mode.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            temporary: TempTablesManager::none(),
            mode: RestrictionMode::Statement,
        }
    }

    /// Makes the tables of `manager` visible to the batch.
    #[must_use]
    pub const fn temporary_tables(mut self, manager: &'a TempTablesManager) -> Self {
        self.temporary = manager;
        self
    }

    /// Selects the mode every compilation of the prepared query applies.
    #[must_use]
    pub const fn restriction_mode(mut self, mode: RestrictionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Prepares in [`RestrictionMode::Restricted`]: every statement is
    /// filtered and every target of the request needs a decision.
    #[must_use]
    pub const fn restricted(self) -> Self {
        self.restriction_mode(RestrictionMode::Restricted)
    }

    /// The temporary tables in effect.
    #[must_use]
    pub const fn temporary_tables_in_effect(&self) -> &'a TempTablesManager {
        self.temporary
    }

    /// The mode in effect.
    #[must_use]
    pub const fn restriction_mode_in_effect(&self) -> RestrictionMode {
        self.mode
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
    field_usage: FieldUsageRequest,
    /// Fixed at preparation; no compile-time value can lower it.
    mode: RestrictionMode,
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

    /// Which fields the batch reads, and in what role.
    ///
    /// Hiding a value in the result hides nothing on its own: a statement
    /// filtering on an attribute learns it from which rows come back, and
    /// ordering, grouping and aggregating leak it the same way. This says
    /// where each field is read, so an application can refuse the roles it
    /// must refuse.
    #[must_use]
    pub const fn field_usage(&self) -> &FieldUsageRequest {
        &self.field_usage
    }

    /// The mode the query was prepared in. Compilation applies this
    /// value; nothing passed to `compile_with` can lower it.
    #[must_use]
    pub const fn restriction_mode(&self) -> RestrictionMode {
        self.mode
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
        core::compile_query(
            &self.source,
            snapshot,
            options,
            self.backend.dialect(),
            self.mode,
        )
    }

    /// Compiles the lookup that resolves this query's deferred reference
    /// presentations, under this query's restriction mode.
    ///
    /// The references come from rows the query already allowed; the
    /// decision this lookup needs is about the *target* of those
    /// references. A target the decision excludes matches no row, so the
    /// application presents nothing for it — the answer a deleted object
    /// already gives — rather than seeing a value it may not read.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic for an invalid plan or reference batch, for a
    /// snapshot other than the one the query was prepared against, and,
    /// in [`RestrictionMode::Restricted`], for a target with no decision.
    pub fn compile_presentation_lookup(
        &self,
        snapshot: &MetadataSnapshot,
        plan: &PresentationPlan,
        references: &[[u8; 16]],
        options: &CompileOptions<'_>,
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        if snapshot.fingerprint() != self.snapshot_fingerprint {
            return Err(QueryDiagnostic::snapshot_mismatch());
        }
        core::compile_presentation_lookup(
            snapshot,
            plan,
            references,
            self.backend.dialect(),
            options,
            self.mode,
        )
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
            self.mode,
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
