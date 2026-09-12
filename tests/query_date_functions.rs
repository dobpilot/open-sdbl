//! Period arithmetic: `КОНЕЦПЕРИОДА`, `ДОБАВИТЬКДАТЕ`, `РАЗНОСТЬДАТ`, and the
//! widened virtual-table period arguments.
//!
//! Expected values follow the platform (8.3.27) measured on a PostgreSQL
//! probe base on 2026-09-12: end of period is the last second, a fractional
//! shift count is rounded for second through month and truncated for the
//! composite periods, and a difference counts crossed unit boundaries.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, ColumnKind, CompileOptions, CompiledQuery, MsSqlBackend, MsSqlDialectLevel,
    ParameterDate, ParameterValue, PostgresBackend, QueryCompiler, QueryDiagnostic,
    QueryDiagnosticKind, QueryParameter,
};

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
    parameters: &[QueryParameter],
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend)
        .compile_with(source, &CompileOptions::new().parameters(parameters))
}

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, PostgresBackend, source, &[])
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn mssql(snapshot: &MetadataSnapshot, year_offset: i32, source: &str) -> CompiledQuery {
    compile(
        snapshot,
        MsSqlBackend::new(year_offset).unwrap(),
        source,
        &[],
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn mssql_2008(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    let backend = MsSqlBackend::new(0)
        .unwrap()
        .with_dialect_level(MsSqlDialectLevel::Sql2008);
    compile(snapshot, backend, source, &[]).unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn date(year: u16, month: u8, day: u8) -> ParameterValue {
    ParameterValue::Date(ParameterDate::new(year, month, day, 0, 0, 0).unwrap())
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn end_of_period_is_the_last_second_of_every_period() {
    let snapshot = snapshot();
    let mssql_snapshot = mssql_snapshot();
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
        let query = format!(
            "ВЫБРАТЬ КОНЕЦПЕРИОДА(Date, {period}) КАК К ИЗ Справочник.OpenSdblMetadataProbe;"
        );
        let compiled = postgres(&snapshot, &query);
        assert_contains(&compiled.sql, "- INTERVAL '1 second')");
        assert_eq!(compiled.columns[0].kind, ColumnKind::DateTime);
        assert_contains(
            &mssql(&mssql_snapshot, 0, &query).sql,
            "DATEADD(second, -1, ",
        );
        assert_contains(
            &mssql_2008(&mssql_snapshot, &query).sql,
            "DATEADD(second, -1, ",
        );
    }

    let source_free = postgres(
        &snapshot,
        "SELECT ENDOFPERIOD(DATETIME(2020, 2, 10), MONTH) AS E;",
    );
    assert_contains(
        &source_free.sql,
        "(date_trunc('month', TIMESTAMP '2020-02-10 00:00:00') + INTERVAL '1 month' - INTERVAL '1 second') AS \"E\"",
    );
    let source_free = mssql(
        &mssql_snapshot,
        0,
        "SELECT ENDOFPERIOD(DATETIME(2020, 2, 10), MONTH) AS E;",
    );
    assert_contains(
        &source_free.sql,
        "DATEADD(second, -1, DATEADD(month, 1, DATETIME2FROMPARTS(YEAR(CONVERT(datetime2, '2020-02-10T00:00:00', 126)), MONTH(CONVERT(datetime2, '2020-02-10T00:00:00', 126)), 1, 0, 0, 0, 0, 0))) AS [E]",
    );
    let legacy = mssql_2008(
        &mssql_snapshot,
        "SELECT ENDOFPERIOD(DATETIME(2020, 2, 10), MONTH) AS E;",
    );
    assert_contains(
        &legacy.sql,
        "DATEADD(second, -1, DATEADD(month, 1, DATEADD(month, DATEDIFF(month, CONVERT(datetime2, '00010101', 112), CONVERT(datetime2, '2020-02-10T00:00:00', 126)), CONVERT(datetime2, '00010101', 112))))",
    );
}

#[test]
fn end_of_ten_days_period_stops_at_the_month_end() {
    let query = "ВЫБРАТЬ КОНЕЦПЕРИОДА(Date, ДЕКАДА) КАК К ИЗ Справочник.OpenSdblMetadataProbe;";
    let postgres = postgres(&snapshot(), query);
    assert_contains(
        &postgres.sql,
        "(CASE WHEN EXTRACT(DAY FROM \"__src\".\"_date_time\") > 20 THEN date_trunc('month', \"__src\".\"_date_time\") + INTERVAL '1 month' ELSE (date_trunc('month', \"__src\".\"_date_time\") + (LEAST(((EXTRACT(DAY FROM \"__src\".\"_date_time\")::integer - 1) / 10), 2) * INTERVAL '10 days')) + INTERVAL '10 days' END - INTERVAL '1 second')",
    );
    let mssql = mssql(&mssql_snapshot(), 0, query);
    assert_contains(
        &mssql.sql,
        "DATEADD(second, -1, CASE WHEN DAY([__src].[_date_time]) <= 20 THEN DATEADD(day, 10, DATETIME2FROMPARTS(YEAR([__src].[_date_time]), MONTH([__src].[_date_time]), CASE WHEN DAY([__src].[_date_time]) <= 10 THEN 1 WHEN DAY([__src].[_date_time]) <= 20 THEN 11 ELSE 21 END, 0, 0, 0, 0, 0)) ELSE DATEADD(month, 1, DATETIME2FROMPARTS(YEAR([__src].[_date_time]), MONTH([__src].[_date_time]), 1, 0, 0, 0, 0, 0)) END)",
    );
}

#[test]
fn date_add_rounds_or_truncates_the_count_like_the_platform() {
    let query = "SELECT DATEADD(DATETIME(2020, 1, 31, 23, 59, 59), MONTH, 1) AS M,
                DATEADD(DATETIME(2020, 1, 1), DAY, 1.5) AS D,
                DATEADD(DATETIME(2020, 1, 1), YEAR, 1.5) AS Y,
                DATEADD(DATETIME(2020, 1, 1), TENDAYS, -1) AS T,
                DATEADD(DATETIME(2020, 1, 1), HALFYEAR, 2) AS H,
                DATEADD(DATETIME(2020, 1, 1), SECOND, 30) AS S,
                DATEADD(DATETIME(2020, 1, 1), WEEK, 2) AS W,
                DATEADD(DATETIME(2020, 1, 1), QUARTER, 1) AS Q;";
    let postgres = postgres(&snapshot(), query);
    for needle in [
        "(TIMESTAMP '2020-01-31 23:59:59' + CAST(1 AS integer) * INTERVAL '1 month') AS \"M\"",
        "CAST(1.5 AS integer) * INTERVAL '1 day') AS \"D\"",
        "CAST(trunc(CAST(1.5 AS numeric)) AS integer) * INTERVAL '1 year') AS \"Y\"",
        "CAST(trunc(CAST((-1) AS numeric)) AS integer) * INTERVAL '10 days') AS \"T\"",
        "CAST(trunc(CAST(2 AS numeric)) AS integer) * INTERVAL '6 months') AS \"H\"",
        "CAST(30 AS integer) * INTERVAL '1 second') AS \"S\"",
        "CAST(2 AS integer) * INTERVAL '7 days') AS \"W\"",
        "CAST(trunc(CAST(1 AS numeric)) AS integer) * INTERVAL '3 months') AS \"Q\"",
    ] {
        assert_contains(&postgres.sql, needle);
    }
    assert!(
        postgres
            .columns
            .iter()
            .all(|column| column.kind == ColumnKind::DateTime)
    );

    let mssql = mssql(&mssql_snapshot(), 0, query);
    for needle in [
        "DATEADD(month, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 1), 0)), CONVERT(datetime2, '2020-01-31T23:59:59', 126)) AS [M]",
        "DATEADD(day, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 1.5), 0)), CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [D]",
        "DATEADD(year, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 1.5), 0, 1)), CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [Y]",
        "DATEADD(day, CONVERT(int, ROUND(CONVERT(numeric(38, 10), (-1)), 0, 1)) * 10, CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [T]",
        "DATEADD(month, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 2), 0, 1)) * 6, CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [H]",
        "DATEADD(second, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 30), 0)), CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [S]",
        "DATEADD(week, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 2), 0)), CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [W]",
        "DATEADD(quarter, CONVERT(int, ROUND(CONVERT(numeric(38, 10), 1), 0, 1)), CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [Q]",
    ] {
        assert_contains(&mssql.sql, needle);
    }
}

#[test]
fn date_add_takes_the_count_from_fields_parameters_and_expressions() {
    let snapshot = accumulation_register_snapshot();
    let from_field = postgres(
        &snapshot,
        "ВЫБРАТЬ ДОБАВИТЬКДАТЕ(Период, ДЕНЬ, Количество) КАК Д ИЗ РегистрНакопления.Остатки;",
    );
    assert_contains(
        &from_field.sql,
        "(\"__src\".\"_period\" + CAST(\"__src\".\"_fld55\" AS integer) * INTERVAL '1 day') AS \"Д\"",
    );

    let from_expression = postgres(
        &snapshot,
        "ВЫБРАТЬ ДОБАВИТЬКДАТЕ(Период, МЕСЯЦ, Количество * 2 + 1) КАК Д ИЗ РегистрНакопления.Остатки;",
    );
    assert_contains(
        &from_expression.sql,
        "CAST(((\"__src\".\"_fld55\" * 2) + 1) AS integer) * INTERVAL '1 month'",
    );

    let from_parameter = compile(
        &snapshot,
        PostgresBackend,
        "ВЫБРАТЬ ДОБАВИТЬКДАТЕ(Период, ЧАС, &Н) КАК Д ИЗ РегистрНакопления.Остатки;",
        &[QueryParameter::new(
            "Н",
            ParameterValue::Number {
                unscaled: 15,
                scale: 1,
            },
        )],
    )
    .unwrap();
    assert_contains(
        &from_parameter.sql,
        "CAST(1.5 AS integer) * INTERVAL '1 hour'",
    );

    QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare("ВЫБРАТЬ ДОБАВИТЬКДАТЕ(Период, ЧАС, &Н) КАК Д ИЗ РегистрНакопления.Остатки;")
        .unwrap();
}

#[test]
fn date_diff_counts_boundaries_on_both_dialects() {
    let query =
        "SELECT DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), DAY) AS D,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), SECOND) AS S,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), MINUTE) AS MI,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), HOUR) AS H,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), MONTH) AS M,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), QUARTER) AS Q,
                DATEDIFF(DATETIME(2020, 12, 31, 23, 59, 59), DATETIME(2021, 1, 1), YEAR) AS Y;";
    let postgres = postgres(&snapshot(), query);
    const FROM: &str = "TIMESTAMP '2020-12-31 23:59:59'";
    const TO: &str = "TIMESTAMP '2021-01-01 00:00:00'";
    for needle in [
        format!("(CAST({TO} AS date) - CAST({FROM} AS date)) AS \"D\""),
        format!("CAST(EXTRACT(EPOCH FROM ({TO} - {FROM})) AS bigint) AS \"S\""),
        format!(
            "(CAST(EXTRACT(EPOCH FROM (date_trunc('minute', {TO}) - date_trunc('minute', {FROM}))) AS bigint) / 60) AS \"MI\""
        ),
        format!(
            "(CAST(EXTRACT(EPOCH FROM (date_trunc('hour', {TO}) - date_trunc('hour', {FROM}))) AS bigint) / 3600) AS \"H\""
        ),
        format!(
            "CAST((EXTRACT(YEAR FROM {TO}) - EXTRACT(YEAR FROM {FROM})) * 12 + EXTRACT(MONTH FROM {TO}) - EXTRACT(MONTH FROM {FROM}) AS integer) AS \"M\""
        ),
        format!(
            "CAST((EXTRACT(YEAR FROM {TO}) - EXTRACT(YEAR FROM {FROM})) * 4 + EXTRACT(QUARTER FROM {TO}) - EXTRACT(QUARTER FROM {FROM}) AS integer) AS \"Q\""
        ),
        format!("CAST(EXTRACT(YEAR FROM {TO}) - EXTRACT(YEAR FROM {FROM}) AS integer) AS \"Y\""),
    ] {
        assert_contains(&postgres.sql, &needle);
    }
    assert!(
        postgres
            .columns
            .iter()
            .all(|column| matches!(column.kind, ColumnKind::Number { .. }))
    );

    let mssql = mssql(&mssql_snapshot(), 0, query);
    const MS_FROM: &str = "CONVERT(datetime2, '2020-12-31T23:59:59', 126)";
    const MS_TO: &str = "CONVERT(datetime2, '2021-01-01T00:00:00', 126)";
    for needle in [
        format!("DATEDIFF(day, {MS_FROM}, {MS_TO}) AS [D]"),
        format!(
            "(DATEDIFF(day, CONVERT(date, {MS_FROM}), CONVERT(date, {MS_TO})) * CAST(86400 AS bigint) + DATEDIFF(second, CONVERT(date, {MS_TO}), {MS_TO}) - DATEDIFF(second, CONVERT(date, {MS_FROM}), {MS_FROM})) AS [S]"
        ),
        format!("* CAST(1440 AS bigint) + DATEDIFF(minute, CONVERT(date, {MS_TO}), {MS_TO})"),
        format!("* CAST(24 AS bigint) + DATEDIFF(hour, CONVERT(date, {MS_TO}), {MS_TO})"),
        format!("DATEDIFF(month, {MS_FROM}, {MS_TO}) AS [M]"),
        format!("DATEDIFF(quarter, {MS_FROM}, {MS_TO}) AS [Q]"),
        format!("DATEDIFF(year, {MS_FROM}, {MS_TO}) AS [Y]"),
    ] {
        assert_contains(&mssql.sql, &needle);
    }
}

#[test]
fn date_diff_shifts_operands_to_the_logical_date_under_a_year_offset() {
    let snapshot = mssql_snapshot();
    let query = "SELECT DATEDIFF(Date, DATETIME(2021, 3, 15), DAY) AS D FROM Catalog.OpenSdblMetadataProbe;";
    let shifted = mssql(&snapshot, 2000, query);
    assert_contains(
        &shifted.sql,
        "DATEDIFF(day, DATEADD(year, -2000, [__src].[_date_time]), DATEADD(year, -2000, DATEADD(year, 2000, CONVERT(datetime2, '2021-03-15T00:00:00', 126)))) AS [D]",
    );
    let plain = mssql(&snapshot, 0, query);
    assert_contains(
        &plain.sql,
        "DATEDIFF(day, [__src].[_date_time], CONVERT(datetime2, '2021-03-15T00:00:00', 126)) AS [D]",
    );
}

#[test]
fn reports_period_and_argument_diagnostics() {
    let snapshot = snapshot();
    for (query, kind, message) in [
        (
            "SELECT BEGINOFPERIOD(DATETIME(2026, 9, 2), SECOND);",
            QueryDiagnosticKind::Syntax,
            "BEGINOFPERIOD does not accept the SECOND period",
        ),
        (
            "SELECT ENDOFPERIOD(DATETIME(2026, 9, 2), СЕКУНДА);",
            QueryDiagnosticKind::Syntax,
            "ENDOFPERIOD does not accept the SECOND period",
        ),
        (
            "SELECT ENDOFPERIOD(DATETIME(2026, 9, 2), CENTURY);",
            QueryDiagnosticKind::UnsupportedFeature,
            "unsupported ENDOFPERIOD period \"CENTURY\"",
        ),
        (
            "SELECT DATEDIFF(DATETIME(2026, 9, 2), DATETIME(2026, 9, 3), WEEK);",
            QueryDiagnosticKind::Syntax,
            "DATEDIFF does not accept the WEEK period",
        ),
        (
            "SELECT DATEDIFF(DATETIME(2026, 9, 2), DATETIME(2026, 9, 3), ДЕКАДА);",
            QueryDiagnosticKind::Syntax,
            "DATEDIFF does not accept the TENDAYS period",
        ),
        (
            "SELECT DATEADD(4, DAY, 1);",
            QueryDiagnosticKind::Syntax,
            "DATEADD first argument must be a date expression",
        ),
        (
            "SELECT DATEDIFF(DATETIME(2026, 9, 2), 4, DAY);",
            QueryDiagnosticKind::Syntax,
            "DATEDIFF second argument must be a date expression",
        ),
        (
            "SELECT DATEADD(DATETIME(2026, 9, 2), DAY, \"x\");",
            QueryDiagnosticKind::Syntax,
            "DATEADD count must be a number",
        ),
        (
            "SELECT ENDOFPERIOD(Code, MONTH) FROM Catalog.OpenSdblMetadataProbe;",
            QueryDiagnosticKind::Syntax,
            "ENDOFPERIOD first argument must resolve to a date field",
        ),
        (
            "SELECT DATEADD(Date, DAY, Code) FROM Catalog.OpenSdblMetadataProbe;",
            QueryDiagnosticKind::Syntax,
            "DATEADD count must be a number",
        ),
        (
            "SELECT DATEADD(Date, DAY) FROM Catalog.OpenSdblMetadataProbe;",
            QueryDiagnosticKind::Syntax,
            "expected \",\"",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, query, &[]).unwrap_err();
        assert_eq!(error.kind(), kind, "{query}: {error}");
        assert!(error.message().contains(message), "{query}: {error}");
        let error =
            compile(&mssql_snapshot(), MsSqlBackend::new(0).unwrap(), query, &[]).unwrap_err();
        assert_eq!(error.kind(), kind, "{query}: {error}");
    }
}

#[test]
fn virtual_table_periods_accept_nested_functions_and_parameters() {
    let snapshot = accumulation_register_snapshot();
    let parameters = [QueryParameter::new("П", date(2026, 8, 15))];
    let turnovers = compile(
        &snapshot,
        PostgresBackend,
        "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(BEGINOFPERIOD(&П, MONTH), ENDOFPERIOD(&П, MONTH));",
        &parameters,
    )
    .unwrap();
    assert_contains(
        &turnovers.sql,
        ">= date_trunc('month', TIMESTAMP '2026-08-15 00:00:00')",
    );
    assert_contains(
        &turnovers.sql,
        "< (date_trunc('month', TIMESTAMP '2026-08-15 00:00:00') + INTERVAL '1 month' - INTERVAL '1 second')",
    );

    let balance = compile(
        &snapshot,
        MsSqlBackend::new(2000).unwrap(),
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(DATEADD(&П, DAY, -1));",
        &parameters,
    )
    .unwrap();
    assert_contains(
        &balance.sql,
        "DATEADD(day, CONVERT(int, ROUND(CONVERT(numeric(38, 10), (-1)), 0)), DATEADD(year, 2000, CONVERT(datetime2, '2026-08-15T00:00:00', 126)))",
    );

    let with_count_parameter = compile(
        &snapshot,
        PostgresBackend,
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(DATEADD(DATETIME(2026, 8, 1), DAY, &Н));",
        &[QueryParameter::new(
            "Н",
            ParameterValue::Number {
                unscaled: 3,
                scale: 0,
            },
        )],
    )
    .unwrap();
    assert_contains(
        &with_count_parameter.sql,
        "(TIMESTAMP '2026-08-01 00:00:00' + CAST(3 AS integer) * INTERVAL '1 day')",
    );

    QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare("SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(BEGINOFPERIOD(&П, MONTH), ENDOFPERIOD(&П, MONTH));")
        .unwrap();

    let wrong_count = compile(
        &snapshot,
        PostgresBackend,
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(DATEADD(&П, DAY, Количество));",
        &parameters,
    )
    .unwrap_err();
    assert_eq!(wrong_count.kind(), QueryDiagnosticKind::Metadata);
    assert!(
        wrong_count
            .message()
            .contains("numeric literal or parameter")
    );

    let wrong_parameter = compile(
        &snapshot,
        PostgresBackend,
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(DATEADD(&П, DAY, &П));",
        &parameters,
    )
    .unwrap_err();
    assert_eq!(wrong_parameter.kind(), QueryDiagnosticKind::Parameter);
}

#[test]
fn period_functions_serve_as_group_keys_and_nest() {
    let snapshot = snapshot();
    let grouped = postgres(
        &snapshot,
        "ВЫБРАТЬ КОНЕЦПЕРИОДА(Date, МЕСЯЦ) КАК М, КОЛИЧЕСТВО(*) КАК N
         ИЗ Справочник.OpenSdblMetadataProbe
         СГРУППИРОВАТЬ ПО КОНЕЦПЕРИОДА(Date, МЕСЯЦ);",
    );
    assert_contains(
        &grouped.sql,
        "GROUP BY (date_trunc('month', \"__src\".\"_date_time\") + INTERVAL '1 month' - INTERVAL '1 second')",
    );

    let nested = postgres(
        &snapshot,
        "SELECT BEGINOFPERIOD(DATEADD(Date, MONTH, 1), YEAR) AS Y,
                DATEDIFF(Date, ENDOFPERIOD(Date, YEAR), DAY) AS Remaining
         FROM Catalog.OpenSdblMetadataProbe
         WHERE DATEDIFF(Date, DATETIME(2026, 1, 1), MONTH) > 0;",
    );
    assert_contains(
        &nested.sql,
        "date_trunc('year', (\"__src\".\"_date_time\" + CAST(1 AS integer) * INTERVAL '1 month')) AS \"Y\"",
    );
    assert_contains(
        &nested.sql,
        "(CAST((date_trunc('year', \"__src\".\"_date_time\") + INTERVAL '1 year' - INTERVAL '1 second') AS date) - CAST(\"__src\".\"_date_time\" AS date)) AS \"Remaining\"",
    );
    assert_contains(&nested.sql, "AS integer) > 0)");
}

#[test]
fn nested_source_free_dates_stay_in_the_storage_domain() {
    let query =
        "SELECT T.D AS D, ENDOFPERIOD(T.D, DAY) AS E FROM (SELECT DATETIME(2020, 1, 1) AS D) AS T;";
    let mssql = mssql(&mssql_snapshot(), 2000, query);
    assert_contains(
        &mssql.sql,
        "(SELECT DATEADD(year, 2000, CONVERT(datetime2, '2020-01-01T00:00:00', 126)) AS [D]) AS [T]",
    );
    assert_contains(&mssql.sql, "DATEADD(year, -2000, [T].[D]) AS [D]");
    let postgres = postgres(&snapshot(), query);
    assert_contains(
        &postgres.sql,
        "(date_trunc('day', \"T\".\"D\") + INTERVAL '1 day' - INTERVAL '1 second') AS \"E\"",
    );
    assert_eq!(postgres.columns[1].kind, ColumnKind::DateTime);
}

#[test]
fn date_functions_accept_parameters_bound_or_not() {
    let snapshot = snapshot();
    let query = "ВЫБРАТЬ РАЗНОСТЬДАТ(&А, &Б, ДЕНЬ) КАК Д, КОНЕЦПЕРИОДА(&А, МЕСЯЦ) КАК К, ДОБАВИТЬКДАТЕ(&А, ДЕНЬ, &Н) КАК П
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ НАЧАЛОПЕРИОДА(&А, МЕСЯЦ) <= Date;";
    QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare(query)
        .unwrap();
    let bound = compile(
        &snapshot,
        PostgresBackend,
        query,
        &[
            QueryParameter::new("А", date(2020, 12, 31)),
            QueryParameter::new("Б", date(2021, 1, 1)),
            QueryParameter::new(
                "Н",
                ParameterValue::Number {
                    unscaled: 3,
                    scale: 0,
                },
            ),
        ],
    )
    .unwrap();
    assert_contains(
        &bound.sql,
        "(CAST(TIMESTAMP '2021-01-01 00:00:00' AS date) - CAST(TIMESTAMP '2020-12-31 00:00:00' AS date)) AS \"Д\"",
    );
    assert_contains(
        &bound.sql,
        "(TIMESTAMP '2020-12-31 00:00:00' + CAST(3 AS integer) * INTERVAL '1 day') AS \"П\"",
    );

    let source_free = compile(
        &snapshot,
        PostgresBackend,
        "SELECT DATEDIFF(&А, DATETIME(2021, 1, 1), YEAR) AS Y;",
        &[QueryParameter::new("А", date(2020, 12, 31))],
    )
    .unwrap();
    assert_contains(
        &source_free.sql,
        "CAST(EXTRACT(YEAR FROM TIMESTAMP '2021-01-01 00:00:00') - EXTRACT(YEAR FROM TIMESTAMP '2020-12-31 00:00:00') AS integer) AS \"Y\"",
    );
    let wrong = compile(
        &snapshot,
        PostgresBackend,
        "SELECT DATEDIFF(&А, DATETIME(2021, 1, 1), YEAR) AS Y;",
        &[QueryParameter::new(
            "А",
            ParameterValue::String("x".to_owned()),
        )],
    )
    .unwrap_err();
    assert_eq!(wrong.kind(), QueryDiagnosticKind::Syntax);
}

#[test]
fn period_keywords_remain_usable_as_names() {
    let compiled = postgres(
        &snapshot(),
        "ВЫБРАТЬ Date КАК КонецПериода, Code КАК ДобавитьКДате ИЗ Справочник.OpenSdblMetadataProbe КАК РазностьДат;",
    );
    assert_eq!(compiled.columns[0].label, "КонецПериода");
    assert_eq!(compiled.columns[1].label, "ДобавитьКДате");
}
