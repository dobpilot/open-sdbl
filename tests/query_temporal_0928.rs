mod support;
use open_sdbl::query::*;
#[test]
fn point_projection_has_an_explicit_logical_recipe() {
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let mut session = SessionParameters::new();
    support::supply_active_separation_conditions(&snapshot, &mut session);
    for name in ["ЗначениеРазделителя", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(QueryParameter::new(
            name,
            ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
    }
    session.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(true),
    ));
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "SELECT Д.МоментВремени AS М FROM Документ.ВходящееПисьмо AS Д",
            &CompileOptions::new().session(&session),
        )
        .unwrap();
    let projection = query
        .logical_projection
        .expect("point pair must have a logical recipe");
    assert_eq!(projection.len(), 1);
    assert_eq!(
        projection[0].value,
        LogicalResultValue::PointInTime {
            date: 0,
            reference: 1
        }
    );
}

fn temporal_snapshot(runtime_typed: bool) -> open_sdbl::metadata::MetadataSnapshot {
    use open_sdbl::metadata::*;
    let base = support::accumulation_register_snapshot();
    let mut entries = base.db_names().entries().to_vec();
    entries.extend([
        DbNameEntry {
            guid: support::guid("11111111-1111-4111-8111-111111111111"),
            alias: "Document".to_owned(),
            number: 53,
        },
        DbNameEntry {
            guid: support::guid("22222222-2222-4222-8222-222222222222"),
            alias: "Document".to_owned(),
            number: 54,
        },
    ]);
    let mut schema = base.schema().clone();
    let mut live = base.live_tables().to_vec();
    let document_base = support::snapshot();
    for number in [53, 54] {
        let mut table = document_base.schema().tables[0].clone();
        table.name = format!("Document{number}");
        table.number = number;
        table
            .columns
            .retain(|column| column.name == "ID" || column.name == "Date_Time");
        schema.tables.push(table);
        let mut table = document_base.live_tables()[0].clone();
        table.name = format!("_document{number}");
        table
            .columns
            .retain(|column| column.name == "_idrref" || column.name == "_date_time");
        live.push(table);
    }
    if runtime_typed {
        let recorder = schema.tables[0]
            .columns
            .iter_mut()
            .find(|column| column.name == "Recorder")
            .unwrap();
        recorder.types.push(ColumnType {
            tag: "R".to_owned(),
            reference_target: Some("Document54".to_owned()),
        });
        live[0].columns.push(LiveColumn {
            name: "_recordertref".to_owned(),
            data_type: "bytea".to_owned(),
        });
    }
    resolve_metadata(
        DbNames::from_entries(entries),
        base.descriptors().to_vec(),
        schema,
        live,
    )
    .snapshot
}
fn date() -> ParameterDate {
    ParameterDate::new(2026, 10, 19, 12, 0, 0).unwrap()
}
fn compile_bound<B: Backend>(
    snapshot: &open_sdbl::metadata::MetadataSnapshot,
    backend: B,
    value: ParameterValue,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let parameters = [QueryParameter::new("Период", value)];
    QueryCompiler::new(snapshot, backend).compile_with(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(&Период)",
        &CompileOptions::new().parameters(&parameters),
    )
}
#[test]
fn measured_date_and_empty_point_boundaries_keep_inclusion() {
    let snapshot = temporal_snapshot(false);
    let object = snapshot.object_id_by_physical_table("Document53").unwrap();
    for kind in [BoundaryKind::Including, BoundaryKind::Excluding] {
        for value in [
            ParameterBoundaryValue::Date(date()),
            ParameterBoundaryValue::PointInTime(ParameterPointInTime {
                date: date(),
                reference: None,
            }),
            ParameterBoundaryValue::PointInTime(ParameterPointInTime {
                date: date(),
                reference: Some(ParameterReference {
                    object,
                    id: [0; 16],
                }),
            }),
        ] {
            let bound = ParameterValue::Boundary(ParameterBoundary { value, kind });
            let pg = compile_bound(&snapshot, PostgresBackend, bound.clone()).unwrap();
            assert!(
                pg.sql.contains(if kind == BoundaryKind::Including {
                    "\"_period\" <="
                } else {
                    "\"_period\" <"
                }),
                "{}",
                pg.sql
            );
            assert!(!pg.sql.contains("recorderrref"));
            let ms = compile_bound(&snapshot, MsSqlBackend::new(2000).unwrap(), bound).unwrap();
            assert!(ms.sql.contains("DATEADD(year, 2000,"), "{}", ms.sql);
            assert!(!ms.sql.contains("(WITH "));
        }
    }
    assert_eq!(
        ParameterBoundary::new(ParameterBoundaryValue::Date(date())).kind,
        BoundaryKind::Including
    );
}
#[test]
fn nonempty_points_preserve_recorder_and_storage_type_for_both_dialects() {
    for runtime_typed in [false, true] {
        let snapshot = temporal_snapshot(runtime_typed);
        let object = snapshot.object_id_by_physical_table("Document53").unwrap();
        let point = ParameterPointInTime {
            date: date(),
            reference: Some(ParameterReference {
                object,
                id: [7; 16],
            }),
        };
        for kind in [BoundaryKind::Including, BoundaryKind::Excluding] {
            let value = ParameterValue::Boundary(ParameterBoundary {
                value: ParameterBoundaryValue::PointInTime(point),
                kind,
            });
            let pg = compile_bound(&snapshot, PostgresBackend, value.clone()).unwrap();
            assert!(pg.sql.contains("_recorderrref"), "{}", pg.sql);
            assert!(pg.sql.contains(if kind == BoundaryKind::Including {
                "\"_recorderrref\" <="
            } else {
                "\"_recorderrref\" <"
            }));
            if runtime_typed {
                assert!(pg.sql.contains("_recordertref"), "{}", pg.sql);
            }
            let ms = compile_bound(&snapshot, MsSqlBackend::new(0).unwrap(), value).unwrap();
            assert!(ms.sql.contains("[_recorderrref]"));
        }
        let raw = compile_bound(
            &snapshot,
            PostgresBackend,
            ParameterValue::PointInTime(point),
        )
        .unwrap();
        assert!(raw.sql.contains("\"_recorderrref\" <"));
    }
}
#[test]
fn temporal_parameters_do_not_escape_into_scalar_or_turnover_positions() {
    let snapshot = temporal_snapshot(false);
    let parameters = [QueryParameter::new(
        "Период",
        ParameterValue::PointInTime(ParameterPointInTime {
            date: date(),
            reference: None,
        }),
    )];
    for source in [
        "SELECT &Период",
        "SELECT Количество FROM AccumulationRegister.Остатки WHERE Количество = &Период",
        "SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(&Период)",
    ] {
        assert!(
            QueryCompiler::new(&snapshot, PostgresBackend)
                .compile_with(source, &CompileOptions::new().parameters(&parameters))
                .is_err(),
            "{source}"
        );
    }
}
#[test]
fn temporal_references_require_document_metadata_even_when_empty() {
    let snapshot = temporal_snapshot(false);
    let object = snapshot.object_id_by_physical_table("AccumRg53").unwrap();
    let error = compile_bound(
        &snapshot,
        PostgresBackend,
        ParameterValue::PointInTime(ParameterPointInTime {
            date: date(),
            reference: Some(ParameterReference {
                object,
                id: [0; 16],
            }),
        }),
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn temporal_balances_keep_restrictions_and_required_access_decisions() {
    let snapshot = temporal_snapshot(true);
    let target = snapshot.object_id_by_physical_table("AccumRg53").unwrap();
    let reference = snapshot.object_id_by_physical_table("Document53").unwrap();
    let parameters = [QueryParameter::new(
        "Период",
        ParameterValue::PointInTime(ParameterPointInTime {
            date: date(),
            reference: Some(ParameterReference {
                object: reference,
                id: [3; 16],
            }),
        }),
    )];
    let restrictions = [AccessRestriction::new(target, "Номенклатура ЕСТЬ НЕ NULL")];
    let source =
        "SELECT ALLOWED КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(&Период)";
    let query = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            source,
            &CompileOptions::new()
                .parameters(&parameters)
                .restrictions(&restrictions),
        )
        .unwrap();
    assert!(
        query
            .sql
            .contains("\"__movement_base\".\"_fld54\" IS NOT NULL")
    );
    assert!(
        query
            .sql
            .contains("\"__totals_base\".\"_fld54\" IS NOT NULL")
    );
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare_with_options(source, &PrepareOptions::new().restricted())
        .unwrap()
        .compile_with(&snapshot, &CompileOptions::new().parameters(&parameters))
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Restriction);
}

#[test]
fn legacy_parameter_binding_errors_keep_their_early_diagnostic() {
    use open_sdbl::metadata::{DbNames, resolve_metadata};
    let base = temporal_snapshot(false);
    let entries = base
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.alias != "AccumRgT")
        .cloned()
        .collect();
    let snapshot = resolve_metadata(
        DbNames::from_entries(entries),
        base.descriptors().to_vec(),
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot;
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(&MissingDate)",
            &CompileOptions::new().parameters(&[]),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Parameter, "{error}");
    assert!(error.message().contains("MissingDate"));
}
