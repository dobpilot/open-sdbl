//! Measured report, processor, register and reference object families.

use super::{
    AccumulationRegisterKind, BusinessProcessNumberSettings,
    BusinessProcessStandardAttributeFillChecking, CatalogCodeType, ConfigDescriptor, Guid, Value,
    at, atom, attribute_bodies, attribute_fill_checking, collection, complete_attributes,
    complete_object_sections, declaration, flag, guid, header, legacy_data_processor,
    legacy_data_processor_attributes, list, membership, object_header, section_body,
    standard_fill_checking_at,
};

// Парные нативные CF/XML подтверждают отчёт 19/18 и обработку 17/12.
// Старая обработка 16/12 сохраняет свой отдельно измеренный заголовок.
pub(super) fn report_class<'a>(value: &'a Value, resource: &Guid) -> Option<&'a [Value]> {
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

pub(super) fn processor_class<'a>(
    value: &'a Value,
    resource: &Guid,
    legacy: bool,
) -> Option<&'a [Value]> {
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

pub(super) fn legacy_enum_header(value: &Value) -> Option<Guid> {
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

pub(super) fn legacy_enum_reference(
    root: &[Value],
    class: &[Value],
    resource: &Guid,
) -> Option<Guid> {
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

// Целая нативная CF/XML-пара подтверждает только Boolean-константу класса
// 14/12. Проверяем локальную форму, не расширяя общие заголовки и грамматику.
pub(super) fn project_legacy_boolean_constant(
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

pub(super) fn project_data_processor(
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

pub(super) fn project_exchange_plan(
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

pub(super) fn project_report(value: &Value, resource: &Guid, descriptors: &mut [ConfigDescriptor]) {
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

pub(super) fn project_business_process(
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

pub(super) fn project_accumulation_register(
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

// Полная измеренная форма общая для подробного и декларационного путей.
pub(super) fn business_reference(class: &[Value], resource: &Guid) -> Option<Guid> {
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
