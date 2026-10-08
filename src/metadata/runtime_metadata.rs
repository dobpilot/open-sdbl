//! Проекция измеренных настроек Config, общая для обоих провайдеров.

use super::{
    AccumulationRegisterKind, AttributeIndexing, BusinessProcessNumberSettings,
    BusinessProcessStandardAttributeFillChecking, CatalogCodeType, CatalogHierarchyType,
    CatalogStandardAttributeFillChecking, CharacteristicSettings, CommonAttributeContentEntry,
    CommonAttributeUsage, CommonAttributeUse, ConfigDeclaredType, ConfigDescriptor,
    ConfigFillingValue, DocumentNumberPeriodicity, DocumentNumberSettings,
    DocumentNumberingSettings, FillChecking, Guid, Value,
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
fn object_header(v: &Value) -> Option<Guid> {
    let values = list(v)?;
    if values.len() == 9
        && values.first().and_then(atom) == Some("3")
        && list(values.get(1)?)?.first().and_then(atom) == Some("1")
    {
        return header(v);
    }
    // Нативные XML подтверждают также заголовок версии 2 без
    // завершающего поля версии 3, с восемью или десятью полями;
    // прочие формы не расширяем.
    if !matches!(values.len(), 8 | 10) || values.first().and_then(atom) != Some("2") {
        return None;
    }
    let reference = list(values.get(1)?)?;
    if reference.len() != 3
        || atom(&reference[0]) != Some("1")
        || atom(&reference[1]) != Some("0")
        || values[2].as_string()?.is_empty()
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

fn fill_checking(value: &Value) -> Option<FillChecking> {
    match atom(value)? {
        "0" => Some(FillChecking::DontCheck),
        "1" => Some(FillChecking::ShowError),
        _ => None,
    }
}

fn standard_fill_checking(class: &[Value]) -> Option<Vec<CatalogStandardAttributeFillChecking>> {
    standard_fill_checking_at(class, 45, 9, false)
}

fn standard_fill_checking_at(
    class: &[Value],
    index: usize,
    count: usize,
    process: bool,
) -> Option<Vec<CatalogStandardAttributeFillChecking>> {
    let outer = list(class.get(index)?)?;
    if outer.len() != 2 || outer.first().and_then(atom) != Some("1") {
        return None;
    }
    let rows = list(&outer[1])?;
    // Полные таблицы из девяти записей справочника и семи бизнес-процесса
    // независимо сверены с нативными XML; ключи двух классов различаются.
    // Неполную или разреженную форму не выдаём как полный набор.
    if rows.len() != 2 + count * 3
        || rows.first().and_then(atom) != Some("1")
        || rows.get(1)?.as_u32()? as usize != count
    {
        return None;
    }
    let mut result = Vec::new();
    for row in rows[2..].chunks_exact(3) {
        let key = list(&row[0])?;
        if key.len() != 1 || atom(&row[1]) != Some("510405d3-2a0c-4fea-960a-7fee59b32f9b") {
            return None;
        }
        let name = match atom(&key[0])? {
            "-9" if process => "Started",
            "-13" if !process => "PredefinedDataName",
            "-10" if !process => "Predefined",
            "-8" if process => "HeadTask",
            "-8" => "Ref",
            "-7" if process => "Completed",
            "-7" => "DeletionMark",
            "-6" if !process => "IsFolder",
            "-5" if process => "Ref",
            "-5" => "Owner",
            "-4" if process => "DeletionMark",
            "-4" => "Parent",
            "-3" if process => "Date",
            "-3" => "Description",
            "-2" if process => "Number",
            "-2" => "Code",
            _ => return None,
        };
        if result
            .iter()
            .any(|entry: &CatalogStandardAttributeFillChecking| entry.name == name)
        {
            return None;
        }
        let bag = list(&row[2])?;
        if !matches!(
            (bag.first().and_then(atom), bag.get(1).and_then(atom)),
            (Some("13"), Some("24")) | (Some("14"), Some("25"))
        ) || bag.get(1)?.as_u32()? as usize * 2 != bag.len().checked_sub(2)?
        {
            return None;
        }
        let mut keys = Vec::new();
        let mut setting = None;
        for pair in bag[2..].chunks_exact(2) {
            let id = guid(&pair[0])?;
            if keys.contains(&id) {
                return None;
            }
            keys.push(id);
            if atom(&pair[0]) == Some("2723eb98-b4c1-498a-a6f3-70444757902f") {
                let value = list(&pair[1])?;
                if value.len() != 3
                    || value[0].as_string() != Some("#")
                    || atom(&value[1]) != Some("98ea8e5a-b586-442b-b944-6e3447734aa7")
                {
                    return None;
                }
                setting = fill_checking(&value[2]);
            }
        }
        result.push(CatalogStandardAttributeFillChecking {
            name: name.into(),
            fill_checking: setting?,
        });
    }
    Some(result)
}

fn attribute_fill_checking(value: &Value, descriptors: &mut [ConfigDescriptor]) {
    let Some(values) = list(value) else {
        return;
    };
    if values.len() == 23
        && values.first().and_then(atom) == Some("27")
        && let Some(id) = at(value, &[1, 1]).and_then(object_header)
        && let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id)
    {
        // Позиция и оба кода сверены по UUID собственных, табличных
        // и общих реквизитов с нативным FillChecking, без анализа BSL.
        d.fill_checking = fill_checking(&values[13]);
    }
    for value in values {
        attribute_fill_checking(value, descriptors);
    }
}

fn common_usage(class: &[Value]) -> Option<CommonAttributeUsage> {
    let auto_use = match class.get(6).and_then(atom)? {
        "0" => CommonAttributeUse::Use,
        "1" => CommonAttributeUse::DontUse,
        _ => return None,
    };
    let rows = list(class.get(2)?)?;
    if rows.first().and_then(atom) != Some("3")
        || (rows.get(1)?.as_u32()? as usize).checked_mul(2)? != rows.len().checked_sub(2)?
    {
        return None;
    }
    let mut content = Vec::new();
    for pair in rows[2..].chunks_exact(2) {
        let object = guid(&pair[0])?;
        if object.is_nil()
            || content
                .iter()
                .any(|entry: &CommonAttributeContentEntry| entry.object == object)
        {
            return None;
        }
        let entry = list(&pair[1])?;
        if entry.len() != 3 || atom(&entry[0]) != Some("2") || guid(&entry[2]).is_none() {
            return None;
        }
        let usage = match atom(&entry[1])? {
            "1" => CommonAttributeUse::Use,
            "2" => CommonAttributeUse::DontUse,
            _ => return None,
        };
        content.push(CommonAttributeContentEntry { object, usage });
    }
    Some(CommonAttributeUsage { auto_use, content })
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
    if matches!(kind, "30" | "33") && class.get(1).and_then(list).is_some() {
        // XML-вход и Config, скомпилированный настоящей 1С, подтверждают
        // Object на 3/4 и Ref на 5/6. Полный блок и длины сверены отдельно;
        // неизвестная форма не возвращается к GUID типа объекта.
        let (length, guid_end) = if kind == "30" { (49, 15) } else { (52, 13) };
        let reference = (|| {
            if class.len() != length
                || list(&class[1])?.len() != 9
                || header(&class[1]).as_ref() != Some(resource)
                || !matches!(atom(&class[2]), Some("0" | "1"))
                || !class[3..guid_end].iter().all(|v| guid(v).is_some())
            {
                return None;
            }
            guid(&class[5]).filter(|id| !id.is_nil())
        })();
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
    if kind == "17" {
        project_data_processor(value, resource, descriptors);
        return;
    }
    if kind == "28" {
        project_accumulation_register(value, resource, descriptors);
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
            d.declared_types = at(value, &[1, 1, 1, 2]).and_then(declaration);
        }
        return;
    }
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
        let rows = collection(root, "31182525-9346-4595-81f8-6f91a72ebe06");
        attribute_bodies(value, descriptors);
        let ids = rows
            .filter(|rows| {
                rows.iter().all(|row| {
                    list(row).is_some_and(|row| row.len() == 2)
                        && at(row, &[0]).and_then(list).is_some_and(|body| {
                            body.len() == 8 && body.first().and_then(atom) == Some("3")
                        })
                        && at(row, &[0, 1]).and_then(list).is_some_and(|body| {
                            body.len() == 23 && body.first().and_then(atom) == Some("27")
                        })
                })
            })
            .and_then(|rows| membership(rows, &[0, 1, 1, 1]))
            .filter(|ids| {
                ids.iter().all(|id| {
                    descriptors
                        .iter()
                        .any(|d| &d.object_guid == id && d.declared_types.is_some())
                })
            });
        if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
            d.characteristic_settings = settings;
            d.characteristic_attributes = ids;
        }
        return;
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
    fn resolved_types(
        types: &[ConfigDeclaredType],
        resolved: &HashMap<Guid, Option<Vec<ConfigDeclaredType>>>,
    ) -> Option<Vec<ConfigDeclaredType>> {
        let mut output = Vec::new();
        for ty in types {
            if let Some(id) = &ty.reference
                && let Some(definition) = resolved.get(id)
            {
                output.extend(definition.as_ref()?.iter().cloned());
            } else {
                output.push(ty.clone());
            }
            if output.len() > 1_000_000 {
                return None;
            }
        }
        Some(output)
    }
    for d in descriptors.iter_mut() {
        if let Some(types) = &d.declared_types {
            d.declared_types = resolved_types(types, &resolved);
            d.reference_types = d
                .declared_types
                .as_ref()
                .map(|types| types.iter().filter_map(|ty| ty.reference.clone()).collect())
                .unwrap_or_default();
        }
        if let Some(settings) = &mut d.characteristic_settings {
            if let Some(types) = resolved_types(&settings.value_types, &resolved) {
                settings.value_types = types;
            } else {
                d.characteristic_settings = None;
            }
        }
    }
    // Неудачное разворачивание типа реквизита не оставляет обещание
    // полного типизированного состава у владельца.
    let unknown = descriptors
        .iter()
        .filter(|d| d.declared_types.is_none())
        .map(|d| d.object_guid.clone())
        .collect::<std::collections::HashSet<_>>();
    let process_sections = descriptors
        .iter()
        .flat_map(|d| {
            [
                d.business_process_tabular_sections.as_ref(),
                d.data_processor_tabular_sections.as_ref(),
            ]
        })
        .flatten()
        .flatten()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for d in descriptors {
        if process_sections.contains(&d.object_guid)
            && d.tabular_section_attributes
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
        {
            d.tabular_section_attributes = None;
        }
        for ids in [
            &mut d.business_process_attributes,
            &mut d.data_processor_attributes,
            &mut d.accumulation_register_dimensions,
            &mut d.accumulation_register_resources,
        ] {
            if ids
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
            {
                *ids = None;
            }
        }
        if d.characteristic_attributes
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
        {
            d.characteristic_attributes = None;
        }
    }
}

/// Членство подтверждается заголовками и уже разобранными типами каждого члена.
fn complete_attributes(
    root: &[Value],
    collection_id: &str,
    descriptors: &[ConfigDescriptor],
) -> Option<Vec<Guid>> {
    let rows = collection(root, collection_id)?;
    let wrapper = match collection_id {
        "87c988de-ecbf-413b-87b0-b9516df05e28" => ("3", 7),
        "ec6bb5e5-b7a8-4d75-bec9-658107a699cf" | "5d24a9d1-098e-11d6-b9b8-0050bae0a95d" => ("0", 2),
        "888744e1-b616-11d4-9436-004095e12fc7" => ("8", 5),
        "b64d9a43-1642-11d6-a3c7-0050bae0a776" => ("8", 6),
        "b64d9a41-1642-11d6-a3c7-0050bae0a776" => ("5", 3),
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

fn project_data_processor(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    // Парные нативные CF/XML подтверждают класс 17/12, объект на позиции 1,
    // менеджер на 7 и отдельные коллекции собственных реквизитов и частей.
    if class.len() != 12
        || !list(&class[3]).is_some_and(|w| w.len() == 2 && atom(&w[0]) == Some("0"))
        || at(value, &[1, 3, 1]).and_then(object_header).as_ref() != Some(resource)
        || ![1, 2, 7, 8, 9].iter().all(|i| guid(&class[*i]).is_some())
    {
        return;
    }
    attribute_bodies(value, descriptors);
    attribute_fill_checking(value, descriptors);
    let attributes = complete_attributes(root, "ec6bb5e5-b7a8-4d75-bec9-658107a699cf", descriptors);
    let rows = collection(root, "2bcef0d1-0981-11d6-b9b8-0050bae0a95d").filter(|rows| {
        rows.iter().all(|row| {
            list(row).is_some_and(|r| r.len() == 3)
                && at(row, &[0])
                    .and_then(list)
                    .is_some_and(|w| w.len() == 2 && atom(&w[0]) == Some("0"))
                && at(row, &[0, 1]).and_then(list).is_some_and(|b| {
                    b.len() == 9
                        && atom(&b[0]) == Some("11")
                        && b[1..5].iter().all(|v| guid(v).is_some())
                })
        })
    });
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
            let fields = list(row).and_then(|r| {
                complete_attributes(r, "5d24a9d1-098e-11d6-b9b8-0050bae0a95d", descriptors)
            });
            if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
                d.tabular_section_attributes = fields;
            }
        }
    }
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        d.data_processor_attributes = attributes;
        d.data_processor_tabular_sections = sections;
        d.data_processor_object_type = guid(&class[1]).filter(|id| !id.is_nil());
    }
}

fn project_business_process(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    if class.len() != 49
        || !matches!(atom(&class[2]), Some("0" | "1"))
        || header(&class[1]).as_ref() != Some(resource)
        || !class[3..15].iter().all(|v| guid(v).is_some())
    {
        return;
    }
    // Собственные XML/Config подтверждают класс 30, позиции 16/18/40,
    // коллекции владельца и ту же форму реквизита 27, что у документа.
    attribute_bodies(value, descriptors);
    attribute_fill_checking(value, descriptors);
    let attributes = complete_attributes(root, "87c988de-ecbf-413b-87b0-b9516df05e28", descriptors);
    let section_rows = collection(root, "a3fe6537-d787-40f7-8a06-419d2f0c1cfd").filter(|rows| {
        rows.iter().all(|row| {
            list(row).is_some_and(|row| row.len() == 3)
                && at(row, &[0]).and_then(list).is_some_and(|wrapper| {
                    wrapper.len() == 3 && wrapper.first().and_then(atom) == Some("1")
                })
                && at(row, &[0, 1]).and_then(list).is_some_and(|body| {
                    body.len() == 9
                        && body.first().and_then(atom) == Some("11")
                        && body[1..5].iter().all(|v| guid(v).is_some())
                })
        })
    });
    let sections = section_rows
        .and_then(|rows| membership(rows, &[0, 1, 5, 1]))
        .filter(|ids| {
            ids.iter()
                .all(|id| descriptors.iter().filter(|d| &d.object_guid == id).count() == 1)
        });
    if let Some(rows) = section_rows {
        for row in rows {
            let Some(id) = at(row, &[0, 1, 5, 1]).and_then(header) else {
                continue;
            };
            let fields = list(row).filter(|r| r.len() == 3).and_then(|r| {
                let body = at(row, &[0, 1]).and_then(list)?;
                if body.len() != 9 || atom(&body[0]) != Some("11") {
                    return None;
                }
                complete_attributes(r, "888744e1-b616-11d4-9436-004095e12fc7", descriptors)
            });
            if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
                d.tabular_section_attributes = fields;
            }
        }
    }
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        d.business_process_object_type = guid(&class[3]).filter(|id| !id.is_nil());
        d.business_process_number = (|| {
            Some(BusinessProcessNumberSettings {
                number_type: match atom(&class[16])? {
                    "0" => CatalogCodeType::Number,
                    "1" => CatalogCodeType::String,
                    _ => return None,
                },
                length: class[18].as_u32()?,
                variable_length: flag(&class[40])?,
            })
        })();
        d.business_process_attributes = attributes;
        d.business_process_tabular_sections = sections;
        d.business_process_standard_fill_checking = standard_fill_checking_at(class, 30, 7, true)
            .map(|rows| {
                rows.into_iter()
                    .map(|r| BusinessProcessStandardAttributeFillChecking {
                        name: r.name,
                        fill_checking: r.fill_checking,
                    })
                    .collect()
            });
    }
}

fn project_accumulation_register(
    value: &Value,
    resource: &Guid,
    descriptors: &mut [ConfigDescriptor],
) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    // Нативные регистры обоих видов подтверждают класс 28/26,
    // собственный заголовок 13/1 и RegisterType в позиции 15.
    if class.len() != 26
        || !class[1..13].iter().all(|v| guid(v).is_some())
        || at(value, &[1, 13, 1]).and_then(header).as_ref() != Some(resource)
    {
        return;
    }
    attribute_bodies(value, descriptors);
    attribute_fill_checking(value, descriptors);
    let dimensions = complete_attributes(root, "b64d9a43-1642-11d6-a3c7-0050bae0a776", descriptors);
    let resources = complete_attributes(root, "b64d9a41-1642-11d6-a3c7-0050bae0a776", descriptors);
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        d.accumulation_register_kind = match atom(&class[15]) {
            Some("0") => Some(AccumulationRegisterKind::Balance),
            Some("1") => Some(AccumulationRegisterKind::Turnovers),
            _ => None,
        };
        d.accumulation_register_dimensions = dimensions;
        d.accumulation_register_resources = resources;
    }
}

#[cfg(test)]
#[path = "runtime_metadata_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "data_processor_metadata_tests.rs"]
mod data_processor_tests;
