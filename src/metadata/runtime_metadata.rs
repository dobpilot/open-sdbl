//! Проекция измеренных настроек Config, общая для обоих провайдеров.

use super::{
    AttributeIndexing, CatalogCodeType, ConfigDeclaredType, ConfigDescriptor, ConfigFillingValue,
    DocumentNumberPeriodicity, DocumentNumberSettings, DocumentNumberingSettings, Guid, Value,
};

fn list(v: &Value) -> Option<&[Value]> {
    v.as_list()
}
fn at<'a>(v: &'a Value, path: &[usize]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, i| list(v)?.get(*i))
}
fn atom(v: &Value) -> Option<&str> {
    match v {
        Value::Atom(s) => Some(s),
        _ => None,
    }
}
fn guid(v: &Value) -> Option<Guid> {
    atom(v)?.parse().ok()
}
fn flag(v: &Value) -> Option<bool> {
    match atom(v)? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
fn header(v: &Value) -> Option<Guid> {
    let values = list(v)?;
    if values.first().and_then(atom) != Some("3") {
        return None;
    }
    let reference = list(values.get(1)?)?;
    if reference.len() != 3
        || atom(&reference[1]) != Some("0")
        || values.get(2)?.as_string()?.is_empty()
    {
        return None;
    }
    let id = guid(&reference[2])?;
    (!id.is_nil()).then_some(id)
}
fn declaration(v: &Value) -> Option<Vec<ConfigDeclaredType>> {
    let values = list(v)?;
    if values.first()?.as_string() != Some("Pattern") {
        return None;
    }
    values[1..]
        .iter()
        .map(|value| {
            let parts = list(value)?;
            let tag = parts.first()?.as_string()?;
            let (reference, qualifiers) = if tag == "#" {
                if parts.len() != 2 {
                    return None;
                }
                (Some(guid(&parts[1])?), Vec::new())
            } else if matches!(tag, "S" | "N" | "D" | "B" | "U") {
                (None, parts[1..].to_vec())
            } else {
                return None;
            };
            Some(ConfigDeclaredType {
                tag: tag.into(),
                reference,
                qualifiers,
            })
        })
        .collect()
}
fn filling(v: &Value) -> Option<ConfigFillingValue> {
    let p = list(v)?;
    match (p.first()?.as_string()?, p.len()) {
        ("U", 1) => Some(ConfigFillingValue::Undefined),
        ("S", 2) => Some(ConfigFillingValue::String(p[1].as_string()?.into())),
        ("B", 2) => Some(ConfigFillingValue::Boolean(flag(&p[1])?)),
        ("N", 2) => Some(ConfigFillingValue::Number(atom(&p[1])?.into())),
        ("#", 3) if atom(&p[1]) == Some("5c14e26f-099b-4d37-84a6-b433d87400da") => {
            let r = list(&p[2])?;
            if r.len() != 3 || atom(&r[0]) != Some("0") {
                return None;
            }
            Some(ConfigFillingValue::Reference {
                type_id: guid(&r[1])?,
                value_id: guid(&r[2])?,
            })
        }
        _ => None,
    }
}
fn collection<'a>(root: &'a [Value], id: &str) -> Option<&'a [Value]> {
    let mut matches = root
        .iter()
        .filter_map(list)
        .filter(|v| v.first().and_then(atom) == Some(id));
    let v = matches.next()?;
    if matches.next().is_some() || v.get(1)?.as_u32()? as usize != v.len() - 2 {
        return None;
    }
    Some(&v[2..])
}
fn membership(rows: &[Value], path: &[usize]) -> Option<Vec<Guid>> {
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        let id = header(at(row, path)?)?;
        if ids.contains(&id) {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}
fn owners(v: &Value) -> Option<Vec<Guid>> {
    let p = list(v)?;
    if atom(p.first()?) != Some("0") || p.get(1)?.as_u32()? as usize != p.len() - 2 {
        return None;
    }
    let mut ids = Vec::new();
    for entry in &p[2..] {
        let e = list(entry)?;
        if e.len() != 3
            || e[0].as_string() != Some("#")
            || atom(&e[1]) != Some("157fa490-4ce9-11d4-9415-008048da11f9")
        {
            return None;
        }
        let target = list(&e[2])?;
        if target.len() != 2 || atom(&target[0]) != Some("1") {
            return None;
        }
        let id = guid(&target[1])?;
        if id.is_nil() || ids.contains(&id) {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}

pub(super) fn project(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    if root.first().and_then(atom) != Some("1") {
        return;
    }
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    let Some(kind) = class.first().and_then(atom) else {
        return;
    };
    if kind == "0" {
        // Определяемый тип УНФ: GUID типа, GUID значения, заголовок, Pattern.
        if class.len() != 5 || at(value, &[1, 3]).and_then(header).as_ref() != Some(resource) {
            return;
        }
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.defined_type_id = guid(&class[1]);
            d.declared_types = declaration(&class[4]);
        }
        return;
    }
    if !matches!(kind, "40" | "56" | "57")
        || (kind == "56" && class.len() != 61)
        || class.len() < 10
        || !class[1..9].iter().all(|v| guid(v).is_some())
        || at(value, &[1, 9, 1]).and_then(header).as_ref() != Some(resource)
    {
        return;
    }
    let own_id = if kind == "40" {
        "45e46cbc-3e24-4165-8b7b-cc98a6f80211"
    } else {
        "cf4abea7-37b2-11d4-940f-008048da11f9"
    };
    let rows = collection(root, own_id);
    let ids = rows.and_then(|rows| membership(rows, &[0, 1, 1, 1]));
    let parts = if kind == "40" {
        collection(root, "21c53e09-8950-4b5e-a6a0-1054f1bbc274")
            .and_then(|rows| membership(rows, &[0, 1, 5, 1]))
    } else {
        None
    };
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        if matches!(kind, "56" | "57") {
            d.catalog_owners = class.get(12).and_then(owners);
        } else {
            // Позиции настроек и все пять кодов сверены независимыми
            // изменениями минимальной нативной конфигурации и полным УНФ.
            d.document_numbering = (|| {
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
            d.document_attributes = ids.clone();
            d.document_tabular_sections = parts;
            // Позиция коллекции и её объектные GUID сверены с нативной XML,
            // включая смешанные классы регистров; чужие GUID не обходятся.
            d.document_registers = (class.len() == 53)
                .then(|| class.get(24).and_then(owners))
                .flatten();
            // Строковый, числовой и отключённый номера проверены Config/XML
            // отдельной нативной базы; поле 44 — допустимая длина, не периодичность.
            d.document_number = (|| {
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

fn attribute_bodies(value: &Value, descriptors: &mut [ConfigDescriptor]) {
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

/// Разворачивает приобретённые определяемые типы один раз до построения снимка.
pub(super) fn resolve_named_types(descriptors: &mut [ConfigDescriptor]) {
    use std::collections::HashMap;
    let mut definitions = HashMap::<Guid, Option<Vec<ConfigDeclaredType>>>::new();
    for d in descriptors.iter() {
        if let Some(id) = &d.defined_type_id {
            definitions
                .entry(id.clone())
                .and_modify(|v| *v = None)
                .or_insert_with(|| d.declared_types.clone());
        }
    }
    if definitions.is_empty() {
        return;
    }
    fn expand(
        types: &[ConfigDeclaredType],
        definitions: &HashMap<Guid, Option<Vec<ConfigDeclaredType>>>,
        path: &mut Vec<Guid>,
    ) -> Option<Vec<ConfigDeclaredType>> {
        let mut result = Vec::new();
        for ty in types {
            if let Some(id) = &ty.reference
                && let Some(definition) = definitions.get(id)
            {
                // Предел глубины совпадает с сериализацией метаданных; цикл и
                // чрезмерное разворачивание не выдаются частичным объявлением.
                if path.len() >= 512 || path.contains(id) {
                    return None;
                }
                path.push(id.clone());
                let members = expand(definition.as_ref()?, definitions, path)?;
                path.pop();
                if result.len().saturating_add(members.len()) > 1_000_000 {
                    return None;
                }
                result.extend(members);
            } else {
                if result.len() >= 1_000_000 {
                    return None;
                }
                result.push(ty.clone());
            }
        }
        Some(result)
    }
    let resolved = definitions
        .iter()
        .map(|(id, types)| {
            (
                id.clone(),
                types
                    .as_ref()
                    .and_then(|types| expand(types, &definitions, &mut vec![id.clone()])),
            )
        })
        .collect::<HashMap<_, _>>();
    for d in descriptors {
        if let Some(types) = &d.declared_types {
            let mut output = Vec::new();
            let mut complete = true;
            for ty in types {
                if let Some(id) = &ty.reference
                    && let Some(definition) = resolved.get(id)
                {
                    if let Some(members) = definition {
                        output.extend(members.iter().cloned());
                    } else {
                        complete = false;
                        break;
                    }
                } else {
                    output.push(ty.clone());
                }
                if output.len() > 1_000_000 {
                    complete = false;
                    break;
                }
            }
            d.declared_types = complete.then_some(output);
            d.reference_types = d
                .declared_types
                .as_ref()
                .map(|types| types.iter().filter_map(|ty| ty.reference.clone()).collect())
                .unwrap_or_default();
        }
    }
}
