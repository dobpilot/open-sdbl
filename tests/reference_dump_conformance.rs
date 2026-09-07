//! Conformance checks over whole metadata resources captured during the
//! service-table inventory. Large resources stay raw-DEFLATE-compressed.

use std::collections::{BTreeMap, BTreeSet};

use open_sdbl::metadata::{
    AllowedLength, DbNames, LiveColumn, LiveTable, MetadataKind, MsSqlMetadataQueries,
    PostgresMetadataQueries, ResolutionFinding, SchemaStorage, collapse_logical_fields,
    inflate_raw_deflate, parse_db_names, parse_schema_storage, recase_postgres_identifier,
    resolve_metadata,
};
use open_sdbl::query::{PostgresBackend, QueryCompiler};

const MSSQL_DB_NAMES: &[u8] =
    include_bytes!("fixtures/service_tables/reference_dumps/mssql/db_names.deflate");
const MSSQL_SCHEMA: &[u8] =
    include_bytes!("fixtures/service_tables/reference_dumps/mssql/schema_storage.deflate");
const POSTGRES_DB_NAMES: &[u8] =
    include_bytes!("fixtures/service_tables/reference_dumps/postgres/db_names.deflate");
const POSTGRES_SCHEMA: &[u8] =
    include_bytes!("fixtures/service_tables/reference_dumps/postgres/schema_storage.deflate");

fn schema(compressed: &[u8]) -> (Vec<u8>, SchemaStorage) {
    let decoded = inflate_raw_deflate(compressed).expect("reference SchemaStorage must inflate");
    let parsed = parse_schema_storage(&decoded).expect("reference SchemaStorage must parse");
    (decoded, parsed)
}

fn live_tables_from_schema(schema: &SchemaStorage) -> Vec<LiveTable> {
    schema
        .tables
        .iter()
        .map(|table| LiveTable {
            name: table.physical_name(),
            columns: table
                .columns
                .iter()
                .flat_map(|column| {
                    column.types.iter().map(|kind| LiveColumn {
                        name: column.physical_name(),
                        data_type: match kind.tag.as_str() {
                            "B" | "L" => "boolean",
                            "N" => "numeric(38,10)",
                            "S" => "mvarchar(1024)",
                            "T" => "timestamp without time zone",
                            "R" | "V" => "bytea",
                            other => panic!("unexpected captured column tag {other}"),
                        }
                        .to_owned(),
                    })
                })
                .collect(),
            indexes: Vec::new(),
        })
        .collect()
}

fn logical_field_identities(
    table: &open_sdbl::metadata::SchemaTable,
    db_names: &DbNames,
) -> BTreeSet<String> {
    collapse_logical_fields(table.columns.iter().map(|column| column.physical_name()))
        .into_iter()
        .map(|field| {
            field
                .name
                .strip_prefix("Fld")
                .and_then(|number| number.parse::<u32>().ok())
                .and_then(|number| db_names.field_guid(number))
                .map_or(field.name, |guid| format!("guid:{guid}"))
        })
        .collect()
}

#[test]
fn whole_schema_storage_dumps_cover_the_real_column_alphabet() {
    for (provider, compressed, expected_tables) in [
        ("mssql", MSSQL_SCHEMA, 2_019),
        ("postgres", POSTGRES_SCHEMA, 3_828),
    ] {
        let (decoded, schema) = schema(compressed);
        assert_eq!(&decoded[..3], &[0xef, 0xbb, 0xbf], "{provider}");
        let tags = schema
            .tables
            .iter()
            .flat_map(|table| &table.columns)
            .flat_map(|column| &column.types)
            .map(|kind| kind.tag.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            tags,
            BTreeSet::from(["B", "L", "N", "R", "S", "T", "V"]),
            "{provider}"
        );
        assert!(
            schema.anomalies.is_empty(),
            "{provider}: {:?}",
            schema.anomalies
        );
        assert_eq!(schema.tables.len(), expected_tables, "{provider}");
    }
}

#[test]
fn whole_db_names_dumps_accept_shared_and_nil_guids() {
    for (provider, compressed, expected) in [
        ("mssql", MSSQL_DB_NAMES, (6_726, 142, 65, 1_160)),
        ("postgres", POSTGRES_DB_NAMES, (15_145, 108, 60, 2_480)),
    ] {
        let names = parse_db_names(compressed).expect("reference DBNames must parse");
        let aliases = names
            .entries()
            .iter()
            .map(|entry| entry.alias.clone())
            .collect::<BTreeSet<_>>();
        let nil = names
            .entries()
            .iter()
            .filter(|entry| entry.guid.is_nil())
            .count();
        let mut frequencies = BTreeMap::new();
        for entry in names.entries().iter().filter(|entry| !entry.guid.is_nil()) {
            *frequencies.entry(entry.guid.as_str()).or_insert(0_usize) += 1;
        }
        let repeated = frequencies.values().filter(|count| **count > 1).count();
        let resolved = resolve_metadata(
            names,
            Vec::new(),
            schema(if provider == "mssql" {
                MSSQL_SCHEMA
            } else {
                POSTGRES_SCHEMA
            })
            .1,
            Vec::new(),
        );
        assert!(
            resolved
                .snapshot
                .objects()
                .iter()
                .all(|object| !object.guid.is_nil())
        );
        let spurious = resolved
            .report
            .findings()
            .iter()
            .filter(|finding| {
                matches!(
                    finding,
                    ResolutionFinding::DuplicateGuid { .. }
                        | ResolutionFinding::UnknownColumnTag { .. }
                        | ResolutionFinding::InvalidSchemaDeclaration { .. }
                )
            })
            .collect::<Vec<_>>();
        assert!(spurious.is_empty(), "{provider}: {spurious:?}");
        assert_eq!(
            (names_len(&resolved), aliases.len(), nil, repeated),
            expected,
            "{provider}"
        );
    }
}

fn names_len(resolved: &open_sdbl::metadata::ResolvedMetadata) -> usize {
    resolved.snapshot.db_names().entries().len()
}

#[test]
fn captured_catalog_shapes_preserve_compounds_and_provider_types() {
    let node = collapse_logical_fields(["_NodeTRef", "_NodeRRef"]);
    assert_eq!(node.len(), 1);
    assert_eq!(node[0].name, "Node");
    assert_eq!(node[0].physical_columns, ["_NodeTRef", "_NodeRRef"]);

    assert!(PostgresMetadataQueries::CATALOG.contains("format_type(a.atttypid, a.atttypmod)"));
    assert!(!PostgresMetadataQueries::CATALOG.contains("information_schema"));
    for physical_type in ["binary", "numeric", "varbinary"] {
        assert!(
            MsSqlMetadataQueries::CATALOG.contains(physical_type),
            "{physical_type}"
        );
    }
    for catalog_fragment in ["c.[max_length]", "c.[precision]", "c.[scale]", "N'max'"] {
        assert!(
            MsSqlMetadataQueries::CATALOG.contains(catalog_fragment),
            "{catalog_fragment}"
        );
    }
    assert_eq!(
        AllowedLength::from_postgres_type("mvarchar(50)"),
        Some(AllowedLength::Variable)
    );

    let captured = include_str!("fixtures/service_tables/live/column_types.tsv");
    for row in [
        "ConfigCAS\tFileName\tnvarchar",
        "ConfigCAS\tBinaryData\tvarbinary",
        "ConfigCAS\tPartNo\tint",
        "_ExtensionsInfo\t_Version\ttimestamp",
    ] {
        assert!(captured.lines().any(|line| line == row), "missing {row}");
    }
    let service_columns = include_str!("fixtures/service_tables/live/service_columns.tsv");
    for row in [
        "_ConfigChngR\t_NodeTRef\tbinary\t4",
        "_ConfigChngR\t_NodeRRef\tbinary\t16",
        "_ConfigChngR\t_MessageNo\tnumeric\t9",
    ] {
        assert!(
            service_columns.lines().any(|line| line == row),
            "missing {row}"
        );
    }
}

#[test]
fn whole_dump_resolution_is_deterministic_at_reference_scale() {
    let db_names = parse_db_names(POSTGRES_DB_NAMES).unwrap();
    let schema = schema(POSTGRES_SCHEMA).1;
    let field_count = schema
        .tables
        .iter()
        .map(|table| table.columns.len())
        .sum::<usize>();
    assert!(schema.tables.len() >= 3_000, "{}", schema.tables.len());
    assert!(field_count >= 20_000, "{field_count}");
    let live_tables = live_tables_from_schema(&schema);
    let first = resolve_metadata(
        db_names.clone(),
        Vec::new(),
        schema.clone(),
        live_tables.clone(),
    );
    let second = resolve_metadata(db_names, Vec::new(), schema, live_tables);
    let prepared = QueryCompiler::new(&first.snapshot, PostgresBackend)
        .prepare("SELECT 1;")
        .expect("constant query must prepare");
    prepared
        .compile(&second.snapshot, &[])
        .expect("identical resolution must retain the same fingerprint");
}

#[test]
fn reference_dumps_decode_to_the_expected_encodings() {
    for compressed in [MSSQL_SCHEMA, POSTGRES_SCHEMA] {
        let decoded = inflate_raw_deflate(compressed).unwrap();
        assert!(decoded.starts_with(&[0xef, 0xbb, 0xbf, b'{']));
        assert!(std::str::from_utf8(&decoded[3..]).is_ok());
    }
    for compressed in [MSSQL_DB_NAMES, POSTGRES_DB_NAMES] {
        let decoded = inflate_raw_deflate(compressed).unwrap();
        let text = std::str::from_utf8(
            decoded
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(&decoded),
        )
        .unwrap();
        assert!(text.starts_with('{'));
        assert!(parse_db_names(compressed).is_ok());
    }
}

#[test]
fn shared_provider_entries_keep_canonical_identity_across_renumbering() {
    let mssql = parse_db_names(MSSQL_DB_NAMES).unwrap();
    let postgres = parse_db_names(POSTGRES_DB_NAMES).unwrap();
    let postgres_entries = postgres
        .entries()
        .iter()
        .map(|entry| ((entry.guid.as_str(), entry.alias.as_str()), entry.number))
        .collect::<BTreeMap<_, _>>();
    let shared = mssql
        .entries()
        .iter()
        .filter(|entry| !entry.guid.is_nil())
        .filter_map(|entry| {
            postgres_entries
                .get(&(entry.guid.as_str(), entry.alias.as_str()))
                .map(|number| (entry, *number))
        })
        .collect::<Vec<_>>();
    assert!(shared.len() >= 3_000, "shared entries: {}", shared.len());
    let mssql_schema = schema(MSSQL_SCHEMA).1;
    let postgres_schema = schema(POSTGRES_SCHEMA).1;
    let mut matched_objects = 0_usize;
    let mut equivalent_field_sets = 0_usize;
    for (entry, postgres_number) in shared {
        let Some(kind) = MetadataKind::from_alias(&entry.alias) else {
            continue;
        };
        assert_eq!(MetadataKind::from_alias(&entry.alias), Some(kind));
        if kind.is_service() {
            continue;
        }
        let mssql_table = format!("{}{}", kind.physical_prefix(), entry.number);
        let postgres_table = format!("{}{}", kind.physical_prefix(), postgres_number);
        if let (Some(mssql_declaration), Some(postgres_declaration)) = (
            mssql_schema.table(&mssql_table),
            postgres_schema.table(&postgres_table),
        ) {
            assert_eq!(
                recase_postgres_identifier(&postgres_table.to_ascii_lowercase()),
                postgres_table
            );
            if logical_field_identities(mssql_declaration, &mssql)
                == logical_field_identities(postgres_declaration, &postgres)
            {
                equivalent_field_sets += 1;
            }
            matched_objects += 1;
        }
    }

    let postgres_tables = postgres_schema
        .tables
        .iter()
        .map(|table| {
            (
                table.name.as_str(),
                table
                    .columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let equivalent = mssql_schema
        .tables
        .iter()
        .filter(|table| {
            let fields = table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<BTreeSet<_>>();
            postgres_tables
                .get(table.name.as_str())
                .is_some_and(|postgres_fields| *postgres_fields == fields)
        })
        .count();
    assert_eq!(matched_objects, 525);
    assert_eq!(equivalent_field_sets, 392);
    assert_eq!(equivalent, 26);
}
