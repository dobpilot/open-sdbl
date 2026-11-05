//! Deferred reference presentations: asking the application for a plan
//! and resolving the payloads a result carries.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use open_sdbl::metadata::{MetadataSnapshot, ObjectId};
use open_sdbl::query::{
    CompiledQuery, PostgresBackend, PresentationPlan, PresentationRequest, QueryCompiler,
};
pub(super) use open_sdbl_db::presentation::default_presentation_plan;
#[cfg(test)]
pub(super) use open_sdbl_db::presentation::default_presentation_template;
use open_sdbl_db::{Cell, DatabaseDialect, DatabaseSession, DbError, QueryRows};

use crate::error::CliError;

use super::*;

#[cfg(test)]
#[path = "../tests/repl_presentation.rs"]
mod tests;

pub(super) fn ensure_session_remains_usable(dead: bool) -> Result<(), CliError> {
    if dead {
        Err(CliError::Db(DbError::Database(
            "database session is no longer usable; reconnect required".to_owned(),
        )))
    } else {
        Ok(())
    }
}

pub(super) fn presentation_plans(
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    snapshot: &MetadataSnapshot,
    request: &PresentationRequest,
) -> Vec<PresentationPlan> {
    let mut plans = Vec::with_capacity(request.targets.len());
    for target in &request.targets {
        plans.push(presentation_plan(cache, snapshot, target.object));
    }
    plans
}

pub(super) fn presentation_plan(
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    snapshot: &MetadataSnapshot,
    object: ObjectId,
) -> PresentationPlan {
    let key = PresentationPlanKey {
        object,
        language: "ru",
        policy_version: PRESENTATION_POLICY_VERSION,
    };
    cache
        .entry(key)
        .or_insert_with(|| default_presentation_plan(snapshot, object))
        .clone()
}

pub(super) async fn resolve_deferred_presentations(
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    cache: &mut HashMap<PresentationPlanKey, PresentationPlan>,
    compiled: &CompiledQuery,
    rows: &mut QueryRows,
) -> Result<(), CliError> {
    if compiled.deferred_presentations.is_empty() || rows.is_empty() {
        return Ok(());
    }

    let mut cells = Vec::new();
    let mut references = BTreeMap::<ObjectId, BTreeSet<[u8; 16]>>::new();
    for (row_index, row) in rows.iter_mut().enumerate() {
        for &column_index in &compiled.deferred_presentations {
            let cell = row.get_mut(column_index).ok_or_else(|| {
                CliError::Data(format!(
                    "database row has no deferred presentation column {column_index}"
                ))
            })?;
            // A row that carries no reference has no presentation, and the
            // platform prints nothing for it; only a reference that names
            // no object is unresolved.
            if cell.is_null() {
                continue;
            }
            let payload = cell.as_bytes().ok_or_else(|| {
                CliError::Data(
                    "database returned a non-binary deferred presentation payload".to_owned(),
                )
            })?;
            let Some((object, reference)) = decode_deferred_reference(payload, snapshot)? else {
                *cell = Cell::Text(String::new());
                continue;
            };
            references.entry(object).or_default().insert(reference);
            cells.push((row_index, column_index, object, reference));
        }
    }

    let dialect = session.dialect();
    let mut presentations = HashMap::<(ObjectId, [u8; 16]), String>::new();
    for (object, object_references) in references {
        let plan = presentation_plan(cache, snapshot, object);
        let object_references = object_references.into_iter().collect::<Vec<_>>();
        for chunk in object_references.chunks(512) {
            let lookup = match dialect {
                DatabaseDialect::Postgres => QueryCompiler::new(snapshot, PostgresBackend)
                    .compile_presentation_lookup(&plan, chunk),
                DatabaseDialect::MsSql { backend } => {
                    QueryCompiler::new(snapshot, backend).compile_presentation_lookup(&plan, chunk)
                }
            }
            .map_err(|error| {
                CliError::Data(format!(
                    "cannot compile deferred presentation lookup: {error}"
                ))
            })?;
            let lookup_rows = session.query(&lookup.sql, lookup.columns.len()).await?;
            for row in lookup_rows {
                let key = row.first().and_then(Cell::as_bytes).ok_or_else(|| {
                    CliError::Data(
                        "presentation lookup returned a row without a reference".to_owned(),
                    )
                })?;
                let reference = <[u8; 16]>::try_from(key).map_err(|_| {
                    CliError::Data(format!(
                        "presentation lookup reference has {} bytes, expected 16",
                        key.len()
                    ))
                })?;
                let presentation = row
                    .get(1)
                    .and_then(Cell::as_text)
                    .map(str::to_owned)
                    .unwrap_or_default();
                presentations.insert((object, reference), presentation);
            }
        }
    }

    for (row_index, column_index, object, reference) in cells {
        let presentation = resolved_presentation(&presentations, object, reference);
        rows[row_index][column_index] = Cell::Text(presentation);
    }
    Ok(())
}

pub(super) fn resolved_presentation(
    presentations: &HashMap<(ObjectId, [u8; 16]), String>,
    object: ObjectId,
    reference: [u8; 16],
) -> String {
    presentations
        .get(&(object, reference))
        .cloned()
        .unwrap_or_else(|| UNRESOLVED_REFERENCE.to_owned())
}

/// Splits the 20-byte `RTRef ‖ RRRef` payload into the big-endian table
/// number and the reference; an all-zero reference is the empty 1C reference.
pub(super) fn split_deferred_payload(payload: &[u8]) -> Result<Option<(u32, [u8; 16])>, CliError> {
    let (database_type, reference) = match (
        payload
            .get(..4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok()),
        payload
            .get(4..)
            .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok()),
    ) {
        (Some(database_type), Some(reference)) => (database_type, reference),
        _ => {
            return Err(CliError::Data(format!(
                "deferred presentation payload has {} bytes, expected 20",
                payload.len()
            )));
        }
    };
    if reference.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    Ok(Some((u32::from_be_bytes(database_type), reference)))
}

pub(super) fn decode_deferred_reference(
    payload: &[u8],
    snapshot: &MetadataSnapshot,
) -> Result<Option<(ObjectId, [u8; 16])>, CliError> {
    let Some((database_type, reference)) = split_deferred_payload(payload)? else {
        return Ok(None);
    };
    let object = snapshot
        .object_id_by_database_type(database_type)
        .map_err(|error| {
            CliError::Data(format!(
                "runtime reference type {database_type} is absent from metadata: {error}"
            ))
        })?;
    Ok(Some((object, reference)))
}
