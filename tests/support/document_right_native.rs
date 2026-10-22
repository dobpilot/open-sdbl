#![allow(dead_code)]

use open_sdbl::metadata::{
    Guid, MetadataRightError, MetadataSnapshot, ResolvedMetadataRight, RoleRights, SchemaStorage,
    parse_config_descriptors, parse_db_names, parse_role_rights, resolve_document_attribute_right,
    resolve_metadata, resolve_metadata_right,
};
use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|root| root.join("tests/fixtures/document-metadata-rights-0929"))
        .find(|path| path.exists())
        .unwrap()
}

pub fn owner() -> Guid {
    "f77edfd7-d0d4-532b-a649-399fb412f0a4".parse().unwrap()
}

pub fn attribute() -> Guid {
    "ae787631-1111-4444-8888-123456789001".parse().unwrap()
}

pub fn snapshot() -> MetadataSnapshot {
    resolve_metadata(
        parse_db_names(&std::fs::read(root().join("dbnames.deflate")).unwrap()).unwrap(),
        parse_config_descriptors(
            owner().as_str(),
            &std::fs::read(root().join("document.deflate")).unwrap(),
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
    pub target: String,
    pub right: String,
    pub operation: String,
    pub privileged: bool,
    pub outcome: String,
    pub role: RoleRights,
}

impl Case {
    pub fn resolve(
        &self,
        snapshot: &MetadataSnapshot,
    ) -> Result<ResolvedMetadataRight, MetadataRightError> {
        if self.target == "attribute" {
            resolve_document_attribute_right(snapshot, &owner(), &attribute(), &self.right)
        } else {
            let standard = match self.target.as_str() {
                "owner" => None,
                "number.ru" => Some("Номер"),
                "number.en" => Some("Number"),
                "date.ru" => Some("Дата"),
                "date.en" => Some("Date"),
                _ => panic!("неизвестная нативная цель"),
            };
            resolve_metadata_right(snapshot, &owner(), &self.right, standard)
        }
    }
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
                target: fields[1].to_owned(),
                right: fields[2].to_owned(),
                operation: fields[3].to_owned(),
                privileged: fields[4] == "1",
                outcome: fields[5].to_owned(),
                role: parse_role_rights(&std::fs::read(root().join(fields[6])).unwrap()).unwrap(),
            }
        })
        .collect()
}
