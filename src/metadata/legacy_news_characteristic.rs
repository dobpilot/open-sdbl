//! Измеренный старый ПВХ: класс 32/56, заголовок владельца 1/9 и реквизиты 1/7.

use super::*;

fn owner(value: &Value) -> Option<Guid> {
    let header = list(value)?;
    if header.len() != 9
        || atom(&header[0]) != Some("1")
        || header[2].as_string()?.is_empty()
        || header[4].as_string().is_none()
        || atom(&header[5]) != Some("0")
        || atom(&header[6]) != Some("1")
        || guid(&header[7]).is_none()
        || atom(&header[8]) != Some("2")
    {
        return None;
    }
    let reference = list(&header[1])?;
    let synonyms = list(&header[3])?;
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

fn attributes(root: &[Value], descriptors: &mut [ConfigDescriptor]) -> Option<Vec<Guid>> {
    let rows = collection(root, "31182525-9346-4595-81f8-6f91a72ebe06")?;
    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        let row = list(row)?;
        if row.len() != 2 || atom(&row[1]) != Some("0") {
            return None;
        }
        let wrapper = list(&row[0])?;
        if wrapper.len() != 5
            || atom(&wrapper[0]) != Some("1")
            || atom(&wrapper[2]) != Some("0")
            || atom(&wrapper[3]) != Some("0")
            || atom(&wrapper[4]) != Some("1")
        {
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
        // Та же измеренная идентичность 1/7 уже используется старой обработкой.
        let id = legacy_data_processor::identity(&typed[1])?;
        let types = declaration(&typed[2])?;
        if fields.iter().any(|(seen, _, _)| seen == &id)
            || descriptors.iter().filter(|d| d.object_guid == id).count() != 1
        {
            return None;
        }
        fields.push((id, types, body));
    }
    // Не публикуем декларации первых строк до проверки всей коллекции.
    let ids = fields.iter().map(|(id, _, _)| id.clone()).collect();
    for (id, types, body) in fields {
        let descriptor = descriptors.iter_mut().find(|d| d.object_guid == id)?;
        descriptor.declared_types = Some(types);
        descriptor.fill_from_filling_value = flag(&body[20]);
        descriptor.filling_value = filling(&body[19]);
    }
    Some(ids)
}

pub(super) fn project(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
    let Some(root) = list(value) else {
        return;
    };
    let Some(class) = root.get(1).and_then(list) else {
        return;
    };
    if class.len() != 56
        || atom(&class[0]) != Some("32")
        || !class[1..13].iter().all(|v| guid(v).is_some())
        || !list(&class[13]).is_some_and(|v| v.len() == 2 && atom(&v[0]) == Some("0"))
        || at(value, &[1, 13, 1]).and_then(owner).as_ref() != Some(resource)
    {
        return;
    }
    // Позиции и квалификаторы сверены с полной нативной XML, а не с новым ПВХ.
    let settings = (|| {
        Some(CharacteristicSettings {
            code_length: class[21].as_u32()?,
            description_length: class[23].as_u32()?,
            value_types: declaration(&class[18])?,
        })
    })();
    let Some(settings) = settings else {
        return;
    };
    let ids = attributes(root, descriptors);
    if let Some(descriptor) = descriptors.iter_mut().find(|d| &d.object_guid == resource) {
        descriptor.characteristic_settings = Some(settings);
        descriptor.characteristic_attributes = ids;
    }
}
