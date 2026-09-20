//! Roles of the configuration extensions.
//!
//! An extension keeps its resources in `ConfigCAS`, a content-addressed
//! store: the row of a resource is named by the hexadecimal of the key of
//! what it holds. `_ExtensionsInfo` carries the key of the root resource
//! of every extension, and that root lists every resource of the
//! extension by name — `<guid>`, `<guid>.0` — with the key of its
//! content. The rights resource of a role is the same format `Config`
//! holds, so the console reads it the same way.

use open_sdbl::metadata::{
    ConfigDescriptor, ContentKey, ExtensionResource, Guid, MsSqlMetadataQueries,
    PostgresMetadataQueries, RoleRights, StorageLayout, extension_root_key, inflate_raw_deflate,
    parse_config_descriptors, parse_extension_index, parse_role_rights,
};

use crate::cells::Cell;
use crate::error::CliError;
use crate::session::{DatabaseDialect, DatabaseSession};

#[cfg(test)]
#[path = "tests/extensions.rs"]
mod tests;

/// The resources every extension of the base declares, read once.
#[derive(Debug, Default)]
pub(crate) struct ExtensionIndex {
    resources: Vec<ExtensionResource>,
}

impl ExtensionIndex {
    /// The key of the resource of a name, whichever extension carries it.
    fn key(&self, name: &str) -> Option<ContentKey> {
        self.resources
            .iter()
            .find(|resource| resource.name == name)
            .map(|resource| resource.key)
    }

    /// Whether the extensions declare nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

/// Reads the resource index of every extension of the base.
///
/// A base without `_ExtensionsInfo` — an older platform — answers an
/// empty index.
pub(crate) async fn read_extension_index(
    session: &mut DatabaseSession,
    layout: StorageLayout,
) -> Result<ExtensionIndex, CliError> {
    let (probe, extensions) = match session.dialect() {
        DatabaseDialect::Postgres => (
            PostgresMetadataQueries::EXTENSIONS_PROBE,
            PostgresMetadataQueries::EXTENSIONS,
        ),
        DatabaseDialect::MsSql { .. } => (
            MsSqlMetadataQueries::EXTENSIONS_PROBE,
            MsSqlMetadataQueries::EXTENSIONS,
        ),
    };
    if !layout.extension_store || !answers_yes(session, probe).await? {
        return Ok(ExtensionIndex::default());
    }
    let rows = session.query(extensions, 2).await?;
    let mut resources = Vec::new();
    for row in &rows {
        let Some(Cell::Bytes(info)) = row.get(1) else {
            continue;
        };
        let Some(root) = extension_root_key(info) else {
            continue;
        };
        let Some(bytes) = read_resource(session, layout, &root).await? else {
            continue;
        };
        let name = match row.first() {
            Some(Cell::Text(name)) => name.trim().to_owned(),
            _ => root.as_hex(),
        };
        // The index is read from the text of the root, the rights and the
        // descriptors from their resources as they are stored.
        let root = inflate_raw_deflate(&bytes)
            .map_err(|error| CliError::Data(format!("extension {name:?}: {error}")))?;
        resources.extend(
            parse_extension_index(&root)
                .map_err(|error| CliError::Data(format!("extension {name:?}: {error}")))?,
        );
    }
    Ok(ExtensionIndex { resources })
}

/// Reads the rights of the roles the extensions declare, with the
/// descriptors naming them.
pub(crate) async fn read_extension_roles(
    session: &mut DatabaseSession,
    layout: StorageLayout,
    index: &ExtensionIndex,
    roles: &[Guid],
) -> Result<(Vec<(Guid, RoleRights)>, Vec<ConfigDescriptor>), CliError> {
    let mut rights = Vec::new();
    let mut descriptors = Vec::new();
    for role in roles {
        let Some(key) = index.key(&format!("{role}.0")) else {
            continue;
        };
        let Some(bytes) = read_resource(session, layout, &key).await? else {
            continue;
        };
        rights.push((
            role.clone(),
            parse_role_rights(&bytes).map_err(|error| {
                CliError::Data(format!("rights of the extension role {role}: {error}"))
            })?,
        ));
        // The descriptor names the role; a role without one keeps its
        // identifier.
        if let Some(key) = index.key(role.as_str())
            && let Some(bytes) = read_resource(session, layout, &key).await?
            && let Ok(named) = parse_config_descriptors(role.as_str(), &bytes)
        {
            descriptors.extend(named);
        }
    }
    Ok((rights, descriptors))
}

/// Reads the parts of one resource of the extension store, as they are
/// stored; `None` when the store carries no such row.
async fn read_resource(
    session: &mut DatabaseSession,
    layout: StorageLayout,
    key: &ContentKey,
) -> Result<Option<Vec<u8>>, CliError> {
    let statement = match session.dialect() {
        DatabaseDialect::Postgres => PostgresMetadataQueries::extension_resource(&layout, key),
        DatabaseDialect::MsSql { .. } => MsSqlMetadataQueries::extension_resource(&layout, key),
    };
    let rows = session.query(&statement, 1).await?;
    let mut data = Vec::new();
    for row in &rows {
        match row.first() {
            Some(Cell::Bytes(bytes)) => data.extend_from_slice(bytes),
            other => {
                return Err(CliError::Data(format!(
                    "extension resource {} is not bytes: {other:?}",
                    key.as_hex()
                )));
            }
        }
    }
    Ok((!data.is_empty()).then_some(data))
}

/// Whether a probe statement answers one.
async fn answers_yes(session: &mut DatabaseSession, probe: &str) -> Result<bool, CliError> {
    Ok(session
        .query(probe, 1)
        .await?
        .first()
        .and_then(|row| row.first())
        .is_some_and(|cell| match cell {
            Cell::Number(value) => value.trim() != "0",
            Cell::Bool(value) => *value,
            _ => false,
        }))
}
