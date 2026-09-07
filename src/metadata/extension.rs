use std::str::FromStr;

use super::config::ConfigDescriptor;
use super::db_names::{DbNameEntry, DbNames};
use super::schema::SchemaStorage;
use super::value::{Value, parse_serialized};
use super::{ExtensionMetadata, Guid, MetadataError, MetadataErrorKind, inflate_raw_deflate};

/// The decoded extension restructure: recognized fields plus malformed
/// records that resemble a field declaration but could not be interpreted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionRestructure {
    /// Successfully decoded extension attribute records.
    pub fields: Vec<ExtensionFieldRestructure>,
    /// Human-readable descriptions of malformed field-like records.
    pub anomalies: Vec<String>,
}

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
pub fn parse_extension_restructure(resource: &[u8]) -> Result<ExtensionRestructure, MetadataError> {
    let text = decode_restructure_text(resource)?;
    let wrapped = format!("{{{}}}", text.trim_start_matches('\u{feff}'));
    let value = parse_serialized(wrapped.as_bytes())?;
    let mut restructure = ExtensionRestructure::default();
    collect_fields(&value, &mut restructure);
    Ok(restructure)
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
    restructure: ExtensionRestructure,
) -> ExtensionMetadata {
    let origin = origin.into();
    let ExtensionRestructure { fields, anomalies } = restructure;
    let mut entries = Vec::with_capacity(fields.len());
    let mut descriptors = Vec::with_capacity(fields.len());
    let mut field_reference_targets = Vec::new();
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
        if let Some(target) = &field.reference_target {
            field_reference_targets.push((field.number, target.clone()));
        }
    }
    ExtensionMetadata {
        origin,
        db_names: DbNames::from_entries(entries),
        descriptors,
        schema: SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        field_reference_targets,
        restructure_anomalies: anomalies,
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

fn collect_fields(value: &Value, restructure: &mut ExtensionRestructure) {
    let Value::List(items) = value else {
        return;
    };
    match field_record(items) {
        FieldRecord::Field(field) => restructure.fields.push(field),
        FieldRecord::Malformed(detail) => restructure.anomalies.push(detail),
        FieldRecord::NotAField => {}
    }
    for item in items {
        collect_fields(item, restructure);
    }
}

enum FieldRecord {
    Field(ExtensionFieldRestructure),
    Malformed(String),
    NotAField,
}

/// Classifies a list node. A record is treated as a field declaration when its
/// second element is a `Fld*` name string; only then is a decode failure a
/// reportable anomaly rather than an unrelated node.
fn field_record(items: &[Value]) -> FieldRecord {
    let Some(physical) = items.get(1).and_then(Value::as_string) else {
        return FieldRecord::NotAField;
    };
    if !physical.starts_with("Fld") {
        return FieldRecord::NotAField;
    }
    let Some(field) = decode_field_record(items) else {
        return FieldRecord::Malformed(format!(
            "malformed extension restructure record for {physical:?}"
        ));
    };
    FieldRecord::Field(field)
}

fn decode_field_record(items: &[Value]) -> Option<ExtensionFieldRestructure> {
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
        let restructure = parse_extension_restructure(&fixture()).unwrap();
        let fields = restructure.fields;
        assert!(restructure.anomalies.is_empty());
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
        let restructure = parse_extension_restructure(&fixture()).unwrap();
        let metadata = extension_metadata_from_restructure("Расширение1", restructure);
        assert_eq!(metadata.origin, "Расширение1");
        assert!(metadata.db_names.field_guid(16536).is_some());
        assert_eq!(metadata.descriptors.len(), 3);
        assert_eq!(metadata.descriptors[0].name, "Расш1_Реквизит1");
    }

    #[test]
    fn reports_a_malformed_field_record_and_keeps_valid_ones() {
        // A `Fld`-named record whose GUID is invalid is a reportable anomaly,
        // while a non-`Fld` sibling node is ignored silently.
        let text = r#"{root,{ {bad-guid,"Fld9001","STRING(1)","Broken"},
            {7d8d7de3-4c7c-45da-82fa-7f097d38a173,"Fld16536","STRING(10)","Good"},
            {something,"NotAField","x","y"} }}"#;
        let restructure = parse_extension_restructure(text.as_bytes()).unwrap();
        assert_eq!(restructure.fields.len(), 1);
        assert_eq!(restructure.fields[0].number, 16536);
        assert_eq!(restructure.anomalies.len(), 1);
        assert!(restructure.anomalies[0].contains("Fld9001"));
    }
}
