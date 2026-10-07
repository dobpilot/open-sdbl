//! Проверки нативного сопоставления прав регистра накопления.
use open_sdbl::metadata::*;
use open_sdbl::query::*;
use open_sdbl_db::access::*;
use open_sdbl_db::{Cell, QueryRows};
use std::{path::PathBuf, str::FromStr};
const ROLE_GUIDS: [&str; 1] = ["76702e9e-fa7a-4b98-befa-f9b37db2dae0"];
fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
fn user_rows() -> QueryRows {
    let text = String::from_utf8(fixture("unf_v8users.tsv")).unwrap();
    text.lines()
        .map(|line| {
            let columns = line.split('|').collect::<Vec<_>>();
            let flag = |index: usize| {
                Cell::Number(if matches!(columns[index], "t" | "true" | "1") {
                    "1".to_owned()
                } else {
                    "0".to_owned()
                })
            };
            let data = (0..columns[7].len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&columns[7][at..at + 2], 16).unwrap())
                .collect::<Vec<_>>();
            vec![
                Cell::Text(columns[0].to_owned()),
                Cell::Text(columns[1].to_owned()),
                Cell::Text(columns[2].to_owned()),
                flag(4),
                flag(5),
                flag(6),
                Cell::Bytes(data),
            ]
        })
        .collect()
}
fn selected_store(snapshot: &MetadataSnapshot, role: &Guid, rights: RoleRights) -> AccessStore {
    let mut store = AccessStore::new(snapshot);
    let mut user = users_from_rows(&user_rows()).unwrap().remove(0);
    user.data.roles = vec![role.clone()];
    store.set_current_user(Some(user));
    store.insert_rights(role.clone(), rights);
    store
}
fn denied_field_entry(object: &Guid, field: &Guid) -> open_sdbl::metadata::ObjectRights {
    let template_role = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    let template = parse_role_rights(&fixture(&format!("unf_{template_role}.0.deflate"))).unwrap();
    let mut entry = template
        .objects
        .into_iter()
        .find(|entry| !entry.members.is_empty())
        .expect("the measured rights fixture has member entries");
    entry.object = object.clone();
    entry.members.truncate(1);
    entry.members[0].id = field.clone();
    let mut read = parse_role_rights(&fixture(
        "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ))
    .unwrap()
    .objects[0]
        .right(&Right::Read)
        .unwrap()
        .clone();
    read.allowed = false;
    read.restrictions.clear();
    entry.rights = vec![read];
    entry
}

fn snapshot_with_members(mode: u8) -> MetadataSnapshot {
    let owner: Guid = "1b9be124-96ea-4346-8f4b-c8b295679d78".parse().unwrap();
    let mut ds = parse_config_descriptors(
        owner.as_str(),
        include_bytes!("../../../tests/fixtures/runtime-object-099/ЗамерОстатков.deflate"),
    )
    .unwrap();
    let d = ds.iter().find(|d| d.object_guid == owner).unwrap();
    let fields = d
        .accumulation_register_dimensions
        .as_ref()
        .unwrap()
        .iter()
        .chain(d.accumulation_register_resources.as_ref().unwrap())
        .cloned()
        .collect::<Vec<_>>();
    if mode == 1 {
        ds.iter_mut()
            .find(|d| d.object_guid == owner)
            .unwrap()
            .accumulation_register_dimensions = None;
    }
    if mode == 2 {
        ds.iter_mut()
            .find(|d| d.object_guid == owner)
            .unwrap()
            .accumulation_register_kind = Some(AccumulationRegisterKind::Turnovers);
    }
    if mode == 3 {
        for d in &mut ds {
            if fields.contains(&d.object_guid) {
                d.declared_types = Some(vec![]);
            }
        }
    }
    if mode == 4 {
        for d in &mut ds {
            if fields.contains(&d.object_guid) {
                d.declared_types = None;
            }
        }
    }
    let mut names = vec![DbNameEntry {
        guid: owner,
        alias: "AccumRg".into(),
        number: 1,
    }];
    names.extend(fields.iter().enumerate().map(|(i, g)| DbNameEntry {
        guid: g.clone(),
        alias: "Fld".into(),
        number: i as u32 + 2,
    }));
    let columns = fields
        .iter()
        .enumerate()
        .map(|(i, _)| {
            format!(
                "{{\"Fld{}\",0,{{1,{{\"S\",2147483657,0,\"\",0}}}},\"\",0}}",
                i + 2
            )
        })
        .collect::<Vec<_>>();
    let schema=parse_schema_storage(format!("{{0,{{1,{{\"AccumRg1\",\"N\",1,\"\",{{{},{}}},{{0}},{{0}},1,\"R\",{{0}},{{0}},\"\",0}}}}}}",columns.len(),columns.join(",")).as_bytes()).unwrap();
    resolve_metadata(DbNames::from_entries(names), ds, schema, vec![]).snapshot
}
#[test]
fn native_dimensions_and_resources_follow_owner_read_not_view_or_defaults() {
    let snapshot = snapshot_with_members(0);
    let owner = snapshot
        .objects()
        .iter()
        .find(|o| o.kind == Some(MetadataKind::AccumulationRegister))
        .unwrap();
    let role: Guid = ROLE_GUIDS[0].parse().unwrap();
    let mut rights = parse_role_rights(&fixture(
        "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ))
    .unwrap();
    let mut grant = rights
        .objects
        .iter()
        .find(|o| o.right(&Right::Read).is_some())
        .unwrap()
        .clone();
    grant.object = owner.guid.clone();
    grant.members.clear();
    grant.rights.retain(|r| r.right == Right::Read);
    grant.rights[0].allowed = true;
    grant.rights[0].restrictions.clear();
    rights.objects = vec![grant];
    rights.set_for_new_objects = false;
    let fields = snapshot
        .fields()
        .iter()
        .map(|f| FieldUse {
            object: ObjectId::from(&owner.guid),
            table_part: None,
            field: FieldId::Metadata(AttributeId::from(&f.guid)),
            usage: FieldUsage::Projection,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    let request = FieldUsageRequest { fields };
    for defaults in [false, true] {
        rights.set_for_attributes_by_default = defaults;
        check_user_field_access(
            &selected_store(&snapshot, &role, rights.clone()),
            &snapshot,
            &request,
        )
        .unwrap();
    }
    assert!(check_user_field_access(&AccessStore::new(&snapshot), &snapshot, &request).is_err());
    let FieldId::Metadata(id) = request.fields[0].field else {
        panic!()
    };
    let attribute = &snapshot.attribute_by_id(id).unwrap().guid;
    let mut view = denied_field_entry(&owner.guid, attribute);
    view.rights[0].right = Right::View;
    rights.objects.push(view);
    check_user_field_access(
        &selected_store(&snapshot, &role, rights.clone()),
        &snapshot,
        &request,
    )
    .unwrap();
    rights.objects[0].rights[0].allowed = false;
    for field in &request.fields {
        let single = FieldUsageRequest {
            fields: vec![field.clone()],
        };
        assert!(
            check_user_field_access(
                &selected_store(&snapshot, &role, rights.clone()),
                &snapshot,
                &single
            )
            .is_err()
        );
    }
    assert!(
        check_user_field_access(
            &selected_store(&snapshot, &role, rights.clone()),
            &snapshot,
            &request
        )
        .is_err()
    );
    rights.objects[0].rights[0].allowed = true;
    rights
        .objects
        .push(denied_field_entry(&owner.guid, attribute));
    assert!(
        check_user_field_access(
            &selected_store(&snapshot, &role, rights),
            &snapshot,
            &request
        )
        .is_err()
    );
}

#[test]
fn incomplete_native_members_remain_unverified() {
    for mode in 1..=4 {
        let snapshot = snapshot_with_members(mode);
        assert_eq!(snapshot.fields().len(), 2, "mode {mode}");
        let owner = snapshot
            .objects()
            .iter()
            .find(|o| o.kind == Some(MetadataKind::AccumulationRegister))
            .unwrap();
        let role: Guid = ROLE_GUIDS[0].parse().unwrap();
        let mut rights = parse_role_rights(&fixture(
            "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
        ))
        .unwrap();
        let mut grant = rights
            .objects
            .iter()
            .find(|o| o.right(&Right::Read).is_some())
            .unwrap()
            .clone();
        grant.object = owner.guid.clone();
        grant.members.clear();
        grant.rights.retain(|r| r.right == Right::Read);
        grant.rights[0].allowed = true;
        grant.rights[0].restrictions.clear();
        rights.objects = vec![grant];
        rights.set_for_new_objects = false;
        for field in snapshot.fields() {
            let request = FieldUsageRequest {
                fields: vec![FieldUse {
                    object: ObjectId::from(&owner.guid),
                    table_part: None,
                    field: FieldId::Metadata(AttributeId::from(&field.guid)),
                    usage: FieldUsage::Filter,
                }],
            };
            assert!(
                check_user_field_access(
                    &selected_store(&snapshot, &role, rights.clone()),
                    &snapshot,
                    &request
                )
                .is_err()
            );
        }
    }
}

#[test]
fn accumulation_multiple_roles_keep_existing_rls_conditions() {
    let snapshot = snapshot_with_members(0);
    let owner = snapshot
        .objects()
        .iter()
        .find(|o| o.kind == Some(MetadataKind::AccumulationRegister))
        .unwrap();
    let role: Guid = ROLE_GUIDS[0].parse().unwrap();
    let mut rights = parse_role_rights(&fixture(
        "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ))
    .unwrap();
    let mut restriction = rights
        .objects
        .iter()
        .flat_map(|o| &o.rights)
        .flat_map(|r| &r.restrictions)
        .next()
        .unwrap()
        .clone();
    restriction.fields.clear();
    restriction.condition = "ГДЕ Категория = \"b\"".into();
    let mut grant = rights
        .objects
        .iter()
        .find(|o| o.right(&Right::Read).is_some())
        .unwrap()
        .clone();
    grant.object = owner.guid.clone();
    grant.members.clear();
    grant.rights.retain(|r| r.right == Right::Read);
    grant.rights[0].allowed = true;
    grant.rights[0].restrictions = vec![restriction.clone()];
    rights.objects = vec![grant];
    rights.set_for_new_objects = false;
    let mut store = selected_store(&snapshot, &role, rights.clone());
    let request = RestrictionRequest {
        targets: vec![RestrictionTarget {
            object: ObjectId::from(&owner.guid),
            table_part: None,
        }],
    };
    let decisions =
        user_decisions(&store, &snapshot, &request, &[], &SessionParameters::new()).unwrap();
    assert!(
        decisions[0]
            .restriction()
            .unwrap()
            .condition()
            .contains("\"b\"")
    );
    let second: Guid = "849f034e-85dc-4515-aae6-240c1e0d46d9".parse().unwrap();
    let mut user = store.current_user().unwrap().clone();
    user.data.roles.push(second.clone());
    store.set_current_user(Some(user));
    assert!(user_decisions(&store, &snapshot, &request, &[], &SessionParameters::new()).is_err());
    restriction.condition = "ГДЕ Категория = \"a\"".into();
    rights.objects[0].rights[0].restrictions = vec![restriction];
    store.insert_rights(second, rights);
    let decisions =
        user_decisions(&store, &snapshot, &request, &[], &SessionParameters::new()).unwrap();
    let condition = decisions[0].restriction().unwrap().condition();
    assert!(
        condition.contains("ИЛИ") && condition.contains("\"a\"") && condition.contains("\"b\""),
        "{condition}"
    );
}
