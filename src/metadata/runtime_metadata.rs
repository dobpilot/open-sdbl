//! Dispatch measured Config families without I/O or public descriptor changes.

use super::{
    AccumulationRegisterKind, AttributeIndexing, BusinessProcessNumberSettings,
    BusinessProcessStandardAttributeFillChecking, CatalogAttributeUse, CatalogCodeType,
    CatalogHierarchyType, CatalogStandardAttributeFillChecking, CharacteristicSettings,
    CommonAttributeCandidate, CommonAttributeConditionalSeparation, CommonAttributeContentEntry,
    CommonAttributeContentUse, CommonAttributeRuntimeContentEntry, CommonAttributeSeparation,
    CommonAttributeUsage, CommonAttributeUse, ConfigDeclaredType, ConfigDescriptor,
    ConfigFillingValue, DocumentNumberPeriodicity, DocumentNumberSettings,
    DocumentNumberingSettings, FillChecking, FunctionalOptionLocation, Guid,
    InformationRegisterPeriodicity, InformationRegisterWriteMode, Value,
};

mod values;
use values::*;
mod attributes;
use attributes::*;
mod sections;
use sections::*;
mod fill_checking;
use fill_checking::*;
mod objects;
use objects::*;
mod named_types;
pub(super) use named_types::resolve_named_types;
mod identity;
use identity::defined_type_class;
pub(super) use identity::{identity_collection, project_header_identity};
mod configuration;
use configuration::{
    common_usage, functional_option_content, project_configuration,
    project_legacy_functional_option,
};
pub(super) use configuration::{resolve_common_attributes, resolve_functional_options};
mod catalogs_documents;
use catalogs_documents::{project_current_catalog_document, project_legacy_catalog_document};

/// A recognized top-level object envelope, before family-specific validation.
/// Recognition alone never authorizes publication of any facts.
#[derive(Clone, Copy)]
struct ObjectRecord<'a> {
    value: &'a Value,
    resource: &'a Guid,
    root: &'a [Value],
    class: &'a [Value],
    kind: &'a str,
}

impl<'a> ObjectRecord<'a> {
    fn recognize(value: &'a Value, resource: &'a Guid) -> Option<Self> {
        let root = list(value)?;
        if root.first().and_then(atom) != Some("1") {
            return None;
        }
        let class = root.get(1).and_then(list)?;
        let kind = class.first().and_then(atom)?;
        Some(Self {
            value,
            resource,
            root,
            class,
            kind,
        })
    }
}

pub(super) fn project(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    if root.first().and_then(atom) == Some("2") {
        project_configuration(value, descriptors);
        return;
    }
    let Some(record) = ObjectRecord::recognize(value, resource) else {
        return;
    };
    let ObjectRecord {
        root, class, kind, ..
    } = record;
    if kind == "1"
        && root.len() == 3
        && atom(&root[2]) == Some("0")
        && class.len() == 2
        && let Some(typed) = list(&class[1])
        && typed.len() == 3
        && atom(&typed[0]) == Some("2")
        && list(&typed[1]).is_some_and(|header| header.len() == 9 && atom(&header[0]) == Some("3"))
        && object_header(&typed[1]).as_ref() == Some(resource)
        && descriptors
            .iter()
            .filter(|d| &d.object_guid == resource)
            .count()
            == 1
    {
        // Нативные Config/XML/EDT параметра сеанса подтверждают Pattern
        // рядом с собственным заголовком. Чужие вложенные типы не ищем;
        // неизвестный член декларации не публикует её известный префикс.
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.declared_types = declaration(&typed[2]);
        }
        return;
    }
    if kind == "14" {
        let _ = project_legacy_boolean_constant(value, resource, descriptors);
        return;
    }
    if matches!(kind, "30" | "33") && class.get(1).and_then(list).is_some() {
        // XML-вход и Config, скомпилированный настоящей 1С, подтверждают
        // Object на 3/4 и Ref на 5/6. Полный блок и длины сверены отдельно;
        // неизвестная форма не возвращается к GUID типа объекта.
        let reference = business_reference(class, resource).filter(|id| !id.is_nil());
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.object_reference_type = reference;
            if kind == "30" && d.object_reference_type.is_some() {
                d.business_process_route_point_reference_type =
                    guid(&class[13]).filter(|id| !id.is_nil());
            }
        }
        if kind == "30" {
            project_business_process(value, resource, descriptors);
        }
        return;
    }
    if kind == "2"
        && class
            .get(1)
            .and_then(list)
            .is_some_and(|header| header.len() == 7)
    {
        let _ = project_legacy_functional_option(value, resource, descriptors);
        return;
    }
    if kind == "2"
        && root.len() == 3
        && atom(&root[2]) == Some("0")
        && class.len() == 5
        && list(&class[1]).is_some_and(|header| header.len() == 9)
        && object_header(&class[1]).as_ref() == Some(resource)
        && functional_option_content(&class[3]).is_some()
        && guid(&class[2]).is_some()
        && flag(&class[4]).is_some()
    {
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.functional_option_location_guid = guid(&class[2]);
            d.functional_option_privileged_get_mode = flag(&class[4]);
            if d.functional_option_location_guid
                .as_ref()
                .is_some_and(Guid::is_nil)
            {
                d.functional_option_location = Some(FunctionalOptionLocation::Absent);
            }
        }
        return;
    }
    if kind == "0" && class.len() == 3 && header(&class[1]).as_ref() == Some(resource) {
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.functional_option_parameter_use = owners(&class[2]);
        }
        return;
    }
    if kind == "33"
        && class.len() == 39
        && class[1..15].iter().all(|v| guid(v).is_some())
        && at(value, &[1, 15, 1]).and_then(header).as_ref() == Some(resource)
    {
        attribute_bodies(value, descriptors);
        let dimensions =
            complete_attributes(root, "13134203-f60b-11d5-a3c7-0050bae0a776", descriptors);
        let resources =
            complete_attributes(root, "13134202-f60b-11d5-a3c7-0050bae0a776", descriptors);
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.information_register_dimensions = dimensions;
            d.information_register_resources = resources;
            d.information_register_periodicity = match atom(&class[18]) {
                Some("0") => Some(InformationRegisterPeriodicity::Nonperiodical),
                Some("4") => Some(InformationRegisterPeriodicity::Day),
                _ => None,
            };
            d.information_register_write_mode = match atom(&class[19]) {
                Some("0") => Some(InformationRegisterWriteMode::Independent),
                Some("1") => Some(InformationRegisterWriteMode::RecorderSubordinate),
                _ => None,
            };
        }
        return;
    }
    // Нативная CF/XML-пара BUH подтверждает отдельную старую форму обработки.
    // Её заголовки не расширяют правила остальных классов Config.
    if kind == "16" && class.len() == 12 {
        legacy_data_processor::project(value, resource, descriptors);
        return;
    }
    if kind == "16"
        && class.len() == 17
        && at(value, &[1, 1, 1, 1]).and_then(header).as_ref() == Some(resource)
    {
        attribute_bodies(value, descriptors);
        return;
    }
    if kind == "19" {
        if let Some(reference) = legacy_enum_reference(root, class, resource) {
            if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
                d.object_reference_type = Some(reference);
            }
            return;
        }
        project_report(value, resource, descriptors);
        return;
    }
    if kind == "17" {
        project_data_processor(value, resource, descriptors);
        return;
    }
    if kind == "28" {
        project_accumulation_register(value, resource, descriptors);
        return;
    }
    if kind == "37" && class.len() == 51 {
        project_exchange_plan(value, resource, descriptors);
        return;
    }
    if matches!(kind, "37" | "38" | "53" | "54") {
        project_legacy_catalog_document(record, descriptors);
        return;
    }
    attribute_fill_checking(value, descriptors);
    if kind == "5" {
        // Класс, вложенный заголовок, Pattern и состав сверены с целыми
        // нативными XML. Список не публикуется частично при неизвестном коде.
        if class.len() != 15
            || at(value, &[1, 1, 0]).and_then(atom) != Some("27")
            || at(value, &[1, 1]).and_then(list).map(<[Value]>::len) != Some(23)
            || at(value, &[1, 1, 1, 1]).and_then(object_header).as_ref() != Some(resource)
        {
            return;
        }
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.common_attribute_usage = common_usage(class);
            // Два независимых CF/XML-контроля отличаются режимом на позиции 5.
            d.common_attribute_separation = match class.get(5).and_then(atom) {
                Some("1") => Some(CommonAttributeSeparation::DontUse),
                Some("0") => Some(CommonAttributeSeparation::Separate),
                _ => None,
            };
            d.declared_types = at(value, &[1, 1, 1, 2]).and_then(declaration);
        }
        return;
    }
    if kind == "0" {
        let Some(class) = defined_type_class(value, resource) else {
            return;
        };
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.defined_type_id = guid(&class[1]);
            d.declared_types = declaration(&class[4]);
        }
        return;
    }
    if kind == "32" {
        legacy_news_characteristic::project(value, resource, descriptors);
        return;
    }
    if kind == "34" {
        // Класс 34/59, заголовок 13/1, Pattern 18 и длины 21/23
        // сверены с нативной XML двух контрольных ресурсов 8.3.27.
        if class.len() != 59
            || !class[1..13].iter().all(|v| guid(v).is_some())
            || at(value, &[1, 13, 1]).and_then(header).as_ref() != Some(resource)
        {
            return;
        }
        let settings = (|| {
            Some(CharacteristicSettings {
                code_length: class[21].as_u32()?,
                description_length: class[23].as_u32()?,
                value_types: declaration(&class[18])?,
            })
        })();
        attribute_bodies(value, descriptors);
        let ids = complete_attributes(root, "31182525-9346-4595-81f8-6f91a72ebe06", descriptors);
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.characteristic_settings = settings;
            d.characteristic_attributes = ids;
        }
        return;
    }
    project_current_catalog_document(record, descriptors);
}

#[cfg(test)]
#[path = "runtime_metadata_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "data_processor_metadata_tests.rs"]
mod data_processor_tests;

#[cfg(test)]
#[path = "report_metadata_tests.rs"]
mod report_metadata_tests;

#[cfg(test)]
#[path = "legacy_business_process_tests.rs"]
mod legacy_business_process_tests;

#[cfg(test)]
#[path = "legacy_buh_processor_tests.rs"]
mod legacy_buh_processor_tests;

#[path = "legacy_data_processor.rs"]
mod legacy_data_processor;

#[cfg(test)]
#[path = "buh_characteristic_tests.rs"]
mod buh_characteristic_tests;

#[cfg(test)]
#[path = "buh_credit_document_tests.rs"]
mod buh_credit_document_tests;

#[cfg(test)]
#[path = "legacy_conversion_empty_types_tests.rs"]
mod legacy_conversion_empty_types_tests;

#[cfg(test)]
#[path = "legacy_news_characteristic_tests.rs"]
mod legacy_news_characteristic_tests;

#[path = "legacy_news_characteristic.rs"]
mod legacy_news_characteristic;

#[cfg(test)]
#[path = "legacy_sales_book_document_tests.rs"]
mod legacy_sales_book_document_tests;

#[cfg(test)]
#[path = "legacy_sales_book_sections_tests.rs"]
mod legacy_sales_book_sections_tests;

#[cfg(test)]
#[path = "legacy_report_deliveries_tests.rs"]
mod legacy_report_deliveries_tests;

#[cfg(test)]
#[path = "legacy_report_event_sections_tests.rs"]
mod legacy_report_event_sections_tests;

#[cfg(test)]
#[path = "legacy_report_directions_comment_tests.rs"]
mod legacy_report_directions_comment_tests;

#[cfg(test)]
#[path = "legacy_vat_real_estate_tests.rs"]
mod legacy_vat_real_estate_tests;

#[cfg(test)]
#[path = "current40_header2_sections_tests.rs"]
mod current40_header2_sections_tests;

#[cfg(test)]
#[path = "legacy38_own_comment_tests.rs"]
mod legacy38_own_comment_tests;

#[cfg(test)]
#[path = "legacy54_catalog_tests.rs"]
mod legacy54_catalog_tests;

#[cfg(test)]
#[path = "current56_catalog_types_tests.rs"]
mod current56_catalog_types_tests;

#[cfg(test)]
#[path = "legacy14_boolean_constant_tests.rs"]
mod legacy14_boolean_constant_tests;

#[cfg(test)]
#[path = "legacy_functional_option_mode_tests.rs"]
mod legacy_functional_option_mode_tests;

#[cfg(test)]
#[path = "exchange_plan_members_tests.rs"]
mod exchange_plan_members_tests;
