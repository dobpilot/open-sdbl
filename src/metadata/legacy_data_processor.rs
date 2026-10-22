//! Измеренная старая форма обработки: класс 16/12 и идентичность 1/7.

use super::*;

pub(super) fn identity(value: &Value) -> Option<Guid> {
    checked_identity(value, false)
}

// Строковый комментарий допускают только измеренные профили реквизитов
// старого документа и функциональной опции; строгие вызывающие не меняются.
pub(super) fn identity_with_text_comment(value: &Value) -> Option<Guid> {
    checked_identity(value, true)
}

fn checked_identity(value: &Value, allow_text_comment: bool) -> Option<Guid> {
    let v = list(value)?;
    if v.len() != 7
        || atom(&v[0]) != Some("1")
        || v[2].as_string()?.is_empty()
        || (!allow_text_comment && v[4].as_string() != Some(""))
        || v[4].as_string().is_none()
        || atom(&v[5]) != Some("0")
        || atom(&v[6]) != Some("0")
    {
        return None;
    }
    let reference = list(&v[1])?;
    let synonyms = list(&v[3])?;
    if reference.len() != 3
        || atom(&reference[0]) != Some("0")
        || atom(&reference[1]) != Some("0")
        || synonyms.len() != 3
        || atom(&synonyms[0]) != Some("1")
        || synonyms[1].as_string() != Some("ru")
        || synonyms[2].as_string()?.is_empty()
    {
        return None;
    }
    guid(&reference[2]).filter(|id| !id.is_nil())
}

// Записываем декларации только после проверки всей коллекции: повреждённая
// последняя строка не превращает предыдущие строки в якобы полный состав.
fn attributes(
    root: &[Value],
    collection_id: &str,
    ds: &mut [ConfigDescriptor],
) -> Option<Vec<Guid>> {
    let rows = collection(root, collection_id)?;
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let r = list(row)?;
        let wrapper = list(r.first()?)?;
        let body = list(wrapper.get(1)?)?;
        if r.len() != 2
            || atom(&r[1]) != Some("0")
            || wrapper.len() != 2
            || atom(&wrapper[0]) != Some("0")
            || body.len() != 23
            || atom(&body[0]) != Some("27")
        {
            return None;
        }
        let typed = list(&body[1])?;
        if typed.len() != 3 || atom(&typed[0]) != Some("2") {
            return None;
        }
        let id = identity(&typed[1])?;
        let types = declaration(&typed[2])?;
        if types.is_empty()
            || parsed.iter().any(|(seen, _, _)| seen == &id)
            || ds.iter().filter(|d| d.object_guid == id).count() != 1
        {
            return None;
        }
        parsed.push((id, types, body));
    }
    let ids = parsed.iter().map(|(id, _, _)| id.clone()).collect();
    for (id, types, body) in parsed {
        let d = ds.iter_mut().find(|d| d.object_guid == id)?;
        d.declared_types = Some(types);
        d.fill_from_filling_value = flag(&body[20]);
        d.filling_value = filling(&body[19]);
    }
    Some(ids)
}

fn sections(root: &[Value], ds: &mut [ConfigDescriptor]) -> Option<Vec<Guid>> {
    let rows = collection(root, "2bcef0d1-0981-11d6-b9b8-0050bae0a95d")?;
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let r = list(row)?;
        let wrapper = list(r.first()?)?;
        let body = list(wrapper.get(1)?)?;
        if r.len() != 3
            || atom(&r[1]) != Some("1")
            || wrapper.len() != 2
            || atom(&wrapper[0]) != Some("0")
            || body.len() != 9
            || atom(&body[0]) != Some("11")
            || !body[1..5].iter().all(|v| guid(v).is_some())
        {
            return None;
        }
        let identity_wrapper = list(&body[5])?;
        if identity_wrapper.len() != 2 || atom(&identity_wrapper[0]) != Some("0") {
            return None;
        }
        let id = identity(&identity_wrapper[1])?;
        if parsed.iter().any(|(seen, _)| seen == &id)
            || ds.iter().filter(|d| d.object_guid == id).count() != 1
        {
            return None;
        }
        parsed.push((id, r));
    }
    let ids = parsed.iter().map(|(id, _)| id.clone()).collect();
    for (id, r) in parsed {
        let fields = attributes(r, "5d24a9d1-098e-11d6-b9b8-0050bae0a95d", ds);
        ds.iter_mut()
            .find(|d| d.object_guid == id)?
            .tabular_section_attributes = fields;
    }
    Some(ids)
}

pub(super) fn project(value: &Value, resource: &Guid, ds: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    if class.len() != 12
        || atom(&class[0]) != Some("16")
        || !list(&class[3]).is_some_and(|v| v.len() == 2 && atom(&v[0]) == Some("0"))
        || at(value, &[1, 3, 1]).and_then(identity).as_ref() != Some(resource)
        || ![1, 2, 4, 7, 8, 9]
            .iter()
            .all(|i| guid(&class[*i]).is_some())
    {
        return;
    }
    let fields = attributes(root, "ec6bb5e5-b7a8-4d75-bec9-658107a699cf", ds);
    let sections = sections(root, ds);
    if let Some(d) = ds.iter_mut().find(|d| &d.object_guid == resource) {
        d.data_processor_attributes = fields;
        d.data_processor_tabular_sections = sections;
        d.data_processor_object_type = guid(&class[1]).filter(|id| !id.is_nil());
    }
}
