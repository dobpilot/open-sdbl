//! Preparing a statement: compiling it, asking for what the compiler
//! needs, and handing it back ready to execute.

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    AccessRestriction, CompileOptions, CompiledQuery, MsSqlBackend, PostgresBackend, Prepared,
    PresentationPlan, PresentationRequest, QueryParameter, RestrictionRequest, SessionParameters,
    TempTablesManager,
};

pub(super) enum PreparedQuery {
    Postgres(Prepared<PostgresBackend>),
    MsSql(Prepared<MsSqlBackend>),
}

impl PreparedQuery {
    pub(super) fn presentation_request(&self) -> &PresentationRequest {
        match self {
            Self::Postgres(query) => query.presentation_request(),
            Self::MsSql(query) => query.presentation_request(),
        }
    }

    pub(super) fn restriction_request(&self) -> &RestrictionRequest {
        match self {
            Self::Postgres(query) => query.restriction_request(),
            Self::MsSql(query) => query.restriction_request(),
        }
    }

    /// Compiles the prepared batch, updating the session's temporary tables.
    /// `None` means the batch only dropped tables and has nothing to run.
    pub(super) fn compile_batch(
        self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
        parameters: &[QueryParameter],
        session: &SessionParameters,
        restrictions: &[AccessRestriction],
        temporary: &mut TempTablesManager,
    ) -> Result<Option<CompiledQuery>, open_sdbl::query::QueryDiagnostic> {
        let options = CompileOptions::new()
            .presentations(plans)
            .parameters(parameters)
            .session(session)
            .restrictions(restrictions)
            .totals_level(true);
        match self {
            Self::Postgres(query) => query.compile_batch(snapshot, &options, temporary),
            Self::MsSql(query) => query.compile_batch(snapshot, &options, temporary),
        }
    }
}
