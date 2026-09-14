//! Register virtual tables: what they group by, and their bare form.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler};

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn sums_over_the_dimensions_the_statement_never_reads() {
    let snapshot = accumulation_register_snapshot();
    // The platform answers one row holding the turnover of the register
    // when the query reads no dimension.
    let resource_only = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОборот КАК Кол ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert_contains(
        &resource_only.sql,
        "FROM (SELECT SUM(\"__aggregate_used\".\"_fld55\") AS \"_fld55\" FROM (SELECT",
    );
    assert!(
        !resource_only.sql.contains("GROUP BY \"__aggregate_used\""),
        "{}",
        resource_only.sql
    );

    // Reading the dimension keeps the relation as it was.
    let with_dimension = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert!(
        !with_dimension.sql.contains("__aggregate_used"),
        "{}",
        with_dimension.sql
    );
    assert_contains(
        &with_dimension.sql,
        "GROUP BY \"__aggregate_base\".\"_fld54\"",
    );

    // Balances aggregate the same way.
    let balance = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОстаток КАК Кол ИЗ РегистрНакопления.Остатки.Остатки КАК О;",
    );
    assert_contains(&balance.sql, "\"__aggregate_used\"");
    assert_contains(&balance.sql, "SUM(\"__aggregate_used\".\"_fld55\")");
}

#[test]
fn accepts_virtual_tables_without_an_argument_list() {
    let snapshot = accumulation_register_snapshot();
    let bare = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Остатки КАК О;",
    );
    let parenthesized = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Остатки() КАК О;",
    );
    assert_eq!(bare.sql, parenthesized.sql);

    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile("ВЫБРАТЬ О.Номенклатура КАК Н ИЗ РегистрНакопления.Остатки.Обороты КАК О;")
        .unwrap();
    assert_contains(&mssql.sql, "GROUP BY [__aggregate_base].[_fld54]");
}

#[test]
fn groups_turnovers_by_the_requested_period() {
    let snapshot = accumulation_register_snapshot();
    let monthly = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , Месяц, ) КАК О;",
    );
    assert_contains(
        &monthly.sql,
        "date_trunc('month', \"__aggregate_base\".\"_period\") AS \"_period\"",
    );
    assert_contains(
        &monthly.sql,
        "GROUP BY date_trunc('month', \"__aggregate_base\".\"_period\"), \"__aggregate_base\".\"_fld54\"",
    );

    // The periodicity splits by period even when the column is not read.
    let unread = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОборот КАК Кол ИЗ РегистрНакопления.Остатки.Обороты(, , День, ) КАК О;",
    );
    assert_contains(
        &unread.sql,
        "\"__aggregate_used\".\"_period\" AS \"_period\"",
    );
    assert_contains(&unread.sql, "GROUP BY \"__aggregate_used\".\"_period\"");

    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile(
            "ВЫБРАТЬ О.Период КАК П, О.КоличествоОборот КАК Кол
             ИЗ РегистрНакопления.Остатки.Обороты(, , Год, ) КАК О;",
        )
        .unwrap();
    assert_contains(
        &mssql.sql,
        "DATETIME2FROMPARTS(YEAR([__aggregate_base].[_period]), 1, 1, 0, 0, 0, 0, 0)",
    );

    let unknown = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ О.КоличествоОборот КАК Кол
             ИЗ РегистрНакопления.Остатки.Обороты(, , Пятилетка, ) КАК О;",
        )
        .unwrap_err();
    assert_eq!(
        unknown.kind(),
        open_sdbl::query::QueryDiagnosticKind::UnsupportedFeature
    );
    assert!(unknown.message().contains("periodicity"), "{unknown}");
}

#[test]
fn compiles_the_balance_and_turnovers_table() {
    let snapshot = accumulation_register_snapshot();
    let whole = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоНачальныйОстаток КАК Нач,
                О.КоличествоПриход КАК Прих, О.КоличествоРасход КАК Расх,
                О.КоличествоОборот КАК Обор, О.КоличествоКонечныйОстаток КАК Кон
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты КАК О;",
    );
    // Receipts carry record kind 0 and expenses 1.
    assert_contains(
        &whole.sql,
        "SUM(CASE WHEN \"__aggregate_base\".\"_recordkind\" = 0 THEN \"__aggregate_base\".\"_fld55\" ELSE 0 END) AS \"_fld55Receipt\"",
    );
    assert_contains(&whole.sql, "SUM(0) AS \"_fld55OpeningBalance\"");
    assert_contains(
        &whole.sql,
        "SUM(0 + CASE WHEN \"__aggregate_base\".\"_recordkind\" = 0 THEN \"__aggregate_base\".\"_fld55\" ELSE -\"__aggregate_base\".\"_fld55\" END) AS \"_fld55ClosingBalance\"",
    );
    assert_eq!(
        whole
            .columns
            .iter()
            .map(|column| column.label.as_str())
            .collect::<Vec<_>>(),
        ["Н", "Нач", "Прих", "Расх", "Обор", "Кон"]
    );

    // An interval splits the movements into the opening balance and the
    // turnover of the period.
    let interval = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоНачальныйОстаток КАК Нач, О.КоличествоОборот КАК Обор
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(ДАТАВРЕМЯ(2024, 2, 1), ДАТАВРЕМЯ(2024, 5, 1), , ) КАК О;",
    );
    assert_contains(
        &interval.sql,
        "WHERE \"__aggregate_base\".\"_active\" = TRUE AND (\"__aggregate_base\".\"_period\" < TIMESTAMP '2024-05-01 00:00:00')",
    );
    assert_contains(
        &interval.sql,
        "CASE WHEN \"__aggregate_base\".\"_period\" < TIMESTAMP '2024-02-01 00:00:00' THEN CASE WHEN",
    );
    // The statement reads no dimension, so the register answers one row.
    assert_contains(&interval.sql, "\"__aggregate_used\"");

    // A periodicity groups the movements into periods, as `Обороты` does,
    // and exposes the period; the balances of such a split are running
    // sums the platform accumulates outside SQL, so they are refused.
    let periodic = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ О.Период КАК Период, О.КоличествоОборот КАК Обор
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Месяц, ) КАК О;",
        )
        .unwrap();
    assert_contains(
        &periodic.sql,
        "date_trunc('month', \"__aggregate_base\".\"_period\") AS \"_period\"",
    );

    for (source, message) in [
        (
            "ВЫБРАТЬ О.КоличествоНачальныйОстаток КАК Нач
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Месяц, ) КАК О;",
            "answers no balance column",
        ),
        (
            "ВЫБРАТЬ О.КоличествоОборот КАК Обор
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Месяц, Регистратор, ) КАК О;",
            "completion method",
        ),
        (
            "ВЫБРАТЬ О.КоличествоОборот КАК Обор
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , , ДвиженияИГраницыПериода, ) КАК О;",
            "needs a periodicity",
        ),
        (
            "ВЫБРАТЬ О.КоличествоОборот КАК Обор
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Регистратор, ) КАК О;",
            "calendar periodicity",
        ),
    ] {
        let error = QueryCompiler::new(&snapshot, PostgresBackend)
            .compile(source)
            .unwrap_err();
        assert_eq!(
            error.kind(),
            open_sdbl::query::QueryDiagnosticKind::UnsupportedFeature,
            "{source}: {error}"
        );
        assert!(error.message().contains(message), "{source}: {error}");
    }
}

#[test]
fn groups_turnovers_by_recorder_and_record() {
    let snapshot = accumulation_register_snapshot();
    let recorder = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.Регистратор КАК Р, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , Регистратор, ) КАК О;",
    );
    // The record's own period is kept, not a truncated one.
    assert_contains(
        &recorder.sql,
        "\"__aggregate_base\".\"_period\" AS \"_period\", \"__aggregate_base\".\"_recorderrref\" AS \"_recorderrref\"",
    );
    assert_contains(
        &recorder.sql,
        "GROUP BY \"__aggregate_base\".\"_period\", \"__aggregate_base\".\"_recorderrref\", \"__aggregate_base\".\"_fld54\"",
    );
    assert!(!recorder.sql.contains("_lineno"), "{}", recorder.sql);

    // The record periodicity adds the line number.
    let record = postgres(
        &snapshot,
        "ВЫБРАТЬ О.НомерСтроки КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , Запись, ) КАК О;",
    );
    assert_contains(
        &record.sql,
        "\"__aggregate_base\".\"_lineno\" AS \"_lineno\"",
    );
    // The statement reads no dimension, so the dimension is summed away
    // while the record split stays.
    assert_contains(
        &record.sql,
        "\"__aggregate_used\".\"_lineno\" AS \"_lineno\"",
    );

    // `НомерСтроки` belongs to the record periodicity alone.
    let missing = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ О.НомерСтроки КАК Н ИЗ РегистрНакопления.Остатки.Обороты(, , Регистратор, ) КАК О;",
        )
        .unwrap_err();
    assert!(missing.message().contains("was not found"), "{missing}");
}

#[test]
fn names_the_movement_type_as_the_query_does() {
    // `ВидДвижения` is how a query writes the movement type; the
    // SchemaStorage spelling is `RecordKind`. Both name the same column,
    // checked on the platform.
    let snapshot = accumulation_register_snapshot();
    let by_query_name = postgres(
        &snapshot,
        "ВЫБРАТЬ Р.ВидДвижения КАК Вид ИЗ РегистрНакопления.Остатки КАК Р;",
    );
    let by_schema_name = postgres(
        &snapshot,
        "ВЫБРАТЬ Р.RecordKind КАК Вид ИЗ РегистрНакопления.Остатки КАК Р;",
    );
    assert_eq!(by_query_name.sql, by_schema_name.sql);
    assert_contains(&by_query_name.sql, "\"_recordkind\" AS \"Вид\"");
}
