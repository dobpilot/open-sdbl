//! Проекция измеренных настроек Config, общая для обоих провайдеров.

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

// Парные нативные CF/XML подтверждают отчёт 19/18 и обработку 17/12.
// Старая обработка 16/12 сохраняет свой отдельно измеренный заголовок.
fn report_class<'a>(value: &'a Value, resource: &Guid) -> Option<&'a [Value]> {
    let root = list(value)?;
    let class = root.get(1).and_then(list)?;
    if class.len() != 18
        || !list(&class[3]).is_some_and(|w| w.len() == 2 && atom(&w[0]) == Some("0"))
        || at(value, &[1, 3, 1]).and_then(object_header).as_ref() != Some(resource)
        || ![1, 2, 12, 13, 14]
            .iter()
            .all(|i| guid(&class[*i]).is_some())
    {
        return None;
    }
    Some(class)
}

fn processor_class<'a>(value: &'a Value, resource: &Guid, legacy: bool) -> Option<&'a [Value]> {
    let root = list(value)?;
    let class = root.get(1).and_then(list)?;
    if (legacy && class.first().and_then(atom) != Some("16"))
        || class.len() != 12
        || !list(&class[3]).is_some_and(|w| w.len() == 2 && atom(&w[0]) == Some("0"))
        || (if legacy {
            at(value, &[1, 3, 1]).and_then(legacy_data_processor::identity)
        } else {
            at(value, &[1, 3, 1]).and_then(object_header)
        })
        .as_ref()
            != Some(resource)
        || (legacy && guid(&class[4]).is_none())
        || ![1, 2, 7, 8, 9].iter().all(|i| guid(&class[*i]).is_some())
    {
        return None;
    }
    Some(class)
}

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

// Измеренная форма заголовка частей текущего документа проверяется локально:
// общий `header` и ранее поддержанные формы `object_header` не расширяем.
fn current40_header2(value: &Value) -> Option<Guid> {
    let values = list(value)?;
    if values.len() != 8
        || atom(&values[0]) != Some("2")
        || values[4].as_string() != Some("")
        || atom(&values[5]) != Some("0")
        || atom(&values[6]) != Some("0")
        || !guid(&values[7])?.is_nil()
    {
        return None;
    }
    let synonyms = list(&values[3])?;
    if synonyms.len() != 3
        || atom(&synonyms[0]) != Some("1")
        || synonyms[1].as_string() != Some("ru")
        || synonyms[2].as_string()?.is_empty()
    {
        return None;
    }
    object_header(value)
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
fn owner_reference(value: &Value) -> Option<Guid> {
    let reference = list(value)?;
    if reference.len() != 3
        || reference[0].as_string() != Some("#")
        || atom(&reference[1]) != Some("157fa490-4ce9-11d4-9415-008048da11f9")
    {
        return None;
    }
    let target = list(&reference[2])?;
    if target.len() != 2 || atom(&target[0]) != Some("1") {
        return None;
    }
    guid(&target[1]).filter(|id| !id.is_nil())
}

fn owners(v: &Value) -> Option<Vec<Guid>> {
    let p = list(v)?;
    if atom(p.first()?) != Some("0") || p.get(1)?.as_u32()? as usize != p.len() - 2 {
        return None;
    }
    let mut ids = Vec::new();
    for entry in &p[2..] {
        let id = owner_reference(entry)?;
        if ids.contains(&id) {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}

// Нативные CF/XML подтверждают оболочку ссылки состава и внутренний
// список владельцев; неизвестные формы не дают частичного результата.
fn functional_option_content(value: &Value) -> Option<()> {
    let content = list(value)?;
    if atom(content.first()?) != Some("0")
        || content.get(1)?.as_u32()? as usize != content.len() - 2
    {
        return None;
    }
    for entry in &content[2..] {
        let entry = list(entry)?;
        if entry.len() != 3
            || entry[0].as_string() != Some("#")
            || atom(&entry[1]) != Some("3ea29ea5-66f6-4e3b-8595-d8940db766a2")
        {
            return None;
        }
        let target = list(&entry[2])?;
        if target.len() != 2 || atom(&target[0]) != Some("0") {
            return None;
        }
        owner_reference(&target[1])?;
    }
    Some(())
}

// Нативные CF/XML подтверждают отдельный старый заголовок опции и
// строковые комментарии. Общие заголовки остальных классов не расширяем.
fn project_legacy_functional_option(
    value: &Value,
    resource: &Guid,
    descriptors: &mut [ConfigDescriptor],
) -> Option<()> {
    let root = list(value)?;
    let class = list(root.get(1)?)?;
    if root.len() != 3
        || atom(&root[0]) != Some("1")
        || atom(&root[2]) != Some("0")
        || class.len() != 5
        || atom(&class[0]) != Some("2")
    {
        return None;
    }
    let id = legacy_data_processor::identity_with_text_comment(&class[1])?;
    if &id != resource || descriptors.iter().filter(|d| d.object_guid == id).count() != 1 {
        return None;
    }
    functional_option_content(&class[3])?;
    let location = guid(&class[2])?;
    let mode = flag(&class[4])?;
    let owner = descriptors.iter_mut().find(|d| d.object_guid == id)?;
    owner.functional_option_location_guid = Some(location.clone());
    owner.functional_option_privileged_get_mode = Some(mode);
    if location.is_nil() {
        owner.functional_option_location = Some(FunctionalOptionLocation::Absent);
    }
    Some(())
}

fn legacy_enum_header(value: &Value) -> Option<Guid> {
    let header = list(value)?;
    if header.len() != 5
        || atom(&header[0]) != Some("0")
        || header[2].as_string()?.is_empty()
        || header[4].as_string().is_none()
    {
        return None;
    }
    let reference = list(&header[1])?;
    if reference.len() != 3 || atom(&reference[0]) != Some("0") || atom(&reference[1]) != Some("0")
    {
        return None;
    }
    guid(&reference[2]).filter(|id| !id.is_nil())
}

fn legacy_enum_reference(root: &[Value], class: &[Value], resource: &Guid) -> Option<Guid> {
    // Неизменённый Config из BUH загружен нативной платформой в собственный CF:
    // обратный XML подтверждает Ref на позиции 1 и Manager на позиции 3.
    // Это профиль Enum с заголовком версии 0, а не любой объект класса 19.
    if root.len() != 7
        || atom(&root[0]) != Some("1")
        || atom(&root[2]) != Some("4")
        || class.len() != 20
        || flag(&class[6]).is_none()
        || !matches!(atom(&class[11]), Some("1" | "2"))
        || atom(&class[12]) != Some("1")
        || ![1, 2, 3, 4, 7, 8]
            .iter()
            .all(|i| guid(&class[*i]).is_some_and(|id| !id.is_nil()))
        || ![9, 10, 13, 14]
            .iter()
            .all(|i| guid(&class[*i]).is_some_and(|id| id.is_nil()))
        || !class[15..19]
            .iter()
            .all(|v| list(v).is_some_and(|v| v.len() == 1 && atom(&v[0]) == Some("0")))
        || !list(&class[19]).is_some_and(|v| {
            v.len() == 2
                && atom(&v[0]) == Some("0")
                && list(&v[1]).is_some_and(|v| v.len() == 1 && atom(&v[0]) == Some("0"))
        })
    {
        return None;
    }
    let wrapper = list(&class[5])?;
    if wrapper.len() != 2
        || atom(&wrapper[0]) != Some("0")
        || legacy_enum_header(&wrapper[1]).as_ref() != Some(resource)
    {
        return None;
    }
    for (row, collection) in root[3..6].iter().zip([
        "33f2e54b-37ce-4a7a-a569-b648d7aa4634",
        "3daea016-69b7-4ed4-9453-127911372fe6",
        "6d8d73a7-ba29-401d-9032-3872ec2d6433",
    ]) {
        let row = list(row)?;
        if row.len() != 2 || atom(&row[0]) != Some(collection) || atom(&row[1]) != Some("0") {
            return None;
        }
    }
    let values = list(&root[6])?;
    if atom(values.first()?) != Some("bee0a08c-07eb-40c0-8544-5c364c171465")
        || values.get(1)?.as_u32()? as usize != values.len() - 2
    {
        return None;
    }
    let mut ids = std::collections::HashSet::new();
    for row in &values[2..] {
        let row = list(row)?;
        if row.len() != 2 || atom(&row[1]) != Some("0") {
            return None;
        }
        let wrapper = list(&row[0])?;
        if wrapper.len() != 2
            || atom(&wrapper[0]) != Some("0")
            || !ids.insert(legacy_enum_header(&wrapper[1])?)
        {
            return None;
        }
    }
    guid(&class[1])
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
            "0" => CommonAttributeContentUse::Auto,
            "1" => CommonAttributeContentUse::Use,
            "2" => CommonAttributeContentUse::DontUse,
            _ => return None,
        };
        content.push(CommonAttributeContentEntry {
            object,
            usage,
            conditional_separation_guid: guid(&entry[2])?,
            conditional_separation: guid(&entry[2])
                .filter(Guid::is_nil)
                .map(|_| CommonAttributeConditionalSeparation::Undefined),
        });
    }
    Some(CommonAttributeUsage { auto_use, content })
}

pub(super) fn project(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    if root.first().and_then(atom) == Some("2") {
        project_configuration(value, descriptors);
        return;
    }
    if root.first().and_then(atom) != Some("1") {
        return;
    }
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    let Some(kind) = class.first().and_then(atom) else {
        return;
    };
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
        let Some(staged) = staged else {
            return;
        };
        if staged.iter().any(|(id, _)| id == resource) {
            return;
        }
        let ids = staged.iter().map(|(id, _)| id.clone()).collect();
        for (id, types) in staged {
            descriptors
                .iter_mut()
                .find(|d| d.object_guid == id)
                .unwrap()
                .declared_types = Some(types);
        }
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
        ) else {
            return;
        };
        if fields.iter().any(|(id, _)| id == resource) {
            return;
        }
        // Ни один тип не публикуется до проверки всего собственного состава.
        let ids = fields.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        for (id, types) in fields {
            descriptors
                .iter_mut()
                .find(|d| d.object_guid == id)
                .unwrap()
                .declared_types = Some(types);
        }
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
            ) {
                let ids = staged.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
                let complete = descriptors
                    .iter()
                    .find(|d| &d.object_guid == resource)
                    .and_then(|d| d.catalog_attributes.as_ref())
                    == Some(&ids);
                if complete && !ids.contains(resource) {
                    for (id, types) in staged {
                        descriptors
                            .iter_mut()
                            .find(|d| d.object_guid == id)
                            .unwrap()
                            .declared_types = Some(types);
                    }
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

#[derive(Clone, Copy)]
enum DocumentAttributeProfile {
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

// Собственный состав проверяется целиком до публикации типов и членства.
// Профиль выбирается только после проверки владельца по нативной CF/XML-паре;
// заголовки старой формы не расширяют общий `header` или современный профиль.
fn staged_document_attributes(
    root: &[Value],
    descriptors: &[ConfigDescriptor],
    collection_id: &str,
    profile: DocumentAttributeProfile,
) -> Option<Vec<(Guid, Vec<ConfigDeclaredType>)>> {
    let rows = collection(root, collection_id)?;
    let mut fields = Vec::with_capacity(rows.len());
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
        if fields.iter().any(|(seen, _)| seen == &id)
            || descriptors.iter().filter(|d| d.object_guid == id).count() != 1
        {
            return None;
        }
        let types = declaration(&typed[2])?;
        if types.is_empty() {
            return None;
        }
        fields.push((id, types));
    }
    Some(fields)
}

fn document_attributes(
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
    )?;
    if matches!(profile, DocumentAttributeProfile::Legacy37)
        && fields.iter().any(|(id, _)| id == resource)
    {
        return None;
    }
    let ids = fields.iter().map(|(id, _)| id.clone()).collect();
    for (id, types) in fields {
        descriptors
            .iter_mut()
            .find(|d| d.object_guid == id)?
            .declared_types = Some(types);
    }
    Some(ids)
}

// В отличие от современного пути, весь новый набор частей публикуется сразу:
// ошибка в последнем поле не оставляет типы полей первых частей.
#[derive(Clone, Copy)]
enum LegacySectionProfile {
    Current40Header2,
    Document37,
    Document38,
    Catalog53,
}

fn legacy_document_sections(
    root: &[Value],
    descriptors: &mut [ConfigDescriptor],
    resource: &Guid,
    own_attributes: &[Guid],
    profile: LegacySectionProfile,
) -> Option<Vec<Guid>> {
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
    let mut staged_fields = Vec::new();
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
        )?;
        let mut field_ids = Vec::with_capacity(fields.len());
        for (field_id, types) in fields {
            if identities.contains(&field_id) {
                return None;
            }
            identities.push(field_id.clone());
            field_ids.push(field_id.clone());
            staged_fields.push((field_id, types));
        }
        parts.push((id, field_ids));
    }
    let ids = parts.iter().map(|(id, _)| id.clone()).collect();
    for (id, types) in staged_fields {
        descriptors
            .iter_mut()
            .find(|d| d.object_guid == id)?
            .declared_types = Some(types);
    }
    for (id, fields) in parts {
        descriptors
            .iter_mut()
            .find(|d| d.object_guid == id)?
            .tabular_section_attributes = Some(fields);
    }
    Some(ids)
}

// Целая нативная CF/XML-пара подтверждает только Boolean-константу класса
// 14/12. Проверяем локальную форму, не расширяя общие заголовки и грамматику.
fn project_legacy_boolean_constant(
    value: &Value,
    resource: &Guid,
    descriptors: &mut [ConfigDescriptor],
) -> Option<()> {
    let root = list(value)?;
    let class = list(root.get(1)?)?;
    let body = list(class.get(1)?)?;
    let typed = list(body.get(1)?)?;
    if root.len() != 3
        || atom(&root[0]) != Some("1")
        || atom(&root[2]) != Some("0")
        || class.len() != 12
        || atom(&class[0]) != Some("14")
        || !class[2..6]
            .iter()
            .all(|v| guid(v).is_some_and(|id| !id.is_nil()))
        || atom(&class[6]) != Some("1")
        || atom(&class[7]) != Some("0")
        || at(value, &[1, 8]).and_then(list).map(<[Value]>::len) != Some(1)
        || at(value, &[1, 8, 0]).and_then(atom) != Some("0")
        || at(value, &[1, 9]).and_then(list).map(<[Value]>::len) != Some(1)
        || at(value, &[1, 9, 0]).and_then(atom) != Some("0")
        || !guid(&class[10]).is_some_and(|id| id.is_nil())
        || atom(&class[11]) != Some("0")
        || body.len() != 23
        || atom(&body[0]) != Some("27")
        || typed.len() != 3
        || atom(&typed[0]) != Some("2")
    {
        return None;
    }
    for index in [2, 5, 7, 10, 13, 17, 20, 21, 22] {
        if atom(&body[index]) != Some("0") {
            return None;
        }
    }
    if body[6].as_string() != Some("")
        || atom(&body[12]) != Some("2")
        || !guid(&body[11]).is_some_and(|id| id.is_nil())
    {
        return None;
    }
    for (index, expected) in [
        (3, &["0"][..]),
        (4, &["0"][..]),
        (14, &["5004", "0"][..]),
        (15, &["3", "0", "0"][..]),
        (16, &["0", "0"][..]),
        (18, &["0"][..]),
    ] {
        let values = list(&body[index])?;
        if values.len() != expected.len()
            || !values
                .iter()
                .zip(expected)
                .all(|(v, s)| atom(v) == Some(*s))
        {
            return None;
        }
    }
    for index in [8, 9] {
        let values = list(&body[index])?;
        if values.len() != 1 || values[0].as_string() != Some("U") {
            return None;
        }
    }
    let filling = list(&body[19])?;
    if filling.len() != 2
        || filling[0].as_string() != Some("S")
        || filling[1].as_string() != Some("")
    {
        return None;
    }
    let id = legacy_data_processor::identity(&typed[1])?;
    if &id != resource || descriptors.iter().filter(|d| d.object_guid == id).count() != 1 {
        return None;
    }
    let types = declaration(&typed[2])?;
    if types.len() != 1
        || types[0].tag != "B"
        || types[0].reference.is_some()
        || !types[0].qualifiers.is_empty()
    {
        return None;
    }
    descriptors
        .iter_mut()
        .find(|d| d.object_guid == id)?
        .declared_types = Some(types);
    Some(())
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
                d.report_tabular_sections.as_ref(),
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
            &mut d.document_attributes,
            &mut d.business_process_attributes,
            &mut d.data_processor_attributes,
            &mut d.report_attributes,
            &mut d.exchange_plan_attributes,
            &mut d.accumulation_register_dimensions,
            &mut d.accumulation_register_resources,
            &mut d.information_register_dimensions,
            &mut d.information_register_resources,
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

// Нативные отчёт и обработка используют одинаковую форму части 11/9 и реквизита 27/23.
fn complete_object_sections(
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

// Неизменённый ресурс обработки и обратная нативная XML-выгрузка подтверждают
// только собственные реквизиты этой формы; заголовки отчётов и частей не расширяем.
fn legacy_data_processor_attributes(
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

fn project_data_processor(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    if processor_class(value, resource, false).is_none() {
        return;
    }
    attribute_bodies(value, descriptors);
    attribute_fill_checking(value, descriptors);
    let attributes = complete_attributes(root, "ec6bb5e5-b7a8-4d75-bec9-658107a699cf", descriptors)
        .or_else(|| legacy_data_processor_attributes(value, root, descriptors));
    let sections = complete_object_sections(
        root,
        "2bcef0d1-0981-11d6-b9b8-0050bae0a95d",
        "5d24a9d1-098e-11d6-b9b8-0050bae0a95d",
        descriptors,
    );
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        d.data_processor_attributes = attributes;
        d.data_processor_tabular_sections = sections;
        d.data_processor_object_type = guid(&class[1]).filter(|id| !id.is_nil());
    }
}

fn project_exchange_plan(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    // Парные нативные Config/XML/EDT подтверждают план обмена 37/51:
    // его собственный заголовок расположен после одиннадцати GUID типов.
    // Документ 37/50 имеет другую форму и сюда не попадает.
    if class.len() != 51
        || atom(&class[0]) != Some("37")
        || !class[1..12].iter().all(|v| guid(v).is_some())
        || object_header(&class[12]).as_ref() != Some(resource)
        || descriptors
            .iter()
            .filter(|d| &d.object_guid == resource)
            .count()
            != 1
    {
        return;
    }
    attribute_bodies(value, descriptors);
    let attributes = complete_attributes(root, "1a1b4fea-e093-470d-94ff-1d2f16cda2ab", descriptors);
    if let Some(owner) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        owner.exchange_plan_attributes = attributes;
    }
}

fn project_report(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    if report_class(value, resource).is_none() {
        return;
    }
    attribute_bodies(value, descriptors);
    attribute_fill_checking(value, descriptors);
    let attributes = complete_attributes(root, "7e7123e0-29e2-11d6-a3c7-0050bae0a776", descriptors);
    let sections = complete_object_sections(
        root,
        "b077d780-29e2-11d6-a3c7-0050bae0a776",
        "c339c860-29e2-11d6-a3c7-0050bae0a776",
        descriptors,
    );
    if let Some(d) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        d.report_attributes = attributes;
        d.report_tabular_sections = sections;
        d.report_object_type = guid(&class[1]).filter(|id| !id.is_nil());
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
    let section_rows = collection(root, "a3fe6537-d787-40f7-8a06-419d2f0c1cfd")
        .filter(|rows| rows.iter().all(|row| section_body(row, 3, "1").is_some()));
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

fn project_configuration(value: &Value, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else { return };
    if root.len() != 11 || atom(&root[2]) != Some("7") {
        return;
    }
    let Some(section) = root.get(3).and_then(list) else {
        return;
    };
    if section.len() != 2 || atom(&section[0]) != Some("9cd510cd-abfc-11d4-9434-004095e12fc7") {
        return;
    }
    let Some(body) = section.get(1).and_then(list) else {
        return;
    };
    let Some(id) = at(value, &[3, 1, 1, 1, 1]).and_then(header) else {
        return;
    };
    let ids = collection(body, "30d554db-541e-4f62-8970-a1c6dcfeb2bc").and_then(|rows| {
        let mut ids = Vec::new();
        for row in rows {
            let id = guid(row)?;
            if id.is_nil() || ids.contains(&id) {
                return None;
            }
            ids.push(id);
        }
        Some(ids)
    });
    if let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id) {
        d.configuration_functional_option_parameters = ids;
        d.configuration_common_attribute_candidates = common_candidates(value);
    }
}

pub(super) fn resolve_functional_options(
    descriptors: &mut [ConfigDescriptor],
    db_names: &super::DbNames,
) {
    let known = descriptors
        .iter()
        .map(|d| d.object_guid.clone())
        .collect::<Vec<_>>();
    for d in descriptors.iter_mut() {
        for ids in [
            &mut d.information_register_dimensions,
            &mut d.information_register_resources,
        ] {
            if ids
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| !known.contains(id)))
            {
                *ids = None;
            }
        }
        if d.functional_option_parameter_use
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| !known.contains(id)))
        {
            d.functional_option_parameter_use = None;
        }
    }
    let parameters = descriptors
        .iter()
        .filter(|d| d.functional_option_parameter_use.is_some())
        .map(|d| d.object_guid.clone())
        .collect::<Vec<_>>();
    for d in descriptors.iter_mut() {
        if d.configuration_functional_option_parameters
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| !parameters.contains(id)))
        {
            d.configuration_functional_option_parameters = None;
        }
    }
    let constants = descriptors
        .iter()
        .filter_map(|d| d.configuration_common_attribute_candidates.as_ref())
        .flatten()
        .filter(|c| c.constant)
        .map(|c| c.object.clone())
        .collect::<Vec<_>>();
    let resolved = descriptors
        .iter()
        .map(|option| {
            let target = option.functional_option_location_guid.as_ref()?;
            if target.is_nil() {
                return Some(FunctionalOptionLocation::Absent);
            }
            let field = descriptors.iter().find(|d| &d.object_guid == target)?;
            if constants.contains(target)
                || db_names
                    .entries()
                    .iter()
                    .any(|entry| &entry.guid == target && entry.alias == "Const")
            {
                return Some(FunctionalOptionLocation::Constant(target.clone()));
            }
            if field.field_purpose == Some(super::ConfigFieldPurpose::InformationRegisterResource)
                && descriptors.iter().any(|owner| {
                    owner.object_guid == field.resource_guid
                        && owner.object_guid == owner.resource_guid
                })
            {
                return Some(FunctionalOptionLocation::InformationRegisterResource {
                    owner: field.resource_guid.clone(),
                    resource: target.clone(),
                });
            }
            let mut owners = descriptors.iter().filter(|owner| {
                owner
                    .catalog_attributes
                    .as_ref()
                    .is_some_and(|ids| ids.contains(target))
            });
            let owner = owners.next()?;
            if owners.next().is_some() {
                return None;
            }
            Some(FunctionalOptionLocation::CatalogAttribute {
                owner: owner.object_guid.clone(),
                attribute: target.clone(),
            })
        })
        .collect::<Vec<_>>();
    for (d, location) in descriptors.iter_mut().zip(resolved) {
        if d.functional_option_location_guid.is_some() {
            d.functional_option_location = location;
        }
    }
}

fn common_candidates(value: &Value) -> Option<Vec<CommonAttributeCandidate>> {
    // Пары CF/XML и нативный обход состава независимо подтверждают классы.
    // Неизвестная непустая коллекция не превращается в частичный полный набор.
    let mut result = Vec::new();
    for (group_index, (path, expected_count)) in [
        (&[3, 1][..], 25),
        (&[4, 1, 1][..], 15),
        (&[5, 1][..], 2),
        (&[6, 1][..], 2),
        (&[7, 1][..], 2),
        (&[8, 1][..], 1),
        (&[9, 1][..], 1),
    ]
    .into_iter()
    .enumerate()
    {
        let group = at(value, path).and_then(list)?;
        if group.get(2)?.as_u32()? as usize != expected_count || group.len() != expected_count + 3 {
            return None;
        }
        let mut categories = Vec::new();
        for row in &group[3..] {
            let row = list(row)?;
            let category = atom(row.first()?)?;
            if !COMMON_ROOT_COLLECTIONS[group_index].contains(&category)
                || categories.contains(&category)
                || row.get(1)?.as_u32()? as usize != row.len().checked_sub(2)?
            {
                return None;
            }
            categories.push(category);
            let constant = category == "0195e80c-b157-11d4-9435-004095e12fc7";
            let separated_only = constant || category == "11bdaf85-d5ad-4d91-bb24-aa0eee139052";
            let unseparated_only = matches!(
                category,
                "4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d" | "bc587f20-35d9-11d6-a3c7-0050bae0a776"
            );
            let included = separated_only
                || unseparated_only
                || matches!(
                    category,
                    "857c4a91-e5f4-4fac-86ec-787626f1c108"
                        | "061d872a-5787-460e-95ac-ed74ea3a3e84"
                        | "13134201-f60b-11d5-a3c7-0050bae0a776"
                        | "82a1b659-b220-4d94-a9bd-14d757b95a48"
                        | "b64d9a40-1642-11d6-a3c7-0050bae0a776"
                        | "cf4abea6-37b2-11d4-940f-008048da11f9"
                        | "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed"
                        | "2deed9b8-0056-4ffe-a473-c20a6c32a0bc"
                        | "30b100d6-b29f-47ac-aec7-cb8ca8a54767"
                        | "f2de87a8-64e5-45eb-a22d-b3aedab050e7"
                        | "3e63355c-1378-4953-be9b-1deb5fb6bec5"
                        | "fcd3404e-1523-48ce-9bc0-ecdb822684a1"
                );
            if path[0] >= 8 && row.len() != 2 {
                return None;
            }
            let mut ids = Vec::new();
            for v in &row[2..] {
                let id = guid(v)?;
                if id.is_nil() || ids.contains(&id) {
                    return None;
                }
                ids.push(id.clone());
                if included {
                    if result
                        .iter()
                        .any(|c: &CommonAttributeCandidate| c.object == id)
                    {
                        return None;
                    }
                    result.push(CommonAttributeCandidate {
                        object: id,
                        constant,
                        separated_only,
                        unseparated_only,
                    });
                }
            }
        }
    }
    result.sort_by(|a, b| a.object.as_str().cmp(b.object.as_str()));
    Some(result)
}

pub(super) fn resolve_common_attributes(
    descriptors: &mut [ConfigDescriptor],
    db_names: &super::DbNames,
) {
    let constant_ids = db_names
        .entries()
        .iter()
        .filter(|e| e.alias == "Const")
        .map(|e| e.guid.clone())
        .chain(
            descriptors
                .iter()
                .filter_map(|d| d.configuration_common_attribute_candidates.as_ref())
                .flatten()
                .filter(|c| c.constant)
                .map(|c| c.object.clone()),
        )
        .collect::<std::collections::HashSet<_>>();
    let constants = descriptors
        .iter()
        .filter(|d| constant_ids.contains(&d.object_guid))
        .filter(|d| {
            d.declared_types
                .as_ref()
                .is_some_and(|ts| ts.len() == 1 && ts[0].tag == "B")
        })
        .map(|d| d.object_guid.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut roots = descriptors
        .iter()
        .filter_map(|d| d.configuration_common_attribute_candidates.clone());
    let candidates = roots.next().filter(|_| roots.next().is_none());
    let known_ids = descriptors
        .iter()
        .map(|d| d.object_guid.clone())
        .collect::<std::collections::HashSet<_>>();
    for d in descriptors.iter_mut() {
        let Some(usage) = d.common_attribute_usage.as_mut() else {
            continue;
        };
        for entry in &mut usage.content {
            entry.conditional_separation = if entry.conditional_separation_guid.is_nil() {
                Some(CommonAttributeConditionalSeparation::Undefined)
            } else if constants.contains(&entry.conditional_separation_guid) {
                Some(CommonAttributeConditionalSeparation::Constant(
                    entry.conditional_separation_guid.clone(),
                ))
            } else {
                None
            };
        }
        d.common_attribute_content = (|| {
            let candidates = candidates.as_ref()?;
            let separation = d.common_attribute_separation?;
            let candidate_ids = candidates
                .iter()
                .map(|c| &c.object)
                .collect::<std::collections::HashSet<_>>();
            let candidates = candidates
                .iter()
                .filter(|c| {
                    (!c.separated_only || separation == CommonAttributeSeparation::Separate)
                        && (!c.unseparated_only || separation == CommonAttributeSeparation::DontUse)
                })
                .collect::<Vec<_>>();
            if candidates.iter().any(|c| !known_ids.contains(&c.object))
                || usage
                    .content
                    .iter()
                    .any(|e| !candidate_ids.contains(&e.object))
            {
                return None;
            }
            let explicit = usage
                .content
                .iter()
                .map(|e| (&e.object, e))
                .collect::<std::collections::HashMap<_, _>>();
            Some(
                candidates
                    .into_iter()
                    .map(|c| {
                        let explicit = explicit.get(&c.object);
                        CommonAttributeRuntimeContentEntry {
                            object: c.object.clone(),
                            usage: explicit.map_or(CommonAttributeContentUse::Auto, |e| e.usage),
                            conditional_separation: explicit.map_or(
                                Some(CommonAttributeConditionalSeparation::Undefined),
                                |e| e.conditional_separation.clone(),
                            ),
                        }
                    })
                    .collect(),
            )
        })();
    }
}

const COMMON_ROOT_COLLECTIONS: [&[&str]; 7] = [
    &[
        "09736b02-9cac-4e3f-b4f7-d3e9576ab948",
        "0c89c792-16c3-11d5-b96b-0050bae0a95d",
        "0fe48980-252d-11d6-a3c7-0050bae0a776",
        "0fffc09c-8f4c-47cc-b41c-8d5c5a221d79",
        "11bdaf85-d5ad-4d91-bb24-aa0eee139052",
        "15794563-ccec-41f6-a83c-ec5f7b9a5bc1",
        "24c43748-c938-45d0-8d14-01424a72b11e",
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc",
        "37f2fa9a-b276-11d4-9435-004095e12fc7",
        "39bddf6a-0c3c-452b-921c-d99cfa1c2f1b",
        "3e5404af-6ef8-4c73-ad11-91bd2dfac4c8",
        "3e7bfcc0-067d-11d6-a3c7-0050bae0a776",
        "46b4cd97-fd13-4eaa-aba2-3bddd7699218",
        "4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc",
        "58848766-36ea-4076-8800-e91eb49590d7",
        "6e6dc072-b7ac-41e7-8f88-278d25b6da2a",
        "7dcd43d9-aca5-4926-b549-1842e6a4e8cf",
        "857c4a91-e5f4-4fac-86ec-787626f1c108",
        "8657032e-7740-4e1d-a3ba-5dd6e8afb78f",
        "9cd510ce-abfc-11d4-9434-004095e12fc7",
        "a7641777-7813-45c6-96ef-9d51587a6ac6",
        "af547940-3268-434f-a3e7-e47d6d2638c3",
        "c045099e-13b9-4fb6-9d50-fca00202971e",
        "cc9df798-7c94-4616-97d2-7aa0b7bc515e",
        "d26096fb-7a5d-4df9-af63-47d04771fa9b",
    ],
    &[
        "0195e80c-b157-11d4-9435-004095e12fc7",
        "061d872a-5787-460e-95ac-ed74ea3a3e84",
        "07ee8426-87f1-11d5-b99c-0050bae0a95d",
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        "1c57eabe-7349-44b3-b1de-ebfeab67b47d",
        "2f1a5187-fb0e-4b05-9489-dc5dd6412348",
        "36a8e346-9aaa-4af9-bdbd-83be3c177977",
        "4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d",
        "631b75a0-29e2-11d6-a3c7-0050bae0a776",
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        "bc587f20-35d9-11d6-a3c7-0050bae0a776",
        "bf845118-327b-4682-b5c6-285d2a0eb296",
        "cf4abea6-37b2-11d4-940f-008048da11f9",
        "f6a80749-5ad7-400b-8519-39dc5dff2542",
    ],
    &[
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed",
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc",
    ],
    &[
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
    ],
    &[
        "3e63355c-1378-4953-be9b-1deb5fb6bec5",
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1",
    ],
    &["5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5"],
    &["bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d"],
];

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

// Полная измеренная форма общая для подробного и декларационного путей.
fn business_reference(class: &[Value], resource: &Guid) -> Option<Guid> {
    let (length, end) = match class.first().and_then(atom)? {
        "30" => (49, 15),
        "33" => (52, 13),
        _ => return None,
    };
    if class.len() != length
        || list(&class[1])?.len() != 9
        || header(&class[1]).as_ref() != Some(resource)
        || !matches!(atom(&class[2]), Some("0" | "1"))
        || !class[3..end].iter().all(|v| guid(v).is_some())
    {
        return None;
    }
    guid(&class[5])
}

fn section_body<'a>(row: &'a Value, wrapper_len: usize, marker: &str) -> Option<&'a [Value]> {
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

/// Коллекции, чьи сырые заголовки нужны обнаружению; реквизиты не удерживаются.
pub(super) fn identity_collection(id: &str) -> bool {
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
fn defined_type_class<'a>(value: &'a Value, resource: &Guid) -> Option<&'a [Value]> {
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

pub(super) fn project_header_identity(value: &Value, projected: &mut super::ConfigHeader) {
    let resource = &projected.object_guid;
    projected.defined_type = defined_type_class(value, resource)
        .and_then(|class| guid(&class[1]))
        .map_or(
            super::ConfigTypeIdentity::Unknown,
            super::ConfigTypeIdentity::measured,
        );
    use super::ConfigTypeIdentity::{self, Unknown};
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
                    .any(|s: &super::ConfigSectionIdentity| s.object_guid == id)
            {
                return None;
            }
            let name = list(identity)?.get(2)?.as_string()?.to_owned();
            sections.push(super::ConfigSectionIdentity {
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
                let id = super::config::value_identity(fields.get(1..4)?)?;
                if values
                    .iter()
                    .any(|v: &super::ConfigValueIdentity| v.object_guid == id)
                {
                    return None;
                }
                values.push(super::ConfigValueIdentity {
                    object_guid: id,
                    name: fields.get(2)?.as_string()?.into(),
                    synonyms: super::config::parse_synonyms(fields.get(3)?),
                });
            }
            Some(values)
        });
    projected.reference_type = reference;
    projected.object_type = object;
    projected.sections = sections;
}

fn own_synonyms(value: &Value, resource: &Guid) -> Option<Vec<super::Synonym>> {
    let values = list(value)?;
    for window in values.windows(3) {
        if super::config::value_identity(window).as_ref() == Some(resource) {
            return Some(super::config::parse_synonyms(&window[2]));
        }
    }
    values
        .iter()
        .find_map(|value| own_synonyms(value, resource))
}

#[cfg(test)]
#[path = "exchange_plan_members_tests.rs"]
mod exchange_plan_members_tests;
