use super::*;
use open_sdbl::metadata::{DbNameEntry, DbNames, StandardFieldId};

const REGISTER: &str = "c34d5785-6f04-45c7-9555-088544ddc489";
const DAY: &str = "7db039d6-6333-5b98-8a07-ba8d95ac84b7";
const DOCUMENT: &str = "f77edfd7-d0d4-532b-a649-399fb412f0a4";

fn native_file(folder: &str, name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/field-read-native-0923")
            .join(folder)
            .join(name),
    )
    .unwrap()
}

fn snapshot(
    folder: &str,
    owners: &[&str],
    mutate: impl FnOnce(&mut Vec<open_sdbl::metadata::ConfigDescriptor>),
) -> MetadataSnapshot {
    let mut ds = Vec::new();
    let mut entries = Vec::new();
    let mut tables = Vec::new();
    for (number, owner) in owners.iter().enumerate() {
        let parsed =
            parse_config_descriptors(owner, &native_file(folder, &format!("{owner}.deflate")))
                .unwrap();
        entries.push(DbNameEntry {
            guid: owner.parse().unwrap(),
            alias: if folder == "register" {
                "InfoRg"
            } else {
                "Document"
            }
            .into(),
            number: number as u32 + 1,
        });
        for (index, d) in parsed
            .iter()
            .filter(|d| d.object_guid.as_str() != *owner)
            .enumerate()
        {
            entries.push(DbNameEntry {
                guid: d.object_guid.clone(),
                alias: "Fld".into(),
                number: 100 + number as u32 * 100 + index as u32,
            });
        }
        let mut columns = vec!["ID".to_owned(), "Period".to_owned(), "Date".to_owned()];
        columns.extend(
            entries
                .iter()
                .filter(|e| {
                    e.alias == "Fld"
                        && e.number >= 100 + number as u32 * 100
                        && e.number < 200 + number as u32 * 100
                })
                .map(|e| format!("Fld{}", e.number)),
        );
        tables.push(open_sdbl::metadata::SchemaTable {
            name: format!(
                "{}{}",
                if folder == "register" {
                    "InfoRg"
                } else {
                    "Document"
                },
                number + 1
            ),
            number: number as u32 + 1,
            owner: None,
            inline_name: None,
            columns: columns
                .into_iter()
                .map(|name| open_sdbl::metadata::SchemaColumn {
                    name,
                    types: vec![],
                })
                .collect(),
            indexes: vec![],
        });
        ds.extend(parsed);
    }
    mutate(&mut ds);
    // Номера DBNames здесь синтетические: нативные CF описывают права и состав,
    // а этот тест не выдаёт модельное физическое размещение за замер платформы.
    let live = tables
        .iter()
        .map(|t| LiveTable {
            name: format!("_{}", t.name).to_ascii_lowercase(),
            columns: t
                .columns
                .iter()
                .map(|c| LiveColumn {
                    name: format!("_{}", c.name).to_ascii_lowercase(),
                    data_type: "bytea".into(),
                })
                .collect(),
            indexes: vec![],
        })
        .collect();
    resolve_metadata(
        DbNames::from_entries(entries),
        ds,
        open_sdbl::metadata::SchemaStorage {
            tables,
            anomalies: vec![],
        },
        live,
    )
    .snapshot
}

fn rights(folder: &str, name: &str) -> (Guid, RoleRights) {
    let id = String::from_utf8(native_file(folder, &format!("{name}.role-guid"))).unwrap();
    let id = id.trim();
    (
        id.parse().unwrap(),
        parse_role_rights(&native_file(folder, &format!("{id}.0.deflate"))).unwrap(),
    )
}

fn request(snapshot: &MetadataSnapshot, owner: &str, fields: &[&str]) -> FieldUsageRequest {
    let owner: Guid = owner.parse().unwrap();
    FieldUsageRequest {
        fields: fields
            .iter()
            .map(|name| FieldUse {
                object: ObjectId::from(&owner),
                table_part: None,
                field: snapshot
                    .field_id(ObjectId::from(&owner), name)
                    .unwrap_or_else(|e| panic!("{owner} {name}: {e:?}")),
                usage: FieldUsage::Projection,
            })
            .collect(),
    }
}

#[test]
fn native_register_and_document_owner_read_is_required_for_both_defaults() {
    for (folder, owners, fields, profiles) in [
        (
            "register",
            vec![REGISTER, DAY],
            vec![
                vec!["НомерСтрокиТаблицыРесурсов", "Мощность"],
                vec!["Период", "НомерСтрокиТаблицыРесурсов", "Мощность"],
            ],
            vec![
                ("ReadOwner", true),
                ("ReadDefaults", true),
                ("NoRead", false),
                ("DefaultsOnly", false),
            ],
        ),
        (
            "document",
            vec![DOCUMENT],
            vec![vec!["Ссылка", "Заполняемый"]],
            vec![
                ("ControlReadOff", true),
                ("ControlReadOn", true),
                ("ControlDenied", false),
            ],
        ),
    ] {
        let snapshot = snapshot(folder, &owners, |_| {});
        for (name, expected) in profiles {
            let (role, rights) = rights(folder, name);
            let store = selected_store(&snapshot, &role, rights);
            for (owner, fields) in owners.iter().zip(&fields) {
                let result =
                    check_user_field_access(&store, &snapshot, &request(&snapshot, owner, fields));
                assert_eq!(
                    result.is_ok(),
                    expected,
                    "{folder} {name} {owner}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn foreign_fields_tabular_sections_and_unknown_standards_remain_denied() {
    for (folder, owners, owner, name, standard, role_name) in [
        (
            "register",
            vec![REGISTER, DAY],
            REGISTER,
            "НомерСтрокиТаблицыРесурсов",
            StandardFieldId::Id,
            "ReadOwner",
        ),
        (
            "document",
            vec![DOCUMENT],
            DOCUMENT,
            "Заполняемый",
            StandardFieldId::Date,
            "ControlReadOff",
        ),
    ] {
        let snapshot = snapshot(folder, &owners, |_| {});
        let (role, rights) = rights(folder, role_name);
        let store = selected_store(&snapshot, &role, rights);
        let mut req = request(&snapshot, owner, &[name]);
        req.fields[0].table_part = Some("Unknown".into());
        assert!(check_user_field_access(&store, &snapshot, &req).is_err());
        req.fields[0].table_part = None;
        req.fields[0].field = FieldId::Standard(standard);
        assert!(check_user_field_access(&store, &snapshot, &req).is_err());
        req.fields[0].field =
            FieldId::Metadata(open_sdbl::metadata::AttributeId::from_bytes([0x55; 16]));
        assert!(check_user_field_access(&store, &snapshot, &req).is_err());
        if folder == "register" {
            req.fields[0].field = request(&snapshot, DAY, &[name]).fields[0].field;
            assert!(check_user_field_access(&store, &snapshot, &req).is_err());
        }
    }
}

#[test]
fn incomplete_owner_types_and_nonperiodic_period_are_not_authorized() {
    for (folder, owner, name, profile) in [
        ("register", REGISTER, "Мощность", "ReadOwner"),
        ("document", DOCUMENT, "Заполняемый", "ControlReadOff"),
    ] {
        for mode in 0..4 {
            let snapshot = snapshot(folder, &[owner], |ds| {
                if mode == 0 {
                    let d = ds
                        .iter_mut()
                        .find(|d| d.object_guid.as_str() == owner)
                        .unwrap();
                    if folder == "register" {
                        d.information_register_dimensions = None;
                    } else {
                        d.document_attributes = None;
                    }
                } else if mode == 1 {
                    let d = ds.iter_mut().find(|d| d.name == name).unwrap();
                    d.declared_types = None;
                } else if mode == 2 {
                    let d = ds.iter_mut().find(|d| d.name == name).unwrap();
                    d.resource_guid = "55555555-5555-4555-8555-555555555555".parse().unwrap();
                } else {
                    let d = ds
                        .iter_mut()
                        .find(|d| d.object_guid.as_str() == owner)
                        .unwrap();
                    if folder == "register" {
                        d.information_register_write_mode = None;
                    } else {
                        d.document_numbering = None;
                    }
                }
            });
            let (role, rights) = rights(folder, profile);
            let req = request(&snapshot, owner, &[name]);
            assert!(
                check_user_field_access(&selected_store(&snapshot, &role, rights), &snapshot, &req)
                    .is_err(),
                "{folder} mode={mode}"
            );
        }
    }
    let snapshot = snapshot("register", &[REGISTER], |_| {});
    let (role, rights) = rights("register", "ReadOwner");
    assert!(
        check_user_field_access(
            &selected_store(&snapshot, &role, rights),
            &snapshot,
            &request(&snapshot, REGISTER, &["Период"])
        )
        .is_err()
    );
}

#[test]
fn explicit_member_read_role_union_and_complete_selected_roles_are_preserved() {
    for (folder, owner, name, profile) in [
        ("register", REGISTER, "Мощность", "ReadOwner"),
        ("document", DOCUMENT, "Заполняемый", "ControlReadOff"),
    ] {
        let snapshot = snapshot(folder, &[owner], |_| {});
        let request = request(&snapshot, owner, &[name]);
        let (role, mut rights) = rights(folder, profile);
        let granting = rights.clone();
        let FieldId::Metadata(id) = request.fields[0].field else {
            panic!()
        };
        rights.objects.push(denied_field_entry(
            &owner.parse().unwrap(),
            &snapshot.attribute_by_id(id).unwrap().guid,
        ));
        let mut store = selected_store(&snapshot, &role, rights);
        assert!(check_user_field_access(&store, &snapshot, &request).is_err());
        // Модельная вторая роль проверяет существующее объединение, не создаёт
        // новый нативный оракул прав отдельного реквизита.
        let second: Guid = "55555555-5555-4555-8555-555555555555".parse().unwrap();
        let mut user = store.current_user().unwrap().clone();
        user.data.roles.push(second.clone());
        store.set_current_user(Some(user));
        assert!(check_user_field_access(&store, &snapshot, &request).is_err());
        store.insert_rights(second, granting);
        check_user_field_access(&store, &snapshot, &request).unwrap();
        store.set_current_user(None);
        assert!(check_user_field_access(&store, &snapshot, &request).is_err());
    }
}
