use open_sdbl::metadata::{
    ColumnType, ConfigFieldPurpose, FieldId, LiveColumn, LiveIndex, LiveTable, LookupError,
    MetadataKind, SchemaColumn, StandardFieldId, parse_config_descriptors, parse_db_names,
    parse_schema_storage, resolve_metadata,
};
use open_sdbl::query::{
    PresentationExpression, PresentationPlan, compile_mssql_query,
    compile_mssql_query_with_year_offset, compile_postgres_query, find_metadata_object,
    prepare_mssql_query, prepare_postgres_query, queryable_field_catalog, queryable_fields,
};

#[test]
fn compiles_native_mssql_projection_filter_and_limit() {
    let snapshot = mssql_snapshot();
    let compiled = compile_mssql_query(
        "SELECT TOP 10 Code, ProbeAttribute FROM Catalog.OpenSdblMetadataProbe WHERE Code = \"\u{420}\u{430}\u{437}\u{43e}\u{432}\u{44b}\u{439}\";",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Code", "ProbeAttribute"]);
    assert_eq!(
        compiled.sql,
        "SELECT TOP (10) CONVERT(nvarchar(max), \"__src\".\"_code\") AS \"Code\", CONVERT(varchar(max), \"__src\".\"_fld54\", 1) AS \"ProbeAttribute\" FROM \"_reference53\" AS \"__src\" WHERE (\"__src\".\"_code\" = N'\u{420}\u{430}\u{437}\u{43e}\u{432}\u{44b}\u{439}')"
    );
    assert!(!compiled.sql.contains("::"));
    assert!(!compiled.sql.contains(" LIMIT "));
}

#[test]
fn preserves_native_mssql_rowversion_projection() {
    for data_type in ["timestamp", "rowversion"] {
        let mut snapshot = mssql_snapshot();
        snapshot.live_tables[0].columns.push(LiveColumn {
            name: "_version".to_owned(),
            data_type: data_type.to_owned(),
        });

        let compiled = compile_mssql_query(
            "SELECT Version FROM Catalog.OpenSdblMetadataProbe;",
            &snapshot,
        )
        .unwrap();

        assert_eq!(compiled.columns, ["Version"]);
        assert_eq!(
            compiled.sql,
            "SELECT \"__src\".\"_version\" AS \"Version\" FROM \"_reference53\" AS \"__src\""
        );
    }
}

#[test]
fn compiles_binary_literals_for_each_sql_dialect() {
    let mut mssql = mssql_snapshot();
    mssql.live_tables[0].columns.push(LiveColumn {
        name: "_version".to_owned(),
        data_type: "timestamp".to_owned(),
    });
    let compiled = compile_mssql_query(
        "SELECT Version FROM Catalog.OpenSdblMetadataProbe WHERE Version > 0x00000000000007D6;",
        &mssql,
    )
    .unwrap();
    assert!(
        compiled
            .sql
            .contains("(\"__src\".\"_version\" > 0x00000000000007D6)")
    );

    let compiled = compile_postgres_query(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = 0XCAFE;",
        &snapshot(),
    )
    .unwrap();
    assert!(
        compiled
            .sql
            .contains("(\"__src\".\"_fld54\" = '\\xCAFE'::bytea)")
    );
}

#[test]
fn compiles_mssql_extension_tables_as_one_source_relation() {
    let mut snapshot = mssql_snapshot();
    let mut extension = snapshot.live_tables[0].clone();
    extension.name = "_reference53X1".to_owned();
    extension
        .columns
        .retain(|column| column.name != "_date_time");
    extension.columns.push(LiveColumn {
        name: "_extension_only".to_owned(),
        data_type: "nvarchar(10)".to_owned(),
    });
    snapshot.live_tables.push(extension);
    let mut unrelated = snapshot.live_tables[0].clone();
    unrelated.name = "_reference53Xother".to_owned();
    snapshot.live_tables.push(unrelated);

    let compiled = compile_mssql_query(
        "SELECT Code, Date FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains("FROM (SELECT"));
    assert!(
        compiled
            .sql
            .contains("FROM \"_reference53\" UNION ALL SELECT")
    );
    assert!(compiled.sql.contains("NULL AS \"_date_time\""));
    assert!(compiled.sql.contains("FROM \"_reference53X1\""));
    assert!(!compiled.sql.contains("_extension_only"));
    assert!(!compiled.sql.contains("_reference53Xother"));
}

#[test]
fn compiles_mssql_presentation_join_over_extension_tables() {
    let mut snapshot = reference_snapshot();
    let mut extension = snapshot.live_tables[0].clone();
    extension.name = "_reference53X1".to_owned();
    snapshot.live_tables.push(extension);
    let prepared = prepare_mssql_query(
        "SELECT Presentation(Организация) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    let target = prepared.presentation_request().targets[0].object;
    let code = FieldId::Standard(StandardFieldId::Code);
    let plan = PresentationPlan {
        object: target,
        fields: vec![code],
        expression: PresentationExpression::Concat(vec![
            PresentationExpression::Literal("[".to_owned()),
            PresentationExpression::Field(code),
            PresentationExpression::Literal("]".to_owned()),
        ]),
    };

    let compiled = prepared.compile(&snapshot, &[plan]).unwrap();

    assert!(
        compiled.sql.contains("LEFT JOIN (SELECT"),
        "{}",
        compiled.sql
    );
    assert!(
        compiled
            .sql
            .contains("FROM \"_reference53\" UNION ALL SELECT")
    );
    assert!(compiled.sql.contains("FROM \"_reference53X1\""));
    assert!(compiled.sql.contains("AS \"__ref1\" ON"));
}

#[test]
fn compiles_mssql_historical_balance_without_postgres_aggregate_syntax() {
    let mut snapshot = accumulation_register_snapshot();
    for table in &mut snapshot.live_tables {
        for column in &mut table.columns {
            if column.name == "_active" {
                column.data_type = "binary(1)".to_owned();
            }
        }
    }
    let compiled = compile_mssql_query_with_year_offset(
        "SELECT TOP 5 \u{41a}\u{43e}\u{43b}\u{438}\u{447}\u{435}\u{441}\u{442}\u{432}\u{43e}\u{41e}\u{441}\u{442}\u{430}\u{442}\u{43e}\u{43a} FROM AccumulationRegister.\u{41e}\u{441}\u{442}\u{430}\u{442}\u{43a}\u{438}.Balance(\"2026-09-01\");",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(compiled.sql.starts_with("SELECT TOP (5)"));
    assert!(compiled.sql.contains("MAX(CASE WHEN"));
    assert!(compiled.sql.contains(" = 0x01"));
    assert!(compiled.sql.contains("DATEADD(year, 2000, N'2026-09-01')"));
    assert!(!compiled.sql.contains(" FILTER ("));
    assert!(!compiled.sql.contains("(WITH "));
}

#[test]
fn translates_mssql_year_offset_in_date_projection_and_filter() {
    let snapshot = mssql_snapshot();
    let compiled = compile_mssql_query_with_year_offset(
        "SELECT Date FROM Catalog.OpenSdblMetadataProbe WHERE Date >= \"2026-09-01\";",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(
        compiled
            .sql
            .contains("DATEADD(year, -2000, \"__src\".\"_date_time\")")
    );
    assert!(compiled.sql.contains("DATEADD(year, 2000, N'2026-09-01')"));
}

#[test]
fn prepares_and_compiles_mssql_presentations() {
    let snapshot = reference_snapshot();
    let prepared = prepare_mssql_query(
        "SELECT TOP 1 Presentation(\u{41e}\u{440}\u{433}\u{430}\u{43d}\u{438}\u{437}\u{430}\u{446}\u{438}\u{44f}) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    let request = prepared.presentation_request();
    assert_eq!(request.targets.len(), 1);
    let plan = PresentationPlan {
        object: request.targets[0].object,
        fields: Vec::new(),
        expression: PresentationExpression::Literal(
            "\u{43e}\u{431}\u{44a}\u{435}\u{43a}\u{442}".to_owned(),
        ),
    };
    let compiled = prepared.compile(&snapshot, &[plan]).unwrap();
    assert!(compiled.sql.starts_with("SELECT TOP (1)"));
    assert!(
        compiled
            .sql
            .contains("N'\u{43e}\u{431}\u{44a}\u{435}\u{43a}\u{442}'")
    );
    assert!(!compiled.sql.contains("::bytea"));
}

#[test]
fn compiles_source_free_literals_and_scalar_presentations() {
    let snapshot = snapshot();
    let literal = compile_postgres_query("SELECT 4;", &snapshot).unwrap();
    assert_eq!(literal.columns, ["column1"]);
    assert_eq!(literal.sql, "SELECT (4)::text AS \"column1\"");

    let presentation = compile_postgres_query("select представление(4);", &snapshot).unwrap();
    assert_eq!(presentation.columns, ["представление"]);
    assert_eq!(presentation.sql, "SELECT (4)::text AS \"представление\"");

    let multiline = compile_postgres_query("select\nпредставление(4);", &snapshot).unwrap();
    assert_eq!(multiline.sql, presentation.sql);
}

#[test]
fn compiles_source_free_arithmetic_and_rejects_fields_without_from() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query("SELECT 2 + 2, \"готово\";", &snapshot).unwrap();
    assert_eq!(compiled.columns, ["column1", "column2"]);
    assert_eq!(
        compiled.sql,
        "SELECT ((2 + 2))::text AS \"column1\", ('готово')::text AS \"column2\""
    );

    let field = compile_postgres_query("SELECT Код;", &snapshot).unwrap_err();
    assert!(field.message().contains("requires FROM"));
    let wildcard = compile_postgres_query("SELECT *;", &snapshot).unwrap_err();
    assert!(wildcard.message().contains("requires FROM"));
}

#[test]
fn compiles_count_all_and_distinct_field() {
    let snapshot = snapshot();
    let all = compile_postgres_query(
        "select count(*) from Справочник.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(all.columns, ["count"]);
    assert_eq!(
        all.sql,
        "SELECT COUNT(*)::text AS \"count\" FROM \"_reference53\" AS \"__src\""
    );

    let distinct = compile_postgres_query(
        "ВЫБРАТЬ КОЛИЧЕСТВО(РАЗЛИЧНЫЕ Код) ИЗ Справочник.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(distinct.columns, ["КОЛИЧЕСТВО"]);
    assert!(
        distinct
            .sql
            .contains("COUNT(DISTINCT \"__src\".\"_code\")::text")
    );
}

#[test]
fn bounds_count_aggregate_shapes() {
    let snapshot = snapshot();
    let source_free = compile_postgres_query("SELECT COUNT(*);", &snapshot).unwrap();
    assert_eq!(source_free.sql, "SELECT COUNT(*)::text AS \"COUNT\"");

    let mixed = compile_postgres_query(
        "SELECT COUNT(*), Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(mixed.message().contains("cannot be mixed"));

    let full = compile_postgres_query(
        "SELECT COUNT(*) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(full.message().contains("transposed FULL JOIN"));
}

#[test]
fn compiles_sum_min_max_and_count_distinct_together() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "SELECT SUM(ProbeAttribute), МИНИМУМ(ProbeAttribute), MAX(ProbeAttribute), КОЛИЧЕСТВО(РАЗЛИЧНЫЕ ProbeAttribute) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(compiled.columns, ["SUM", "МИНИМУМ", "MAX", "КОЛИЧЕСТВО"]);
    assert!(compiled.sql.contains("SUM(\"__src\".\"_fld54\")::text"));
    assert!(compiled.sql.contains("MIN(\"__src\".\"_fld54\")::text"));
    assert!(compiled.sql.contains("MAX(\"__src\".\"_fld54\")::text"));
    assert!(
        compiled
            .sql
            .contains("COUNT(DISTINCT \"__src\".\"_fld54\")::text")
    );
}

#[test]
fn rejects_wildcard_and_distinct_for_non_count_aggregates() {
    let snapshot = snapshot();
    let wildcard = compile_postgres_query(
        "SELECT SUM(*) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("only by COUNT"));

    let distinct = compile_postgres_query(
        "SELECT MIN(DISTINCT Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(distinct.message().contains("only by COUNT"));
}

#[test]
fn compiles_information_register_slice_last_by_config_dimensions() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT ProbeAttribute, Period FROM InformationRegister.Prices.SliceLast();",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "DENSE_RANK() OVER (PARTITION BY \"__slice_base\".\"_fld54\" ORDER BY \"__slice_base\".\"_period\" DESC)"
    ));
    assert!(
        compiled
            .sql
            .contains("FROM \"_inforg53\" AS \"__slice_base\"")
    );
    assert!(
        compiled
            .sql
            .contains("\"__open_sdbl_slice_rank\" = 1) AS \"__src\"")
    );
}

#[test]
fn resolves_information_register_field_purpose_from_config() {
    let base = snapshot();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "2dcd4b0a02310c00d0bbcc3a81f4df2cbd815748d21666258c75557a7745dd3f78cbc35a0e08b4aa58c98ac64e31b652b14a211c43028fda7ae6b8e1b85fa7f5e7018bf686e5820bd153c09149d1b996508215244aa4d2494ac9e0fe834fc659db078b458cbe0fd4cc094b34961143aaacdfe1a1fd36e775ea6bf6dfb4f71b",
        ),
    )
    .unwrap();
    let resolved = resolve_metadata(
        base.db_names.clone(),
        descriptors,
        base.schema.clone(),
        base.live_tables.clone(),
    );
    assert_eq!(
        resolved.fields[0].purpose,
        Some(ConfigFieldPurpose::InformationRegisterDimension)
    );
}

#[test]
fn resolves_accumulation_register_field_purpose_from_config() {
    let base = snapshot();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "05c13b0ec3300800d0bb6406096c8ccbde03f40ae08f942189d48e96efdef756a87473c9c82a0999bba2e75691a850f820af5581612d0682549a69f48cd39ba0a43131d40a5669e6537279596c383edf27c6fbbcc6fd3b9ffb80457bef3f",
        ),
    )
    .unwrap();
    let resolved = resolve_metadata(
        base.db_names.clone(),
        descriptors,
        base.schema.clone(),
        base.live_tables.clone(),
    );
    assert_eq!(
        resolved.fields[0].purpose,
        Some(ConfigFieldPurpose::AccumulationRegisterDimension)
    );
}

#[test]
fn applies_slice_last_parameters_before_and_where_after_ranking() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ Период ИЗ РегистрСведений.Prices.СрезПоследних(\"2026-08-30\", ProbeAttribute ЕСТЬ НЕ NULL) ГДЕ Period > \"2020-01-01\";",
        &snapshot,
    )
    .unwrap();

    let rank = compiled.sql.find("DENSE_RANK()").unwrap();
    let bound = compiled
        .sql
        .find("\"__slice_base\".\"_period\" <= '2026-08-30'")
        .unwrap();
    let virtual_condition = compiled
        .sql
        .find("\"__slice_base\".\"_fld54\" IS NOT NULL")
        .unwrap();
    let outer_where = compiled
        .sql
        .rfind("WHERE (\"__src\".\"_period\" > '2020-01-01')")
        .unwrap();
    assert!(rank < bound && bound < outer_where);
    assert!(rank < virtual_condition && virtual_condition < outer_where);
}

#[test]
fn supports_slice_last_as_a_join_source() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT l.Period, r.Period FROM InformationRegister.Prices.SliceLast() l LEFT JOIN InformationRegister.Prices r ON l.ProbeAttribute = r.ProbeAttribute;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains("DENSE_RANK() OVER"));
    assert!(
        compiled
            .sql
            .contains(") AS \"l\" LEFT JOIN \"_inforg53\" AS \"r\"")
    );
}

#[test]
fn rejects_slice_last_for_invalid_sources_and_arguments() {
    let catalog = snapshot();
    let wrong_kind = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe.SliceLast();",
        &catalog,
    )
    .unwrap_err();
    assert!(
        wrong_kind
            .message()
            .contains("only for information registers")
    );

    let register = information_register_snapshot();
    let expression = compile_postgres_query(
        "SELECT Period FROM InformationRegister.Prices.SliceLast(2 + 2);",
        &register,
    )
    .unwrap_err();
    assert!(expression.message().contains("scalar literal"));

    let parameter = compile_postgres_query(
        "SELECT Period FROM InformationRegister.Prices.SliceLast(&Period);",
        &register,
    )
    .unwrap_err();
    assert!(parameter.message().contains("parameters are not supported"));
}

#[test]
fn compiles_information_register_slice_first_by_config_dimensions() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT ProbeAttribute, Period FROM InformationRegister.Prices.SliceFirst();",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "DENSE_RANK() OVER (PARTITION BY \"__slice_base\".\"_fld54\" ORDER BY \"__slice_base\".\"_period\" ASC)"
    ));
    assert!(
        compiled
            .sql
            .contains("\"__open_sdbl_slice_rank\" = 1) AS \"__src\"")
    );
}

#[test]
fn applies_slice_first_lower_bound_before_and_where_after_ranking() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ Период ИЗ РегистрСведений.Prices.СрезПервых(\"2026-08-01\", ProbeAttribute ЕСТЬ НЕ NULL) ГДЕ Period < \"2026-09-01\";",
        &snapshot,
    )
    .unwrap();

    let rank = compiled.sql.find("DENSE_RANK()").unwrap();
    let bound = compiled
        .sql
        .find("\"__slice_base\".\"_period\" >= '2026-08-01'")
        .unwrap();
    let virtual_condition = compiled
        .sql
        .find("\"__slice_base\".\"_fld54\" IS NOT NULL")
        .unwrap();
    let outer_where = compiled
        .sql
        .rfind("WHERE (\"__src\".\"_period\" < '2026-09-01')")
        .unwrap();
    assert!(rank < bound && bound < outer_where);
    assert!(rank < virtual_condition && virtual_condition < outer_where);
}

#[test]
fn supports_slice_first_as_a_join_source() {
    let snapshot = information_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT l.Period, r.Period FROM InformationRegister.Prices.SliceFirst() l INNER JOIN InformationRegister.Prices.SliceLast() r ON l.ProbeAttribute = r.ProbeAttribute;",
        &snapshot,
    )
    .unwrap();

    assert!(
        compiled
            .sql
            .contains("ORDER BY \"__slice_base\".\"_period\" ASC")
    );
    assert!(
        compiled
            .sql
            .contains("ORDER BY \"__slice_base\".\"_period\" DESC")
    );
    assert!(compiled.sql.contains(") AS \"l\" INNER JOIN (SELECT"));
}

#[test]
fn rejects_slice_first_for_invalid_sources_and_arguments() {
    let catalog = snapshot();
    let wrong_kind = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe.SliceFirst();",
        &catalog,
    )
    .unwrap_err();
    assert!(
        wrong_kind
            .message()
            .contains("SliceFirst is supported only for information registers")
    );

    let mut register = information_register_snapshot();
    register.live_tables[0]
        .columns
        .retain(|column| column.name != "_period");
    let missing_period = compile_postgres_query(
        "SELECT ProbeAttribute FROM InformationRegister.Prices.SliceFirst();",
        &register,
    )
    .unwrap_err();
    assert!(missing_period.message().contains("requires a live Period"));

    let register = information_register_snapshot();
    let parameter = compile_postgres_query(
        "SELECT Period FROM InformationRegister.Prices.SliceFirst(&Period);",
        &register,
    )
    .unwrap_err();
    assert!(parameter.message().contains("parameters are not supported"));
}

#[test]
fn compiles_current_accumulation_register_balances() {
    let snapshot = accumulation_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT Номенклатура, КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Номенклатура", "КоличествоОстаток"]);
    assert!(
        compiled
            .sql
            .contains("FROM \"_accumrgt56\" AS \"__totals_base\"")
    );
    assert!(
        compiled
            .sql
            .contains("SELECT MAX(\"__totals_latest\".\"_period\")")
    );
    assert!(!compiled.sql.contains("_accumrg53"));
    assert!(
        compiled
            .sql
            .contains("GROUP BY \"__totals_base\".\"_fld54\"")
    );
    assert!(!compiled.sql.contains("_splitter"));
    assert!(compiled.sql.contains(" HAVING (SUM("));
}

#[test]
fn applies_balance_period_and_condition_before_outer_where() {
    let snapshot = accumulation_register_snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки(\"2026-09-01\", Номенклатура ЕСТЬ НЕ NULL) ГДЕ КоличествоОстаток > 0;",
        &snapshot,
    )
    .unwrap();

    let anchor = compiled
        .sql
        .find("MAX(\"__anchor_totals\".\"_period\") FILTER")
        .unwrap();
    let totals_condition = compiled
        .sql
        .find("\"__totals_base\".\"_fld54\" IS NOT NULL")
        .unwrap();
    let movement_condition = compiled
        .sql
        .find("\"__movement_base\".\"_fld54\" IS NOT NULL")
        .unwrap();
    let union = compiled.sql.find(" UNION ALL ").unwrap();
    let grouping = compiled.sql.rfind(" GROUP BY ").unwrap();
    let outer_where = compiled
        .sql
        .rfind("WHERE (\"__src\".\"_fld55\" > 0)")
        .unwrap();
    assert!(
        anchor < totals_condition
            && totals_condition < union
            && union < movement_condition
            && movement_condition < grouping
            && grouping < outer_where
    );
    assert!(
        compiled
            .sql
            .contains("CASE WHEN \"__balance_anchor\".\"__period\" <= '2026-09-01'")
    );
    assert!(compiled.sql.contains("SELECT COALESCE(MAX("));
    assert!(
        compiled
            .sql
            .contains("FROM \"_accumrg53\" AS \"__movement_base\"")
    );
}

#[test]
fn compiles_bounded_accumulation_register_turnovers() {
    let snapshot = accumulation_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(\"2026-08-01\", \"2026-09-01\",, Номенклатура IS NOT NULL);",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Номенклатура", "КоличествоОборот"]);
    assert!(compiled.sql.contains("\"_period\" >= '2026-08-01'"));
    assert!(compiled.sql.contains("\"_period\" < '2026-09-01'"));
    assert!(compiled.sql.contains("\"_fld54\" IS NOT NULL"));
    assert!(!compiled.sql.contains(" HAVING "));
    assert!(
        compiled
            .sql
            .contains("FROM \"_accumrg53\" AS \"__aggregate_base\"")
    );
    assert!(!compiled.sql.contains("_accumrgt56"));
}

#[test]
fn supports_balance_and_turnovers_as_join_sources() {
    let snapshot = accumulation_register_snapshot();
    let compiled = compile_postgres_query(
        "SELECT b.КоличествоОстаток, t.КоличествоОборот FROM AccumulationRegister.Остатки.Balance() b LEFT JOIN AccumulationRegister.Остатки.Turnovers() t ON b.Номенклатура = t.Номенклатура;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains(") AS \"b\" LEFT JOIN (SELECT"));
    assert!(
        compiled
            .sql
            .contains("ON \"b\".\"_fld54\" = \"t\".\"_fld54\"")
    );
}

#[test]
fn rejects_invalid_accumulation_virtual_table_shapes() {
    let catalog = snapshot();
    let wrong_kind = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe.Balance();",
        &catalog,
    )
    .unwrap_err();
    assert!(
        wrong_kind
            .message()
            .contains("only for accumulation registers")
    );

    let register = accumulation_register_snapshot();
    let periodicity = compile_postgres_query(
        "SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(,,Day,);",
        &register,
    )
    .unwrap_err();
    assert!(
        periodicity
            .message()
            .contains("periodicity is not supported")
    );

    let resource_condition = compile_postgres_query(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(, Количество > 0);",
        &register,
    )
    .unwrap_err();
    assert!(resource_condition.message().contains("was not found"));

    let mut turnover_only = accumulation_register_snapshot();
    turnover_only.live_tables[0]
        .columns
        .retain(|column| column.name != "_recordkind");
    let balance = compile_postgres_query(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &turnover_only,
    )
    .unwrap_err();
    assert!(balance.message().contains("turnover-only"));

    let mut missing_mapping = accumulation_register_snapshot();
    missing_mapping.db_names = snapshot().db_names;
    let balance = compile_postgres_query(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &missing_mapping,
    )
    .unwrap_err();
    assert!(balance.message().contains("AccumRgT entry"));
    let turnovers = compile_postgres_query(
        "SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers();",
        &missing_mapping,
    )
    .unwrap();
    assert!(turnovers.sql.contains("_accumrg53"));
    assert!(!turnovers.sql.contains("_accumrgt56"));

    let mut missing_live_totals = accumulation_register_snapshot();
    missing_live_totals
        .live_tables
        .retain(|table| table.name != "_accumrgt56");
    let balance = compile_postgres_query(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &missing_live_totals,
    )
    .unwrap_err();
    assert!(balance.message().contains("is not live"));

    let mut missing_totals_resource = accumulation_register_snapshot();
    missing_totals_resource.schema.tables[1]
        .columns
        .retain(|column| column.name != "Fld55");
    let balance = compile_postgres_query(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &missing_totals_resource,
    )
    .unwrap_err();
    assert!(balance.message().contains("does not declare field"));
}

#[test]
fn indexed_metadata_lookups_use_guids_and_numeric_standard_fields() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    assert_eq!(snapshot.object_id_by_database_type(53).unwrap(), object);
    assert_eq!(
        snapshot.field_id(object, "Код").unwrap(),
        FieldId::Standard(StandardFieldId::Code)
    );
    assert_eq!(
        snapshot.attribute_id(object, "Код").unwrap_err(),
        LookupError::StandardFieldHasNoMetadataGuid(StandardFieldId::Code)
    );
    let attribute = snapshot.attribute_id(object, "ProbeAttribute").unwrap();
    assert_eq!(
        snapshot.field_id(object, "ProbeAttribute").unwrap(),
        FieldId::Metadata(attribute)
    );
}

#[test]
fn prepares_one_guid_batch_and_compiles_safe_presentations() {
    let snapshot = snapshot();
    let source = "SELECT REFPRESENTATION(Ссылка), PRESENTATION(4), Ссылка.Представление FROM Catalog.OpenSdblMetadataProbe;";
    let prepared = prepare_postgres_query(source, &snapshot).unwrap();
    let request = prepared.presentation_request();
    assert_eq!(request.targets.len(), 1);
    let object = request.targets[0].object;
    let code = FieldId::Standard(StandardFieldId::Code);
    let plan = PresentationPlan {
        object,
        fields: vec![code],
        expression: PresentationExpression::Concat(vec![
            PresentationExpression::Literal("[".to_owned()),
            PresentationExpression::Field(code),
            PresentationExpression::Literal("]".to_owned()),
        ]),
    };
    let compiled = prepared.compile(&snapshot, &[plan]).unwrap();
    assert_eq!(compiled.columns.len(), 3);
    assert!(
        compiled
            .sql
            .contains("concat('[', COALESCE(\"__src\".\"_code\"::text, ''), ']')")
    );
    assert!(compiled.sql.contains("(4)::text"));
    assert!(!compiled.sql.contains("LEFT JOIN"));
}

#[test]
fn presentation_plans_are_required_and_field_ids_are_validated() {
    let snapshot = snapshot();
    let source = "SELECT ПРЕДСТАВЛЕНИЕССЫЛКИ(Ссылка) FROM Справочник.OpenSdblMetadataProbe;";
    let missing = compile_postgres_query(source, &snapshot).unwrap_err();
    assert!(missing.message().contains("missing presentation plan"));

    let prepared = prepare_postgres_query(source, &snapshot).unwrap();
    let object = prepared.presentation_request().targets[0].object;
    let foreign = FieldId::Metadata(open_sdbl::metadata::AttributeId::from_bytes([0xff; 16]));
    let invalid = prepared
        .compile(
            &snapshot,
            &[PresentationPlan {
                object,
                fields: vec![foreign],
                expression: PresentationExpression::Field(foreign),
            }],
        )
        .unwrap_err();
    assert!(invalid.message().contains("invalid presentation field"));
}

#[test]
fn compiles_fixed_and_multi_target_reference_presentations() {
    for (multiple, expected_targets) in [(false, 1), (true, 2)] {
        let snapshot = presentation_reference_snapshot(multiple);
        let source = "SELECT REFPRESENTATION(ProbeAttribute) FROM Catalog.OpenSdblMetadataProbe;";
        let prepared = prepare_postgres_query(source, &snapshot).unwrap();
        assert_eq!(
            prepared.presentation_request().targets.len(),
            expected_targets
        );
        let plans = prepared
            .presentation_request()
            .targets
            .iter()
            .map(|target| PresentationPlan {
                object: target.object,
                fields: vec![FieldId::Standard(StandardFieldId::Code)],
                expression: PresentationExpression::Field(FieldId::Standard(StandardFieldId::Code)),
            })
            .collect::<Vec<_>>();
        let compiled = prepared.compile(&snapshot, &plans).unwrap();
        assert_eq!(compiled.sql.matches("LEFT JOIN").count(), expected_targets);
        if multiple {
            assert!(compiled.sql.contains("\\x00000039"));
            assert!(compiled.sql.contains("\\x0000003a"));
            assert!(compiled.sql.contains("CASE WHEN"));
        }
    }
}

#[test]
fn preserves_presentations_through_full_join_and_union_branches() {
    let snapshot = snapshot();
    let source = "SELECT REFPRESENTATION(l.Ссылка) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code UNION ALL SELECT REFPRESENTATION(Ссылка) FROM Catalog.OpenSdblMetadataProbe;";
    let prepared = prepare_postgres_query(source, &snapshot).unwrap();
    assert_eq!(prepared.presentation_request().targets.len(), 1);
    let object = prepared.presentation_request().targets[0].object;
    let code = FieldId::Standard(StandardFieldId::Code);
    let compiled = prepared
        .compile(
            &snapshot,
            &[PresentationPlan {
                object,
                fields: vec![code],
                expression: PresentationExpression::Field(code),
            }],
        )
        .unwrap();
    assert_eq!(compiled.columns.len(), 1);
    assert!(compiled.sql.matches("UNION ALL").count() >= 2);
    assert!(compiled.sql.contains("AS \"__full\""));
}

#[test]
fn compiles_a_russian_catalog_query_through_authoritative_metadata() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ ПЕРВЫЕ 5 Код, ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК p ГДЕ p.Код = \"A\" УПОРЯДОЧИТЬ ПО p.Код ВОЗР;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Code", "ProbeAttribute"]);
    assert_eq!(
        compiled.sql,
        "SELECT \"p\".\"_code\"::text AS \"Code\", \"p\".\"_fld54\"::text AS \"ProbeAttribute\" FROM \"_reference53\" AS \"p\" WHERE (\"p\".\"_code\" = 'A') ORDER BY \"p\".\"_code\" ASC LIMIT 5"
    );
}

#[test]
fn exposes_standard_and_custom_fields_for_description() {
    let snapshot = snapshot();
    let object = find_metadata_object(&snapshot, "Catalog.OpenSdblMetadataProbe").unwrap();
    let fields = queryable_fields(&snapshot, object).unwrap();

    assert!(fields.iter().any(|field| {
        field.name == "Code" && field.aliases.iter().any(|alias| alias == "Код")
    }));
    assert!(fields.iter().any(|field| {
        field.name == "ProbeAttribute" && field.columns[0].physical_name == "_fld54"
    }));
    assert!(fields.iter().any(|field| {
        field.name == "Date" && field.aliases.iter().any(|alias| alias == "Дата")
    }));
    assert_eq!(
        find_metadata_object(&snapshot, "_Reference53")
            .unwrap()
            .guid
            .as_str(),
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694"
    );
}

#[test]
fn queryable_field_catalog_reflects_current_mutable_snapshot_vectors() {
    let mut snapshot = snapshot();
    snapshot.fields[0].name = Some("RenamedAttribute".to_owned());
    let object = &snapshot.objects[0];
    let object_id = open_sdbl::metadata::ObjectId::from(&object.guid);
    let expected = queryable_fields(&snapshot, object).unwrap();
    let catalog = queryable_field_catalog(&snapshot);
    assert_eq!(catalog.get(&object_id), Some(&expected));
    assert!(
        catalog[&object_id]
            .iter()
            .any(|field| field.name == "RenamedAttribute")
    );

    snapshot.live_tables.clear();
    assert!(!queryable_field_catalog(&snapshot).contains_key(&object_id));
}

#[test]
fn rejects_parameters_and_unsupported_clauses_before_sql_generation() {
    let snapshot = snapshot();
    let parameter = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(parameter.message().contains("parameters are not supported"));

    let unsupported = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(unsupported.message().contains("unsupported query syntax"));
}

#[test]
fn compiles_english_distinct_wildcard_and_real_document_date_spelling() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "SELECT DISTINCT * FROM Catalog.OpenSdblMetadataProbe ORDER BY Date DESC;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.starts_with("SELECT DISTINCT "));
    assert!(compiled.sql.contains("\"_date_time\"::text AS \"Date\""));
    assert!(
        compiled
            .sql
            .ends_with("ORDER BY \"__src\".\"_date_time\" DESC")
    );
    assert!(
        compiled
            .columns
            .iter()
            .any(|column| column == "ProbeAttribute")
    );
}

#[test]
fn compiles_russian_descending_order() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ Дата ИЗ Справочник.OpenSdblMetadataProbe УПОРЯДОЧИТЬ ПО Дата УБЫВ;",
        &snapshot,
    )
    .unwrap();
    assert!(
        compiled
            .sql
            .ends_with("ORDER BY \"__src\".\"_date_time\" DESC")
    );
}

#[test]
fn diagnoses_missing_fields_and_ambiguous_bare_objects() {
    let mut snapshot = snapshot();
    let missing = compile_postgres_query(
        "SELECT Missing FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(missing.line(), 1);
    assert!(missing.column() > 1);
    assert!(missing.message().contains("was not found"));

    snapshot.objects.push(snapshot.objects[0].clone());
    let ambiguous = find_metadata_object(&snapshot, "OpenSdblMetadataProbe").unwrap_err();
    assert!(ambiguous.message().contains("ambiguous"));
}

#[test]
fn expands_a_compound_projection_and_rejects_it_in_predicates() {
    let mut snapshot = snapshot();
    let table = &mut snapshot.live_tables[0];
    table.columns.retain(|column| column.name != "_fld54");
    table.columns.extend([
        LiveColumn {
            name: "_fld54_tref".to_owned(),
            data_type: "bytea".to_owned(),
        },
        LiveColumn {
            name: "_fld54_rrref".to_owned(),
            data_type: "bytea".to_owned(),
        },
    ]);

    let compiled = compile_postgres_query(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        compiled.columns,
        ["ProbeAttribute_TRef", "ProbeAttribute_RRRef"]
    );
    assert!(compiled.sql.contains("\"_fld54_tref\"::text"));
    assert!(compiled.sql.contains("\"_fld54_rrref\"::text"));

    let error = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute IS NULL;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("compound field"));
}

#[test]
fn dereferences_a_reference_property_with_one_reused_left_join() {
    let snapshot = reference_snapshot();
    let compiled = compile_postgres_query(
        "SELECT Организация.Код FROM Catalog.OpenSdblMetadataProbe WHERE Организация.Код = \"A\" ORDER BY Организация.Код;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Организация.Код"]);
    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 1);
    assert_eq!(
        compiled.sql,
        "SELECT \"__ref1\".\"_code\"::text AS \"Организация.Код\" FROM \"_reference53\" AS \"__src\" LEFT JOIN \"_reference57\" AS \"__ref1\" ON \"__src\".\"_fld54\" = \"__ref1\".\"_idrref\" WHERE (\"__ref1\".\"_code\" = 'A') ORDER BY \"__ref1\".\"_code\" ASC"
    );
}

#[test]
fn supports_a_qualified_reference_path_and_rejects_non_references() {
    let snapshot = reference_snapshot();
    let explicit = compile_postgres_query(
        "ВЫБРАТЬ d.Организация.Код ИЗ Справочник.OpenSdblMetadataProbe КАК d;",
        &snapshot,
    )
    .unwrap();
    let implicit = compile_postgres_query(
        "SELECT d.Организация.Code FROM Catalog.OpenSdblMetadataProbe d;",
        &snapshot,
    )
    .unwrap();
    assert!(
        explicit
            .sql
            .contains("FROM \"_reference53\" AS \"d\" LEFT JOIN")
    );
    assert!(
        implicit
            .sql
            .contains("FROM \"_reference53\" AS \"d\" LEFT JOIN")
    );
    assert_eq!(implicit.columns, ["Организация.Code"]);

    let error = compile_postgres_query(
        "SELECT Code.Value FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("no unique SchemaStorage reference target")
    );

    let deep = compile_postgres_query(
        "SELECT d.Организация.Ссылка.Код FROM Catalog.OpenSdblMetadataProbe AS d;",
        &snapshot,
    )
    .unwrap_err();
    assert!(deep.message().contains("deeper than one hop"));

    let collision = compile_postgres_query(
        "SELECT __ref1.Организация.Код FROM Catalog.OpenSdblMetadataProbe AS __ref1;",
        &snapshot,
    )
    .unwrap();
    assert!(collision.sql.contains("AS \"__ref2\" ON"));
}

#[test]
fn leaves_where_and_order_clauses_after_an_unaliased_source() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = \"A\" ORDER BY Code;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains(" AS \"__src\" WHERE "));
    assert!(compiled.sql.contains(" ORDER BY \"__src\".\"_code\" ASC"));
}

#[test]
fn compiles_mixed_union_operators_and_orders_the_combined_result() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p WHERE p.Code = \"A\"
         ОБЪЕДИНИТЬ
         ВЫБРАТЬ q.Дата ИЗ Справочник.OpenSdblMetadataProbe КАК q
         UNION ALL
         SELECT r.ProbeAttribute FROM Catalog.OpenSdblMetadataProbe AS r
         ORDER BY Code DESC;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Code"]);
    assert_eq!(compiled.sql.matches(" UNION (").count(), 1);
    assert_eq!(compiled.sql.matches(" UNION ALL (").count(), 1);
    assert!(
        compiled
            .sql
            .contains("FROM \"_reference53\" AS \"p\" WHERE")
    );
    assert!(compiled.sql.contains("FROM \"_reference53\" AS \"q\""));
    assert!(compiled.sql.contains("FROM \"_reference53\" AS \"r\""));
    assert!(compiled.sql.ends_with("ORDER BY 1 DESC"));
}

#[test]
fn compiles_reference_joins_independently_in_union_branches() {
    let snapshot = reference_snapshot();
    let compiled = compile_postgres_query(
        "SELECT p.Организация.Код FROM Catalog.OpenSdblMetadataProbe p
         ОБЪЕДИНИТЬ ВСЕ
         SELECT q.Организация.Код FROM Catalog.OpenSdblMetadataProbe q
         ORDER BY Организация.Код;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 2);
    assert_eq!(compiled.sql.matches(" AS \"__ref1\" ON ").count(), 2);
    assert!(compiled.sql.ends_with("ORDER BY 1 ASC"));
}

#[test]
fn rejects_incompatible_union_projections_and_branch_local_ordering() {
    let snapshot = snapshot();
    let logical_mismatch = compile_postgres_query(
        "SELECT Code, Date FROM Catalog.OpenSdblMetadataProbe
         UNION SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        logical_mismatch
            .message()
            .contains("projects 1 logical fields")
    );

    let local_order = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe ORDER BY Code
         UNION SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(local_order.message().contains("unsupported query syntax"));

    let missing_order_field = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe
         UNION SELECT Code FROM Catalog.OpenSdblMetadataProbe
         ORDER BY Date;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        missing_order_field
            .message()
            .contains("must occur in the first branch projection")
    );
}

#[test]
fn rejects_union_branches_with_different_compound_expansion_widths() {
    let mut snapshot = snapshot();
    let table = &mut snapshot.live_tables[0];
    table.columns.retain(|column| column.name != "_fld54");
    table.columns.extend([
        LiveColumn {
            name: "_fld54_tref".to_owned(),
            data_type: "bytea".to_owned(),
        },
        LiveColumn {
            name: "_fld54_rrref".to_owned(),
            data_type: "bytea".to_owned(),
        },
    ]);

    let mismatch = compile_postgres_query(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe
         UNION SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        mismatch
            .message()
            .contains("1 logical fields and 1 SQL columns")
    );
    assert!(
        mismatch
            .message()
            .contains("expected 1 logical fields and 2 SQL columns")
    );
}

#[test]
fn compiles_inner_left_and_right_join_spellings_and_repeated_terminators() {
    let snapshot = snapshot();
    let cases = [
        ("JOIN", " INNER JOIN "),
        ("ЛЕВОЕ ВНЕШНЕЕ СОЕДИНЕНИЕ", " LEFT JOIN "),
        ("RIGHT OUTER JOIN", " RIGHT JOIN "),
    ];
    for (source_operator, sql_operator) in cases {
        let query = format!(
            "SELECT l.Code, r.Date FROM Catalog.OpenSdblMetadataProbe l \
             {source_operator} Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;;"
        );
        let compiled = compile_postgres_query(&query, &snapshot).unwrap();
        assert!(compiled.sql.contains(sql_operator), "{}", compiled.sql);
        assert!(!compiled.sql.contains(" UNION ALL "));
    }
}

#[test]
fn resolves_a_one_hop_reference_from_one_join_side() {
    let snapshot = reference_snapshot();
    let compiled = compile_postgres_query(
        "SELECT Организация.Код, t.Code
         FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(compiled.columns, ["Организация.Код", "Code"]);
    assert!(
        compiled
            .sql
            .contains("LEFT JOIN \"_reference57\" AS \"t\" ON \"p\".\"_code\" = \"t\".\"_code\"")
    );
    assert!(
        compiled.sql.contains(
            "LEFT JOIN \"_reference57\" AS \"__left_ref1\" ON \"p\".\"_fld54\" = \"__left_ref1\".\"_idrref\""
        )
    );
}

#[test]
fn transposes_full_join_to_duplicate_safe_union_all() {
    let snapshot = snapshot();
    let compiled = compile_postgres_query(
        "SELECT DISTINCT TOP 3 l.Code, r.Date
         FROM Catalog.OpenSdblMetadataProbe l
         ПОЛНОЕ ВНЕШНЕЕ СОЕДИНЕНИЕ Catalog.OpenSdblMetadataProbe r
         ПО l.Code = r.Code
         WHERE l.Code IS NOT NULL
         ORDER BY l.Code;;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.starts_with("SELECT DISTINCT * FROM ((SELECT "));
    assert!(!compiled.sql.contains("FULL JOIN"));
    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 2);
    assert_eq!(compiled.sql.matches(" UNION ALL ").count(), 1);
    assert!(compiled.sql.contains("(\"l\".\"_code\" IS NULL)"));
    assert_eq!(compiled.sql.matches("IS NOT NULL").count(), 2);
    assert!(compiled.sql.ends_with("ORDER BY 1 ASC LIMIT 3"));
}

#[test]
fn rejects_unsafe_or_ambiguous_join_shapes() {
    let snapshot = reference_snapshot();
    let wildcard = compile_postgres_query(
        "SELECT * FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("wildcard projection"));

    let inequality = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code > t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        inequality
            .message()
            .contains("cross-source field equalities")
    );

    let same_alias = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации p ON p.Code = p.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(same_alias.message().contains("distinct aliases"));

    let reference_condition = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Организация.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(reference_condition.message().contains("direct fields only"));

    let ambiguous = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(ambiguous.message().contains("ambiguous in JOIN sources"));
}

#[test]
fn rejects_an_ambiguous_schema_reference_target() {
    let mut snapshot = reference_snapshot();
    let source_field = snapshot.schema.tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld54")
        .unwrap();
    source_field.types.push(ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Reference58".to_owned()),
    });

    let error = compile_postgres_query(
        "SELECT Организация.Код FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("no unique SchemaStorage reference target")
    );
}

fn snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let db_names = parse_db_names(&hex(
        "0dcab10d03310800c05d5c83f46f631b16c800d9003054518a6f2def9e5c7dbbc23636f5390c5d6e435a9391755e98a94d92570c2128efc878e2eb51a0b703bb769761ab61aa13528d441bd271928b26b5ce62505e9ff5ff74ce0f",
    ))
    .unwrap();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        ),
    )
    .unwrap();
    let schema = parse_schema_storage(
        br#"{0,{1,{"Reference53","N",53,"",{4,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Date_Time",0,{1,{"T",0,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0}},{0},{1,{"Code",1,{2,"Code","ID"},0,0,0,{0},0,0}},1,"R",{0},{0},"",0}}}"#,
    )
    .unwrap();
    let live_tables = vec![LiveTable {
        name: "_reference53".to_owned(),
        columns: vec![
            LiveColumn {
                name: "_idrref".to_owned(),
                data_type: "bytea".to_owned(),
            },
            LiveColumn {
                name: "_code".to_owned(),
                data_type: "mvarchar(9)".to_owned(),
            },
            LiveColumn {
                name: "_date_time".to_owned(),
                data_type: "timestamp without time zone".to_owned(),
            },
            LiveColumn {
                name: "_fld54".to_owned(),
                data_type: "bytea".to_owned(),
            },
        ],
        indexes: vec![LiveIndex {
            name: "_reference53_2".to_owned(),
            columns: vec!["_code".to_owned(), "_idrref".to_owned()],
            unique: true,
        }],
    }];
    resolve_metadata(db_names, descriptors, schema, live_tables)
}

fn mssql_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    for table in &mut snapshot.live_tables {
        for column in &mut table.columns {
            column.data_type = match column.data_type.as_str() {
                "bytea" => "binary(16)".to_owned(),
                "mvarchar(9)" => "nvarchar(9)".to_owned(),
                "timestamp without time zone" => "datetime2".to_owned(),
                other => other.to_owned(),
            };
        }
    }
    snapshot
}

fn reference_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    snapshot.fields[0].name = Some("Организация".to_owned());
    let source_field = snapshot.schema.tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld54")
        .unwrap();
    source_field.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Reference57".to_owned()),
    }];

    let mut target_object = snapshot.objects[0].clone();
    target_object.name = Some("Организации".to_owned());
    target_object.number = Some(57);
    target_object.physical_table = Some("_Reference57".to_owned());
    snapshot.objects.push(target_object);

    let mut target_schema = snapshot.schema.tables[0].clone();
    target_schema.name = "Reference57".to_owned();
    target_schema.number = 57;
    target_schema
        .columns
        .retain(|column| column.name != "Fld54");
    snapshot.schema.tables.push(target_schema);

    let mut target_live = snapshot.live_tables[0].clone();
    target_live.name = "_reference57".to_owned();
    target_live.columns.retain(|column| column.name != "_fld54");
    snapshot.live_tables.push(target_live);
    snapshot
}

fn information_register_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    let object = &mut snapshot.objects[0];
    object.kind = Some(MetadataKind::InformationRegister);
    object.name = Some("Prices".to_owned());
    object.physical_table = Some("_InfoRg53".to_owned());

    let schema = &mut snapshot.schema.tables[0];
    schema.name = "InfoRg53".to_owned();
    let date = schema
        .columns
        .iter_mut()
        .find(|column| column.name == "Date_Time")
        .unwrap();
    date.name = "Period".to_owned();

    let live = &mut snapshot.live_tables[0];
    live.name = "_inforg53".to_owned();
    let date = live
        .columns
        .iter_mut()
        .find(|column| column.name == "_date_time")
        .unwrap();
    date.name = "_period".to_owned();

    let field = &mut snapshot.fields[0];
    field.owner_tables = vec!["_InfoRg53".to_owned()];
    field.purpose = Some(ConfigFieldPurpose::InformationRegisterDimension);
    snapshot
}

fn accumulation_register_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    snapshot.db_names = parse_db_names(&hex(
        "95cbb11142210c00d05da8933bf9249094360ee0b94012c0464b2b8edd3d0b07f8fd7b8b60b9b845ab8ea1d9917a13146b179cd38a4ee9a32a41ba467cdef767022efbe47924e0ba611d1c5abd179c1684748c895e95b151a84d2a2cea906eaf9e8069c36a5d8af33170fe12556ba8ae8c212377b1c12de7bfe7bdbf",
    ))
    .unwrap();
    let object = &mut snapshot.objects[0];
    object.kind = Some(MetadataKind::AccumulationRegister);
    object.name = Some("Остатки".to_owned());
    object.physical_table = Some("_AccumRg53".to_owned());

    let schema = &mut snapshot.schema.tables[0];
    schema.name = "AccumRg53".to_owned();
    schema
        .columns
        .retain(|column| !matches!(column.name.as_str(), "ID" | "Code"));
    let date = schema
        .columns
        .iter_mut()
        .find(|column| column.name == "Date_Time")
        .unwrap();
    date.name = "Period".to_owned();
    schema.columns.extend([
        SchemaColumn {
            name: "Active".to_owned(),
            types: vec![ColumnType {
                tag: "L".to_owned(),
                reference_target: None,
            }],
        },
        SchemaColumn {
            name: "RecordKind".to_owned(),
            types: vec![ColumnType {
                tag: "N".to_owned(),
                reference_target: None,
            }],
        },
        SchemaColumn {
            name: "Fld55".to_owned(),
            types: vec![ColumnType {
                tag: "N".to_owned(),
                reference_target: None,
            }],
        },
    ]);

    let live = &mut snapshot.live_tables[0];
    live.name = "_accumrg53".to_owned();
    live.columns
        .retain(|column| !matches!(column.name.as_str(), "_idrref" | "_code"));
    let date = live
        .columns
        .iter_mut()
        .find(|column| column.name == "_date_time")
        .unwrap();
    date.name = "_period".to_owned();
    live.columns.extend([
        LiveColumn {
            name: "_active".to_owned(),
            data_type: "boolean".to_owned(),
        },
        LiveColumn {
            name: "_recordkind".to_owned(),
            data_type: "numeric(1,0)".to_owned(),
        },
        LiveColumn {
            name: "_fld55".to_owned(),
            data_type: "numeric(10,2)".to_owned(),
        },
    ]);

    let dimension = &mut snapshot.fields[0];
    dimension.name = Some("Номенклатура".to_owned());
    dimension.owner_tables = vec!["_AccumRg53".to_owned()];
    dimension.purpose = Some(ConfigFieldPurpose::AccumulationRegisterDimension);
    let mut resource = dimension.clone();
    resource.name = Some("Количество".to_owned());
    resource.number = 55;
    resource.physical_name = "_Fld55".to_owned();
    resource.purpose = Some(ConfigFieldPurpose::AccumulationRegisterResource);
    snapshot.fields.push(resource);

    let mut totals_schema = snapshot.schema.tables[0].clone();
    totals_schema.name = "AccumRgT56".to_owned();
    totals_schema.number = 56;
    totals_schema
        .columns
        .retain(|column| matches!(column.name.as_str(), "Period" | "Fld54" | "Fld55"));
    totals_schema.columns.push(SchemaColumn {
        name: "Splitter".to_owned(),
        types: vec![ColumnType {
            tag: "N".to_owned(),
            reference_target: None,
        }],
    });
    totals_schema.indexes.clear();
    snapshot.schema.tables.push(totals_schema);

    let mut totals_live = snapshot.live_tables[0].clone();
    totals_live.name = "_accumrgt56".to_owned();
    totals_live
        .columns
        .retain(|column| matches!(column.name.as_str(), "_period" | "_fld54" | "_fld55"));
    totals_live.columns.push(LiveColumn {
        name: "_splitter".to_owned(),
        data_type: "numeric(10,0)".to_owned(),
    });
    totals_live.indexes.clear();
    snapshot.live_tables.push(totals_live);
    snapshot
}

fn presentation_reference_snapshot(multiple: bool) -> open_sdbl::metadata::MetadataSnapshot {
    let db_names = parse_db_names(&hex(if multiple {
        "55ce3112c3200c04c0bf5073333612207d200fc80f10a02a9322adc77f0f850b7bebbbb93b381e26d67a2d86aebb81471548ab1bdc1ba9cb98453986f7f4f99bdf3e43cc74c623e5aec506c15b67709a0e2b9a51b96b73a62c6a31bc3e63e579e5f70bd2022622083323df3c57ea6a950bea021659df5415ede6d992f3fc03"
    } else {
        "55cd310e82210c40e1bb30d3c49f16682fe001bc012ded641c5c097797c141bff9256f615eca3aac3705934b816667e0d16f10315082a737a19c1e1efef69779ca15775ea59a349d08318c808a0768930a9d4c46105616cde9fe9ca7a7d35f5f500e2044042622a83ffe2f7def0f"
    }))
    .unwrap();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        ),
    )
    .unwrap();
    let extra_type = if multiple {
        ",{\"R\",0,0,\"Reference58\",2}"
    } else {
        ""
    };
    let third_table = if multiple {
        r#",{"Reference58","N",58,"",{2,{"ID",0,{1,{"R",0,0,"Reference58",2}},"",0},{"Code",0,{1,{"S",10,0,"",0}},"",0}},{0},{0},1,"R",{0},{0},"",0}"#
    } else {
        ""
    };
    let table_count = if multiple { 3 } else { 2 };
    let schema_text = format!(
        r#"{{0,{{{table_count},{{"Reference53","N",53,"",{{2,{{"ID",0,{{1,{{"R",0,0,"Reference53",2}}}},"",0}},{{"Fld54",0,{{{},{{"R",0,0,"Reference57",2}}{extra_type}}},"",0}}}},{{0}},{{0}},1,"R",{{0}},{{0}},"",0}},{{"Reference57","N",57,"",{{2,{{"ID",0,{{1,{{"R",0,0,"Reference57",2}}}},"",0}},{{"Code",0,{{1,{{"S",10,0,"",0}}}},"",0}}}},{{0}},{{0}},1,"R",{{0}},{{0}},"",0}}{third_table}}}}}"#,
        if multiple { 2 } else { 1 }
    );
    let schema = parse_schema_storage(schema_text.as_bytes()).unwrap();
    let mut live_tables = vec![
        LiveTable {
            name: "_reference53".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_fld54_rtref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_fld54_rrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
            ],
            indexes: Vec::new(),
        },
        LiveTable {
            name: "_reference57".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_code".to_owned(),
                    data_type: "mvarchar(10)".to_owned(),
                },
            ],
            indexes: Vec::new(),
        },
    ];
    if multiple {
        let mut target = live_tables[1].clone();
        target.name = "_reference58".to_owned();
        live_tables.push(target);
    }
    resolve_metadata(db_names, descriptors, schema, live_tables)
}

fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
