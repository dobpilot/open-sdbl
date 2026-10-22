//! Measured legacy and current catalog/document profiles.

use super::{
    AttributeIndexing, CatalogAttributeUse, CatalogCodeType, CatalogHierarchyType,
    ConfigDescriptor, DocumentAttributeProfile, DocumentNumberPeriodicity, DocumentNumberSettings,
    DocumentNumberingSettings, Guid, LegacySectionProfile, ObjectRecord, Value, at, atom,
    attribute_bodies, collection, current40_header2, declaration, document_attributes, filling,
    flag, guid, header, legacy_data_processor, legacy_document_sections, list, membership,
    object_header, owners, staged_document_attributes, standard_fill_checking,
};

/// Settings have independent evidence: one malformed setting must not erase
/// another recognized value. None and a measured empty register list stay distinct.
struct DocumentSettings {
    numbering: Option<DocumentNumberingSettings>,
    number: Option<DocumentNumberSettings>,
    registers: Option<Vec<Guid>>,
}

impl DocumentSettings {
    fn parse(class: &[Value]) -> Self {
        let numbering = (|| {
            if class.len() != 53 {
                return None;
            }
            let periodicity = match class.get(13).and_then(atom)? {
                "0" => DocumentNumberPeriodicity::Nonperiodical,
                "1" => DocumentNumberPeriodicity::Year,
                "2" => DocumentNumberPeriodicity::Quarter,
                "3" => DocumentNumberPeriodicity::Month,
                "4" => DocumentNumberPeriodicity::Day,
                _ => return None,
            };
            let numerator = class.get(10).and_then(guid)?;
            Some(DocumentNumberingSettings {
                periodicity,
                numerator: (!numerator.is_nil()).then_some(numerator),
            })
        })();
        let registers = (class.len() == 53)
            .then(|| class.get(24).and_then(owners))
            .flatten();
        let number = (|| {
            if class.len() != 53 {
                return None;
            }
            Some(DocumentNumberSettings {
                number_type: match class.get(11).and_then(atom)? {
                    "0" => CatalogCodeType::Number,
                    "1" => CatalogCodeType::String,
                    _ => return None,
                },
                length: class.get(12)?.as_u32()?,
                variable_length: flag(class.get(44)?)?,
            })
        })();
        Self {
            numbering,
            number,
            registers,
        }
    }

    fn apply(self, descriptor: &mut ConfigDescriptor) {
        descriptor.document_numbering = self.numbering;
        descriptor.document_number = self.number;
        descriptor.document_registers = self.registers;
    }
}

pub(super) fn project_legacy_catalog_document(
    record: ObjectRecord<'_>,
    descriptors: &mut [ConfigDescriptor],
) {
    let ObjectRecord {
        value,
        resource,
        root,
        class,
        kind,
    } = record;
    if matches!(kind, "37" | "38") {
        let (class_len, own_profile, section_profile) = if kind == "37" {
            (
                50,
                DocumentAttributeProfile::Legacy37,
                LegacySectionProfile::Document37,
            )
        } else {
            (
                51,
                DocumentAttributeProfile::Legacy38,
                LegacySectionProfile::Document38,
            )
        };
        // Старая форма документа проверяется отдельно и не проходит через
        // общий обход реквизитов: повреждённая коллекция не публикует префикс.
        if class.len() != class_len
            || !class[1..9].iter().all(|v| guid(v).is_some())
            || !list(&class[9]).is_some_and(|v| v.len() == 2 && atom(&v[0]) == Some("0"))
            || at(value, &[1, 9, 1])
                .and_then(legacy_data_processor::identity)
                .as_ref()
                != Some(resource)
            || (kind == "37"
                && descriptors
                    .iter()
                    .filter(|d| &d.object_guid == resource)
                    .count()
                    != 1)
        {
            return;
        }
        let ids = document_attributes(root, descriptors, resource, own_profile);
        let sections = ids.as_ref().and_then(|own_attributes| {
            legacy_document_sections(root, descriptors, resource, own_attributes, section_profile)
        });
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.document_attributes = ids;
            d.document_tabular_sections = sections;
        }
        return;
    }
    if kind == "54" {
        // Старая форма справочника подтверждена целой нативной CF/XML-парой.
        // Её собственные реквизиты не расширяют общие правила заголовков.
        if class.len() != 59
            || !class[1..9].iter().all(|v| guid(v).is_some())
            || !list(&class[9]).is_some_and(|v| v.len() == 2 && atom(&v[0]) == Some("0"))
            || at(value, &[1, 9, 1])
                .and_then(legacy_data_processor::identity)
                .as_ref()
                != Some(resource)
            || descriptors
                .iter()
                .filter(|d| &d.object_guid == resource)
                .count()
                != 1
        {
            return;
        }
        let staged = staged_document_attributes(
            root,
            descriptors,
            "cf4abea7-37b2-11d4-940f-008048da11f9",
            DocumentAttributeProfile::Legacy54Catalog,
        );
        let Some(staged) = staged.known() else {
            return;
        };
        if staged.contains(resource) {
            return;
        }
        let ids = staged.apply(descriptors);
        let owner = descriptors
            .iter_mut()
            .find(|d| &d.object_guid == resource)
            .unwrap();
        owner.catalog_attributes = Some(ids);
        // Измерена только пустая коллекция: неизвестные строки частей
        // не превращаются в якобы полный пустой состав.
        owner.catalog_tabular_sections = collection(root, "932159f9-95b2-4e76-a8dd-8849fe5c5ded")
            .filter(|rows| rows.is_empty())
            .map(|_| Vec::new());
        return;
    }
    if kind == "53" {
        // Старая форма справочника сверена с целой нативной CF/XML-парой.
        // Проверяем её локально: общий заголовок и современный путь не меняются.
        let recognized = (|| {
            if class.len() != 58 || !class[1..9].iter().all(|v| guid(v).is_some()) {
                return None;
            }
            let wrapper = list(&class[9])?;
            if wrapper.len() != 2 || atom(&wrapper[0]) != Some("0") {
                return None;
            }
            let identity = list(&wrapper[1])?;
            if identity.len() != 9
                || atom(&identity[0]) != Some("1")
                || identity[2].as_string()?.is_empty()
                || identity[4].as_string().is_none()
                || atom(&identity[5]) != Some("0")
                || atom(&identity[6]) != Some("1")
                || guid(&identity[7])?.is_nil()
                || atom(&identity[8]) != Some("2")
            {
                return None;
            }
            let synonym = list(&identity[3])?;
            if synonym.len() != 3
                || atom(&synonym[0]) != Some("1")
                || synonym[1].as_string() != Some("ru")
                || synonym[2].as_string()?.is_empty()
            {
                return None;
            }
            let reference = list(&identity[1])?;
            if reference.len() != 3
                || atom(&reference[0]) != Some("0")
                || atom(&reference[1]) != Some("0")
                || guid(&reference[2]).as_ref() != Some(resource)
                || descriptors
                    .iter()
                    .filter(|d| &d.object_guid == resource)
                    .count()
                    != 1
            {
                return None;
            }
            Some(())
        })();
        if recognized.is_none() {
            return;
        }
        let Some(fields) = staged_document_attributes(
            root,
            descriptors,
            "cf4abea7-37b2-11d4-940f-008048da11f9",
            DocumentAttributeProfile::Legacy53Catalog,
        )
        .known() else {
            return;
        };
        if fields.contains(resource) {
            return;
        }
        // Ни один тип не публикуется до проверки всего собственного состава.
        let ids = fields.apply(descriptors);
        let sections = legacy_document_sections(
            root,
            descriptors,
            resource,
            &ids,
            LegacySectionProfile::Catalog53,
        );
        let owner = descriptors
            .iter_mut()
            .find(|d| &d.object_guid == resource)
            .unwrap();
        owner.catalog_attributes = Some(ids);
        owner.catalog_tabular_sections = sections;
    }
}

pub(super) fn project_current_catalog_document(
    record: ObjectRecord<'_>,
    descriptors: &mut [ConfigDescriptor],
) {
    let ObjectRecord {
        value,
        resource,
        root,
        class,
        kind,
    } = record;
    if matches!(kind, "56" | "57") && class.len() == 61 {
        if let Some(rows) = collection(root, "cf4abea7-37b2-11d4-940f-008048da11f9") {
            for row in rows {
                let Some(body) = at(row, &[0]).and_then(list) else {
                    continue;
                };
                if body.len() != 8 || atom(&body[0]) != Some("6") {
                    continue;
                }
                let Some(id) = at(row, &[0, 1, 1, 1]).and_then(header) else {
                    continue;
                };
                if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
                    d.catalog_attribute_use = match atom(&body[3]) {
                        Some("0") => Some(CatalogAttributeUse::ForItem),
                        Some("1") => Some(CatalogAttributeUse::ForFolder),
                        Some("2") => Some(CatalogAttributeUse::ForFolderAndItem),
                        _ => None,
                    };
                }
            }
        }
    }
    // Владельцы и вид иерархии имеют независимое доказательство полноты;
    // неизвестность собственных реквизитов не превращает их в неизвестные.
    if matches!(kind, "56" | "57")
        && class.len() == 61
        && class[1..9].iter().all(|v| guid(v).is_some())
        && at(value, &[1, 9, 1]).and_then(object_header).as_ref() == Some(resource)
        && let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource)
    {
        d.catalog_standard_fill_checking = standard_fill_checking(class);
        d.catalog_owners = class.get(12).and_then(owners);
        d.catalog_hierarchy_type = match class.get(36).and_then(atom) {
            Some("0") => Some(CatalogHierarchyType::FoldersAndItems),
            Some("1") => Some(CatalogHierarchyType::Items),
            _ => None,
        };
    }
    if kind == "56" && class.len() == 61 {
        // Нативная CF/XML-пара подтверждает эту форму заголовков отдельно.
        // Полный состав уже известен: добавляем только целиком проверенные
        // декларации, не меняя права, стандартные поля и прежние настройки.
        let recognized = (|| {
            let wrapper = list(&class[9])?;
            if wrapper.len() != 2 || atom(&wrapper[0]) != Some("0") {
                return None;
            }
            let identity = list(&wrapper[1])?;
            if identity.len() != 10
                || atom(&identity[0]) != Some("2")
                || identity[4].as_string() != Some("")
                || atom(&identity[5]) != Some("0")
                || atom(&identity[6]) != Some("1")
                || atom(&identity[7]) != Some("f440939a-f130-413b-9c9a-2b18c4af69c6")
                || atom(&identity[8]) != Some("2")
                || !guid(&identity[9])?.is_nil()
            {
                return None;
            }
            let synonym = list(&identity[3])?;
            if synonym.len() != 3
                || atom(&synonym[0]) != Some("1")
                || synonym[1].as_string() != Some("ru")
                || synonym[2].as_string()?.is_empty()
            {
                return None;
            }
            object_header(&wrapper[1])
        })();
        if recognized.as_ref() == Some(resource)
            && class[1..9].iter().all(|v| guid(v).is_some())
            && descriptors
                .iter()
                .filter(|d| &d.object_guid == resource)
                .count()
                == 1
        {
            if let Some(staged) = staged_document_attributes(
                root,
                descriptors,
                "cf4abea7-37b2-11d4-940f-008048da11f9",
                DocumentAttributeProfile::Current56Header2Catalog,
            )
            .known()
            {
                let ids = staged.ids().cloned().collect::<Vec<_>>();
                let complete = descriptors
                    .iter()
                    .find(|d| &d.object_guid == resource)
                    .and_then(|d| d.catalog_attributes.as_ref())
                    == Some(&ids);
                if complete && !ids.contains(resource) {
                    let _ = staged.apply(descriptors);
                }
            }
            return;
        }
    }
    if !matches!(kind, "40" | "56" | "57")
        || (kind == "56" && class.len() != 61)
        || class.len() < 10
        || !class[1..9].iter().all(|v| guid(v).is_some())
        || at(value, &[1, 9, 1])
            .and_then(|v| {
                if kind == "40" && class.len() == 53 {
                    let identity = list(v)?;
                    matches!(
                        (identity.first().and_then(atom), identity.len()),
                        (Some("2"), 8) | (Some("3"), 9)
                    )
                    .then(|| object_header(v))
                    .flatten()
                } else {
                    header(v)
                }
            })
            .as_ref()
            != Some(resource)
    {
        return;
    }
    let own_id = if kind == "40" {
        "45e46cbc-3e24-4165-8b7b-cc98a6f80211"
    } else {
        "cf4abea7-37b2-11d4-940f-008048da11f9"
    };
    let rows = collection(root, own_id);
    let ids = if kind == "40" && class.len() == 53 {
        document_attributes(
            root,
            descriptors,
            resource,
            DocumentAttributeProfile::Current40,
        )
    } else {
        rows.and_then(|rows| membership(rows, &[0, 1, 1, 1]))
    };
    let parts = if kind == "40" {
        collection(root, "21c53e09-8950-4b5e-a6a0-1054f1bbc274")
            .and_then(|rows| membership(rows, &[0, 1, 5, 1]))
    } else {
        None
    };
    // Новая форма — только запасной путь для неизвестных частей при полном
    // собственном составе. Уже известный результат современного пути сохраняем.
    let parts = parts.or_else(|| {
        if kind != "40"
            || class.len() != 53
            || at(value, &[1, 9, 1]).and_then(current40_header2).as_ref() != Some(resource)
            || descriptors
                .iter()
                .filter(|d| &d.object_guid == resource)
                .count()
                != 1
        {
            return None;
        }
        let own = ids.as_deref()?;
        if own.contains(resource) {
            return None;
        }
        legacy_document_sections(
            root,
            descriptors,
            resource,
            own,
            LegacySectionProfile::Current40Header2,
        )
    });
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        if matches!(kind, "56" | "57") {
            d.catalog_owners = class.get(12).and_then(owners);
        } else {
            // Позиции настроек и все пять кодов сверены независимыми
            // изменениями минимальной нативной конфигурации и полным УНФ.
            DocumentSettings::parse(class).apply(d);
            d.document_attributes = ids.clone();
            d.document_tabular_sections = parts;
        }
    }
    if kind == "40"
        && let Some(sections) = collection(root, "21c53e09-8950-4b5e-a6a0-1054f1bbc274")
        && let Some(section_ids) = membership(sections, &[0, 1, 5, 1])
    {
        for (section, id) in sections.iter().zip(section_ids) {
            let fields = (|| {
                // Собственная коллекция части измерена отдельно от коллекции
                // документа: соседние заголовки и рекурсивные GUID не подходят.
                let row = list(section)?;
                let body = at(section, &[0, 1]).and_then(list)?;
                if row.len() != 3 || body.len() != 9 || atom(&body[0]) != Some("11") {
                    return None;
                }
                let members = collection(row, "888744e1-b616-11d4-9436-004095e12fc7")?;
                let ids = membership(members, &[0, 1, 1, 1])?;
                ids.iter()
                    .all(|id| descriptors.iter().any(|d| &d.object_guid == id))
                    .then_some(ids)
            })();
            if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
                d.tabular_section_attributes = fields;
            }
        }
    }
    attribute_bodies(value, descriptors);
    if let (Some(rows), Some(ids)) = (rows, ids) {
        for (row, id) in rows.iter().zip(ids) {
            let Some(b) = at(row, &[0, 1]).and_then(list) else {
                continue;
            };
            if b.len() != 23 || b.first().and_then(atom) != Some("27") {
                continue;
            }
            let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) else {
                continue;
            };
            d.attribute_indexing = match at(row, &[0, 2]).and_then(atom) {
                Some("0") => Some(AttributeIndexing::Disabled),
                Some("1") => Some(AttributeIndexing::Enabled),
                Some("2") => Some(AttributeIndexing::WithAdditionalOrder),
                _ => None,
            };
            d.fill_from_filling_value = flag(&b[20]);
            d.filling_value = filling(&b[19]);
            d.declared_types = at(row, &[0, 1, 1, 2]).and_then(declaration);
        }
    }
}
