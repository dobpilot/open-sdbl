use std::str::FromStr;

use super::config::ConfigDescriptor;
use super::db_names::{DbNameEntry, DbNames};
use super::schema::SchemaStorage;
use super::value::{Value, parse_serialized, parse_serialized_sequence};
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
            separation: None,
            reference_types: Vec::new(),
            object_reference_type: None,
            balance: None,
            chart_of_accounts: None,
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

/// The key of a resource in the content-addressed store of the
/// configuration extensions: the twenty bytes the store names its rows
/// by, in lower-case hexadecimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentKey([u8; 20]);

impl ContentKey {
    /// The key as the store spells the name of its row.
    #[must_use]
    pub fn as_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The bytes of the key.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

/// The marker `ExtensionZippedInfo` writes before the key of the root.
const EXTENSION_INFO_MARKER: usize = 4;

/// The key of the root resource of an extension, which
/// `_ExtensionsInfo.ExtensionZippedInfo` carries after a four-byte
/// marker.
///
/// Answers `None` when the record is shorter than the marker and the key.
#[must_use]
pub fn extension_root_key(info: &[u8]) -> Option<ContentKey> {
    let key = info.get(EXTENSION_INFO_MARKER..EXTENSION_INFO_MARKER + 20)?;
    Some(ContentKey(key.try_into().ok()?))
}

/// What `_ExtensionsInfo.ExtensionZippedInfo` records about one
/// configuration extension.
///
/// After the four-byte marker and the twenty-byte root key the record is
/// a tag-length-value stream: `0x97` introduces a UTF-16 string of *n*
/// code units — the localized synonym — and `0x9a` a string of *n* bytes
/// — the version. The decoder reads those two and stops at anything it
/// does not know, because the format is only partly measured: the key is
/// what reading an extension needs, and it is answered whatever follows
/// it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInfo {
    /// The key of the root resource of the extension.
    pub root_key: ContentKey,
    /// The localized synonym record, when the stream carried one.
    pub synonym: Option<String>,
    /// The version of the extension, when the stream carried one.
    pub version: Option<String>,
    /// Whether the base applies the extension, or `None` when the record
    /// is not in the shape the flag was measured in.
    ///
    /// Measured on a PostgreSQL base carrying one extension the platform
    /// applies and one it does not: of the 177 bytes of each record, the
    /// two differ in the root key, in the one character of the synonym
    /// that names them apart, and in one byte — three from the end,
    /// `0x82` where the base applies the extension and `0x81` where it
    /// does not. The demo base writes the same two bytes in the same
    /// place for its one applied extension.
    ///
    /// The flag is read from that place and nowhere else, and only when
    /// the decoder followed the record's structure all the way to its
    /// terminator and reached that byte as a standalone tag. It is
    /// deliberately not derived by scanning the stream for bytes with
    /// the high bit set: a counted field this decoder does not know
    /// carries payload bytes that look exactly like flags, and reading
    /// one of those would report an extension the base applies as
    /// inactive — a consumer would then skip live code. Anything the
    /// decoder could not follow answers `None`, which a consumer must
    /// decide about rather than act on.
    pub active: Option<bool>,
}

/// The byte a record ends with, in every record measured so far.
const EXTENSION_INFO_TERMINATOR: u8 = 0x20;
/// How far from the end of the record the applicability flag sits.
const EXTENSION_INFO_FLAG_FROM_END: usize = 3;
/// The flag of an extension the base applies.
const EXTENSION_INFO_APPLIED: u8 = 0x82;
/// The flag of an extension the base does not apply.
const EXTENSION_INFO_NOT_APPLIED: u8 = 0x81;

/// The tag introducing a UTF-16 string counted in code units.
const EXTENSION_INFO_UTF16: u8 = 0x97;
/// The tag introducing a byte string counted in bytes.
const EXTENSION_INFO_BYTES: u8 = 0x9a;
/// The tags measured to stand alone, carrying no payload after them.
const EXTENSION_INFO_SINGLE: [u8; 4] = [0x81, 0x82, 0xa1, 0xa2];

/// Reads what `_ExtensionsInfo.ExtensionZippedInfo` records about one
/// extension.
///
/// Answers `None` only when the record is shorter than the marker and the
/// root key. A stream that ends inside a field, or carries a tag this
/// decoder does not know, answers the key and whatever was decoded before
/// it, with [`ExtensionInfo::active`] unknown.
#[must_use]
pub fn parse_extension_info(info: &[u8]) -> Option<ExtensionInfo> {
    let root_key = extension_root_key(info)?;
    let mut record = ExtensionInfo {
        root_key,
        synonym: None,
        version: None,
        active: None,
    };
    let flag_at = info.len().checked_sub(EXTENSION_INFO_FLAG_FROM_END);
    let mut flag = None;
    let mut offset = EXTENSION_INFO_MARKER + 20;
    while let Some(&tag) = info.get(offset) {
        // A field the decoder knows but cannot read whole ends the walk;
        // so does a tag it does not know, because its length is unknown
        // and skipping it would resume in the middle of its payload.
        let Some(next) = read_field(info, offset, tag, &mut record) else {
            break;
        };
        // Only a byte the walk itself reached as a standalone tag can be
        // the flag. A byte inside the payload of a counted field is never
        // considered, however much it looks like one.
        if next == offset + 1 && Some(offset) == flag_at {
            flag = Some(tag);
        }
        offset = next;
    }
    // The walk ends on the terminator, which is no field of its own. Any
    // other stopping place means the decoder lost the structure, and a
    // flag read out of a record it did not follow says nothing.
    if offset + 1 == info.len() && info.get(offset) == Some(&EXTENSION_INFO_TERMINATOR) {
        record.active = match flag {
            Some(EXTENSION_INFO_APPLIED) => Some(true),
            Some(EXTENSION_INFO_NOT_APPLIED) => Some(false),
            _ => None,
        };
    }
    Some(record)
}

/// Reads one field, answering where the next begins, or `None` when the
/// tag is unknown or the field does not fit in what is left.
fn read_field(info: &[u8], offset: usize, tag: u8, record: &mut ExtensionInfo) -> Option<usize> {
    if EXTENSION_INFO_SINGLE.contains(&tag) {
        return Some(offset + 1);
    }
    let count = usize::from(*info.get(offset + 1)?);
    let start = offset + 2;
    match tag {
        EXTENSION_INFO_UTF16 => {
            let end = start.checked_add(count.checked_mul(2)?)?;
            let text = info.get(start..end)?;
            if record.synonym.is_none() {
                record.synonym = decode_utf16_le(text);
            }
            Some(end)
        }
        EXTENSION_INFO_BYTES => {
            let end = start.checked_add(count)?;
            let text = info.get(start..end)?;
            // The stream writes counted byte runs that are not text as
            // well; only a printable run can be the version.
            if record.version.is_none() && !text.is_empty() && text.iter().all(u8::is_ascii_graphic)
            {
                record.version = String::from_utf8(text.to_vec()).ok();
            }
            Some(end)
        }
        // Everything else is a byte whose meaning, and therefore whose
        // length, is unmeasured. Walking past it would be guessing.
        _ => None,
    }
}

/// Decodes a UTF-16 little-endian run, answering `None` when it is not
/// valid UTF-16.
fn decode_utf16_le(bytes: &[u8]) -> Option<String> {
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}

/// One resource of an extension, as its root lists it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionResource {
    /// The name the resource has, as `Config` would name it: `<guid>`,
    /// `<guid>.0`, `<guid>.2`.
    pub name: String,
    /// The key of its content in the store.
    pub key: ContentKey,
}

/// Reads the resources an extension's root resource lists: pairs of a
/// name and the base64 of the key of its content.
///
/// # Errors
///
/// Returns [`MetadataError`] when the root is not the serialization, or
/// lists a pair the key of which is not twenty base64-encoded bytes.
pub fn parse_extension_index(root: &[u8]) -> Result<Vec<ExtensionResource>, MetadataError> {
    let records = parse_serialized_sequence(root)?;
    // The index is the record whose count is followed by name/key pairs.
    for record in &records {
        let Some(items) = record.as_list() else {
            continue;
        };
        let Some(count) = items.first().and_then(Value::as_u32) else {
            continue;
        };
        if items.len() != count as usize * 2 + 1 {
            continue;
        }
        let mut resources = Vec::with_capacity(count as usize);
        for pair in items[1..].chunks_exact(2) {
            let (Some(name), Some(key)) = (pair[0].as_string(), pair[1].as_scalar()) else {
                resources.clear();
                break;
            };
            let Some(key) = decode_content_key(key) else {
                return Err(MetadataError::new(
                    MetadataErrorKind::Serialization,
                    format!("extension index: {key:?} is not the key of a resource"),
                ));
            };
            resources.push(ExtensionResource {
                name: name.to_owned(),
                key,
            });
        }
        if !resources.is_empty() {
            return Ok(resources);
        }
    }
    Err(MetadataError::new(
        MetadataErrorKind::Serialization,
        "extension root lists no resources".to_owned(),
    ))
}

/// Reads the base64 of a twenty-byte key.
fn decode_content_key(text: &str) -> Option<ContentKey> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if text.len() != 28 || !text.ends_with('=') {
        return None;
    }
    let mut bytes = Vec::with_capacity(20);
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for character in text.trim_end_matches('=').bytes() {
        let value = ALPHABET.iter().position(|letter| *letter == character)?;
        accumulator = (accumulator << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((accumulator >> bits) as u8);
        }
    }
    Some(ContentKey(bytes.try_into().ok()?))
}
