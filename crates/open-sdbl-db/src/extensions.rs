//! Roles of the configuration extensions.
//!
//! An extension keeps its resources in `ConfigCAS`, a content-addressed
//! store: the row of a resource is named by the hexadecimal of the key of
//! what it holds. `_ExtensionsInfo` carries the key of the root resource
//! of every extension, and that root lists every resource of the
//! extension by name — `<guid>`, `<guid>.0` — with the key of its
//! content. The rights resource of a role is the same format `Config`
//! holds, so this module reads it the same way.

use open_sdbl::metadata::{
    ConfigDescriptor, ContentKey, ExtensionResource, Guid, MsSqlMetadataQueries,
    PostgresMetadataQueries, RoleRights, StorageLayout, Value, extension_root_key,
    inflate_raw_deflate, parse_config_descriptors, parse_extension_index, parse_role_rights,
    parse_serialized,
};

use crate::cells::Cell;
use crate::error::DbError;
use crate::session::{DatabaseDialect, DatabaseSession};

#[cfg(test)]
#[path = "tests/extensions.rs"]
mod tests;

/// The resources every extension of the base declares, read once.
#[derive(Debug, Default)]
pub struct ExtensionIndex {
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
    /// Whether the base declares no extension resource at all.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

/// Reads the resource index of every extension of the base.
///
/// A base without `_ExtensionsInfo` — an older platform — answers an
/// empty index.
pub async fn read_extension_index(
    session: &mut DatabaseSession,
    layout: StorageLayout,
) -> Result<ExtensionIndex, DbError> {
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
    // The statement answers identity, order, name and the info record.
    let rows = session.query(extensions, 4).await?;
    let mut resources = Vec::new();
    for row in &rows {
        let Some(Cell::Bytes(info)) = row.get(3) else {
            continue;
        };
        let Some(root) = extension_root_key(info) else {
            continue;
        };
        let Some(bytes) = read_resource(session, layout, &root).await? else {
            continue;
        };
        let name = match row.get(2) {
            Some(Cell::Text(name)) => name.trim().to_owned(),
            _ => root.as_hex(),
        };
        // The index is read from the text of the root, the rights and the
        // descriptors from their resources as they are stored.
        let root = inflate_raw_deflate(&bytes)
            .map_err(|error| DbError::Data(format!("extension {name:?}: {error}")))?;
        resources.extend(
            parse_extension_index(&root)
                .map_err(|error| DbError::Data(format!("extension {name:?}: {error}")))?,
        );
    }
    Ok(ExtensionIndex { resources })
}

/// Reads the rights of the roles the extensions declare, with the
/// descriptors naming them.
pub async fn read_extension_roles(
    session: &mut DatabaseSession,
    layout: StorageLayout,
    index: &ExtensionIndex,
    roles: &[Guid],
) -> Result<(Vec<(Guid, RoleRights)>, Vec<ConfigDescriptor>), DbError> {
    let mut rights = Vec::new();
    let mut descriptors = Vec::new();
    for role in roles {
        let Some(key) = index.key(&format!("{role}.0")) else {
            // Измеренная роль может явно не иметь подобъекта прав. Само
            // отсутствие `.0` этого не доказывает: нужен полный дескриптор.
            if let Some(key) = index.key(role.as_str())
                && let Some(bytes) = read_resource(session, layout, &key).await?
                && let Some(empty) = decode_role_without_rights(role, &bytes)
            {
                rights.push((role.clone(), empty));
                if let Ok(named) = parse_config_descriptors(role.as_str(), &bytes) {
                    descriptors.extend(named);
                }
            }
            continue;
        };
        let Some(bytes) = read_resource(session, layout, &key).await? else {
            continue;
        };
        rights.push((
            role.clone(),
            parse_role_rights(&bytes).map_err(|error| {
                DbError::Data(format!("rights of the extension role {role}: {error}"))
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

// Отсутствие подобъекта прав измерено переносом неизменённого дескриптора
// `ДемоПроф_ОбщиеПрава` в собственный CF. Нативная выгрузка не создаёт
// `Rights.xml`; явно пустые права имеют другой признак и отдельный `.0`.
// Заимствованные роли и остальные формы здесь не распознаются.
fn decode_role_without_rights(role: &Guid, bytes: &[u8]) -> Option<RoleRights> {
    let decoded = inflate_raw_deflate(bytes).ok()?;
    let value = parse_serialized(&decoded).ok()?;
    if !role_descriptor_has_no_rights(role, &value) {
        return None;
    }
    // `RoleRights` создаёт существующий декодер. Эти неизменённые байты
    // нативного пустого `.0` не содержат объектов, шаблонов или разрешающих
    // флагов; отдельная фикстура сверяет их результат с измеренной ролью.
    parse_role_rights(&[
        0x7b, 0xbf, 0x7b, 0x7f, 0xb5, 0xa1, 0x81, 0x0e, 0x2f, 0x57, 0xb5, 0x41, 0x2d, 0x94, 0x34,
        0x31, 0xb2, 0x34, 0xb1, 0x34, 0x33, 0x37, 0xb2, 0x34, 0x45, 0x66, 0x1a, 0x20, 0x71, 0x6a,
        0x01,
    ])
    .ok()
}

fn role_descriptor_has_no_rights(role: &Guid, value: &Value) -> bool {
    let Value::List(root) = value else {
        return false;
    };
    let [
        Value::Atom(version),
        Value::List(class),
        Value::Atom(root_tail),
    ] = root.as_slice()
    else {
        return false;
    };
    let [
        Value::Atom(kind),
        Value::List(header),
        Value::Atom(rights_present),
        Value::Atom(class_flag),
        Value::Atom(class_tail),
    ] = class.as_slice()
    else {
        return false;
    };
    let [
        Value::Atom(header_version),
        Value::List(identity),
        Value::String(name),
        Value::List(synonym),
        Value::String(_comment),
        Value::Atom(header_flag),
        Value::Atom(header_mode),
        Value::Atom(owner),
        Value::Atom(header_tail),
    ] = header.as_slice()
    else {
        return false;
    };
    let [
        Value::Atom(own),
        Value::Atom(identity_flag),
        Value::Atom(identifier),
    ] = identity.as_slice()
    else {
        return false;
    };
    let [
        Value::Atom(synonym_count),
        Value::String(language),
        Value::String(_text),
    ] = synonym.as_slice()
    else {
        return false;
    };
    version == "1"
        && root_tail == "0"
        && kind == "6"
        && rights_present == "0"
        && class_flag == "0"
        && class_tail == "0"
        && header_version == "3"
        && own == "1"
        && identity_flag == "0"
        && !role.is_nil()
        && identifier.parse::<Guid>().ok().as_ref() == Some(role)
        && !name.is_empty()
        && synonym_count == "1"
        && !language.is_empty()
        && header_flag == "0"
        && header_mode == "0"
        && owner == Guid::NIL
        && header_tail == "0"
}

/// Reads the parts of one resource of the extension store, as they are
/// stored; `None` when the store carries no such row.
async fn read_resource(
    session: &mut DatabaseSession,
    layout: StorageLayout,
    key: &ContentKey,
) -> Result<Option<Vec<u8>>, DbError> {
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
                return Err(DbError::Data(format!(
                    "extension resource {} is not bytes: {other:?}",
                    key.as_hex()
                )));
            }
        }
    }
    Ok((!data.is_empty()).then_some(data))
}

/// Whether a probe statement answers one.
async fn answers_yes(session: &mut DatabaseSession, probe: &str) -> Result<bool, DbError> {
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
