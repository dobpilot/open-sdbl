//! Tests of the `access` module on the fixtures under `tests/fixtures/access`
//! of the library crate.

use std::path::PathBuf;
use std::str::FromStr;

use open_sdbl::metadata::{
    LiveColumn, LiveIndex, LiveTable, parse_config_descriptors, parse_db_names,
    parse_schema_storage, resolve_metadata,
};
use open_sdbl::query::{FieldUsage, FieldUsageRequest, FieldUse};

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/access")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

const ROLE_GUIDS: [&str; 4] = [
    "76702e9e-fa7a-4b98-befa-f9b37db2dae0",
    "849f034e-85dc-4515-aae6-240c1e0d46d9",
    "e960b3eb-ad6f-4804-b318-acbcdc4b8f98",
    "6b6566ca-81d0-4376-93d8-290301f2b00f",
];

fn catalog() -> RoleCatalog {
    let guids = ROLE_GUIDS.map(|guid| Guid::from_str(guid).unwrap());
    let mut descriptors = Vec::new();
    for guid in &guids {
        descriptors.extend(
            parse_config_descriptors(guid.as_str(), &fixture(&format!("unf_{guid}.deflate")))
                .unwrap(),
        );
    }
    RoleCatalog::from_descriptors(&guids, &descriptors)
}

/// The `v8users` rows of the УНФ demo as the users statement answers them.
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

#[test]
fn decodes_users_from_rows_and_lists_them() {
    let users = users_from_rows(&user_rows()).unwrap();
    assert!(users.iter().all(|user| user.email.is_empty()));
    // The eighth column carries the e-mail on a current platform.
    let mut with_email = user_rows();
    for row in &mut with_email {
        row.push(Cell::Text("user@example.org".to_owned()));
    }
    let with_email = users_from_rows(&with_email).unwrap();
    assert!(
        with_email
            .iter()
            .all(|user| user.email == "user@example.org")
    );
    assert!(list_users(&with_email, &catalog()).contains("\tuser@example.org\tyes\t"));
    assert!(describe_user(&with_email[0], &catalog()).contains("email: user@example.org\n"));
    assert_eq!(users.len(), 4);
    let catalog = catalog();
    let listing = list_users(&users, &catalog);
    assert!(listing.starts_with("name\tdescription\tos login\temail\tshow\tauth\tadmin\troles\n"));
    assert!(
        listing.contains("Абдулов (директор)\tАбдулов Юрий Владимирович\t\t\tyes\tyes\tyes\t3\n"),
        "{listing}"
    );
    assert!(listing.ends_with("# 4 users\n"));

    let director = users
        .iter()
        .find(|user| user.name == "Абдулов (директор)")
        .unwrap();
    let description = describe_user(director, &catalog);
    assert!(description.contains("roles (3):\n"), "{description}");
    assert!(description.contains("  АдминистраторСистемы\tАдминистратор системы\n"));
    assert!(description.contains("  ПолныеПрава\t"));
}

#[test]
fn lists_roles_with_a_filter() {
    let catalog = catalog();
    let all = list_roles(&catalog, None);
    assert!(all.ends_with("# 4 roles\n"), "{all}");
    let filtered = list_roles(&catalog, Some("полные"));
    assert!(filtered.starts_with("ПолныеПрава\t"), "{filtered}");
    assert!(filtered.ends_with("# 1 roles\n"));
}

#[test]
fn assembles_rights_resources_from_rows() {
    let guid = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    let bytes = fixture(&format!("unf_{guid}.0.deflate"));
    let (head, tail) = bytes.split_at(bytes.len() / 2);
    let rows = vec![
        vec![
            Cell::Text(format!("{guid}.0")),
            Cell::Number("0".to_owned()),
            Cell::Bytes(head.to_vec()),
        ],
        vec![
            Cell::Text(format!("{guid}.0")),
            Cell::Number("1".to_owned()),
            Cell::Bytes(tail.to_vec()),
        ],
    ];
    let decoded = role_rights_from_rows(&rows).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].0, guid);
    assert!(!decoded[0].1.objects.is_empty());

    let odd = vec![vec![
        Cell::Text("root".to_owned()),
        Cell::Number("0".to_owned()),
        Cell::Bytes(Vec::new()),
    ]];
    assert!(role_rights_from_rows(&odd).is_err());
}

#[test]
fn describes_a_role_without_metadata_by_identifier() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let guid = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    let rights = parse_role_rights(&fixture(&format!("unf_{guid}.0.deflate"))).unwrap();
    let catalog = catalog();
    let role = catalog.by_guid(&guid).unwrap();
    let text = describe_role(role, &rights, &snapshot, None).unwrap();
    assert!(text.starts_with("role: АдминистраторСистемы\t"), "{text}");
    assert!(text.contains("objects;"), "{text}");
    // An object the snapshot does not know is named by its identifier.
    assert!(text.contains("-"), "{text}");
    let unknown = describe_role(role, &rights, &snapshot, Some("Справочник.Нет"));
    assert!(unknown.is_err());
}

#[test]
fn lists_the_restrictions_of_the_rights_read() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let mut store = AccessStore::new(&snapshot);
    // Nothing read yet: the listing names the commands that read rights.
    let empty = list_restrictions(&store, &snapshot);
    assert!(empty.ends_with("read them\n"), "{empty}");
    assert!(empty.contains("no rights read yet"), "{empty}");

    // A role of the УНФ demo that restricts nothing.
    let unrestricting = Guid::from_str(ROLE_GUIDS[0]).unwrap();
    store.insert_rights(
        unrestricting.clone(),
        parse_role_rights(&fixture(&format!("unf_{unrestricting}.0.deflate"))).unwrap(),
    );
    let listing = list_restrictions(&store, &snapshot);
    assert!(
        listing.starts_with("role\tobject\tright\ttexts\n"),
        "{listing}"
    );
    assert!(
        listing.ends_with("# 0 restrictions in 0 of 1 roles read\n"),
        "{listing}"
    );

    // A role of БП 3.0 that restricts reading of one catalog; the УНФ
    // catalog does not name it, so it stands by its identifier.
    let restricting = "feadebe9-a90e-48b0-a89f-e1f5d4e23041";
    store.insert_rights(
        Guid::from_str(restricting).unwrap(),
        parse_role_rights(&fixture(&format!("buh_{restricting}.0.deflate"))).unwrap(),
    );
    let listing = list_restrictions(&store, &snapshot);
    assert!(
        listing.contains(&format!(
            "{restricting}\td04d020d-c006-49a9-a8fe-788954f09f8d\tЧтение\t1\n"
        )),
        "{listing}"
    );
    let lines = listing.lines().count() - 2;
    assert!(
        listing.ends_with(&format!("# {lines} restrictions in 1 of 2 roles read\n")),
        "{listing}"
    );
    // Reading and changing the same object are two lines.
    assert!(
        listing.contains("e5fe2cdc-ea80-4667-a204-f7e4dff144e2\tИзменение\t1\n"),
        "{listing}"
    );
}

#[test]
fn collapses_a_condition_to_one_line_outside_its_strings() {
    assert_eq!(one_line("  Поле\n  =\t1  "), "Поле = 1");
    assert_eq!(
        one_line("СтрСодержит(П, \",  А  Б ,\") И Х"),
        "СтрСодержит(П, \",  А  Б ,\") И Х"
    );
    assert_eq!(one_line(""), "");
}

#[test]
fn derives_nothing_without_a_current_user() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let mut store = AccessStore::new(&snapshot);
    let guid = Guid::from_str("feadebe9-a90e-48b0-a89f-e1f5d4e23041").unwrap();
    store.insert_rights(
        guid,
        parse_role_rights(&fixture(
            "buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
        ))
        .unwrap(),
    );
    let session = SessionParameters::new();
    let (derived, failures) = role_restrictions(&store, &snapshot, &session);
    assert!(derived.is_empty() && failures.is_empty());

    let mut restrictions = crate::restrict::RestrictionStore::new();
    let report = derive_restrictions(&store, &snapshot, &session, &mut restrictions);
    assert_eq!(report, "0 restrictions derived into \\restrict.\n");
}

#[test]
fn shows_the_restriction_templates_of_a_role() {
    let guid = Guid::from_str("feadebe9-a90e-48b0-a89f-e1f5d4e23041").unwrap();
    let rights = parse_role_rights(&fixture(&format!("buh_{guid}.0.deflate"))).unwrap();
    let catalog = catalog();
    let role = catalog.roles()[0].clone();
    let listing = describe_templates(&role, &rights, None).unwrap();
    assert!(
        listing.contains("ДляРегистра(Регистр, Поле1, Поле2, Поле3, Поле4, Поле5)\t"),
        "{listing}"
    );
    assert!(listing.contains("ПоЗначениям()\t"), "{listing}");
    // Every template of the role parses.
    assert_eq!(listing.matches("\tok\n").count(), 4, "{listing}");
    assert!(listing.ends_with("# 4 templates\n"), "{listing}");

    let body = describe_templates(&role, &rights, Some("длярегистра")).unwrap();
    assert!(
        body.contains("template: ДляРегистра(Регистр, Поле1"),
        "{body}"
    );
    assert!(body.contains("#Регистр"), "{body}");
    // The outline stands before the body.
    assert!(body.contains("\nbranches: "), "{body}");
    assert!(body.contains("\ncalls: "), "{body}");
    assert!(body.contains("Регистр"), "{body}");

    let unknown = describe_templates(&role, &rights, Some("Нет")).unwrap_err();
    assert!(
        unknown.to_string().contains("carries no template"),
        "{unknown}"
    );

    // The signatures close the role description.
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let described = describe_role(&role, &rights, &snapshot, None).unwrap();
    assert!(
        described.contains("\n  ДляОбъекта(ПолеОбъекта)\n"),
        "{described}"
    );
}

#[test]
fn a_role_the_base_carries_no_rights_of_is_asked_for_once() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let mut store = AccessStore::new(&snapshot);
    let guid = Guid::from_str("262144f7-02b6-4906-89fd-297cc72fe383").unwrap();
    let roles = std::slice::from_ref(&guid);
    assert_eq!(store.missing_rights(roles), vec![guid.clone()]);
    // Once the base answers nothing for it, it is not asked for again.
    store.unreadable.push(guid.clone());
    assert!(store.missing_rights(roles).is_empty());
    assert!(store.rights_of(roles).is_empty());
    // The configuration does not declare it, so it stands by identifier.
    assert_eq!(store.role_name(&guid), guid.as_str());
    assert!(
        missing_rights("ЧтениеЭД")
            .to_string()
            .contains("is not in Config")
    );
}

#[test]
fn decisions_refuse_to_answer_without_a_current_user() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let store = AccessStore::new(&snapshot);
    let error = user_decisions(
        &store,
        &snapshot,
        &open_sdbl::query::RestrictionRequest::default(),
        &[],
        &SessionParameters::new(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("needs a current user"),
        "no user must never read as permission: {error}"
    );
}

#[test]
fn decisions_refuse_to_answer_for_a_role_whose_rights_are_unread() {
    let snapshot = crate::enumeration_snapshot_support::enumeration_snapshot();
    let mut store = AccessStore::new(&snapshot);
    let user = users_from_rows(&user_rows())
        .unwrap()
        .into_iter()
        .find(|user| !user.data.roles.is_empty())
        .expect("the fixture has a user with roles");
    store.set_current_user(Some(user));
    let error = user_decisions(
        &store,
        &snapshot,
        &open_sdbl::query::RestrictionRequest::default(),
        &[],
        &SessionParameters::new(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("are unread"),
        "an unread role must never read as permission: {error}"
    );
}

fn field_snapshot() -> MetadataSnapshot {
    let hex = crate::hex_test_support::hex;
    let db_names = parse_db_names(&hex(
        "0dcab10d03310800c05d5c83f46f631b16c800d9003054518a6f2def9e5c7dbbc23636f5390c5d6e435a9391755e98a94d92570c2128efc878e2eb51a0b703bb769761ab61aa13528d441bd271928b26b5ce62505e9ff5ff74ce0f",
    ))
    .unwrap();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex("4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f"),
    )
    .unwrap();
    let schema = parse_schema_storage(
        br#"{0,{1,{"Reference53","N",53,"",{4,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Date_Time",0,{1,{"T",0,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0}},{0},{1,{"Code",1,{2,"Code","ID"},0,0,0,{0},0}},1,"R",{0},{0},"",0}}}"#,
    )
    .unwrap();
    let live = LiveTable {
        name: "_reference53".to_owned(),
        columns: ["_idrref", "_code", "_date_time", "_fld54"]
            .into_iter()
            .map(|name| LiveColumn {
                name: name.to_owned(),
                data_type: "bytea".to_owned(),
            })
            .collect(),
        indexes: vec![LiveIndex {
            name: "_reference53_2".to_owned(),
            columns: vec!["_code".to_owned(), "_idrref".to_owned()],
            unique: true,
        }],
    };
    resolve_metadata(db_names, descriptors, schema, vec![live]).snapshot
}

fn field_fixture() -> (MetadataSnapshot, Guid, RoleRights, FieldUse, FieldUse) {
    let snapshot = field_snapshot();
    let object = snapshot
        .objects()
        .iter()
        .find(|object| object.name.as_deref() == Some("OpenSdblMetadataProbe"))
        .expect("the probe catalog exists");
    let field = snapshot
        .fields()
        .iter()
        .find(|field| field.name.as_deref() == Some("ProbeAttribute"))
        .expect("the probe attribute exists");
    let role = Guid::from_str("feadebe9-a90e-48b0-a89f-e1f5d4e23041").unwrap();
    let mut rights = parse_role_rights(&fixture(&format!("buh_{role}.0.deflate"))).unwrap();
    let mut object_rights = rights.objects[0].clone();
    object_rights.object = object.guid.clone();
    object_rights.members.clear();
    object_rights
        .rights
        .retain(|entry| entry.right == Right::Read);
    object_rights.rights[0].allowed = true;
    rights.objects = vec![object_rights];
    rights.set_for_new_objects = false;
    rights.set_for_attributes_by_default = true;
    let object_id = ObjectId::from(&object.guid);
    let attribute_id = open_sdbl::metadata::AttributeId::from(&field.guid);
    (
        snapshot,
        role,
        rights,
        FieldUse {
            object: object_id,
            table_part: None,
            field: FieldId::Metadata(attribute_id),
            usage: FieldUsage::Filter,
        },
        FieldUse {
            object: object_id,
            table_part: None,
            field: FieldId::Standard(open_sdbl::metadata::StandardFieldId::Code),
            usage: FieldUsage::Projection,
        },
    )
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

#[test]
fn field_check_requires_selected_user_and_every_roles_rights() {
    let (snapshot, role, rights, field, _) = field_fixture();
    let request = FieldUsageRequest {
        fields: vec![field],
    };
    let store = AccessStore::new(&snapshot);
    assert!(check_user_field_access(&store, &snapshot, &FieldUsageRequest::default()).is_err());
    assert!(
        check_user_field_access(&store, &snapshot, &request)
            .unwrap_err()
            .to_string()
            .contains("needs a current user")
    );
    let mut store = selected_store(&snapshot, &role, rights);
    let missing = Guid::from_str(ROLE_GUIDS[1]).unwrap();
    store.current.as_mut().unwrap().data.roles.push(missing);
    assert!(
        check_user_field_access(&store, &snapshot, &request)
            .unwrap_err()
            .to_string()
            .contains("are unread")
    );
}

#[test]
fn field_check_rejects_an_explicitly_denied_filter_and_accepts_a_second_grant() {
    let (snapshot, role, mut rights, filter, _) = field_fixture();
    let object = snapshot.object_by_id(filter.object).unwrap().guid.clone();
    let FieldId::Metadata(attribute) = filter.field else {
        unreachable!()
    };
    let field = snapshot.attribute_by_id(attribute).unwrap().guid.clone();
    rights.objects.push(denied_field_entry(&object, &field));
    let mut store = selected_store(&snapshot, &role, rights.clone());
    let request = FieldUsageRequest {
        fields: vec![filter.clone()],
    };
    assert!(
        check_user_field_access(&store, &snapshot, &request)
            .unwrap_err()
            .to_string()
            .contains("no role grants")
    );

    let second_role = Guid::from_str(ROLE_GUIDS[1]).unwrap();
    store
        .current
        .as_mut()
        .unwrap()
        .data
        .roles
        .push(second_role.clone());
    rights.objects.last_mut().unwrap().rights[0].allowed = true;
    store.insert_rights(second_role, rights);
    check_user_field_access(&store, &snapshot, &request).unwrap();
}

#[test]
fn standard_and_section_fields_fail_closed_when_member_mapping_is_unknown() {
    let (snapshot, role, mut rights, filter, mut standard) = field_fixture();
    let object = snapshot.object_by_id(filter.object).unwrap().guid.clone();
    let FieldId::Metadata(attribute) = filter.field else {
        unreachable!()
    };
    let field = snapshot.attribute_by_id(attribute).unwrap().guid.clone();
    let store = selected_store(&snapshot, &role, rights.clone());
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![standard.clone()],
            },
        )
        .unwrap_err()
        .to_string()
        .contains("cannot be mapped")
    );
    rights.objects.push(denied_field_entry(&object, &field));
    let store = selected_store(&snapshot, &role, rights);
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![standard.clone()]
            },
        )
        .unwrap_err()
        .to_string()
        .contains("cannot be mapped")
    );
    standard.table_part = Some("Items".to_owned());
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![standard]
            },
        )
        .unwrap_err()
        .to_string()
        .contains("tabular-section")
    );
}

#[test]
fn an_attribute_without_an_exact_grant_stays_denied_despite_the_default_flag() {
    let (snapshot, role, mut rights, field, _) = field_fixture();
    assert!(rights.set_for_attributes_by_default);
    let store = selected_store(&snapshot, &role, rights.clone());
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![field.clone()],
            },
        )
        .unwrap_err()
        .to_string()
        .contains("no role grants")
    );

    let object = snapshot.object_by_id(field.object).unwrap().guid.clone();
    let FieldId::Metadata(attribute) = field.field else {
        unreachable!()
    };
    let field_guid = snapshot.attribute_by_id(attribute).unwrap().guid.clone();
    let mut exact = denied_field_entry(&object, &field_guid);
    exact.rights[0].allowed = true;
    rights.objects.push(exact);
    let store = selected_store(&snapshot, &role, rights);
    check_user_field_access(
        &store,
        &snapshot,
        &FieldUsageRequest {
            fields: vec![field],
        },
    )
    .unwrap();
}

#[test]
fn missing_rls_parameter_keeps_its_name_through_both_database_apis() {
    let (snapshot, role, mut rights, field, _) = field_fixture();
    rights.objects[0].rights[0].restrictions[0].condition =
        "#Если &НужноеЗначение #Тогда ТекущаяТаблица ГДЕ Ссылка = Ссылка #КонецЕсли".to_owned();
    let store = selected_store(&snapshot, &role, rights);
    let request = RestrictionRequest {
        targets: vec![RestrictionTarget {
            object: field.object,
            table_part: None,
        }],
    };
    for error in [
        user_decisions(&store, &snapshot, &request, &[], &SessionParameters::new()).unwrap_err(),
        user_restrictions(&store, &snapshot, &request, &[], &SessionParameters::new()).unwrap_err(),
    ] {
        assert_eq!(error.missing_session_parameter(), Some("НужноеЗначение"));
        assert!(matches!(error, DbError::MissingSessionParameter { .. }));
    }
}

fn constant_field_fixture() -> (MetadataSnapshot, Guid, RoleRights, FieldUse) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/separators");
    let db_names = parse_db_names(&std::fs::read(root.join("db_names.deflate")).unwrap()).unwrap();
    let mut descriptors = Vec::new();
    for entry in std::fs::read_dir(root.join("config")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap();
        descriptors.extend(parse_config_descriptors(name, &std::fs::read(&path).unwrap()).unwrap());
    }
    let schema =
        parse_schema_storage(&std::fs::read(root.join("schema_storage.txt")).unwrap()).unwrap();
    let snapshot = resolve_metadata(db_names, descriptors, schema, Vec::new()).snapshot;
    let object = snapshot
        .objects()
        .iter()
        .find(|object| object.kind == Some(MetadataKind::Constant))
        .unwrap();
    let object_id = ObjectId::from(&object.guid);
    let field = snapshot
        .field_id(object_id, object.name.as_deref().unwrap())
        .unwrap();
    let (_, role, mut rights, _, _) = field_fixture();
    rights.objects[0].object = object.guid.clone();
    (
        snapshot,
        role,
        rights,
        FieldUse {
            object: object_id,
            table_part: None,
            field,
            usage: FieldUsage::Projection,
        },
    )
}

#[test]
fn the_constant_value_uses_object_read_independently_of_attribute_defaults() {
    for default in [false, true] {
        let (snapshot, role, mut rights, field) = constant_field_fixture();
        rights.set_for_attributes_by_default = default;
        let store = selected_store(&snapshot, &role, rights);
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![field],
            },
        )
        .unwrap();
    }
}

#[test]
fn a_constant_value_does_not_bypass_an_object_read_refusal() {
    let (snapshot, role, mut rights, field) = constant_field_fixture();
    rights.objects[0].rights[0].allowed = false;
    let store = selected_store(&snapshot, &role, rights);
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![field]
            }
        )
        .is_err()
    );
}

#[test]
fn another_role_can_grant_the_constant_value() {
    let (snapshot, role, mut rights, field) = constant_field_fixture();
    let second = Guid::from_str("12345678-1234-1234-1234-123456789abc").unwrap();
    let grant = rights.clone();
    rights.objects[0].rights[0].allowed = false;
    let mut store = selected_store(&snapshot, &role, rights);
    let mut user = store.current_user().unwrap().clone();
    user.data.roles.push(second.clone());
    store.set_current_user(Some(user));
    store.insert_rights(second, grant);
    check_user_field_access(
        &store,
        &snapshot,
        &FieldUsageRequest {
            fields: vec![field],
        },
    )
    .unwrap();
}

#[test]
fn constant_values_still_reject_unread_roles_and_mismatched_identifiers() {
    let (snapshot, role, rights, mut field) = constant_field_fixture();
    let mut store = selected_store(&snapshot, &role, rights);
    let mut user = store.current_user().unwrap().clone();
    user.data
        .roles
        .push(Guid::from_str("12345678-1234-1234-1234-123456789abc").unwrap());
    store.set_current_user(Some(user));
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![field.clone()]
            }
        )
        .is_err()
    );
    user = store.current_user().unwrap().clone();
    user.data.roles.truncate(1);
    store.set_current_user(Some(user));
    field.field = FieldId::Metadata(open_sdbl::metadata::AttributeId::from(
        &Guid::from_str("12345678-1234-1234-1234-123456789abc").unwrap(),
    ));
    assert!(
        check_user_field_access(
            &store,
            &snapshot,
            &FieldUsageRequest {
                fields: vec![field]
            }
        )
        .is_err()
    );
}

fn verified_catalog_snapshot() -> MetadataSnapshot {
    use open_sdbl::metadata::{DbNameEntry, DbNames};
    let owner: Guid = "92876c9e-52ff-480d-9878-5766f4f9d12c".parse().unwrap();
    let attribute: Guid = "b729d739-5249-4765-85ee-fd792bb22911".parse().unwrap();
    let db_names = DbNames::from_entries(vec![
        DbNameEntry {
            guid: owner.clone(),
            alias: "Reference".into(),
            number: 53,
        },
        DbNameEntry {
            guid: attribute,
            alias: "Fld".into(),
            number: 54,
        },
    ]);
    let descriptors =
        parse_config_descriptors(owner.as_str(), &fixture("native-096/catalog.deflate")).unwrap();
    let schema=parse_schema_storage(br#"{0,{1,{"Reference53","N",53,"",{6,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Description",0,{1,{"S",2147483657,0,"",0}},"",0},{"Marked",0,{1,{"B",16,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0},{"Date_Time",0,{1,{"T",0,0,"",0}},"",0}},{0},{0},1,"R",{0},{0},"",0}}}"#).unwrap();
    resolve_metadata(
        db_names,
        descriptors,
        schema,
        vec![LiveTable {
            name: "_reference53".into(),
            columns: [
                "_idrref",
                "_code",
                "_description",
                "_marked",
                "_fld54",
                "_date_time",
            ]
            .into_iter()
            .map(|name| LiveColumn {
                name: name.into(),
                data_type: "bytea".into(),
            })
            .collect(),
            indexes: Vec::new(),
        }],
    )
    .snapshot
}

#[test]
fn verified_catalog_programmatic_reads_follow_object_read_without_ui_defaults() {
    let snapshot = verified_catalog_snapshot();
    let (_, role, mut rights, _, _) = field_fixture();
    let owner: Guid = "92876c9e-52ff-480d-9878-5766f4f9d12c".parse().unwrap();
    rights.objects[0].object = owner.clone();
    let fields = [
        "Ссылка",
        "Код",
        "Наименование",
        "ПометкаУдаления",
        "Недействителен",
    ]
    .into_iter()
    .map(|name| FieldUse {
        object: ObjectId::from(&owner),
        table_part: None,
        field: snapshot
            .field_id(ObjectId::from(&owner), name)
            .unwrap_or_else(|error| panic!("{name}: {error:?}")),
        usage: FieldUsage::Projection,
    })
    .collect::<Vec<_>>();
    let request = FieldUsageRequest {
        fields: fields.clone(),
    };
    for defaults in [false, true] {
        rights.set_for_attributes_by_default = defaults;
        check_user_field_access(
            &selected_store(&snapshot, &role, rights.clone()),
            &snapshot,
            &request,
        )
        .unwrap();
    }
    let FieldId::Metadata(id) = fields[4].field else {
        panic!()
    };
    let attr = snapshot.attribute_by_id(id).unwrap().guid.clone();
    let mut view = denied_field_entry(&owner, &attr);
    view.rights[0].right = Right::View;
    rights.objects.push(view);
    check_user_field_access(
        &selected_store(&snapshot, &role, rights.clone()),
        &snapshot,
        &request,
    )
    .unwrap();
    rights.objects[0].rights[0].allowed = false;
    assert!(
        check_user_field_access(
            &selected_store(&snapshot, &role, rights.clone()),
            &snapshot,
            &request
        )
        .is_err()
    );
}

#[test]
fn verified_catalog_still_denies_explicit_member_read_and_unsupported_standard_fields() {
    let snapshot = verified_catalog_snapshot();
    let (_, role, mut rights, _, _) = field_fixture();
    let owner: Guid = "92876c9e-52ff-480d-9878-5766f4f9d12c".parse().unwrap();
    rights.objects[0].object = owner.clone();
    let custom = snapshot
        .field_id(ObjectId::from(&owner), "Недействителен")
        .unwrap();
    let FieldId::Metadata(id) = custom else {
        panic!()
    };
    rights.objects.push(denied_field_entry(
        &owner,
        &snapshot.attribute_by_id(id).unwrap().guid,
    ));
    for field in [
        custom,
        FieldId::Standard(open_sdbl::metadata::StandardFieldId::Date),
    ] {
        let request = FieldUsageRequest {
            fields: vec![FieldUse {
                object: ObjectId::from(&owner),
                table_part: None,
                field,
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

#[path = "field_read_0923.rs"]
mod field_read_0923;

#[cfg(test)]
#[path = "user_language.rs"]
mod user_language;
