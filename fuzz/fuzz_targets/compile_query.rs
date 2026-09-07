#![no_main]

use std::sync::OnceLock;
use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use open_sdbl::metadata::{
    ConfigDescriptor, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaColumn, SchemaStorage,
    SchemaTable, parse_db_names, resolve_metadata,
};
use open_sdbl::query::{PostgresBackend, QueryCompiler};

fn snapshot() -> &'static MetadataSnapshot {
    static SNAPSHOT: OnceLock<MetadataSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(|| {
        let object = "b8bac76b-c91b-4d78-8a70-ffa39f8de694";
        let field = "03bd775a-e0a1-4205-82ce-6068e73ad134";
        let db_names = format!("{{2,{{{object},\"Reference\",53}},{{{field},\"Fld\",54}}}}");
        let object = Guid::from_str(object).expect("fixed object GUID");
        let field = Guid::from_str(field).expect("fixed field GUID");
        resolve_metadata(
            parse_db_names(&stored_deflate(db_names.as_bytes())).expect("fixed DBNames fixture"),
            vec![
                descriptor(&object, &object, "Probe"),
                descriptor(&object, &field, "Value"),
            ],
            SchemaStorage {
                tables: vec![SchemaTable {
                    name: "Reference53".to_owned(),
                    number: 53,
                    owner: None,
                    inline_name: None,
                    columns: vec![SchemaColumn {
                        name: "Fld54".to_owned(),
                        types: Vec::new(),
                    }],
                    indexes: Vec::new(),
                }],
                anomalies: Vec::new(),
            },
            vec![LiveTable {
                name: "_reference53".to_owned(),
                columns: vec![LiveColumn {
                    name: "_fld54".to_owned(),
                    data_type: "bytea".to_owned(),
                }],
                indexes: Vec::new(),
            }],
        )
        .snapshot
    })
}

fn descriptor(resource: &Guid, object: &Guid, name: &str) -> ConfigDescriptor {
    ConfigDescriptor {
        resource_guid: resource.clone(),
        object_guid: object.clone(),
        marker: "1".to_owned(),
        name: name.to_owned(),
        synonyms: Vec::new(),
        comment: None,
        field_purpose: None,
        enumeration_value: false,
    }
}

fn stored_deflate(value: &[u8]) -> Vec<u8> {
    let Ok(length) = u16::try_from(value.len()) else {
        return Vec::new();
    };
    let mut compressed = Vec::with_capacity(value.len() + 5);
    compressed.push(1);
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(value);
    compressed
}

fuzz_target!(|input: &[u8]| {
    if let Ok(source) = std::str::from_utf8(input) {
        let _ = QueryCompiler::new(snapshot(), PostgresBackend).compile(source);
    }
});
