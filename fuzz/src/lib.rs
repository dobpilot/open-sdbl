//! Deterministic metadata for the query compiler fuzz target.
//!
//! The fixture is built from the public metadata API only, so the fuzz
//! workspace stays buildable by an ordinary `cargo check` and the snapshot it
//! produces reaches the compiler paths that depend on metadata: ordinary
//! fields, reference dereferences, and tabular-section sources.

use std::str::FromStr;
use std::sync::OnceLock;

use open_sdbl::metadata::{
    ColumnType, ConfigDescriptor, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaColumn,
    SchemaStorage, SchemaTable, parse_db_names, resolve_metadata,
};

/// The catalog the fuzz fixture selects from.
pub const PROBE: &str = "Probe";
/// The reference field of [`PROBE`], pointing at [`TARGET`].
pub const PROBE_REFERENCE: &str = "Owner";
/// The tabular section of [`PROBE`].
pub const PROBE_TABULAR_SECTION: &str = "Items";
/// The catalog [`PROBE_REFERENCE`] points at.
pub const TARGET: &str = "Target";

const PROBE_GUID: &str = "b8bac76b-c91b-4d78-8a70-ffa39f8de694";
const PROBE_REFERENCE_GUID: &str = "03bd775a-e0a1-4205-82ce-6068e73ad134";
const TABULAR_SECTION_GUID: &str = "11111111-1111-4111-8111-111111111111";
const TABULAR_FIELD_GUID: &str = "22222222-2222-4222-8222-222222222222";
const TARGET_GUID: &str = "33333333-3333-4333-8333-333333333333";

/// The fixed snapshot every fuzz iteration compiles against.
pub fn snapshot() -> &'static MetadataSnapshot {
    static SNAPSHOT: OnceLock<MetadataSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(build_snapshot)
}

fn build_snapshot() -> MetadataSnapshot {
    let db_names = format!(
        "{{5,\
         {{{PROBE_GUID},\"Reference\",53}},\
         {{{PROBE_REFERENCE_GUID},\"Fld\",54}},\
         {{{TABULAR_SECTION_GUID},\"VT\",55}},\
         {{{TABULAR_FIELD_GUID},\"Fld\",56}},\
         {{{TARGET_GUID},\"Reference\",57}}}}"
    );
    let probe = guid(PROBE_GUID);
    let probe_reference = guid(PROBE_REFERENCE_GUID);
    let tabular_section = guid(TABULAR_SECTION_GUID);
    let tabular_field = guid(TABULAR_FIELD_GUID);
    let target = guid(TARGET_GUID);
    resolve_metadata(
        parse_db_names(&stored_deflate(db_names.as_bytes())).expect("fixed DBNames fixture"),
        vec![
            descriptor(&probe, &probe, PROBE),
            descriptor(&probe, &probe_reference, PROBE_REFERENCE),
            descriptor(&probe, &tabular_section, PROBE_TABULAR_SECTION),
            descriptor(&probe, &tabular_field, "Note"),
            descriptor(&target, &target, TARGET),
        ],
        SchemaStorage {
            tables: vec![
                schema_table(
                    "Reference53",
                    53,
                    vec![
                        column("ID", "R", Some("Reference53")),
                        column("Code", "S", None),
                        column("Description", "S", None),
                        column("Fld54", "R", Some("Reference57")),
                    ],
                ),
                schema_table(
                    "Reference53_VT55",
                    55,
                    vec![
                        column("Reference53_IDRRef", "R", Some("Reference53")),
                        column("LineNo55", "N", None),
                        column("Fld56", "S", None),
                    ],
                ),
                schema_table(
                    "Reference57",
                    57,
                    vec![
                        column("ID", "R", Some("Reference57")),
                        column("Code", "S", None),
                        column("Description", "S", None),
                    ],
                ),
            ],
            anomalies: Vec::new(),
        },
        vec![
            live_table(
                "_reference53",
                &[
                    ("_idrref", "bytea"),
                    ("_code", "mvarchar(9)"),
                    ("_description", "mvarchar(150)"),
                    ("_fld54", "bytea"),
                ],
            ),
            live_table(
                "_reference53_vt55",
                &[
                    ("_reference53_idrref", "bytea"),
                    ("_lineno55", "numeric(5,0)"),
                    ("_fld56", "mvarchar(150)"),
                ],
            ),
            live_table(
                "_reference57",
                &[
                    ("_idrref", "bytea"),
                    ("_code", "mvarchar(9)"),
                    ("_description", "mvarchar(150)"),
                ],
            ),
        ],
    )
    .snapshot
}

fn guid(value: &str) -> Guid {
    Guid::from_str(value).expect("fixed fixture GUID")
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
        separation: None,
        reference_types: Vec::new(),
        object_reference_type: None,
    }
}

fn schema_table(name: &str, number: u32, columns: Vec<SchemaColumn>) -> SchemaTable {
    SchemaTable {
        name: name.to_owned(),
        number,
        owner: None,
        inline_name: None,
        columns,
        indexes: Vec::new(),
    }
}

fn column(name: &str, tag: &str, reference_target: Option<&str>) -> SchemaColumn {
    SchemaColumn {
        name: name.to_owned(),
        types: vec![ColumnType {
            tag: tag.to_owned(),
            reference_target: reference_target.map(str::to_owned),
        }],
    }
}

fn live_table(name: &str, columns: &[(&str, &str)]) -> LiveTable {
    LiveTable {
        name: name.to_owned(),
        columns: columns
            .iter()
            .map(|(column, data_type)| LiveColumn {
                name: (*column).to_owned(),
                data_type: (*data_type).to_owned(),
            })
            .collect(),
        indexes: Vec::new(),
    }
}

fn stored_deflate(value: &[u8]) -> Vec<u8> {
    let length = u16::try_from(value.len()).expect("fixed fixture fits one stored block");
    let mut compressed = Vec::with_capacity(value.len() + 5);
    compressed.push(1);
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(value);
    compressed
}

#[cfg(test)]
mod tests {
    use super::snapshot;
    use open_sdbl::query::{PostgresBackend, QueryCompiler};

    fn compile(source: &str) -> String {
        QueryCompiler::new(snapshot(), PostgresBackend)
            .compile(source)
            .unwrap_or_else(|error| panic!("{source} must compile: {error}"))
            .sql
            .clone()
    }

    /// Without this the fuzzer only ever reaches the parser, because every
    /// name it invents is rejected before code generation starts.
    #[test]
    fn reaches_fields_dereferences_and_tabular_sections() {
        assert!(compile("SELECT Code FROM Catalog.Probe").contains("_code"));

        let dereference = compile("SELECT Owner.Code FROM Catalog.Probe");
        assert!(dereference.contains("_reference57"), "{dereference}");
        assert!(dereference.contains("LEFT JOIN"), "{dereference}");

        let tabular = compile("SELECT Note FROM Catalog.Probe.Items");
        assert!(tabular.contains("_reference53_vt55"), "{tabular}");
    }
}
