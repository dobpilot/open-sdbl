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
        "FROM (SELECT SUM(\"__aggregate_used\".\"_fld55\") AS \"_fld55\",",
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

    // A balance with a calendar periodicity runs over the month buckets.
    let monthly_balance = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ О.Период КАК Период, О.КоличествоНачальныйОстаток КАК Нач
             ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Месяц, ) КАК О;",
        )
        .unwrap();
    assert_contains(
        &monthly_balance.sql,
        "ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING), 0) AS \"_fld55OpeningBalance\"",
    );

    let source = "ВЫБРАТЬ О.КоличествоОборот КАК Обор
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Месяц, Регистратор, ) КАК О;";
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(source)
        .unwrap_err();
    assert_eq!(
        error.kind(),
        open_sdbl::query::QueryDiagnosticKind::UnsupportedFeature,
        "{source}: {error}"
    );
    assert!(
        error.message().contains("completion method"),
        "{source}: {error}"
    );
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

#[test]
fn reads_a_reference_pair_as_a_value() {
    // A recorder of several document kinds is stored as an `RTRef`/`RRRef`
    // pair without a `_TYPE` member, because such a value is always a
    // reference. Measured on 8.3.27: the platform takes the pair as a
    // value everywhere, answers its type as the reference tag beside the
    // table number, and aggregates the concatenated members.
    let mut session = open_sdbl::query::SessionParameters::new();
    for name in ["ЗначениеРазделителя", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(open_sdbl::query::QueryParameter::new(
            name,
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
    }
    session.set(open_sdbl::query::QueryParameter::new(
        "ИспользованиеРазделителя",
        open_sdbl::query::ParameterValue::Boolean(false),
    ));
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let compile = |source: &str| {
        QueryCompiler::new(&snapshot, PostgresBackend)
            .compile_with(
                source,
                &open_sdbl::query::CompileOptions::new().session(&session),
            )
            .unwrap_or_else(|error| panic!("{source}: {error}"))
    };

    let typed = compile(
        "ВЫБРАТЬ ТИПЗНАЧЕНИЯ(Р.Регистратор) КАК Тип
         ИЗ РегистрНакопления.КоличествоПредметовВПапках КАК Р;",
    );
    assert_contains(
        &typed.sql,
        "(decode('08', 'hex') || \"Р\".\"_recordertref\") AS \"Тип\"",
    );
    assert_eq!(typed.columns[0].kind, open_sdbl::query::ColumnKind::Type);

    let aggregated = compile(
        "ВЫБРАТЬ МАКСИМУМ(Р.Регистратор) КАК Макс
         ИЗ РегистрНакопления.КоличествоПредметовВПапках КАК Р;",
    );
    assert_contains(
        &aggregated.sql,
        "MAX((\"Р\".\"_recordertref\" || \"Р\".\"_recorderrref\"))",
    );
    // The aggregate of a payload is a payload: runtime-typed, so that
    // `ЕСТЬNULL(МАКСИМУМ(…), НЕОПРЕДЕЛЕНО)` needs no widening.
    assert_eq!(
        aggregated.columns[0].kind,
        open_sdbl::query::ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        }
    );
    let coalesced = compile(
        "ВЫБРАТЬ ЕСТЬNULL(МАКСИМУМ(Р.Регистратор), НЕОПРЕДЕЛЕНО) КАК Макс
         ИЗ РегистрНакопления.КоличествоПредметовВПапках КАК Р;",
    );
    assert_contains(
        &coalesced.sql,
        "COALESCE(MAX((\"Р\".\"_recordertref\" || \"Р\".\"_recorderrref\")), NULL)",
    );
}

#[test]
fn turnovers_of_a_balance_register_answer_receipts_and_expenses() {
    let snapshot = accumulation_register_snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоПриход КАК П, О.КоличествоРасход КАК Р
         ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert_contains(
        &compiled.sql,
        "SUM(CASE WHEN \"__aggregate_base\".\"_recordkind\" = 0 THEN \"__aggregate_base\".\"_fld55\" ELSE 0 END) AS \"_fld55Receipt\"",
    );
    assert_contains(
        &compiled.sql,
        "SUM(CASE WHEN \"__aggregate_base\".\"_recordkind\" = 1 THEN \"__aggregate_base\".\"_fld55\" ELSE 0 END) AS \"_fld55Expense\"",
    );
    // The two columns are resources: they are summed away with the
    // dimensions the statement never reads.
    let pruned = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоПриход КАК П ИЗ РегистрНакопления.Остатки.Обороты КАК О;",
    );
    assert_contains(
        &pruned.sql,
        "SUM(\"__aggregate_used\".\"_fld55Receipt\") AS \"_fld55Receipt\"",
    );
    let english = postgres(
        &snapshot,
        "SELECT O.КоличествоReceipt AS P FROM AccumulationRegister.Остатки.Turnovers AS O;",
    );
    assert_contains(&english.sql, "\"_fld55Receipt\"");
}

#[test]
fn the_period_periodicity_does_not_split() {
    let snapshot = accumulation_register_snapshot();
    let plain = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , , ) КАК О;",
    );
    for periodicity in ["Период", "Period"] {
        let split = postgres(
            &snapshot,
            &format!(
                "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
                 ИЗ РегистрНакопления.Остатки.Обороты(, , {periodicity}, ) КАК О;"
            ),
        );
        assert_eq!(split.sql, plain.sql, "{periodicity}");
    }
}

#[test]
fn auto_splits_by_the_fields_the_statement_reads() {
    let snapshot = accumulation_register_snapshot();
    // Reading a calendar level and the recorder keeps both groupings.
    let split = postgres(
        &snapshot,
        "ВЫБРАТЬ О.ПериодМесяц КАК М, О.Регистратор КАК Р, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , Авто, ) КАК О;",
    );
    assert_contains(&split.sql, "AS \"_PeriodMonth\"");
    assert_contains(&split.sql, "\"__aggregate_used\".\"_PeriodMonth\"");
    assert_contains(&split.sql, "\"__aggregate_used\".\"_recorderrref\"");
    assert!(
        !split.sql.contains("\"__aggregate_used\".\"_PeriodYear\""),
        "{}",
        split.sql
    );
    // Reading none of them answers the whole interval by dimension.
    let whole = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.Обороты(, , Auto, ) КАК О;",
    );
    assert_contains(&whole.sql, "GROUP BY \"__aggregate_used\".\"_fld54\")");
    assert!(
        !whole.sql.contains("\"__aggregate_used\".\"_Period"),
        "{}",
        whole.sql
    );
}

#[test]
fn auto_balances_run_over_the_grain_the_statement_reads() {
    let snapshot = accumulation_register_snapshot();
    let whole = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоКонечныйОстаток КАК Кол
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Авто, ДвиженияИГраницыПериода, ) КАК О;",
    );
    assert_contains(&whole.sql, "\"_fld55ClosingBalance\"");
    assert!(!whole.sql.contains(" OVER ("), "{}", whole.sql);
    // Reading the recorder with a balance takes the record grain: the
    // balances are running sums over the buckets, the movements before
    // the interval forming the first bucket, dropped after the window.
    let by_recorder = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Регистратор КАК Р, О.КоличествоНачальныйОстаток КАК НО, О.КоличествоКонечныйОстаток КАК КО
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(ДАТАВРЕМЯ(2024, 2, 1), ДАТАВРЕМЯ(2024, 3, 1), Авто, , ) КАК О;",
    );
    assert_contains(
        &by_recorder.sql,
        "COALESCE(SUM(SUM(CASE WHEN \"__aggregate_base\".\"_recordkind\" = 0 THEN \"__aggregate_base\".\"_fld55\" ELSE -\"__aggregate_base\".\"_fld55\" END)) OVER (PARTITION BY \"__aggregate_base\".\"_fld54\" ORDER BY CASE WHEN",
    );
    assert_contains(
        &by_recorder.sql,
        "ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING), 0) AS \"_fld55OpeningBalance\"",
    );
    assert_contains(
        &by_recorder.sql,
        "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS \"_fld55ClosingBalance\"",
    );
    assert_contains(
        &by_recorder.sql,
        "AS \"__periods\" WHERE \"__periods\".\"__inside\" = 1)",
    );
    assert_contains(&by_recorder.sql, "\"__aggregate_used\".\"_recorderrref\"");
    // A calendar level takes that level's buckets.
    let monthly = postgres(
        &snapshot,
        "ВЫБРАТЬ О.ПериодМесяц КАК М, О.КоличествоКонечныйОстаток КАК КО
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Авто, , ) КАК О;",
    );
    assert_contains(
        &monthly.sql,
        "date_trunc('month', \"__aggregate_base\".\"_period\") AS \"_period\"",
    );
    assert_contains(&monthly.sql, "AS \"_PeriodMonth\"");
    assert!(!monthly.sql.contains("_PeriodDay"), "{}", monthly.sql);
    // Turnovers alone keep the relation without windows.
    let turnover = postgres(
        &snapshot,
        "ВЫБРАТЬ О.ПериодДень КАК Д, О.КоличествоОборот КАК Кол
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , Авто, , ) КАК О;",
    );
    assert!(!turnover.sql.contains(" OVER ("), "{}", turnover.sql);
    assert_contains(&turnover.sql, "\"__aggregate_used\".\"_PeriodDay\"");
}

#[test]
fn periodic_balances_are_refused_on_sql_server_2008() {
    let snapshot = accumulation_register_snapshot();
    let source = "ВЫБРАТЬ О.Период КАК П, О.КоличествоКонечныйОстаток КАК КО
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , МЕСЯЦ, , ) КАК О;";
    let modern = QueryCompiler::new(&snapshot, MsSqlBackend::new(2000).unwrap())
        .compile(source)
        .unwrap();
    assert_contains(
        &modern.sql,
        "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW",
    );
    let legacy = MsSqlBackend::new(2000)
        .unwrap()
        .with_dialect_level(open_sdbl::query::MsSqlDialectLevel::Sql2008);
    let error = QueryCompiler::new(&snapshot, legacy)
        .compile(source)
        .unwrap_err();
    assert!(
        error.message().contains("no balance column"),
        "{}",
        error.message()
    );
}

#[test]
fn completion_method_without_a_periodicity_is_accepted() {
    let snapshot = accumulation_register_snapshot();
    let whole = postgres(
        &snapshot,
        "ВЫБРАТЬ О.КоличествоОборот КАК Обор
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(, , , ДвиженияИГраницыПериода, ) КАК О;",
    );
    assert!(!whole.sql.contains("OVER ("), "{}", whole.sql);
}

#[test]
fn balance_and_turnovers_split_by_recorder_and_record() {
    let snapshot = accumulation_register_snapshot();
    let by_recorder = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.Регистратор КАК Р, О.КоличествоНачальныйОстаток КАК Нач, О.КоличествоОборот КАК Об
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(ДАТАВРЕМЯ(2024, 2, 1), ДАТАВРЕМЯ(2024, 5, 1), Регистратор, ) КАК О;",
    );
    // The bucket is the record period and the recorder; the movements
    // before the interval form the first bucket, dropped after the window.
    assert_contains(
        &by_recorder.sql,
        "THEN NULL ELSE \"__aggregate_base\".\"_recorderrref\" END AS \"_recorderrref\"",
    );
    assert_contains(
        &by_recorder.sql,
        ") OVER (PARTITION BY \"__aggregate_base\".\"_fld54\" ORDER BY",
    );
    assert!(!by_recorder.sql.contains("_lineno"), "{}", by_recorder.sql);
    let by_record = postgres(
        &snapshot,
        "ВЫБРАТЬ О.Регистратор КАК Р, О.НомерСтроки КАК Н, О.КоличествоКонечныйОстаток КАК Кон
         ИЗ РегистрНакопления.Остатки.ОстаткиИОбороты(ДАТАВРЕМЯ(2024, 2, 1), ДАТАВРЕМЯ(2024, 5, 1), Запись, ) КАК О;",
    );
    assert_contains(
        &by_record.sql,
        "\"__aggregate_base\".\"_lineno\" END AS \"_lineno\"",
    );
}
