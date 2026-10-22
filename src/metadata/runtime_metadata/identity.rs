//! Header-only projection using the same measured identities as full projection.

use super::{
    Guid, Value, at, atom, business_reference, collection, current40_header2, guid, header,
    legacy_data_processor, legacy_enum_reference, list, processor_class, report_class,
    section_body,
};

/// Коллекции, чьи сырые заголовки нужны обнаружению; реквизиты не удерживаются.
pub(in crate::metadata) fn identity_collection(id: &str) -> bool {
    matches!(
        id,
        "21c53e09-8950-4b5e-a6a0-1054f1bbc274"
            | "a3fe6537-d787-40f7-8a06-419d2f0c1cfd"
            | "2bcef0d1-0981-11d6-b9b8-0050bae0a95d"
            | "b077d780-29e2-11d6-a3c7-0050bae0a776"
            | "bee0a08c-07eb-40c0-8544-5c364c171465"
    )
}

// Определяемый тип УНФ/БУХ: GUID типа, GUID значения, собственный заголовок и Pattern.
// Один измеренный предикат сохраняет согласованность полной проекции и заголовка.
pub(super) fn defined_type_class<'a>(value: &'a Value, resource: &Guid) -> Option<&'a [Value]> {
    let class = at(value, &[1]).and_then(list)?;
    (class.len() == 5
        && class.first().and_then(atom) == Some("0")
        // Нативная пара Config/XML БУХ подтверждает старый заголовок 1/7;
        // используем строгую проверку этой формы только внутри определяемого типа.
        && header(&class[3])
            .or_else(|| legacy_data_processor::identity(&class[3]))
            .as_ref()
            == Some(resource))
    .then_some(class)
}

pub(in crate::metadata) fn project_header_identity(
    value: &Value,
    projected: &mut crate::metadata::ConfigHeader,
) {
    let resource = &projected.object_guid;
    projected.defined_type = defined_type_class(value, resource)
        .and_then(|class| guid(&class[1]))
        .map_or(
            crate::metadata::ConfigTypeIdentity::Unknown,
            crate::metadata::ConfigTypeIdentity::measured,
        );
    use crate::metadata::ConfigTypeIdentity::{self, Unknown};
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    let kind = class.first().and_then(atom);
    let mut reference = if kind == Some("19")
        || (matches!(kind, Some("30" | "33")) && class.get(1).and_then(list).is_some())
    {
        Unknown
    } else {
        // Та же измеренная позиция, которую использует потоковый ConfigParser.
        let position = if kind == Some("20") { 1 } else { 3 };
        class
            .get(position)
            .and_then(guid)
            .map_or(Unknown, ConfigTypeIdentity::measured)
    };
    if kind == Some("19")
        && let Some(id) = legacy_enum_reference(root, class, resource)
    {
        reference = ConfigTypeIdentity::measured(id);
    }
    if matches!(kind, Some("30" | "33")) && class.get(1).and_then(list).is_some() {
        reference =
            business_reference(class, resource).map_or(Unknown, ConfigTypeIdentity::measured);
    }
    let object = match kind {
        Some("16") => processor_class(value, resource, true).and_then(|c| guid(&c[1])),
        Some("17") => processor_class(value, resource, false).and_then(|c| guid(&c[1])),
        Some("19") => report_class(value, resource).and_then(|c| guid(&c[1])),
        Some("30")
            if class.len() == 49
                && header(&class[1]).as_ref() == Some(resource)
                && matches!(atom(&class[2]), Some("0" | "1"))
                && class[3..15].iter().all(|v| guid(v).is_some()) =>
        {
            guid(&class[3])
        }
        _ => None,
    }
    .map_or(Unknown, ConfigTypeIdentity::measured);
    let section_id = match kind {
        Some("37" | "38" | "40") => Some("21c53e09-8950-4b5e-a6a0-1054f1bbc274"),
        Some("30") => Some("a3fe6537-d787-40f7-8a06-419d2f0c1cfd"),
        Some("16" | "17") => Some("2bcef0d1-0981-11d6-b9b8-0050bae0a95d"),
        Some("19") if matches!(object, ConfigTypeIdentity::Known(_)) => {
            Some("b077d780-29e2-11d6-a3c7-0050bae0a776")
        }
        _ => None,
    };
    let sections = section_id.and_then(|id| {
        let rows = collection(root, id)?;
        let mut sections = Vec::new();
        for row in rows {
            let identity = if kind == Some("40") {
                // Современный путь использует именно это членство, не код оболочки.
                at(row, &[0, 1, 5, 1])?
            } else {
                let (wrapper_len, marker) = match kind {
                    Some("30") => (3, "1"),
                    Some("37" | "38") => (2, "1"),
                    _ => (2, "0"),
                };
                let body = section_body(row, wrapper_len, marker)?;
                let identity_wrapper = list(&body[5])?;
                if identity_wrapper.len() != 2 || atom(&identity_wrapper[0]) != Some("0") {
                    return None;
                }
                &identity_wrapper[1]
            };
            let id = header(identity)
                .or_else(|| current40_header2(identity))
                .or_else(|| legacy_data_processor::identity(identity))?;
            if &id == resource
                || sections
                    .iter()
                    .any(|s: &crate::metadata::ConfigSectionIdentity| s.object_guid == id)
            {
                return None;
            }
            let name = list(identity)?.get(2)?.as_string()?.to_owned();
            sections.push(crate::metadata::ConfigSectionIdentity {
                object_guid: id,
                name,
                generated_types: None,
            });
        }
        Some(sections)
    });
    projected.synonyms = own_synonyms(value, resource).unwrap_or_default();
    projected.route_point_reference_type = if kind == Some("30")
        && business_reference(class, resource).is_some_and(|id| !id.is_nil())
    {
        class
            .get(13)
            .and_then(guid)
            .map_or(Unknown, ConfigTypeIdentity::measured)
    } else {
        Unknown
    };
    projected.enumeration_values = collection(root, "bee0a08c-07eb-40c0-8544-5c364c171465")
        .and_then(|rows| {
            let mut values = Vec::new();
            for row in rows {
                let row = list(row)?;
                if row.len() != 2 || atom(&row[1]) != Some("0") {
                    return None;
                }
                let wrapper = list(&row[0])?;
                if wrapper.len() != 2 || atom(&wrapper[0]) != Some("0") {
                    return None;
                }
                let identity = &wrapper[1];
                let fields = list(identity)?;
                // Тот же self-reference/name/synonym заголовок, которому
                // потоковый декодер назначает роль значения этой коллекции.
                let id = crate::metadata::config::value_identity(fields.get(1..4)?)?;
                if values
                    .iter()
                    .any(|v: &crate::metadata::ConfigValueIdentity| v.object_guid == id)
                {
                    return None;
                }
                values.push(crate::metadata::ConfigValueIdentity {
                    object_guid: id,
                    name: fields.get(2)?.as_string()?.into(),
                    synonyms: crate::metadata::config::parse_synonyms(fields.get(3)?),
                });
            }
            Some(values)
        });
    projected.reference_type = reference;
    projected.object_type = object;
    projected.sections = sections;
}

pub(super) fn own_synonyms(
    value: &Value,
    resource: &Guid,
) -> Option<Vec<crate::metadata::Synonym>> {
    let values = list(value)?;
    for window in values.windows(3) {
        if crate::metadata::config::value_identity(window).as_ref() == Some(resource) {
            return Some(crate::metadata::config::parse_synonyms(&window[2]));
        }
    }
    values
        .iter()
        .find_map(|value| own_synonyms(value, resource))
}
