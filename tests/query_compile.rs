use std::str::FromStr;

use open_sdbl::metadata::{
    ColumnType, ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue, FieldId, Guid,
    LiveColumn, LiveIndex, LiveTable, LookupError, MetadataKind, SchemaColumn, SchemaStorage,
    SchemaTable, StandardFieldId, parse_config_descriptors, parse_db_names, parse_schema_storage,
    resolve_metadata, resolve_metadata_with_predefined_values,
};
use open_sdbl::query::{
    PresentationExpression, PresentationPlan, compile_mssql_presentation_lookup_with_year_offset,
    compile_mssql_query, compile_mssql_query_with_year_offset,
    compile_postgres_presentation_lookup, compile_postgres_query, find_metadata_object,
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
fn compiles_enumeration_value_in_physical_one_c_byte_order() {
    let snapshot = enumeration_value_snapshot();
    let postgres = compile_postgres_query(
        "ВЫБРАТЬ ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус);",
        &snapshot,
    )
    .unwrap();
    assert!(
        postgres
            .sql
            .contains("decode('9022249e3a1ac4b94be8faddd2f8bde9', 'hex')")
    );

    let mssql = compile_mssql_query(
        "SELECT VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус);",
        &snapshot,
    )
    .unwrap();
    assert!(mssql.sql.contains("0x9022249e3a1ac4b94be8faddd2f8bde9"));
}

#[test]
fn compiles_catalog_value_as_a_predefined_id_lookup() {
    let snapshot = catalog_value_snapshot();
    let postgres = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE ID = VALUE(Catalog.OpenSdblMetadataProbe.Утвержден);",
        &snapshot,
    )
    .unwrap();
    assert!(
        postgres
            .sql
            .contains("FROM \"_reference53\" AS \"__open_sdbl_value\"")
    );
    assert!(postgres.sql.contains(
        "\"__open_sdbl_value\".\"_predefinedid\" = decode('a3dae56fa2f94623445632b52e22ad88', 'hex')"
    ));
    assert!(
        postgres
            .sql
            .contains("SELECT \"__open_sdbl_value\".\"_idrref\"")
    );

    let mssql = compile_mssql_query(
        "SELECT VALUE(Catalog.OpenSdblMetadataProbe.ДополнительныеУсловияПоДоговору_Проверен);",
        &snapshot,
    )
    .unwrap();
    assert!(mssql.sql.contains("0xa161ed47a2787c5a437832a3f6fa6a92"));
    assert!(mssql.sql.contains("\"_predefinedid\""));
}

#[test]
fn compiles_in_list_with_several_catalog_values() {
    let snapshot = catalog_value_snapshot();
    let query = "ВЫБРАТЬ Код ИЗ Справочник.OpenSdblMetadataProbe
        ГДЕ Ссылка В (
            ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.Утвержден),
            ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.ДополнительныеУсловияПоДоговору_Проверен)
        );";

    let postgres = compile_postgres_query(query, &snapshot).unwrap();
    assert!(postgres.sql.contains("\"__src\".\"_idrref\" IN ("));
    assert_eq!(
        postgres
            .sql
            .matches("SELECT \"__open_sdbl_value\".\"_idrref\"")
            .count(),
        2
    );
    let approved = postgres
        .sql
        .find("a3dae56fa2f94623445632b52e22ad88")
        .unwrap();
    let checked = postgres
        .sql
        .find("a161ed47a2787c5a437832a3f6fa6a92")
        .unwrap();
    assert!(approved < checked, "{}", postgres.sql);

    let mssql = compile_mssql_query(query, &snapshot).unwrap();
    assert!(mssql.sql.contains("\"__src\".\"_idrref\" IN ("));
    assert!(mssql.sql.contains("0xa3dae56fa2f94623445632b52e22ad88"));
    assert!(mssql.sql.contains("0xa161ed47a2787c5a437832a3f6fa6a92"));
}

#[test]
fn compiles_in_lists_in_source_free_and_joined_queries() {
    let snapshot = snapshot();
    let source_free = compile_postgres_query("SELECT 2 IN (1, 2);", &snapshot).unwrap();
    assert!(source_free.sql.contains("(2 IN (1, 2))"));

    let joined = compile_mssql_query(
        "SELECT l.Code FROM Catalog.OpenSdblMetadataProbe l
         INNER JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code
         WHERE l.Code IN (\"Первый\", \"Второй\");",
        &mssql_snapshot(),
    )
    .unwrap();
    assert!(
        joined
            .sql
            .contains("(\"l\".\"_code\" IN (N'Первый', N'Второй'))")
    );
}

#[test]
fn diagnoses_empty_and_malformed_in_lists() {
    let snapshot = snapshot();
    let empty = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN ();",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        empty
            .message()
            .contains("IN list must contain at least one expression")
    );

    let trailing = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code В (\"A\",);",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        trailing
            .message()
            .contains("expected expression after ',' in IN list")
    );

    let unclosed = compile_postgres_query(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (\"A\";",
        &snapshot,
    )
    .unwrap_err();
    assert!(unclosed.message().contains("expected \")\""));
}

#[test]
fn diagnoses_invalid_value_kinds_paths_and_names() {
    let snapshot = catalog_value_snapshot();
    let error = compile_postgres_query(
        "SELECT VALUE(Document.OpenSdblMetadataProbe.Утвержден);",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("only catalogs and enumerations"));

    let error = compile_postgres_query(
        "SELECT VALUE(Catalog.OpenSdblMetadataProbe.Absent);",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("metadata value was not found"));

    let error = compile_postgres_query("SELECT VALUE(Catalog.OnlyTwo);", &snapshot).unwrap_err();
    assert!(error.message().contains("expected \".\""));
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
fn applies_projection_aliases_and_diagnoses_a_missing_alias() {
    let snapshot = snapshot();
    let field = compile_postgres_query(
        "SELECT Code AS ResultCode FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(field.columns, ["ResultCode"]);
    assert!(field.sql.contains("AS \"ResultCode\""));

    let scalar = compile_postgres_query("SELECT 2 + 2 КАК Результат;", &snapshot).unwrap();
    assert_eq!(scalar.columns, ["Результат"]);
    assert_eq!(scalar.sql, "SELECT ((2 + 2))::text AS \"Результат\"");

    let aggregate = compile_postgres_query(
        "SELECT COUNT(*) AS RowCount FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(aggregate.columns, ["RowCount"]);
    assert!(aggregate.sql.contains("COUNT(*)::text AS \"RowCount\""));

    let error = compile_postgres_query(
        "SELECT Code AS FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("expected projection alias"));
}

#[test]
fn compiles_datetime_and_begin_of_period_for_postgres() {
    let snapshot = snapshot();
    let source_free = compile_postgres_query(
        "SELECT DATETIME(2024, 2, 29, 12, 34, 56) AS Moment,
                BEGINOFPERIOD(DATETIME(2024, 8, 29, 12, 34, 56), MONTH) AS PeriodStart;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(source_free.columns, ["Moment", "PeriodStart"]);
    assert!(
        source_free
            .sql
            .contains("(TIMESTAMP '2024-02-29 12:34:56')::text AS \"Moment\"")
    );
    assert!(source_free.sql.contains(
        "(date_trunc('month', TIMESTAMP '2024-08-29 12:34:56'))::text AS \"PeriodStart\""
    ));

    let source_backed = compile_postgres_query(
        "ВЫБРАТЬ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ) КАК НачалоМесяца
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ Дата >= ДАТАВРЕМЯ(2026, 9, 2);",
        &snapshot,
    )
    .unwrap();
    assert!(
        source_backed
            .sql
            .contains("(date_trunc('month', \"__src\".\"_date_time\"))::text AS \"НачалоМесяца\""),
        "{}",
        source_backed.sql
    );
    assert!(
        source_backed
            .sql
            .contains("(\"__src\".\"_date_time\" >= TIMESTAMP '2026-09-02 00:00:00')")
    );
}

#[test]
fn compiles_datetime_and_begin_of_period_for_mssql_year_offset() {
    let snapshot = mssql_snapshot();
    let compiled = compile_mssql_query_with_year_offset(
        "SELECT BEGINOFPERIOD(Date, MONTH) AS MonthStart
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Date >= DATETIME(2026, 9, 2, 10, 11, 12);",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "CONVERT(nvarchar(max), DATEADD(year, -2000, DATETIME2FROMPARTS(YEAR(\"__src\".\"_date_time\"), MONTH(\"__src\".\"_date_time\"), 1, 0, 0, 0, 0, 0))) AS \"MonthStart\""
    ));
    assert!(
        compiled
            .sql
            .contains("DATEADD(year, 2000, CONVERT(datetime2, '2026-09-02T10:11:12', 126))")
    );

    let virtual_table = compile_mssql_query_with_year_offset(
        "SELECT КоличествоОстаток
         FROM AccumulationRegister.Остатки.Balance(DATETIME(2026, 9, 2));",
        &accumulation_register_snapshot(),
        2000,
    )
    .unwrap();
    assert!(
        virtual_table
            .sql
            .contains("DATEADD(year, 2000, CONVERT(datetime2, '2026-09-02T00:00:00', 126))")
    );
}

#[test]
fn compiles_date_functions_in_joined_projection_and_filter() {
    let snapshot = reference_snapshot();
    let compiled = compile_postgres_query(
        "SELECT BEGINOFPERIOD(p.Date, DAY) AS StartDay
         FROM Catalog.OpenSdblMetadataProbe AS p
         INNER JOIN Catalog.Организации AS o ON p.Code = o.Code
         WHERE p.Date >= DATETIME(2026, 9, 2);",
        &snapshot,
    )
    .unwrap();

    assert!(
        compiled
            .sql
            .contains("(date_trunc('day', \"p\".\"_date_time\"))::text AS \"StartDay\"")
    );
    assert!(
        compiled
            .sql
            .contains("(\"p\".\"_date_time\" >= TIMESTAMP '2026-09-02 00:00:00')")
    );
}

#[test]
fn compiles_every_begin_of_period_kind_for_both_dialects() {
    let snapshot = snapshot();
    for period in [
        "МИНУТА",
        "ЧАС",
        "ДЕНЬ",
        "НЕДЕЛЯ",
        "ДЕКАДА",
        "МЕСЯЦ",
        "КВАРТАЛ",
        "ПОЛУГОДИЕ",
        "ГОД",
    ] {
        let query = format!("SELECT BEGINOFPERIOD(DATETIME(2026, 9, 22, 12, 34, 56), {period});");
        compile_postgres_query(&query, &snapshot).unwrap();
        compile_mssql_query(&query, &mssql_snapshot()).unwrap();
    }
}

#[test]
fn validates_datetime_components_periods_and_mssql_offset_range() {
    let snapshot = snapshot();
    for (query, message) in [
        ("SELECT DATETIME(2026, 9);", "requires 3 to 6"),
        ("SELECT DATETIME(2025, 2, 29);", "day must be"),
        ("SELECT DATETIME(2026, 9, 2, 24);", "hour must be"),
        ("SELECT DATETIME(2026.5, 9, 2);", "integer literals"),
        (
            "SELECT BEGINOFPERIOD(DATETIME(2026, 9, 2), CENTURY);",
            "unsupported BEGINOFPERIOD period",
        ),
        (
            "SELECT BEGINOFPERIOD(4, MONTH);",
            "first argument must be a date expression",
        ),
    ] {
        let error = compile_postgres_query(query, &snapshot).unwrap_err();
        assert!(error.message().contains(message), "{error}");
    }

    let error = compile_mssql_query_with_year_offset(
        "SELECT DATETIME(9000, 1, 1) FROM Catalog.OpenSdblMetadataProbe;",
        &mssql_snapshot(),
        2000,
    )
    .unwrap_err();
    assert!(error.message().contains("outside 1..=9999"));

    let error = compile_postgres_query(
        "SELECT BEGINOFPERIOD(Code, MONTH) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("must resolve to a date field"));
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
            assert!(compiled.sql.contains("decode('00000039', 'hex')"));
            assert!(compiled.sql.contains("decode('0000003a', 'hex')"));
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

    let aliased = compile_postgres_query(
        "SELECT ProbeAttribute AS Value FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(aliased.columns, ["Value_TRef", "Value_RRRef"]);

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
fn compiles_join_key_with_additional_in_and_value_predicates() {
    let snapshot = catalog_value_snapshot();
    let query = "SELECT l.Code FROM Catalog.OpenSdblMetadataProbe l
        INNER JOIN Catalog.OpenSdblMetadataProbe r
        ON l.Code = r.Code
            AND l.ID IN (
                VALUE(Catalog.OpenSdblMetadataProbe.Утвержден),
                VALUE(Catalog.OpenSdblMetadataProbe.ДополнительныеУсловияПоДоговору_Проверен)
            )
            AND l.Code <> \"Исключен\";";

    for compiled in [
        compile_postgres_query(query, &snapshot).unwrap(),
        compile_mssql_query(query, &snapshot).unwrap(),
    ] {
        let on = compiled.sql.split_once(" ON ").unwrap().1;
        assert!(on.starts_with("\"l\".\"_code\" = \"r\".\"_code\" AND "));
        assert!(on.contains("(\"l\".\"_idrref\" IN ("));
        assert!(on.contains("a3dae56fa2f94623445632b52e22ad88"));
        assert!(on.contains("a161ed47a2787c5a437832a3f6fa6a92"));
        assert!(on.contains(" AND (\"l\".\"_code\" <> "));
    }
}

#[test]
fn keeps_additional_full_join_predicates_in_both_on_clauses() {
    let compiled = compile_postgres_query(
        "SELECT l.Code, r.Date FROM Catalog.OpenSdblMetadataProbe l
         FULL JOIN Catalog.OpenSdblMetadataProbe r
         ON l.Code = r.Code AND l.Code <> \"Исключен\";",
        &snapshot(),
    )
    .unwrap();

    let condition = "ON \"l\".\"_code\" = \"r\".\"_code\" AND (\"l\".\"_code\" <> 'Исключен')";
    assert_eq!(
        compiled.sql.matches(condition).count(),
        2,
        "{}",
        compiled.sql
    );
    assert!(!compiled.sql.contains("WHERE (\"l\".\"_code\" <>"));
    assert!(compiled.sql.contains("WHERE (\"l\".\"_code\" IS NULL)"));
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
            .contains("top-level cross-source field equality")
    );

    let nested_anchor = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t
         ON p.Code = t.Code OR p.Code <> t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        nested_anchor
            .message()
            .contains("top-level cross-source field equality")
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

    let additional_reference_condition = compile_postgres_query(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t
         ON p.Code = t.Code AND p.Организация.Code = \"A\";",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        additional_reference_condition
            .message()
            .contains("direct fields only")
    );

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

#[test]
fn compiles_document_tabular_section_from_extension_table() {
    let snapshot = tabular_section_snapshot();
    let compiled = compile_postgres_query(
        "ВЫБРАТЬ
            строки.ЦФО КАК ЦФО,
            строки.Ссылка.ДоговорКонтрагента КАК Договор,
            строки.СуммаБезНДС КАК СуммаБезНДС,
            строки.Сумма КАК СуммаСНДС,
            строки.Период КАК Период,
            строки.ЦФО.Сам_БизнесРегион КАК Город
         ИЗ РегистрСведений.бит_СтатусыОбъектов КАК статусы
         ВНУТРЕННЕЕ СОЕДИНЕНИЕ
             Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК строки
         ПО статусы.Объект = строки.Ссылка;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(
        compiled.columns,
        [
            "ЦФО",
            "Договор",
            "СуммаБезНДС",
            "СуммаСНДС",
            "Период",
            "Город"
        ]
    );
    assert!(compiled.sql.contains("FROM \"_inforg60\" AS \"статусы\""));
    assert!(
        compiled
            .sql
            .contains("JOIN \"_document53_vt54X1\" AS \"строки\"")
    );
    assert!(compiled.sql.contains("LEFT JOIN \"_document53\""));
    assert!(compiled.sql.contains("LEFT JOIN \"_reference62\""));
    assert!(compiled.sql.contains("AS \"Договор\""));
    assert!(compiled.sql.contains("AS \"Город\""));
    assert!(compiled.sql.contains(
        "(\"статусы\".\"_fld61_rrref\" = \"строки\".\"_document53_idrref\" AND \"статусы\".\"_fld61_rtref\" = decode('00000035', 'hex'))"
    ));

    let direct = compile_postgres_query(
        "SELECT Ссылка, НомерСтроки, Сумма
         FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(direct.columns, ["ID", "LineNo", "Сумма"]);
    assert!(direct.sql.contains("FROM \"_document53_vt54X1\""));
    assert!(
        direct
            .sql
            .contains("\"_document53_idrref\"::text AS \"ID\"")
    );
    assert!(direct.sql.contains("\"_lineno54\"::text AS \"LineNo\""));
}

#[test]
fn presents_references_reached_through_dereferenced_join_paths() {
    let snapshot = dereferenced_presentation_snapshot();
    let source = "SELECT
            REFPRESENTATION(строки.ЦФО) AS ЦФО,
            REFPRESENTATION(строки.Ссылка.ДоговорКонтрагента) AS Договор,
            REFPRESENTATION(строки.ЦФО.Сам_БизнесРегион) AS Город
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";

    let postgres = prepare_postgres_query(source, &snapshot).unwrap();
    assert_eq!(postgres.presentation_request().targets.len(), 1);
    let object = postgres.presentation_request().targets[0].object;
    let id = FieldId::Standard(StandardFieldId::Id);
    let plan = PresentationPlan {
        object,
        fields: vec![id],
        expression: PresentationExpression::Field(id),
    };
    let postgres = postgres
        .compile(&snapshot, std::slice::from_ref(&plan))
        .unwrap();
    assert_dereferenced_presentation_joins(&postgres.sql);

    let mssql = prepare_mssql_query(source, &snapshot).unwrap();
    assert_eq!(mssql.presentation_request().targets.len(), 1);
    let mssql = mssql.compile(&snapshot, &[plan]).unwrap();
    assert_dereferenced_presentation_joins(&mssql.sql);
}

#[test]
fn reuses_a_dereference_join_for_projection_and_presentation() {
    let snapshot = dereferenced_presentation_snapshot();
    let source = "SELECT
            строки.ЦФО.Сам_БизнесРегион,
            REFPRESENTATION(строки.ЦФО.Сам_БизнесРегион)
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";
    let prepared = prepare_postgres_query(source, &snapshot).unwrap();
    let object = prepared.presentation_request().targets[0].object;
    let id = FieldId::Standard(StandardFieldId::Id);
    let compiled = prepared
        .compile(
            &snapshot,
            &[PresentationPlan {
                object,
                fields: vec![id],
                expression: PresentationExpression::Field(id),
            }],
        )
        .unwrap();

    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 2);
    assert!(compiled.sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref1\" ON \"строки\".\"_fld55\" = \"__right_ref1\".\"_idrref\""
    ));
    assert!(compiled.sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref2\" ON \"__right_ref1\".\"_fld63\" = \"__right_ref2\".\"_idrref\""
    ));
}

#[test]
fn presents_a_scalar_from_a_dereferenced_join_alias_without_a_plan() {
    let snapshot = tabular_section_snapshot();
    let source = "SELECT PRESENTATION(строки.ЦФО.Сам_БизнесРегион)
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";
    let prepared = prepare_postgres_query(source, &snapshot).unwrap();
    assert!(prepared.presentation_request().targets.is_empty());
    let compiled = prepared.compile(&snapshot, &[]).unwrap();

    assert!(compiled.sql.contains("\"__right_ref1\".\"_fld63\""));
    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 1);
}

#[test]
fn defers_a_universal_reference_reached_through_a_join_path() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let source = "SELECT TOP 10 REFPRESENTATION(строки.Ссылка.ДоговорКонтрагента) AS Договор
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";

    let postgres = prepare_postgres_query(source, &snapshot).unwrap();
    assert!(postgres.presentation_request().targets.is_empty());
    let postgres = postgres.compile(&snapshot, &[]).unwrap();
    assert_eq!(postgres.deferred_presentations, [0]);
    assert!(
        postgres
            .sql
            .contains("encode(\"__right_ref1\".\"_fld59_rtref\", 'hex')")
    );
    assert!(
        postgres
            .sql
            .contains("encode(\"__right_ref1\".\"_fld59_rrref\", 'hex')")
    );
    assert!(postgres.sql.ends_with(" LIMIT 10"));
    assert_eq!(postgres.sql.matches(" LEFT JOIN ").count(), 1);

    let mssql = prepare_mssql_query(source, &snapshot).unwrap();
    let mssql = mssql.compile(&snapshot, &[]).unwrap();
    assert_eq!(mssql.deferred_presentations, [0]);
    assert!(
        mssql
            .sql
            .contains("CONVERT(varchar(max), \"__right_ref1\".\"_fld59_rtref\", 2)")
    );
    assert!(mssql.sql.starts_with("SELECT TOP (10) "));
}

#[test]
fn compiles_safe_batched_deferred_presentation_lookups() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let object = snapshot.object_id_by_database_type(62).unwrap();
    let id = FieldId::Standard(StandardFieldId::Id);
    let plan = PresentationPlan {
        object,
        fields: vec![id],
        expression: PresentationExpression::Field(id),
    };
    let references = [[0x11; 16], [0x22; 16]];

    let postgres = compile_postgres_presentation_lookup(&snapshot, &plan, &references).unwrap();
    assert!(postgres.deferred_presentations.is_empty());
    assert!(postgres.sql.contains(
        "WHERE \"__presentation_target\".\"_idrref\" IN (decode('11111111111111111111111111111111', 'hex'), decode('22222222222222222222222222222222', 'hex'))"
    ));

    let mssql =
        compile_mssql_presentation_lookup_with_year_offset(&snapshot, &plan, &references, 2000)
            .unwrap();
    assert!(mssql.sql.contains(
        "WHERE \"__presentation_target\".\"_idrref\" IN (0x11111111111111111111111111111111, 0x22222222222222222222222222222222)"
    ));
}

fn assert_dereferenced_presentation_joins(sql: &str) {
    assert_eq!(sql.matches(" LEFT JOIN ").count(), 4, "{sql}");
    assert!(sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref1\" ON \"строки\".\"_fld55\" = \"__right_ref1\".\"_idrref\""
    ));
    assert!(sql.contains(
        "LEFT JOIN \"_document53\" AS \"__right_ref2\" ON \"строки\".\"_document53_idrref\" = \"__right_ref2\".\"_idrref\""
    ));
    assert!(sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref3\" ON \"__right_ref2\".\"_fld59\" = \"__right_ref3\".\"_idrref\""
    ));
    assert!(sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref4\" ON \"__right_ref1\".\"_fld63\" = \"__right_ref4\".\"_idrref\""
    ));
}

#[test]
fn diagnoses_a_tabular_section_missing_from_schema_storage() {
    let mut snapshot = tabular_section_snapshot();
    snapshot
        .schema
        .tables
        .retain(|table| table.name != "Document53_VT54X1");

    let error = compile_postgres_query(
        "SELECT Сумма
         FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
        &snapshot,
    )
    .unwrap_err();

    assert!(error.message().contains("absent from SchemaStorage"));
    assert!(error.message().contains("_Document53_VT54"));
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

fn tabular_section_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let db_names = parse_db_names(&hex(
        "4d8dbb6a03311045ff45b5062cefac34ea4d20ad09e9f721b9b1d7109c6ad97fcf48cc4d728a832e9c417b087e9f659e9614675a729889d72424533a51add390abac2566f6eef25cbe1f657b393f0e87df83415ddc2498c0bbcf0fcd59f3b3415ddc2498c0bbb7fbaafda8fd605017370926401fb56783fe247801f449fbd1a02e6e124c805eb48f067571936002f459fb645017370926f0ee7dabcfebcdf978d21331a88b7f5ff20ffb2206edb3415ddc2498c0bb6ba9e5ab6c4bd1abb35e4d067571936002fc321cc70f",
    ))
    .unwrap();
    let parent = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let information_register = guid("77777777-7777-4777-8777-777777777777");
    let catalog = guid("99999999-9999-4999-8999-999999999999");
    let descriptors = vec![
        descriptor(&parent, &parent, "бит_ДополнительныеУсловияПоДоговору"),
        descriptor(
            &parent,
            &guid("11111111-1111-4111-8111-111111111111"),
            "ГрафикНачислений",
        ),
        descriptor(
            &parent,
            &guid("22222222-2222-4222-8222-222222222222"),
            "ЦФО",
        ),
        descriptor(
            &parent,
            &guid("33333333-3333-4333-8333-333333333333"),
            "СуммаБезНДС",
        ),
        descriptor(
            &parent,
            &guid("44444444-4444-4444-8444-444444444444"),
            "Сумма",
        ),
        descriptor(
            &parent,
            &guid("55555555-5555-4555-8555-555555555555"),
            "Период",
        ),
        descriptor(
            &parent,
            &guid("66666666-6666-4666-8666-666666666666"),
            "ДоговорКонтрагента",
        ),
        descriptor(
            &information_register,
            &information_register,
            "бит_СтатусыОбъектов",
        ),
        descriptor(
            &information_register,
            &guid("88888888-8888-4888-8888-888888888888"),
            "Объект",
        ),
        descriptor(&catalog, &catalog, "ЦентрыФинансовойОтветственности"),
        descriptor(
            &catalog,
            &guid("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            "Сам_БизнесРегион",
        ),
    ];
    let schema = SchemaStorage {
        tables: vec![
            schema_table(
                "Document53",
                53,
                vec![
                    schema_column("ID", "R", Some("Document53")),
                    schema_column("Fld59", "R", Some("Reference62")),
                ],
            ),
            schema_table(
                "Document53_VT54X1",
                54,
                vec![
                    schema_column("Document53_IDRRef", "R", Some("Document53")),
                    schema_column("LineNo54", "N", None),
                    schema_column("Fld55", "R", Some("Reference62")),
                    schema_column("Fld56", "N", None),
                    schema_column("Fld57", "N", None),
                    schema_column("Fld58", "T", None),
                ],
            ),
            schema_table(
                "InfoRg60",
                60,
                vec![schema_column("Fld61", "R", Some("Document53"))],
            ),
            schema_table(
                "Reference62",
                62,
                vec![
                    schema_column("ID", "R", Some("Reference62")),
                    schema_column("Fld63", "B", None),
                ],
            ),
        ],
    };
    let live_tables = vec![
        live_table("_document53", &["_idrref", "_fld59"]),
        live_table(
            "_document53_vt54X1",
            &[
                "_document53_idrref",
                "_lineno54",
                "_fld55",
                "_fld56",
                "_fld57",
                "_fld58",
            ],
        ),
        live_table(
            "_inforg60",
            &["_fld61_type", "_fld61_rtref", "_fld61_rrref"],
        ),
        live_table("_reference62", &["_idrref", "_fld63"]),
    ];
    resolve_metadata(db_names, descriptors, schema, live_tables)
}

fn dereferenced_presentation_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = tabular_section_snapshot();
    let business_region = snapshot
        .schema
        .tables
        .iter_mut()
        .find(|table| table.name == "Reference62")
        .unwrap()
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld63")
        .unwrap();
    business_region.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Reference62".to_owned()),
    }];
    snapshot
}

fn universal_dereferenced_presentation_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = tabular_section_snapshot();
    let agreement = snapshot
        .schema
        .tables
        .iter_mut()
        .find(|table| table.name == "Document53")
        .unwrap()
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld59")
        .unwrap();
    agreement.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some(String::new()),
    }];
    let document = snapshot
        .live_tables
        .iter_mut()
        .find(|table| table.name == "_document53")
        .unwrap();
    document.columns.retain(|column| column.name != "_fld59");
    document.columns.extend(
        ["_fld59_type", "_fld59_rtref", "_fld59_rrref"]
            .into_iter()
            .map(|name| LiveColumn {
                name: name.to_owned(),
                data_type: "bytea".to_owned(),
            }),
    );
    snapshot
}

fn enumeration_value_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let owner = guid("c8b21fea-1e3d-4ae9-8719-7ff4db08af97");
    let value = guid("d2f8bde9-fadd-4be8-9022-249e3a1ac4b9");
    let db_names = parse_db_names(&hex(
        "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500",
    ))
    .unwrap();
    resolve_metadata(
        db_names,
        vec![
            descriptor(&owner, &owner, "бит_ВидыСтатусовОбъектов"),
            descriptor(&owner, &value, "Статус"),
        ],
        SchemaStorage { tables: Vec::new() },
        vec![live_table("_enum99", &["_idrref", "_enumorder"])],
    )
}

fn catalog_value_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let base = snapshot();
    let owner = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let mut live_tables = base.live_tables.clone();
    live_tables[0].columns.push(LiveColumn {
        name: "_predefinedid".to_owned(),
        data_type: "bytea".to_owned(),
    });
    resolve_metadata_with_predefined_values(
        base.db_names.clone(),
        base.descriptors.clone(),
        vec![
            ConfigPredefinedValue {
                owner_guid: owner.clone(),
                value_guid: guid("2e22ad88-32b5-4456-a3da-e56fa2f94623"),
                name: "Утвержден".to_owned(),
            },
            ConfigPredefinedValue {
                owner_guid: owner,
                value_guid: guid("f6fa6a92-32a3-4378-a161-ed47a2787c5a"),
                name: "ДополнительныеУсловияПоДоговору_Проверен".to_owned(),
            },
        ],
        base.schema.clone(),
        live_tables,
    )
}

fn guid(value: &str) -> Guid {
    Guid::from_str(value).unwrap()
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
    }
}

fn schema_table(name: &str, number: u32, columns: Vec<SchemaColumn>) -> SchemaTable {
    SchemaTable {
        name: name.to_owned(),
        number,
        columns,
        indexes: Vec::new(),
    }
}

fn schema_column(name: &str, tag: &str, reference_target: Option<&str>) -> SchemaColumn {
    SchemaColumn {
        name: name.to_owned(),
        types: vec![ColumnType {
            tag: tag.to_owned(),
            reference_target: reference_target.map(str::to_owned),
        }],
    }
}

fn live_table(name: &str, columns: &[&str]) -> LiveTable {
    LiveTable {
        name: name.to_owned(),
        columns: columns
            .iter()
            .map(|column| LiveColumn {
                name: (*column).to_owned(),
                data_type: "bytea".to_owned(),
            })
            .collect(),
        indexes: Vec::new(),
    }
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
