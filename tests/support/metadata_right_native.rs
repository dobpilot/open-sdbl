#![allow(dead_code)]

use open_sdbl::metadata::{
    Guid, MetadataSnapshot, RoleRights, SchemaStorage, parse_config_descriptors, parse_db_names,
    parse_role_rights, resolve_metadata,
};
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|root| root.join("tests/fixtures/metadata-right-native-0922"))
        .find(|path| path.exists())
        .unwrap()
}

pub fn owner() -> Guid {
    "1316b080-3b2c-501b-ace3-a1234a3fedd1".parse().unwrap()
}

pub fn snapshot() -> MetadataSnapshot {
    resolve_metadata(
        parse_db_names(&std::fs::read(root().join("dbnames.deflate")).unwrap()).unwrap(),
        parse_config_descriptors(
            owner().as_str(),
            &std::fs::read(root().join("catalog.deflate")).unwrap(),
        )
        .unwrap(),
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot
}

pub struct Case {
    pub context: String,
    pub method: String,
    pub right: String,
    pub standard: Option<String>,
    pub privileged: bool,
    pub outcome: String,
    pub roles: Vec<RoleRights>,
}

pub fn cases() -> Vec<Case> {
    std::fs::read_to_string(root().join("cases.tsv"))
        .unwrap()
        .lines()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7);
            Case {
                context: fields[0].to_owned(),
                method: fields[1].to_owned(),
                right: fields[2].to_owned(),
                standard: match fields[3] {
                    "undefined" => None,
                    "empty" => Some(String::new()),
                    name => Some(name.to_owned()),
                },
                privileged: fields[4] == "true",
                outcome: fields[5].to_owned(),
                roles: fields[6]
                    .split(',')
                    .filter(|name| !name.is_empty())
                    .map(|name| {
                        parse_role_rights(&std::fs::read(root().join(name)).unwrap()).unwrap()
                    })
                    .collect(),
            }
        })
        .collect()
}
