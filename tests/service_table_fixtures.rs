//! Baseline behavior for platform service tables captured from a live base.
//!
//! The fixtures under `tests/fixtures/service_tables/` are verbatim excerpts
//! from a real 8.3 information base (see `docs/service-tables-inventory.md`).
//! These tests pin the CURRENT behavior of the library on that material;
//! the `support-extension-service-tables` change will flip the marked
//! expectations as phases 2-3 land.

mod support;

use open_sdbl::metadata::{
    ConfigDescriptor, ExtensionMetadata, LiveColumn, LiveTable, MetadataKind, ResolutionFinding,
    SchemaStorage, Synonym, parse_db_names, parse_schema_storage, resolve_metadata,
    resolve_metadata_with_extensions,
};
use open_sdbl::query::queryable_fields;
use open_sdbl::query::{MsSqlBackend, PostgresBackend, QueryCompiler, QueryDiagnosticKind};
use support::stored_deflate;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/service_tables/{name}")).unwrap()
}

fn schema_envelope(records: &[&str]) -> Vec<u8> {
    format!(
        "{{0,\n{{{},\n{}\n}}\n}}",
        records.len(),
        records.join(",\n")
    )
    .into_bytes()
}

fn live_tables_from_fixture() -> Vec<LiveTable> {
    let spec = fixture("live/service_columns.tsv");
    let mut tables: Vec<LiveTable> = Vec::new();
    for line in spec.lines() {
        let mut cells = line.split('\t');
        let (Some(table), Some(column), Some(data_type)) =
            (cells.next(), cells.next(), cells.next())
        else {
            continue;
        };
        let entry = match tables.iter_mut().find(|entry| entry.name == table) {
            Some(entry) => entry,
            None => {
                tables.push(LiveTable {
                    name: table.to_owned(),
                    columns: Vec::new(),
                    indexes: Vec::new(),
                });
                tables.last_mut().unwrap()
            }
        };
        entry.columns.push(LiveColumn {
            name: column.to_owned(),
            data_type: data_type.to_owned(),
        });
    }
    assert!(!tables.is_empty());
    tables
}

#[test]
fn top_level_service_declarations_parse_from_live_fixtures() {
    let records = [
        fixture("exts_chngr_with_extprops.txt"),
        fixture("accumrg_chngr.txt"),
        fixture("crg_recalc.txt"),
        fixture("acc_extdim_inline.txt"),
        fixture("ckinds_baseck_inline.txt"),
        fixture("ckinds_leadingck_inline.txt"),
    ];
    let refs: Vec<&str> = records.iter().map(String::as_str).collect();
    let schema = parse_schema_storage(&schema_envelope(&refs)).unwrap();

    let names: Vec<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"ExtsChngR"), "{names:?}");
    assert!(names.contains(&"AccumRgChngR1273"), "{names:?}");
    assert!(names.contains(&"CRgRecalc3975"), "{names:?}");

    let chngr = schema.table("_AccumRgChngR1273").unwrap();
    let columns: Vec<&str> = chngr.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(columns[..2], ["Node", "MessageNo"], "{columns:?}");

    // Phase 2: service inline declarations use the same physical projection
    // rules as tabular sections.
    assert!(
        schema.table("_ExtsChngR_ExtProps").is_some(),
        "inline ExtProps was not projected"
    );
    assert!(schema.table("_Acc3930_ExtDim3937").is_some());
    assert!(schema.table("_CKinds3929_BaseCK").is_some());
    assert!(schema.table("_CKinds3929_LeadingCK").is_some());
}

#[test]
fn live_service_tables_resolve_as_typed_metadata() {
    let records = [
        fixture("exts_chngr_with_extprops.txt"),
        fixture("accumrg_chngr.txt"),
        fixture("crg_recalc.txt"),
        fixture("acc_extdim_inline.txt"),
        fixture("ckinds_baseck_inline.txt"),
        fixture("ckinds_leadingck_inline.txt"),
    ];
    let refs: Vec<&str> = records.iter().map(String::as_str).collect();
    let schema = parse_schema_storage(&schema_envelope(&refs)).unwrap();
    let db_names =
        parse_db_names(&stored_deflate(fixture("db_names_service.txt").as_bytes())).unwrap();
    let resolved = resolve_metadata(db_names, Vec::new(), schema, live_tables_from_fixture());

    // Phase 3: non-nil service aliases resolve as typed metadata. Platform
    // rows carrying the all-zero GUID remain non-objects.
    let kinds = resolved
        .snapshot
        .objects()
        .iter()
        .filter_map(|object| object.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&MetadataKind::ExtraDimension), "{kinds:?}");
    assert!(kinds.contains(&MetadataKind::Recalculation), "{kinds:?}");
    assert!(
        kinds.contains(&MetadataKind::ChangeRegistration),
        "{kinds:?}"
    );
    assert_eq!(resolved.snapshot.objects().len(), 3);

    // Phase 2 declarations are no longer reported as absent.
    let undeclared: Vec<&str> = resolved
        .report
        .findings()
        .iter()
        .filter_map(|finding| match finding {
            ResolutionFinding::TableNotDeclared { table } => Some(table.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        !undeclared.contains(&"_Acc3930_ExtDim3937"),
        "{undeclared:?}"
    );
    assert!(
        !undeclared.contains(&"_ExtsChngR_ExtProps"),
        "{undeclared:?}"
    );
    // Declared top-level service tables must NOT be flagged as undeclared.
    assert!(!undeclared.contains(&"_ExtsChngR"), "{undeclared:?}");
    assert!(!undeclared.contains(&"_CRgRecalc3975"), "{undeclared:?}");
}

#[test]
fn reports_a_service_db_names_entry_without_a_matching_schema_table() {
    let owner = "44947459-75aa-48e8-b40f-907177c2afb3";
    let db_names = parse_db_names(&stored_deflate(
        format!("{{1,{{{owner},\"AccumRgChngR\",9999}}}}").as_bytes(),
    ))
    .unwrap();
    let declaration = fixture("accumrg_chngr.txt");
    let schema = parse_schema_storage(&schema_envelope(&[&declaration])).unwrap();

    let resolved = resolve_metadata(db_names, Vec::new(), schema, Vec::new());

    assert!(resolved.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::ServiceTableMappingMissing {
            guid,
            alias,
            number: 9999,
        } if guid.as_str() == owner && alias == "AccumRgChngR"
    )));
    assert!(resolved.snapshot.objects().is_empty());
}

fn columns_of(fixture_name: &str) -> Vec<String> {
    fixture(fixture_name)
        .lines()
        .filter_map(|line| line.split('|').nth(1).map(str::to_owned))
        .collect()
}

fn live_table_from_pg_fixture(fixture_name: &str) -> LiveTable {
    let rows = fixture(fixture_name);
    let mut table = LiveTable {
        name: String::new(),
        columns: Vec::new(),
        indexes: Vec::new(),
    };
    for row in rows.lines() {
        let mut cells = row.split('|');
        table.name = cells.next().unwrap().to_owned();
        table.columns.push(LiveColumn {
            name: cells.next().unwrap().to_owned(),
            data_type: cells.next().unwrap().to_owned(),
        });
    }
    table
}

fn descriptor(resource: &str, object: &str, name: &str) -> ConfigDescriptor {
    ConfigDescriptor {
        resource_guid: resource.parse().unwrap(),
        object_guid: object.parse().unwrap(),
        marker: "1".to_owned(),
        name: name.to_owned(),
        synonyms: Vec::<Synonym>::new(),
        comment: None,
        field_purpose: None,
        enumeration_value: false,
    }
}

#[test]
fn extension_adds_columns_absent_from_the_base_declaration() {
    // Captured from the PostgreSQL reference base: a catalog whose
    // configuration extension adds three attributes living only in the
    // physical `…x1` table.
    let base = columns_of("pg/ext_reference_base_columns.tsv");
    let extended = columns_of("pg/ext_reference_x1_columns.tsv");
    assert!(!base.is_empty() && !extended.is_empty());

    let added: Vec<&String> = extended.iter().filter(|c| !base.contains(c)).collect();
    assert_eq!(added.len(), 3, "unexpected extension columns: {added:?}");
    assert!(
        added.iter().any(|c| c.as_str() == "_fld16538rref"),
        "{added:?}"
    );

    // The extension attributes exist only in extension metadata, so the base
    // declaration must remain unchanged even after extension support lands.
    let base_decl = fixture("pg/reference14574_base_decl.txt");
    for added_column in &added {
        let number: String = added_column.chars().filter(char::is_ascii_digit).collect();
        assert!(
            !base_decl.contains(&number),
            "extension attribute {added_column} unexpectedly present in the base declaration"
        );
    }

    let owner = "11111111-1111-4111-8111-111111111111";
    let target = "55555555-5555-4555-8555-555555555555";
    let field_guids = [
        "22222222-2222-4222-8222-222222222222",
        "33333333-3333-4333-8333-333333333333",
        "44444444-4444-4444-8444-444444444444",
    ];
    let base_db_names = parse_db_names(&stored_deflate(
        format!("{{2,{{{owner},\"Reference\",14574}},{{{target},\"Reference\",243}}}}").as_bytes(),
    ))
    .unwrap();
    let target_schema = r#"{"Reference243","N",243,"",{2,{"ID",0,{1,{"R",0,0,"Reference243",2}},"",0},{"Description",0,{1,{"S",50,0,"",0}},"",0}},{0},{0},1,"R",{0},{0},"",0}"#;
    let base_schema = parse_schema_storage(&schema_envelope(&[&base_decl, target_schema])).unwrap();
    let base_descriptors = vec![
        descriptor(owner, owner, "ExtendedCatalog"),
        descriptor(target, target, "TargetCatalog"),
    ];
    let live_tables = vec![
        live_table_from_pg_fixture("pg/ext_reference_base_columns.tsv"),
        live_table_from_pg_fixture("pg/ext_reference_x1_columns.tsv"),
        LiveTable {
            name: "_really_unknown".to_owned(),
            columns: Vec::new(),
            indexes: Vec::new(),
        },
        LiveTable {
            name: "_reference243".to_owned(),
            columns: [("_idrref", "bytea"), ("_description", "mvarchar(50)")]
                .into_iter()
                .map(|(name, data_type)| LiveColumn {
                    name: name.to_owned(),
                    data_type: data_type.to_owned(),
                })
                .collect(),
            indexes: Vec::new(),
        },
    ];
    let base_only = resolve_metadata(
        base_db_names.clone(),
        base_descriptors.clone(),
        base_schema.clone(),
        live_tables.clone(),
    );
    let owner_id = base_only
        .snapshot
        .object_id(MetadataKind::Catalog, "ExtendedCatalog")
        .unwrap();
    assert!(
        base_only
            .snapshot
            .attribute_id(owner_id, "ExtensionText")
            .is_err()
    );

    let extension_db_names = parse_db_names(&stored_deflate(
        format!(
            "{{3,{{{},\"Fld\",16536}},{{{},\"Fld\",16537}},{{{},\"Fld\",16538}}}}",
            field_guids[0], field_guids[1], field_guids[2]
        )
        .as_bytes(),
    ))
    .unwrap();
    let extension_schema_text = fixture("synthetic_extension_schema.txt");
    let extension_schema =
        parse_schema_storage(&schema_envelope(&[&extension_schema_text])).unwrap();
    let extension = ExtensionMetadata {
        origin: "SyntheticExtension".to_owned(),
        db_names: extension_db_names,
        descriptors: vec![
            descriptor(owner, field_guids[0], "ExtensionText"),
            descriptor(owner, field_guids[1], "ExtensionNumber"),
            descriptor(owner, field_guids[2], "ExtensionReference"),
        ],
        schema: extension_schema,
    };
    let resolved = resolve_metadata_with_extensions(
        base_db_names,
        base_descriptors,
        vec![extension],
        base_schema,
        live_tables,
    );
    let owner_id = resolved
        .snapshot
        .object_id(MetadataKind::Catalog, "ExtendedCatalog")
        .unwrap();
    let extension_field = resolved
        .snapshot
        .attribute_by_id(
            resolved
                .snapshot
                .attribute_id(owner_id, "ExtensionText")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        extension_field.extension_origin.as_deref(),
        Some("SyntheticExtension")
    );
    let object = resolved.snapshot.object_by_id(owner_id).unwrap();
    let fields = queryable_fields(&resolved.snapshot, object).unwrap();
    assert!(fields.iter().any(|field| field.name == "ExtensionText"));

    let query = "SELECT ExtensionText, ExtensionReference.Description FROM Catalog.ExtendedCatalog WHERE ExtensionNumber > 0 ORDER BY ExtensionText;";
    let postgres = QueryCompiler::new(&resolved.snapshot, PostgresBackend)
        .compile(query)
        .unwrap();
    let mssql = QueryCompiler::new(&resolved.snapshot, MsSqlBackend::default())
        .compile(query)
        .unwrap();
    for column in ["_fld16536", "_fld16537", "_fld16538rref"] {
        assert!(postgres.sql.contains(column), "{}", postgres.sql);
        assert!(mssql.sql.contains(column), "{}", mssql.sql);
    }
    assert!(postgres.sql.contains("_reference14574x1"));
    assert!(mssql.sql.contains("_reference14574x1"));

    let undeclared = resolved
        .report
        .findings()
        .iter()
        .filter_map(|finding| match finding {
            ResolutionFinding::TableNotDeclared { table } => Some(table.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!undeclared.contains(&"_reference14574x1"), "{undeclared:?}");
    assert!(undeclared.contains(&"_really_unknown"), "{undeclared:?}");
}

#[test]
fn extension_field_number_collision_retains_the_base_mapping_and_reports_it() {
    let base_guid = "11111111-1111-4111-8111-111111111111";
    let extension_guid = "22222222-2222-4222-8222-222222222222";
    let base_db_names = parse_db_names(&stored_deflate(
        format!("{{1,{{{base_guid},\"Fld\",42}}}}").as_bytes(),
    ))
    .unwrap();
    let extension_db_names = parse_db_names(&stored_deflate(
        format!("{{1,{{{extension_guid},\"Fld\",42}}}}").as_bytes(),
    ))
    .unwrap();
    let empty_schema = || SchemaStorage {
        tables: Vec::new(),
        anomalies: Vec::new(),
    };
    let resolved = resolve_metadata_with_extensions(
        base_db_names,
        vec![descriptor(base_guid, base_guid, "BaseField")],
        vec![ExtensionMetadata {
            origin: "ConflictingExtension".to_owned(),
            db_names: extension_db_names,
            descriptors: vec![descriptor(base_guid, extension_guid, "ExtensionField")],
            schema: empty_schema(),
        }],
        empty_schema(),
        Vec::new(),
    );

    assert_eq!(
        resolved
            .snapshot
            .db_names()
            .field_guid(42)
            .unwrap()
            .as_str(),
        base_guid
    );
    let fields = resolved
        .snapshot
        .fields()
        .iter()
        .filter(|field| field.number == 42)
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].guid.as_str(), base_guid);
    assert_eq!(fields[0].name.as_deref(), Some("BaseField"));
    assert!(resolved.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::ExtensionFieldNumberConflict {
            extension,
            number: 42,
            base_guid: reported_base,
            extension_guid: reported_extension,
        } if extension == "ConflictingExtension"
            && reported_base.as_str() == base_guid
            && reported_extension.as_str() == extension_guid
    )));
}

#[test]
fn compiles_service_table_sources_on_both_dialects() {
    let account = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let calculation_kinds = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let exchange_plan = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
    let registered = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    let extra_dimension = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    let db_names = parse_db_names(&stored_deflate(
        format!(
            "{{7,{{{account},\"Acc\",3930}},{{{calculation_kinds},\"CKinds\",3929}},{{{exchange_plan},\"Node\",10}},{{{registered},\"AccumRg\",1273}},{{{registered},\"AccumRgChngR\",1273}},{{{extra_dimension},\"ExtDim\",3937}},{{ffffffff-ffff-4fff-8fff-ffffffffffff,\"CRgRecalc\",3975}}}}"
        )
        .as_bytes(),
    ))
    .unwrap();
    let descriptors = vec![
        descriptor(account, account, "MainAccounts"),
        descriptor(calculation_kinds, calculation_kinds, "Payroll"),
        descriptor(exchange_plan, exchange_plan, "MainExchange"),
        descriptor(registered, registered, "RegisteredTotals"),
    ];
    let displaced = fixture("ckinds_baseck_inline.txt")
        .replace("BaseCKBaseCK", "DisplacedCKDisplCK")
        .replace("PredefinedBaseCK", "PredefinedDisplCK")
        .replace("BaseCKLineNo", "DisplacedCKLineNo")
        .replace("\"BaseCK\"", "\"DisplacedCK\"");
    let records = [
        r#"{"Acc3930","N",3930,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        r#"{"CKinds3929","N",3929,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        r#"{"Node10","N",10,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        r#"{"AccumRg1273","N",1273,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        fixture("acc_extdim_inline.txt"),
        fixture("ckinds_baseck_inline.txt"),
        fixture("ckinds_leadingck_inline.txt"),
        displaced,
        fixture("accumrg_chngr.txt"),
        fixture("crg_recalc.txt"),
    ];
    let refs = records.iter().map(String::as_str).collect::<Vec<_>>();
    let schema = parse_schema_storage(&schema_envelope(&refs)).unwrap();
    let mut live_tables = live_tables_from_fixture();
    for name in ["_Acc3930", "_CKinds3929", "_Node10", "_AccumRg1273"] {
        live_tables.push(LiveTable {
            name: name.to_owned(),
            columns: Vec::new(),
            indexes: Vec::new(),
        });
    }
    live_tables.push(LiveTable {
        name: "_AccumRgChngR1273".to_owned(),
        columns: [
            ("_NodeTRef", "binary(4)"),
            ("_NodeRRef", "binary(16)"),
            ("_MessageNo", "numeric(10,0)"),
            ("_RecorderRRef", "binary(16)"),
            ("_Fld2683", "numeric(7,0)"),
        ]
        .into_iter()
        .map(|(name, data_type)| LiveColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
        })
        .collect(),
        indexes: Vec::new(),
    });
    let snapshot = resolve_metadata(db_names, descriptors, schema, live_tables).snapshot;
    let dependencies = snapshot
        .objects()
        .iter()
        .filter(|object| object.kind == Some(MetadataKind::CalculationKindDependency))
        .collect::<Vec<_>>();
    assert_eq!(dependencies.len(), 3);
    assert!(dependencies.iter().all(|object| object.owner.is_some()));

    let queries = [
        (
            "SELECT Node, MessageNo FROM AccumulationRegister.RegisteredTotals.Changes;",
            "_AccumRgChngR1273",
        ),
        (
            "ВЫБРАТЬ Node, MessageNo ИЗ РегистрНакопления.RegisteredTotals.Изменения;",
            "_AccumRgChngR1273",
        ),
        (
            "SELECT LineNo FROM ChartOfCalculationTypes.Payroll.BaseCalculationKinds;",
            "_CKinds3929_BaseCK",
        ),
        (
            "SELECT LineNo FROM ПланВидовРасчета.Payroll.ВедущиеВидыРасчета;",
            "_CKinds3929_LeadingCK",
        ),
        (
            "SELECT LineNo FROM ChartOfCalculationTypes.Payroll.DisplacedCalculationKinds;",
            "_CKinds3929_DisplacedCK",
        ),
        (
            "SELECT DimKind FROM ChartOfAccounts.MainAccounts.ExtraDimensions;",
            "_Acc3930_ExtDim3937",
        ),
    ];
    for (query, table) in queries {
        let postgres = QueryCompiler::new(&snapshot, PostgresBackend).compile(query);
        let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::default()).compile(query);
        assert!(
            postgres
                .as_ref()
                .unwrap()
                .sql
                .contains(&format!("\"{table}\""))
        );
        assert!(mssql.as_ref().unwrap().sql.contains(&format!("[{table}]")));
        assert_eq!(
            postgres.as_ref().unwrap().columns,
            mssql.as_ref().unwrap().columns
        );
    }

    let unsupported = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Recorder FROM Recalculation.CRgRecalc;")
        .unwrap_err();
    assert_eq!(unsupported.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn change_registration_resolves_by_registered_object_ownership() {
    let first = "11111111-1111-4111-8111-111111111111";
    let second = "22222222-2222-4222-8222-222222222222";
    let db_names = parse_db_names(&stored_deflate(
        format!(
            "{{4,{{{first},\"AccumRg\",1273}},{{{first},\"AccumRgChngR\",1273}},{{{second},\"InfoRg\",2000}},{{{second},\"InfoRgChngR\",2000}}}}"
        )
        .as_bytes(),
    ))
    .unwrap();
    let descriptors = vec![
        descriptor(first, first, "FirstRegisteredObject"),
        descriptor(second, second, "SecondRegisteredObject"),
    ];
    let first_changes = fixture("accumrg_chngr.txt");
    let second_changes = first_changes
        .replace("AccumRgChngR1273", "InfoRgChngR2000")
        .replace("Fld2683", "Fld3000");
    let records = [
        r#"{"AccumRg1273","N",1273,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        r#"{"InfoRg2000","N",2000,"",{0},{0},{0},1,"R",{0},{0},"",0}"#.to_owned(),
        first_changes,
        second_changes,
    ];
    let refs = records.iter().map(String::as_str).collect::<Vec<_>>();
    let schema = parse_schema_storage(&schema_envelope(&refs)).unwrap();
    let service_columns = |name: &str, field: &str| LiveTable {
        name: name.to_owned(),
        columns: [
            ("_NodeTRef", "binary(4)"),
            ("_NodeRRef", "binary(16)"),
            ("_MessageNo", "numeric(10,0)"),
            ("_RecorderRRef", "binary(16)"),
            (field, "numeric(7,0)"),
        ]
        .into_iter()
        .map(|(name, data_type)| LiveColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
        })
        .collect(),
        indexes: Vec::new(),
    };
    let live_tables = vec![
        LiveTable {
            name: "_AccumRg1273".to_owned(),
            columns: Vec::new(),
            indexes: Vec::new(),
        },
        LiveTable {
            name: "_InfoRg2000".to_owned(),
            columns: Vec::new(),
            indexes: Vec::new(),
        },
        service_columns("_AccumRgChngR1273", "_Fld2683"),
        service_columns("_InfoRgChngR2000", "_Fld3000"),
    ];
    let snapshot = resolve_metadata(db_names, descriptors, schema, live_tables).snapshot;

    let first_sql = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Node, MessageNo FROM AccumulationRegister.FirstRegisteredObject.Changes;")
        .unwrap()
        .sql;
    let second_sql = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Node, MessageNo FROM InformationRegister.SecondRegisteredObject.Changes;")
        .unwrap()
        .sql;

    assert!(first_sql.contains("\"_AccumRgChngR1273\""), "{first_sql}");
    assert!(!first_sql.contains("\"_InfoRgChngR2000\""), "{first_sql}");
    assert!(second_sql.contains("\"_InfoRgChngR2000\""), "{second_sql}");
    assert!(
        !second_sql.contains("\"_AccumRgChngR1273\""),
        "{second_sql}"
    );
}
