use std::str::FromStr;

use super::config::ConfigDescriptor;
use super::db_names::{DbNameEntry, DbNames};
use super::schema::SchemaStorage;
use super::value::{Value, parse_serialized};
use super::{ExtensionMetadata, Guid, MetadataError, MetadataErrorKind, inflate_raw_deflate};

/// One extension-added attribute as recorded by the restructure resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFieldRestructure {
    /// Attribute metadata GUID.
    pub guid: Guid,
    /// Physical field number parsed from the `Fld<N>` name.
    pub number: u32,
    /// Logical attribute name (`Расш1_Реквизит1`).
    pub name: String,
    /// Declared 1C storage type such as `STRING(10) VARYING` or `NUMERIC(10)`.
    pub sql_type: String,
    /// Canonical target table for a `REF(...)` attribute.
    pub reference_target: Option<String>,
}

/// Decodes `_ExtensionsRestruct._restructData` into typed field records.
///
/// The resource is a brace-serialized value whose top level is an unwrapped
/// `<guid>,{...}` pair, optionally raw-DEFLATE compressed and optionally
/// carrying a UTF-8 byte-order mark. Each extension attribute appears as a
/// `{<guid>,"Fld<N>","<type>","<name>",...}` record anywhere in the tree.
///
/// # Errors
///
/// Returns [`MetadataError`] when the resource cannot be decoded as the
/// brace-serialized restructure format.
pub fn parse_extension_restructure(
    resource: &[u8],
) -> Result<Vec<ExtensionFieldRestructure>, MetadataError> {
    let text = decode_restructure_text(resource)?;
    let wrapped = format!("{{{}}}", text.trim_start_matches('\u{feff}'));
    let value = parse_serialized(wrapped.as_bytes())?;
    let mut fields = Vec::new();
    collect_fields(&value, &mut fields);
    Ok(fields)
}

/// Builds caller-ready [`ExtensionMetadata`] from a decoded restructure.
///
/// The returned value carries an extension `DBNames` mapping, name
/// descriptors, and a schema declaration for reference targets, so
/// `resolve_metadata_with_extensions` merges the attributes into their owning
/// object without any further application decoding.
#[must_use]
pub fn extension_metadata_from_restructure(
    origin: impl Into<String>,
    fields: Vec<ExtensionFieldRestructure>,
) -> ExtensionMetadata {
    let origin = origin.into();
    let mut entries = Vec::with_capacity(fields.len());
    let mut descriptors = Vec::with_capacity(fields.len());
    for field in &fields {
        entries.push(DbNameEntry {
            guid: field.guid.clone(),
            alias: "Fld".to_owned(),
            number: field.number,
        });
        descriptors.push(ConfigDescriptor {
            resource_guid: field.guid.clone(),
            object_guid: field.guid.clone(),
            marker: "1".to_owned(),
            name: field.name.clone(),
            synonyms: Vec::new(),
            comment: None,
            field_purpose: None,
            enumeration_value: false,
        });
    }
    ExtensionMetadata {
        origin,
        db_names: DbNames::from_entries(entries),
        descriptors,
        schema: SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
    }
}

fn decode_restructure_text(resource: &[u8]) -> Result<String, MetadataError> {
    let stripped = resource.strip_prefix(b"\xef\xbb\xbf").unwrap_or(resource);
    if stripped.first() == Some(&b'{') || stripped.iter().take(8).all(u8::is_ascii_graphic) {
        if let Ok(text) = std::str::from_utf8(stripped) {
            return Ok(text.to_owned());
        }
    }
    let inflated = inflate_raw_deflate(resource)?;
    let stripped = inflated
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&inflated)
        .to_vec();
    String::from_utf8(stripped).map_err(|_| {
        MetadataError::new(
            MetadataErrorKind::Serialization,
            "extension restructure is not valid UTF-8",
        )
    })
}

fn collect_fields(value: &Value, fields: &mut Vec<ExtensionFieldRestructure>) {
    let Value::List(items) = value else {
        return;
    };
    if let Some(field) = field_record(items) {
        fields.push(field);
    }
    for item in items {
        collect_fields(item, fields);
    }
}

fn field_record(items: &[Value]) -> Option<ExtensionFieldRestructure> {
    let guid = Guid::from_str(items.first()?.as_scalar()?).ok()?;
    let physical = items.get(1)?.as_string()?;
    let number = physical.strip_prefix("Fld")?.parse::<u32>().ok()?;
    let sql_type = items.get(2)?.as_string()?.to_owned();
    let name = items.get(3)?.as_string()?.to_owned();
    if name.is_empty() {
        return None;
    }
    Some(ExtensionFieldRestructure {
        guid,
        number,
        name,
        reference_target: reference_target(&sql_type),
        sql_type,
    })
}

fn reference_target(sql_type: &str) -> Option<String> {
    let inner = sql_type.strip_prefix("REF(")?.strip_suffix(')')?;
    (!inner.is_empty()).then(|| inner.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/service_tables/extension_attrs/restruct_reference14574.txt")
            .unwrap()
    }

    #[test]
    fn parses_the_three_extension_attributes() {
        let fields = parse_extension_restructure(&fixture()).unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].number, 16536);
        assert_eq!(fields[0].name, "Расш1_Реквизит1");
        assert!(fields[0].reference_target.is_none());
        assert_eq!(fields[1].number, 16537);
        assert_eq!(fields[1].name, "Расш1_Реквизит2");
        assert_eq!(fields[2].number, 16538);
        assert_eq!(
            fields[2].reference_target.as_deref(),
            Some("Reference16531")
        );
    }

    #[test]
    fn builds_extension_metadata_with_field_mapping() {
        let fields = parse_extension_restructure(&fixture()).unwrap();
        let metadata = extension_metadata_from_restructure("Расширение1", fields);
        assert_eq!(metadata.origin, "Расширение1");
        assert!(metadata.db_names.field_guid(16536).is_some());
        assert_eq!(metadata.descriptors.len(), 3);
        assert_eq!(metadata.descriptors[0].name, "Расш1_Реквизит1");
    }
}
