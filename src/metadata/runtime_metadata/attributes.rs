//! Measured attribute profiles and complete attribute collections.

use super::{
    CollectionState, ConfigDeclaredType, ConfigDescriptor, Guid, Value, at, atom, collection,
    current40_header2, declaration, filling, flag, guid, header, inspect_collection,
    legacy_data_processor, list, membership, object_header,
};

#[derive(Clone, Copy)]
pub(super) enum DocumentAttributeProfile {
    Current40,
    Current40Header2Section,
    Current56Header2Catalog,
    Legacy37,
    Legacy38,
    Legacy38Section,
    Legacy53Catalog,
    Legacy54Catalog,
    Legacy53CatalogSection,
}

/// A complete, ordered attribute collection validated against the current
/// descriptor slice. Empty collections are known empty; a failed parse is None.
/// Indices stay private and are used only before the slice can be reordered.
pub(super) struct ValidatedAttributes {
    fields: Vec<ValidatedAttribute>,
}

struct ValidatedAttribute {
    descriptor: usize,
    id: Guid,
    types: Vec<ConfigDeclaredType>,
}

impl ValidatedAttributes {
    pub(super) fn ids(&self) -> impl Iterator<Item = &Guid> {
        self.fields.iter().map(|field| &field.id)
    }

    pub(super) fn contains(&self, id: &Guid) -> bool {
        self.ids().any(|field| field == id)
    }

    pub(super) fn apply(self, descriptors: &mut [ConfigDescriptor]) -> Vec<Guid> {
        self.fields
            .into_iter()
            .map(|field| {
                let descriptor = &mut descriptors[field.descriptor];
                debug_assert_eq!(descriptor.object_guid, field.id);
                descriptor.declared_types = Some(field.types);
                field.id
            })
            .collect()
    }
}

// Собственный состав проверяется целиком до публикации типов и членства.
// Профиль выбирается только после проверки владельца по нативной CF/XML-паре;
// заголовки старой формы не расширяют общий `header` или современный профиль.
pub(super) fn staged_document_attributes(
    root: &[Value],
    descriptors: &[ConfigDescriptor],
    collection_id: &str,
    profile: DocumentAttributeProfile,
) -> CollectionState<ValidatedAttributes> {
    inspect_collection(root, collection_id)
        .validate(|rows| validate_attribute_rows(rows, descriptors, profile))
}

fn validate_attribute_rows(
    rows: &[Value],
    descriptors: &[ConfigDescriptor],
    profile: DocumentAttributeProfile,
) -> Option<ValidatedAttributes> {
    let mut fields: Vec<ValidatedAttribute> = Vec::with_capacity(rows.len());
    for row in rows {
        let row = list(row)?;
        if row.len() != 2 {
            return None;
        }
        let wrapper = list(&row[0])?;
        let wrapper_shape = (wrapper.first().and_then(atom), wrapper.len());
        let supported_wrapper = match profile {
            DocumentAttributeProfile::Current40 => {
                matches!(wrapper_shape, (Some("5"), 5) | (Some("6"), 7))
            }
            DocumentAttributeProfile::Current56Header2Catalog => {
                wrapper_shape == (Some("5"), 6)
                    && atom(&wrapper[2]) == Some("0")
                    && atom(&wrapper[3]) == Some("0")
                    && atom(&wrapper[4]) == Some("1")
                    && atom(&wrapper[5]) == Some("1")
            }
            DocumentAttributeProfile::Current40Header2Section => {
                wrapper_shape == (Some("8"), 5)
                    && atom(&wrapper[2]) == Some("0")
                    && atom(&wrapper[3]) == Some("1")
                    && atom(&wrapper[4]) == Some("1")
            }
            DocumentAttributeProfile::Legacy37 => {
                wrapper_shape == (Some("3"), 4)
                    && matches!(atom(&wrapper[2]), Some("0" | "2"))
                    && atom(&wrapper[3]) == Some("1")
            }
            DocumentAttributeProfile::Legacy38 => wrapper_shape == (Some("4"), 5),
            DocumentAttributeProfile::Legacy38Section => wrapper_shape == (Some("7"), 5),
            DocumentAttributeProfile::Legacy53Catalog => wrapper_shape == (Some("3"), 5),
            DocumentAttributeProfile::Legacy54Catalog => {
                wrapper_shape == (Some("4"), 6)
                    && matches!(atom(&wrapper[2]), Some("0" | "1"))
                    && atom(&wrapper[3]) == Some("0")
                    && atom(&wrapper[4]) == Some("1")
                    && atom(&wrapper[5]) == Some("1")
            }
            DocumentAttributeProfile::Legacy53CatalogSection => {
                wrapper_shape == (Some("6"), 4)
                    && matches!(atom(&wrapper[2]), Some("0" | "1"))
                    && atom(&wrapper[3]) == Some("1")
            }
        };
        if !supported_wrapper {
            return None;
        }
        let body = list(&wrapper[1])?;
        if body.len() != 23 || atom(&body[0]) != Some("27") {
            return None;
        }
        let typed = list(&body[1])?;
        if typed.len() != 3 || atom(&typed[0]) != Some("2") {
            return None;
        }
        let id = match profile {
            DocumentAttributeProfile::Current40Header2Section
            | DocumentAttributeProfile::Current56Header2Catalog => current40_header2(&typed[1])?,
            DocumentAttributeProfile::Current40 => {
                let identity = list(&typed[1])?;
                if !matches!(
                    (identity.first().and_then(atom), identity.len()),
                    (Some("2"), 8) | (Some("3"), 9)
                ) {
                    return None;
                }
                object_header(&typed[1])?
            }
            DocumentAttributeProfile::Legacy38 => {
                legacy_data_processor::identity_with_text_comment(&typed[1])?
            }
            DocumentAttributeProfile::Legacy37
            | DocumentAttributeProfile::Legacy38Section
            | DocumentAttributeProfile::Legacy53Catalog
            | DocumentAttributeProfile::Legacy54Catalog
            | DocumentAttributeProfile::Legacy53CatalogSection => {
                legacy_data_processor::identity(&typed[1])?
            }
        };
        if fields.iter().any(|field| field.id == id) {
            return None;
        }
        let mut matches = descriptors
            .iter()
            .enumerate()
            .filter(|(_, d)| d.object_guid == id);
        let descriptor = matches.next()?.0;
        if matches.next().is_some() {
            return None;
        }
        let types = declaration(&typed[2])?;
        if types.is_empty() {
            return None;
        }
        fields.push(ValidatedAttribute {
            descriptor,
            id,
            types,
        });
    }
    Some(ValidatedAttributes { fields })
}

pub(super) fn document_attributes(
    root: &[Value],
    descriptors: &mut [ConfigDescriptor],
    resource: &Guid,
    profile: DocumentAttributeProfile,
) -> Option<Vec<Guid>> {
    let fields = staged_document_attributes(
        root,
        descriptors,
        "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
        profile,
    )
    .known()?;
    if matches!(profile, DocumentAttributeProfile::Legacy37) && fields.contains(resource) {
        return None;
    }
    Some(fields.apply(descriptors))
}

pub(super) fn attribute_bodies(value: &Value, descriptors: &mut [ConfigDescriptor]) {
    let Some(values) = list(value) else {
        return;
    };
    if values.len() == 23
        && values.first().and_then(atom) == Some("27")
        && let Some(id) = at(value, &[1, 1]).and_then(header)
        && let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id)
    {
        d.declared_types = at(value, &[1, 2]).and_then(declaration);
        d.fill_from_filling_value = flag(&values[20]);
        d.filling_value = filling(&values[19]);
    }
    for value in values {
        attribute_bodies(value, descriptors);
    }
}

/// Членство подтверждается заголовками и уже разобранными типами каждого члена.
pub(super) fn complete_attributes(
    root: &[Value],
    collection_id: &str,
    descriptors: &[ConfigDescriptor],
) -> Option<Vec<Guid>> {
    let rows = collection(root, collection_id)?;
    let wrapper = match collection_id {
        // Полная нативная CF/XML-пара BUH подтверждает также оболочку 2/5.
        // Выбор первой строки требует однородности всей коллекции ниже.
        "87c988de-ecbf-413b-87b0-b9516df05e28"
            if rows
                .first()
                .and_then(|row| at(row, &[0]))
                .and_then(list)
                .is_some_and(|body| {
                    body.len() == 5 && body.first().and_then(atom) == Some("2")
                }) =>
        {
            ("2", 5)
        }
        "87c988de-ecbf-413b-87b0-b9516df05e28" => ("3", 7),
        // Нативная CF/XML-пара ПВХ BUH подтверждает однородную оболочку 2/6.
        "31182525-9346-4595-81f8-6f91a72ebe06"
            if rows
                .first()
                .and_then(|row| at(row, &[0]))
                .and_then(list)
                .is_some_and(|body| {
                    body.len() == 6 && body.first().and_then(atom) == Some("2")
                }) =>
        {
            ("2", 6)
        }
        "31182525-9346-4595-81f8-6f91a72ebe06" => ("3", 8),
        "ec6bb5e5-b7a8-4d75-bec9-658107a699cf"
        | "7e7123e0-29e2-11d6-a3c7-0050bae0a776"
        | "c339c860-29e2-11d6-a3c7-0050bae0a776"
        | "5d24a9d1-098e-11d6-b9b8-0050bae0a95d" => ("0", 2),
        "1a1b4fea-e093-470d-94ff-1d2f16cda2ab" => ("4", 7),
        "888744e1-b616-11d4-9436-004095e12fc7" => ("8", 5),
        "b64d9a43-1642-11d6-a3c7-0050bae0a776" => ("8", 6),
        "b64d9a41-1642-11d6-a3c7-0050bae0a776" => ("5", 3),
        "13134203-f60b-11d5-a3c7-0050bae0a776" => ("10", 9),
        "13134202-f60b-11d5-a3c7-0050bae0a776" => ("8", 7),
        _ => return None,
    };
    if !rows.iter().all(|row| {
        list(row).is_some_and(|row| row.len() == 2)
            && at(row, &[0]).and_then(list).is_some_and(|body| {
                body.len() == wrapper.1 && body.first().and_then(atom) == Some(wrapper.0)
            })
            && at(row, &[0, 1])
                .and_then(list)
                .is_some_and(|body| body.len() == 23 && body.first().and_then(atom) == Some("27"))
    }) {
        return None;
    }
    let ids = membership(rows, &[0, 1, 1, 1])?;
    ids.iter()
        .all(|id| {
            descriptors
                .iter()
                .filter(|d| &d.object_guid == id && d.declared_types.is_some())
                .count()
                == 1
        })
        .then_some(ids)
}

// Неизменённый ресурс обработки и обратная нативная XML-выгрузка подтверждают
// только собственные реквизиты этой формы; заголовки отчётов и частей не расширяем.
pub(super) fn legacy_data_processor_attributes(
    value: &Value,
    root: &[Value],
    descriptors: &mut [ConfigDescriptor],
) -> Option<Vec<Guid>> {
    let owner_header = at(value, &[1, 3, 1]).and_then(list)?;
    if owner_header.len() != 8 || atom(&owner_header[0]) != Some("2") {
        return None;
    }
    let rows = collection(root, "ec6bb5e5-b7a8-4d75-bec9-658107a699cf")?;
    let mut attributes = Vec::with_capacity(rows.len());
    for row in rows {
        let row = list(row)?;
        if row.len() != 2 || atom(&row[1]) != Some("0") {
            return None;
        }
        let wrapper = list(&row[0])?;
        if wrapper.len() != 2 || atom(&wrapper[0]) != Some("0") {
            return None;
        }
        let body = list(&wrapper[1])?;
        if body.len() != 23 || atom(&body[0]) != Some("27") {
            return None;
        }
        let typed = list(&body[1])?;
        if typed.len() != 3 || atom(&typed[0]) != Some("2") {
            return None;
        }
        let identity = list(&typed[1])?;
        if identity.len() != 8
            || atom(&identity[0]) != Some("2")
            || identity[4].as_string().is_none()
            || atom(&identity[5]) != Some("0")
            || atom(&identity[6]) != Some("0")
            || !guid(&identity[7]).is_some_and(|id| id.is_nil())
        {
            return None;
        }
        let synonyms = list(&identity[3])?;
        if synonyms.len() != 3
            || atom(&synonyms[0]) != Some("1")
            || synonyms[1].as_string() != Some("ru")
            || synonyms[2].as_string().is_none_or(str::is_empty)
        {
            return None;
        }
        let id = object_header(&typed[1])?;
        if attributes.iter().any(|(seen, _, _)| seen == &id)
            || descriptors.iter().filter(|d| d.object_guid == id).count() != 1
        {
            return None;
        }
        // Нативные CF/XML обработки конвертации подтверждают пустой `Pattern`:
        // это известная пустая декларация, а не ошибка чтения или семантика `Any`.
        let types = declaration(&typed[2])?;
        attributes.push((id, types, body));
    }
    let ids = attributes.iter().map(|(id, _, _)| id.clone()).collect();
    for (id, types, body) in attributes {
        let descriptor = descriptors.iter_mut().find(|d| d.object_guid == id)?;
        descriptor.declared_types = Some(types);
        descriptor.fill_from_filling_value = flag(&body[20]);
        descriptor.filling_value = filling(&body[19]);
    }
    Some(ids)
}
