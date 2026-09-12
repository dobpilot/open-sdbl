//! Date-part functions: `ГОД` … `СЕКУНДА`, with the platform's week and
//! weekday numbering measured on 8.3.27 (2026-09-12).

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    Backend, ColumnKind, CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler,
    QueryDiagnostic, QueryDiagnosticKind,
};

fn compile<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Result<CompiledQuery, QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile(source)
}

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, PostgresBackend, source).unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn mssql(snapshot: &MetadataSnapshot, year_offset: i32, source: &str) -> CompiledQuery {
    compile(snapshot, MsSqlBackend::new(year_offset).unwrap(), source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const COLUMN: &str = "\"__src\".\"_date_time\"";
const MS_COLUMN: &str = "[__src].[_date_time]";

#[test]
fn extracts_every_part_as_an_integer_on_both_dialects() {
    let query =
        "ВЫБРАТЬ ГОД(Date) КАК Г, КВАРТАЛ(Date) КАК К, МЕСЯЦ(Date) КАК М, ДЕНЬГОДА(Date) КАК ДГ,
                ДЕНЬ(Date) КАК Д, НЕДЕЛЯ(Date) КАК Н, ДЕНЬНЕДЕЛИ(Date) КАК ДН, ЧАС(Date) КАК Ч,
                МИНУТА(Date) КАК Мн, СЕКУНДА(Date) КАК С
         ИЗ Справочник.OpenSdblMetadataProbe;";
    let postgres = postgres(&snapshot(), query);
    for needle in [
        format!("CAST(EXTRACT(YEAR FROM {COLUMN}) AS integer) AS \"Г\""),
        format!("CAST(EXTRACT(QUARTER FROM {COLUMN}) AS integer) AS \"К\""),
        format!("CAST(EXTRACT(MONTH FROM {COLUMN}) AS integer) AS \"М\""),
        format!("CAST(EXTRACT(DOY FROM {COLUMN}) AS integer) AS \"ДГ\""),
        format!("CAST(EXTRACT(DAY FROM {COLUMN}) AS integer) AS \"Д\""),
        format!(
            "((CAST(EXTRACT(DOY FROM {COLUMN}) AS integer) + CAST(EXTRACT(ISODOW FROM date_trunc('year', {COLUMN})) AS integer) - 2) / 7 + 1) AS \"Н\""
        ),
        format!("CAST(EXTRACT(ISODOW FROM {COLUMN}) AS integer) AS \"ДН\""),
        format!("CAST(EXTRACT(HOUR FROM {COLUMN}) AS integer) AS \"Ч\""),
        format!("CAST(EXTRACT(MINUTE FROM {COLUMN}) AS integer) AS \"Мн\""),
        format!("CAST(EXTRACT(SECOND FROM {COLUMN}) AS integer) AS \"С\""),
    ] {
        assert_contains(&postgres.sql, &needle);
    }
    assert_eq!(postgres.columns.len(), 10);
    assert!(
        postgres
            .columns
            .iter()
            .all(|column| matches!(column.kind, ColumnKind::Number { .. }))
    );

    let mssql = mssql(&mssql_snapshot(), 0, query);
    let day_number =
        format!("DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, {MS_COLUMN}))");
    for needle in [
        format!("DATEPART(year, {MS_COLUMN}) AS [Г]"),
        format!("DATEPART(quarter, {MS_COLUMN}) AS [К]"),
        format!("DATEPART(month, {MS_COLUMN}) AS [М]"),
        format!("DATEPART(dayofyear, {MS_COLUMN}) AS [ДГ]"),
        format!("DATEPART(day, {MS_COLUMN}) AS [Д]"),
        format!(
            "((DATEPART(dayofyear, {MS_COLUMN}) - 1 + (({day_number} - DATEPART(dayofyear, {MS_COLUMN}) + 1) % 7 + 7) % 7) / 7 + 1) AS [Н]"
        ),
        format!("(({day_number} % 7 + 7) % 7 + 1) AS [ДН]"),
        format!("DATEPART(hour, {MS_COLUMN}) AS [Ч]"),
        format!("DATEPART(minute, {MS_COLUMN}) AS [Мн]"),
        format!("DATEPART(second, {MS_COLUMN}) AS [С]"),
    ] {
        assert_contains(&mssql.sql, &needle);
    }
}

#[test]
fn takes_parts_from_the_logical_date_under_a_year_offset() {
    let query = "SELECT YEAR(Date) AS Y, WEEKDAY(Date) AS W FROM Catalog.OpenSdblMetadataProbe;";
    let shifted = mssql(&mssql_snapshot(), 2000, query);
    assert_contains(
        &shifted.sql,
        "DATEPART(year, DATEADD(year, -2000, [__src].[_date_time])) AS [Y]",
    );
    assert_contains(
        &shifted.sql,
        "((DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, DATEADD(year, -2000, [__src].[_date_time]))) % 7 + 7) % 7 + 1) AS [W]",
    );

    let source_free = mssql(
        &mssql_snapshot(),
        2000,
        "SELECT YEAR(DATETIME(2021, 1, 4)) AS Y, WEEKDAY(DATETIME(2021, 1, 4)) AS W;",
    );
    assert_contains(
        &source_free.sql,
        "DATEPART(year, CONVERT(datetime2, '2021-01-04T00:00:00', 126)) AS [Y]",
    );
}

#[test]
fn parts_group_filter_and_nest() {
    let snapshot = snapshot();
    let grouped = postgres(
        &snapshot,
        "ВЫБРАТЬ ГОД(Date) КАК Год, МЕСЯЦ(Date) КАК Месяц, КОЛИЧЕСТВО(*) КАК Н
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ ДЕНЬНЕДЕЛИ(Date) В (6, 7)
         СГРУППИРОВАТЬ ПО ГОД(Date), МЕСЯЦ(Date);",
    );
    assert_contains(
        &grouped.sql,
        "GROUP BY CAST(EXTRACT(YEAR FROM \"__src\".\"_date_time\") AS integer), CAST(EXTRACT(MONTH FROM \"__src\".\"_date_time\") AS integer)",
    );
    assert_contains(
        &grouped.sql,
        "(CAST(EXTRACT(ISODOW FROM \"__src\".\"_date_time\") AS integer) IN (6, 7))",
    );
    assert_eq!(grouped.columns[0].label, "Год");
    assert_eq!(grouped.columns[1].label, "Месяц");

    let nested = postgres(
        &snapshot,
        "SELECT YEAR(ENDOFPERIOD(DATEADD(Date, MONTH, 1), YEAR)) AS Y,
                DATEADD(Date, DAY, DAY(Date)) AS Shifted
         FROM Catalog.OpenSdblMetadataProbe;",
    );
    assert_contains(
        &nested.sql,
        "CAST(EXTRACT(YEAR FROM (date_trunc('year', (\"__src\".\"_date_time\" + CAST(1 AS integer) * INTERVAL '1 month')) + INTERVAL '1 year' - INTERVAL '1 second')) AS integer) AS \"Y\"",
    );
    assert_contains(
        &nested.sql,
        "(\"__src\".\"_date_time\" + CAST(CAST(EXTRACT(DAY FROM \"__src\".\"_date_time\") AS integer) AS integer) * INTERVAL '1 day') AS \"Shifted\"",
    );
}

#[test]
fn part_keywords_stay_usable_as_periods_aliases_and_names() {
    let snapshot = snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ НАЧАЛОПЕРИОДА(Date, ДЕНЬ) КАК День, КОНЕЦПЕРИОДА(Date, МЕСЯЦ) КАК Месяц,
                ДОБАВИТЬКДАТЕ(Date, НЕДЕЛЯ, 1) КАК Неделя, ГОД(Date) КАК Год
         ИЗ Справочник.OpenSdblMetadataProbe КАК Час
         УПОРЯДОЧИТЬ ПО Час.Date;",
    );
    assert_eq!(
        compiled
            .columns
            .iter()
            .map(|column| column.label.as_str())
            .collect::<Vec<_>>(),
        ["День", "Месяц", "Неделя", "Год"]
    );
    assert_contains(
        &compiled.sql,
        "date_trunc('day', \"Час\".\"_date_time\") AS \"День\"",
    );

    let english = postgres(
        &snapshot,
        "SELECT Date AS Day, Code AS Week FROM Catalog.OpenSdblMetadataProbe AS Month WHERE Month.Code <> \"\";",
    );
    assert_eq!(english.columns[0].label, "Day");
    assert_eq!(english.columns[1].label, "Week");
}

#[test]
fn rejects_non_date_arguments() {
    let snapshot = snapshot();
    for (query, message) in [
        (
            "SELECT YEAR(Code) FROM Catalog.OpenSdblMetadataProbe;",
            "YEAR first argument must resolve to a date field",
        ),
        (
            "SELECT MONTH(4);",
            "MONTH first argument must be a date expression",
        ),
        (
            "SELECT WEEK(\"2020-01-01\");",
            "WEEK first argument must be a date expression",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, query).unwrap_err();
        assert_eq!(
            error.kind(),
            QueryDiagnosticKind::Syntax,
            "{query}: {error}"
        );
        assert!(error.message().contains(message), "{query}: {error}");
    }
    let error = compile(
        &snapshot,
        PostgresBackend,
        "SELECT YEAR(Date, 1) FROM Catalog.OpenSdblMetadataProbe;",
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Syntax);
}
