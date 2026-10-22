//! Tabular-section recognition and collection publication.

use super::{
    ConfigDescriptor, DocumentAttributeProfile, Guid, ValidatedAttributes, Value, at, atom,
    collection, complete_attributes, current40_header2, guid, header, legacy_data_processor, list,
    membership, staged_document_attributes,
};

// В отличие от современного пути, весь новый набор частей публикуется сразу:
// ошибка в последнем поле не оставляет типы полей первых частей.
#[derive(Clone, Copy)]
pub(super) enum LegacySectionProfile {
    Current40Header2,
    Document37,
    Document38,
    Catalog53,
}

/// All section identities and all their fields have been checked before any
/// mutation. A malformed final member cannot publish a valid-looking prefix.
struct ValidatedSections {
    parts: Vec<ValidatedSection>,
}

struct ValidatedSection {
    id: Guid,
    descriptor: usize,
    fields: ValidatedAttributes,
}

impl ValidatedSections {
    fn parse(
        root: &[Value],
        descriptors: &[ConfigDescriptor],
        resource: &Guid,
        own_attributes: &[Guid],
        profile: LegacySectionProfile,
    ) -> Option<Self> {
        let (collection_id, wrapper_len, field_profile) = match profile {
            LegacySectionProfile::Current40Header2 => (
                "21c53e09-8950-4b5e-a6a0-1054f1bbc274",
                2,
                DocumentAttributeProfile::Current40Header2Section,
            ),
            LegacySectionProfile::Document37 => (
                "21c53e09-8950-4b5e-a6a0-1054f1bbc274",
                2,
                DocumentAttributeProfile::Legacy53CatalogSection,
            ),
            LegacySectionProfile::Document38 => (
                "21c53e09-8950-4b5e-a6a0-1054f1bbc274",
                2,
                DocumentAttributeProfile::Legacy38Section,
            ),
            LegacySectionProfile::Catalog53 => (
                "932159f9-95b2-4e76-a8dd-8849fe5c5ded",
                3,
                DocumentAttributeProfile::Legacy53CatalogSection,
            ),
        };
        let rows = collection(root, collection_id)?;
        let mut parts = Vec::with_capacity(rows.len());
        // Часть или её поле не могут переиспользовать описатель владельца
        // либо собственного реквизита и переписать его при общей публикации.
        let mut identities = vec![resource.clone()];
        identities.extend_from_slice(own_attributes);
        for row in rows {
            let row = list(row)?;
            if row.len() != 3 || atom(&row[1]) != Some("1") {
                return None;
            }
            let wrapper = list(&row[0])?;
            if wrapper.len() != wrapper_len
                || atom(&wrapper[0]) != Some("1")
                || (wrapper_len == 3 && atom(&wrapper[2]) != Some("0"))
            {
                return None;
            }
            let body = list(&wrapper[1])?;
            if body.len() != 9
                || atom(&body[0]) != Some("11")
                || !body[1..5].iter().all(|v| guid(v).is_some())
            {
                return None;
            }
            let identity_wrapper = list(&body[5])?;
            if identity_wrapper.len() != 2 || atom(&identity_wrapper[0]) != Some("0") {
                return None;
            }
            let id = match profile {
                LegacySectionProfile::Current40Header2 => current40_header2(&identity_wrapper[1])?,
                _ => legacy_data_processor::identity(&identity_wrapper[1])?,
            };
            if identities.contains(&id)
                || descriptors.iter().filter(|d| d.object_guid == id).count() != 1
            {
                return None;
            }
            identities.push(id.clone());
            let fields = staged_document_attributes(
                row,
                descriptors,
                "888744e1-b616-11d4-9436-004095e12fc7",
                field_profile,
            )
            .known()?;
            for field_id in fields.ids() {
                if identities.contains(field_id) {
                    return None;
                }
                identities.push(field_id.clone());
            }
            let descriptor = descriptors.iter().position(|d| d.object_guid == id)?;
            parts.push(ValidatedSection {
                id,
                descriptor,
                fields,
            });
        }
        Some(Self { parts })
    }

    fn apply(self, descriptors: &mut [ConfigDescriptor]) -> Vec<Guid> {
        // Preserve the existing order: first field types, then section membership.
        let parts = self
            .parts
            .into_iter()
            .map(|part| {
                let fields = part.fields.apply(descriptors);
                (part.id, part.descriptor, fields)
            })
            .collect::<Vec<_>>();
        parts
            .into_iter()
            .map(|(id, index, fields)| {
                let descriptor = &mut descriptors[index];
                debug_assert_eq!(descriptor.object_guid, id);
                descriptor.tabular_section_attributes = Some(fields);
                id
            })
            .collect()
    }
}

pub(super) fn legacy_document_sections(
    root: &[Value],
    descriptors: &mut [ConfigDescriptor],
    resource: &Guid,
    own_attributes: &[Guid],
    profile: LegacySectionProfile,
) -> Option<Vec<Guid>> {
    Some(
        ValidatedSections::parse(root, descriptors, resource, own_attributes, profile)?
            .apply(descriptors),
    )
}

// Нативные отчёт и обработка используют одинаковую форму части 11/9 и реквизита 27/23.
pub(super) fn complete_object_sections(
    root: &[Value],
    section_collection: &str,
    field_collection: &str,
    descriptors: &mut [ConfigDescriptor],
) -> Option<Vec<Guid>> {
    let rows = collection(root, section_collection)
        .filter(|rows| rows.iter().all(|row| section_body(row, 2, "0").is_some()));
    let sections = rows
        .and_then(|r| membership(r, &[0, 1, 5, 1]))
        .filter(|ids| {
            ids.iter()
                .all(|id| descriptors.iter().filter(|d| &d.object_guid == id).count() == 1)
        });
    if let Some(rows) = rows {
        for row in rows {
            let Some(id) = at(row, &[0, 1, 5, 1]).and_then(header) else {
                continue;
            };
            let fields =
                list(row).and_then(|r| complete_attributes(r, field_collection, descriptors));
            if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
                d.tabular_section_attributes = fields;
            }
        }
    }
    sections
}

pub(super) fn section_body<'a>(
    row: &'a Value,
    wrapper_len: usize,
    marker: &str,
) -> Option<&'a [Value]> {
    let row = list(row)?;
    if row.len() != 3 {
        return None;
    }
    let wrapper = list(&row[0])?;
    if wrapper.len() != wrapper_len || atom(&wrapper[0]) != Some(marker) {
        return None;
    }
    let body = list(&wrapper[1])?;
    (body.len() == 9
        && atom(&body[0]) == Some("11")
        && body[1..5].iter().all(|v| guid(v).is_some()))
    .then_some(body)
}
