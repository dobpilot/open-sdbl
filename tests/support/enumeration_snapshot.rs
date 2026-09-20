//! The metadata snapshot the parameter, access and restriction tests
//! resolve: one enumeration and its owner, over a live `_enum99` table.

use open_sdbl::metadata::{
    ConfigDescriptor, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaStorage, parse_db_names,
    resolve_metadata,
};
use std::str::FromStr;

pub(crate) fn enumeration_snapshot() -> MetadataSnapshot {
    let owner = Guid::from_str("c8b21fea-1e3d-4ae9-8719-7ff4db08af97").unwrap();
    let value = Guid::from_str("d2f8bde9-fadd-4be8-9022-249e3a1ac4b9").unwrap();
    let db_names = parse_db_names(&crate::hex_test_support::hex(
        "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500",
    ))
    .unwrap();
    let descriptor = |object_guid: Guid, name: &str, enumeration_value: bool| ConfigDescriptor {
        resource_guid: owner.clone(),
        object_guid,
        marker: "1".to_owned(),
        name: name.to_owned(),
        synonyms: Vec::new(),
        comment: None,
        field_purpose: None,
        enumeration_value,
        separation: None,
        reference_types: Vec::new(),
        object_reference_type: None,
        balance: None,
        chart_of_accounts: None,
    };
    let status = descriptor(value, "Статус", true);
    let object = descriptor(owner.clone(), "бит_ВидыСтатусовОбъектов", false);
    resolve_metadata(
        db_names,
        vec![object, status],
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        vec![LiveTable {
            name: "_enum99".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_enumorder".to_owned(),
                    data_type: "numeric".to_owned(),
                },
            ],
            indexes: Vec::new(),
        }],
    )
    .snapshot
}
