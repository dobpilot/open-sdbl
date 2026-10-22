//! Values a configuration stores in a `ХранилищеЗначения` field.
//!
//! The column of the table holds a reference, not the value: a 64-byte
//! record starting with `STORHDR`, carrying the key of the content and
//! its length. The content itself is kept in the platform table
//! `binarydata`, in parts ordered by their offset; concatenated, it is a
//! ten-byte header followed by the brace serialization of the value, with
//! a UTF-8 byte-order mark in front, raw-deflated when the field
//! compresses its value.

use std::borrow::Cow;

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

// Общая распаковка сохраняет прежнее распознавание тела после десятибайтовой
// оболочки. Поля длины и версии оболочки здесь не получают новой интерпретации.
fn decoded_stored_value_content(
    content: &[u8],
    limit: usize,
) -> Result<Cow<'_, [u8]>, MetadataError> {
    if content.starts_with(MARKER) {
        return Err(malformed("reference is not stored-value content"));
    }
    let body = content
        .get(CONTENT_HEADER..)
        .ok_or_else(|| malformed("content shorter than its header"))?;
    let serialized = body.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(body);
    if serialized.first() == Some(&b'{') {
        if body.len() > limit {
            return Err(malformed("decoded content exceeds its byte limit"));
        }
        return Ok(Cow::Borrowed(body));
    }
    let inflated = inflate_raw_deflate_bounded(serialized, limit)
        .map_err(|error| malformed(&format!("content is not the serialization: {error}")))?;
    Ok(Cow::Owned(inflated))
}

/// Возвращает UTF-8 текст внутренней сериализации хранилища без начального BOM.
///
/// `content` содержит встроенные байты значения либо уже объединённые части
/// внешнего содержимого. Ссылка `STORHDR` содержимым не является; функция
/// не читает `binarydata` и не выполняет других операций ввода-вывода.
///
/// `limit` ограничивает распакованное тело в байтах, включая возможный BOM
/// и исключая десятибайтовую оболочку. Размер входного среза отдельно не
/// ограничивается: его память принадлежит вызывающему коду. Несжатый текст
/// заимствуется из входа, сжатый использует выделенную распаковщиком память.
/// Поля версии и объявленной длины оболочки не получают дополнительных проверок.
///
/// Текст не разбирается в дерево и не сериализуется повторно. Проверку полной
/// грамматики и поддержанных видов значений выполняет вызывающий декодер;
/// [`decode_stored_value`] дополнительно использует существующий парсер дерева.
///
/// # Errors
///
/// Возвращает [`MetadataError`] для внешней ссылки вместо содержимого,
/// короткой оболочки, повреждённого DEFLATE или UTF-8 и превышения `limit`.
/// Корректность полной грамматики сериализации эта функция не гарантирует.
pub fn decode_stored_value_text(
    content: &[u8],
    limit: usize,
) -> Result<Cow<'_, str>, MetadataError> {
    match decoded_stored_value_content(content, limit)? {
        Cow::Borrowed(bytes) => {
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
            std::str::from_utf8(bytes)
                .map(Cow::Borrowed)
                .map_err(|error| {
                    MetadataError::utf8(error.valid_up_to(), "metadata is not valid UTF-8")
                })
        }
        Cow::Owned(mut bytes) => {
            if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
                bytes.drain(..3);
            }
            String::from_utf8(bytes).map(Cow::Owned).map_err(|error| {
                MetadataError::utf8(
                    error.utf8_error().valid_up_to(),
                    "metadata is not valid UTF-8",
                )
            })
        }
    }
}

/// Разбирает содержимое хранилища существующим парсером внутренней сериализации.
///
/// `content` содержит встроенное значение либо объединённые части `binarydata`
/// в порядке смещений. Распаковка общая с [`decode_stored_value_text`]; её
/// ограничения входа, выходной длины и внешних ссылок применяются также здесь.
///
/// # Errors
///
/// Возвращает [`MetadataError`] при повреждённой оболочке, DEFLATE или UTF-8,
/// превышении `limit`, внешней ссылке вместо содержимого или ошибке грамматики.
pub fn decode_stored_value(content: &[u8], limit: usize) -> Result<Value, MetadataError> {
    parse_serialized(&decoded_stored_value_content(content, limit)?)
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
