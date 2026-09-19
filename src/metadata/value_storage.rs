//! Values a configuration stores in a `ХранилищеЗначения` field.
//!
//! The column of the table holds a reference, not the value: a 64-byte
//! record starting with `STORHDR`, carrying the key of the content and
//! its length. The content itself is kept in the platform table
//! `binarydata`, in parts ordered by their offset; concatenated, it is a
//! ten-byte header followed by the brace serialization of the value, with
//! a UTF-8 byte-order mark in front, raw-deflated when the field
//! compresses its value.

use super::deflate::inflate_raw_deflate_bounded;
use super::value::{Value, parse_serialized};
use super::{MetadataError, MetadataErrorKind};

/// The marker of a stored-value reference.
const MARKER: &[u8] = b"STORHDR";

/// The header before the serialized value: two bytes and the length.
const CONTENT_HEADER: usize = 10;

/// The reference a `ХранилищеЗначения` column holds.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredValueRef {
    /// The key of the content in `binarydata`.
    pub key: [u8; 16],
    /// The length of the content the reference declares.
    pub length: u64,
}

impl StoredValueRef {
    /// The key as the lower-case hexadecimal the statements take.
    #[must_use]
    pub fn key_hex(&self) -> String {
        self.key.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Reads the reference a `ХранилищеЗначения` column holds.
///
/// Answers `None` when the bytes are not such a reference — an empty
/// value, or a column of another kind.
#[must_use]
pub fn parse_stored_value_ref(bytes: &[u8]) -> Option<StoredValueRef> {
    if !bytes.starts_with(MARKER) || bytes.len() < 40 {
        return None;
    }
    let key = bytes[16..32].try_into().ok()?;
    let length = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
    Some(StoredValueRef { key, length })
}

fn malformed(message: &str) -> MetadataError {
    MetadataError::new(
        MetadataErrorKind::Serialization,
        format!("stored value: {message}"),
    )
}

/// Decodes the content of a stored value into its brace-serialized tree.
///
/// `content` is every part of `binarydata` for the key, in the order of
/// their offsets, concatenated.
///
/// # Errors
///
/// Returns [`MetadataError`] when the content is shorter than its header,
/// is neither the serialization nor a deflate stream of one, or is not
/// valid UTF-8.
pub fn decode_stored_value(content: &[u8], limit: usize) -> Result<Value, MetadataError> {
    let body = content
        .get(CONTENT_HEADER..)
        .ok_or_else(|| malformed("content shorter than its header"))?;
    let body = body.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(body);
    if body.first() == Some(&b'{') {
        return parse_serialized(body);
    }
    // A compressed field keeps the same serialization, deflated.
    let inflated = inflate_raw_deflate_bounded(body, limit)
        .map_err(|error| malformed(&format!("content is not the serialization: {error}")))?;
    let inflated = inflated
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&inflated);
    parse_serialized(inflated)
}

/// The entries of a serialized `Соответствие`: the value is a record
/// `{"#", <guid>, {N, {<ключ>, <значение>}…}}`, and each key that is a
/// string is answered with its value.
#[must_use]
pub fn stored_map_entries(value: &Value) -> Vec<(String, &Value)> {
    let Some(record) = value.as_list() else {
        return Vec::new();
    };
    let Some(pairs) = record.iter().find_map(Value::as_list) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for pair in pairs.iter().skip(1) {
        let Some(pair) = pair.as_list() else {
            continue;
        };
        let Some(key) = pair.first().and_then(entry_string) else {
            continue;
        };
        if let Some(value) = pair.get(1) {
            entries.push((key, value));
        }
    }
    entries
}

/// The text of an entry `{"S","…"}`, or of a bare string.
#[must_use]
pub fn entry_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_string() {
        return Some(text.to_owned());
    }
    let list = value.as_list()?;
    let kind = list.first().and_then(Value::as_string)?;
    (kind == "S")
        .then(|| {
            list.get(1)
                .and_then(Value::as_string)
                .map(ToOwned::to_owned)
        })
        .flatten()
}
