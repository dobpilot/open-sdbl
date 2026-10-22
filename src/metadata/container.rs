//! Общий декодер именованных документов контейнера 1С.

use super::{MetadataError, MetadataErrorKind};

/// Читает именованный документ контейнера, возвращая `None` при его отсутствии.
///
/// `document_limit` ограничивает каждый собираемый документ отдельно, включая
/// оглавление и описания имён; это не общий предел памяти одного вызова.
///
/// # Errors
///
/// Возвращает ошибку при повреждении заголовков, имён или цепочек блоков,
/// а также до сборки документа, чей заявленный размер превышает предел.
pub fn read_container_entry(
    container: &[u8],
    wanted: &str,
    document_limit: usize,
) -> Result<Option<Vec<u8>>, MetadataError> {
    if container.len() < 16 {
        return Err(malformed("короткий заголовок".into()));
    }
    let toc = document(container, 16, 0x200, document_limit)?;
    let marker = i32::MAX.to_le_bytes();
    if toc.len() % 12 != 0 {
        return Err(malformed("неполная запись оглавления контейнера".into()));
    }
    for entry in toc.chunks_exact(12) {
        if entry[8..] != marker {
            return Err(malformed("неверный маркер оглавления контейнера".into()));
        }
        let description = u32::from_le_bytes(entry[..4].try_into().unwrap()) as usize;
        let data = u32::from_le_bytes(entry[4..8].try_into().unwrap()) as usize;
        let description = document(container, description, 0, document_limit)?;
        if description.len() < 20 {
            return Err(malformed("короткое описание файла".into()));
        }
        let name = utf16_name(&description[20..])?;
        if name == wanted {
            return document(container, data, 0, document_limit).map(Some);
        }
    }
    Ok(None)
}

fn utf16_name(bytes: &[u8]) -> Result<String, MetadataError> {
    let end = bytes
        .chunks_exact(2)
        .position(|p| p == [0, 0])
        .unwrap_or(bytes.len() / 2);
    let units = bytes[..end * 2]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| malformed("имя файла не UTF-16LE".into()))
}

fn document(
    container: &[u8],
    mut offset: usize,
    minimum: usize,
    document_limit: usize,
) -> Result<Vec<u8>, MetadataError> {
    let mut out = Vec::new();
    let mut size = None;
    let mut blocks = 0;
    let mut visited = std::collections::HashSet::new();
    loop {
        blocks += 1;
        if blocks > 1_000_000
            || !visited.insert(offset)
            || offset
                .checked_add(31)
                .is_none_or(|end| end > container.len())
        {
            return Err(malformed("цепочка блоков повреждена".into()));
        }
        let header = &container[offset..offset + 31];
        let hex = |range: std::ops::Range<usize>| {
            std::str::from_utf8(&header[range])
                .ok()
                .and_then(|s| usize::from_str_radix(s, 16).ok())
                .ok_or_else(|| malformed("неверный заголовок блока".into()))
        };
        let total = hex(2..10)?;
        let block = hex(11..19)?.max(minimum);
        let next = hex(20..28)?;
        let total = *size.get_or_insert(total);
        if total > document_limit {
            return Err(malformed(
                "документ контейнера превысил предел байтов".into(),
            ));
        }
        let take = block.min(total.saturating_sub(out.len()));
        let start = offset + 31;
        let end = start
            .checked_add(take)
            .filter(|end| *end <= container.len())
            .ok_or_else(|| malformed("блок выходит за контейнер".into()))?;
        out.extend_from_slice(&container[start..end]);
        if out.len() >= total {
            out.truncate(total);
            return Ok(out);
        }
        if next == i32::MAX as usize {
            return Err(malformed("цепочка блоков оборвана".into()));
        }
        offset = next;
    }
}

fn malformed(message: String) -> MetadataError {
    MetadataError::new(MetadataErrorKind::Serialization, message)
}

/// Текст значения настройки до материализации объектной моделью хоста.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodedCommonSetting {
    /// Внутренняя сериализация самого значения настройки.
    Value(String),
    /// Внутренняя сериализация содержимого `ХранилищеЗначения`.
    ValueStorage(String),
}

/// Декодирует измеренные скалярные и составные настройки платформы.
///
/// Документы читаются только из переданного контейнера; ссылка `#file`
/// не означает путь файловой системы. Предел применяется к каждому
/// документу отдельно и к распакованному содержимому хранилища.
///
/// # Errors
///
/// Возвращает ошибку повреждённого контейнера, отсутствующего обязательного
/// потока, неверной сериализации, неподдержанной ссылки хранилища или
/// превышения предела документа/распакованного значения.
pub fn decode_common_setting(
    container: &[u8],
    document_limit: usize,
) -> Result<DecodedCommonSetting, MetadataError> {
    let root = read_container_entry(container, "$Stream$", document_limit)?
        .ok_or_else(|| malformed("отсутствует корневой поток настройки".into()))?;
    let text = std::str::from_utf8(&root)
        .map_err(|error| MetadataError::utf8(error.valid_up_to(), "поток настройки не UTF-8"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value = super::parse_serialized(text.as_bytes())?;
    if let Some(fields) = value.as_list()
        && fields.first().and_then(super::Value::as_string) == Some("#")
        && fields
            .get(1)
            .and_then(super::Value::as_scalar)
            .is_some_and(|id| id.eq_ignore_ascii_case("e199ca70-93cf-46ce-a54b-6edc88c3a296"))
    {
        let reference = if fields.len() == 3 {
            fields[2]
                .as_list()
                .filter(|items| items.len() == 1)
                .and_then(|items| items[0].as_scalar())
                .and_then(|value| value.strip_prefix("#file:"))
                .filter(|value| !value.is_empty())
        } else {
            None
        };
        let reference = reference
            .ok_or_else(|| malformed("неподдержанная ссылка потока хранилища настройки".into()))?;
        let contents = read_container_entry(container, reference, document_limit)?
            .ok_or_else(|| malformed("отсутствует поток хранилища настройки".into()))?;
        let text = super::decode_stored_value_text(&contents, document_limit)?;
        super::parse_serialized(text.as_bytes())?;
        return Ok(DecodedCommonSetting::ValueStorage(text.into_owned()));
    }
    Ok(DecodedCommonSetting::Value(text.to_owned()))
}

#[cfg(test)]
#[path = "container_tests.rs"]
mod tests;
