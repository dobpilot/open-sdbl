//! A measured accumulation register, one user and one role reading it.

use std::path::PathBuf;

use open_sdbl::metadata::{
    DbNameEntry, DbNames, Guid, LiveColumn, LiveTable, MetadataKind, MetadataSnapshot, Right,
    RoleRights, parse_config_descriptors, parse_role_rights, parse_schema_storage,
    resolve_metadata,
};
use open_sdbl_db::access::users_from_rows;
use open_sdbl_db::{AccessStore, Cell, QueryRows};

pub(crate) const ROLE: &str = "76702e9e-fa7a-4b98-befa-f9b37db2dae0";
pub(crate) const OWNER: &str = "1b9be124-96ea-4346-8f4b-c8b295679d78";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn user_rows() -> QueryRows {
    let text = String::from_utf8(fixture("access/unf_v8users.tsv")).unwrap();
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

pub(crate) fn snapshot() -> MetadataSnapshot {
    let owner: Guid = OWNER.parse().unwrap();
    let descriptors = parse_config_descriptors(
        owner.as_str(),
        &fixture("runtime-object-099/ЗамерОстатков.deflate"),
    )
    .unwrap();
    let descriptor = descriptors.iter().find(|d| d.object_guid == owner).unwrap();
    let fields = descriptor
        .accumulation_register_dimensions
        .as_ref()
        .unwrap()
        .iter()
        .chain(descriptor.accumulation_register_resources.as_ref().unwrap())
        .cloned()
        .collect::<Vec<_>>();
    let mut names = vec![DbNameEntry {
        guid: owner,
        alias: "AccumRg".into(),
        number: 1,
    }];
    names.extend(fields.iter().enumerate().map(|(i, g)| DbNameEntry {
        guid: g.clone(),
        alias: "Fld".into(),
        number: u32::try_from(i).unwrap() + 2,
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
    let schema = parse_schema_storage(
        format!(
            "{{0,{{1,{{\"AccumRg1\",\"N\",1,\"\",{{{},{}}},{{0}},{{0}},1,\"R\",{{0}},{{0}},\"\",0}}}}}}",
            columns.len(),
            columns.join(",")
        )
        .as_bytes(),
    )
    .unwrap();
    let mut live = [
        ("_period", "timestamp without time zone"),
        ("_recordertref", "bytea"),
        ("_recorderrref", "bytea"),
        ("_lineno", "numeric(9,0)"),
        ("_active", "boolean"),
        ("_recordkind", "numeric(1,0)"),
    ]
    .into_iter()
    .map(|(name, data_type)| LiveColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    })
    .collect::<Vec<_>>();
    for (index, field) in fields.iter().enumerate() {
        let declared = descriptors
            .iter()
            .find(|descriptor| &descriptor.object_guid == field)
            .and_then(|descriptor| descriptor.declared_types.as_ref())
            .and_then(|types| types.first())
            .map(|declared| declared.tag.clone());
        live.push(LiveColumn {
            name: format!("_fld{}", index + 2),
            data_type: match declared.as_deref() {
                Some("N") => "numeric(15,2)".to_owned(),
                _ => "mvarchar(12)".to_owned(),
            },
        });
    }
    let live = vec![LiveTable {
        name: "_accumrg1".to_owned(),
        columns: live,
        indexes: Vec::new(),
    }];
    resolve_metadata(DbNames::from_entries(names), descriptors, schema, live).snapshot
}

/// The rights of one role: `read` decides whether it may read the
/// register.
pub(crate) fn rights(snapshot: &MetadataSnapshot, read: bool) -> RoleRights {
    let owner = snapshot
        .objects()
        .iter()
        .find(|object| object.kind == Some(MetadataKind::AccumulationRegister))
        .unwrap();
    let mut rights = parse_role_rights(&fixture(
        "access/buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ))
    .unwrap();
    let mut grant = rights
        .objects
        .iter()
        .find(|object| object.right(&Right::Read).is_some())
        .unwrap()
        .clone();
    grant.object = owner.guid.clone();
    grant.members.clear();
    grant.rights.retain(|right| right.right == Right::Read);
    grant.rights[0].allowed = read;
    grant.rights[0].restrictions.clear();
    rights.objects = vec![grant];
    rights.set_for_new_objects = false;
    rights
}

/// The users of the fixture, each holding only the test role, and the
/// rights of that role.
pub(crate) fn store(snapshot: &MetadataSnapshot, read: bool) -> AccessStore {
    store_with(snapshot, read, |_| {})
}

/// [`store`] with the users adjusted.
pub(crate) fn store_with(
    snapshot: &MetadataSnapshot,
    read: bool,
    adjust: impl Fn(&mut open_sdbl::metadata::InfoBaseUser),
) -> AccessStore {
    let role: Guid = ROLE.parse().unwrap();
    let mut store = AccessStore::new(snapshot);
    let users = users_from_rows(&user_rows())
        .unwrap()
        .into_iter()
        .map(|mut user| {
            user.data.roles = vec![role.clone()];
            adjust(&mut user);
            user
        })
        .collect();
    store.set_users(users);
    store.insert_rights(role, rights(snapshot, read));
    store
}

/// Any row-level restriction a measured rights fixture carries, to be
/// re-aimed at the register.
pub(crate) fn sample_restriction() -> open_sdbl::metadata::Restriction {
    for name in [
        "access/unf_6b6566ca-81d0-4376-93d8-290301f2b00f.0.deflate",
        "access/unf_76702e9e-fa7a-4b98-befa-f9b37db2dae0.0.deflate",
        "access/unf_849f034e-85dc-4515-aae6-240c1e0d46d9.0.deflate",
        "access/unf_e960b3eb-ad6f-4804-b318-acbcdc4b8f98.0.deflate",
        "access/buh_e5c73637-e8d6-47e0-9c15-2fa1802ee5b0.0.deflate",
        "access/buh_feadebe9-a90e-48b0-a89f-e1f5d4e23041.0.deflate",
    ] {
        let Ok(rights) = parse_role_rights(&fixture(name)) else {
            continue;
        };
        for object in rights.objects {
            for right in object.rights {
                if let Some(restriction) = right.restrictions.into_iter().next() {
                    return restriction;
                }
            }
        }
    }
    panic!("no rights fixture carries a restriction")
}

/// A store whose role reads the register where `condition` holds.
pub(crate) fn restricted_store(snapshot: &MetadataSnapshot, condition: &str) -> AccessStore {
    let role: Guid = ROLE.parse().unwrap();
    let mut store = store(snapshot, true);
    let mut rights = rights(snapshot, true);
    let mut restriction = sample_restriction();
    restriction.condition = condition.to_owned();
    restriction.fields.clear();
    rights.objects[0].rights[0].restrictions = vec![restriction];
    store.insert_rights(role, rights);
    store
}
