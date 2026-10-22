//! Configuration-level candidates, common attributes and functional options.

use super::{
    CommonAttributeCandidate, CommonAttributeConditionalSeparation, CommonAttributeContentEntry,
    CommonAttributeContentUse, CommonAttributeRuntimeContentEntry, CommonAttributeSeparation,
    CommonAttributeUsage, CommonAttributeUse, ConfigDescriptor, FunctionalOptionLocation, Guid,
    Value, at, atom, collection, flag, guid, header, legacy_data_processor, list, owner_reference,
};

// Нативные CF/XML подтверждают оболочку ссылки состава и внутренний
// список владельцев; неизвестные формы не дают частичного результата.
pub(super) fn functional_option_content(value: &Value) -> Option<()> {
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
pub(super) fn project_legacy_functional_option(
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

pub(super) fn common_usage(class: &[Value]) -> Option<CommonAttributeUsage> {
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

pub(super) fn project_configuration(value: &Value, descriptors: &mut [ConfigDescriptor]) {
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

pub(in crate::metadata) fn resolve_functional_options(
    descriptors: &mut [ConfigDescriptor],
    db_names: &crate::metadata::DbNames,
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
            if field.field_purpose
                == Some(crate::metadata::ConfigFieldPurpose::InformationRegisterResource)
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

pub(super) fn common_candidates(value: &Value) -> Option<Vec<CommonAttributeCandidate>> {
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

pub(in crate::metadata) fn resolve_common_attributes(
    descriptors: &mut [ConfigDescriptor],
    db_names: &crate::metadata::DbNames,
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

pub(super) const COMMON_ROOT_COLLECTIONS: [&[&str]; 7] = [
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
