//! The scalar string and arithmetic library of the query language.

mod support;

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

fn mssql(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    compile(snapshot, MsSqlBackend::new(0).unwrap(), source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const CATALOG: &str = "Справочник.OpenSdblMetadataProbe";

#[test]
fn renders_string_functions_on_both_dialects() {
    let snapshot = support::snapshot();
    let query = format!(
        "ВЫБРАТЬ ПОДСТРОКА(Код, 2, 3) КАК А1, Лев(Код, 3) КАК А2, Прав(Код, 3) КАК А3,
         СокрЛП(Код) КАК А4, СокрЛ(Код) КАК А5, СокрП(Код) КАК А6, ВРег(Код) КАК А7,
         НРег(Код) КАК А8, СтрЗаменить(Код, \"а\", \"б\") КАК А9 ИЗ {CATALOG};"
    );
    let compiled = postgres(&snapshot, &query);
    // The 1C character types have no text operators, so the column is cast.
    let code = "\"__src\".\"_code\"::text";
    for needle in [
        &format!("substring({code} from 2 for 3)"),
        &format!("substring({code} from 1 for 3)"),
        // `right` arrived in PostgreSQL 9.1, which the generated SQL predates.
        &format!("substring({code} from greatest(length({code}) - (3) + 1, 1))"),
        &format!("btrim({code})"),
        &format!("ltrim({code})"),
        &format!("rtrim({code})"),
        &format!("upper({code})"),
        &format!("lower({code})"),
        &format!("replace({code}, 'а', 'б')"),
    ] {
        assert_contains(&compiled.sql, needle);
    }
    for column in &compiled.columns {
        assert_eq!(column.kind, ColumnKind::String { length: None });
    }

    let mssql = mssql(&snapshot, &query);
    for needle in [
        "SUBSTRING([__src].[_code], 2, 3)",
        "LEFT([__src].[_code], 3)",
        "RIGHT([__src].[_code], 3)",
        "LTRIM(RTRIM([__src].[_code]))",
        "REPLACE([__src].[_code], N'а', N'б')",
    ] {
        assert_contains(&mssql.sql, needle);
    }
}

#[test]
fn counts_characters_and_finds_substrings() {
    let snapshot = support::snapshot();
    let query =
        format!("ВЫБРАТЬ ДлинаСтроки(Код) КАК А1, СтрНайти(Код, \"ан\") КАК А2 ИЗ {CATALOG};");
    let compiled = postgres(&snapshot, &query);
    assert_contains(&compiled.sql, "length(\"__src\".\"_code\"::text)");
    assert_contains(&compiled.sql, "position('ан' in \"__src\".\"_code\"::text)");
    assert_eq!(
        compiled.columns[0].kind,
        ColumnKind::Number {
            precision: None,
            scale: None
        }
    );

    // `LEN` ignores trailing spaces, which the platform counts, so the
    // length is measured with a sentinel.
    let mssql = mssql(&snapshot, &query);
    assert_contains(&mssql.sql, "(LEN([__src].[_code] + N'.') - 1)");
    assert_contains(&mssql.sql, "CHARINDEX(N'ан', [__src].[_code])");
}

#[test]
fn renders_arithmetic_functions() {
    let snapshot = support::snapshot();
    let query = "ВЫБРАТЬ Окр(1.5, 2) КАК А1, Окр(1.5) КАК А2, Цел(-7.9) КАК А3, Sqrt(2) КАК А4,
         Exp(1) КАК А5, Log(10) КАК А6, Log10(100) КАК А7, Pow(2, 10) КАК А8, Cos(0) КАК А9,
         ATan(1) КАК А10;";
    let compiled = postgres(&snapshot, query);
    for needle in [
        "round((1.5)::numeric, 2)",
        "round((1.5)::numeric, 0)",
        "trunc(((-7.9))::numeric)",
        "sqrt(2)",
        "exp(1)",
        // The platform's LOG is the natural logarithm.
        "ln(10)",
        "log(100)",
        "power(2, 10)",
        "cos((0)::double precision)",
        "atan((1)::double precision)",
    ] {
        assert_contains(&compiled.sql, needle);
    }

    let mssql = mssql(&snapshot, query);
    for needle in [
        "ROUND(1.5, 2)",
        "ROUND((-7.9), 0, 1)",
        "SQRT(2)",
        "LOG(10)",
        "LOG10(100)",
        "POWER(2, 10)",
        "ATAN(1)",
    ] {
        assert_contains(&mssql.sql, needle);
    }
}

#[test]
fn checks_argument_kinds_and_arity() {
    let snapshot = support::snapshot();
    for (query, kind, message) in [
        (
            format!("ВЫБРАТЬ ВРег(Дата) КАК А1 ИЗ {CATALOG};"),
            QueryDiagnosticKind::Syntax,
            "UPPER argument 1 must be a string",
        ),
        (
            format!("ВЫБРАТЬ Sqrt(Код) КАК А1 ИЗ {CATALOG};"),
            QueryDiagnosticKind::Syntax,
            "SQRT argument 1 must be a number",
        ),
        (
            format!("ВЫБРАТЬ ПОДСТРОКА(Код, 1) КАК А1 ИЗ {CATALOG};"),
            QueryDiagnosticKind::Syntax,
            "SUBSTRING takes 3 arguments, found 2",
        ),
        (
            format!("ВЫБРАТЬ Окр(1, 2, 3) КАК А1 ИЗ {CATALOG};"),
            QueryDiagnosticKind::Syntax,
            "ROUND takes 1 or 2 arguments, found 3",
        ),
    ] {
        let error = compile(&snapshot, PostgresBackend, &query).unwrap_err();
        assert_eq!(error.kind(), kind, "{query}: {error}");
        assert!(error.message().contains(message), "{query}: {error}");
    }
}

#[test]
fn keeps_the_names_usable_as_identifiers() {
    let snapshot = support::snapshot();
    // The names stay contextual identifiers, and a join still parses.
    let compiled = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Код КАК Окр, Дата КАК Лог ИЗ {CATALOG} УПОРЯДОЧИТЬ ПО Окр;"),
    );
    assert_eq!(
        compiled
            .columns
            .iter()
            .map(|column| column.label.as_str())
            .collect::<Vec<_>>(),
        ["Окр", "Лог"]
    );

    let joined = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ a.Код КАК Код ИЗ {CATALOG} КАК a
             ЛЕВОЕ СОЕДИНЕНИЕ {CATALOG} КАК b ПО b.Ссылка = a.Ссылка;"
        ),
    );
    assert_contains(&joined.sql, "LEFT JOIN");
}
