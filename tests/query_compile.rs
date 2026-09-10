mod support;

use support::*;

use open_sdbl::metadata::{
    ColumnType, ConfigFieldPurpose, FieldId, LiveColumn, LiveTable, LookupError, MetadataKind,
    MetadataSnapshot, ResolutionFinding, SchemaAnomaly, SchemaColumn, StandardFieldId,
    parse_config_descriptors, parse_db_names, resolve_metadata,
};
use open_sdbl::query::{
    Backend, ColumnKind, MsSqlBackend, MsSqlDialectLevel, PostgresBackend, Prepared,
    PresentationExpression, PresentationPlan, QueryCompiler, QueryDiagnosticKind,
    find_metadata_object, queryable_field_catalog, queryable_fields,
};

fn labels(compiled: &open_sdbl::query::CompiledQuery) -> Vec<&str> {
    compiled
        .columns
        .iter()
        .map(|column| column.label.as_str())
        .collect()
}

fn kinds(compiled: &open_sdbl::query::CompiledQuery) -> Vec<&ColumnKind> {
    compiled.columns.iter().map(|column| &column.kind).collect()
}

fn compile_backend_generic<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
) -> Result<open_sdbl::query::CompiledQuery, open_sdbl::query::QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend).compile(source)
}

fn mssql_backend(year_offset: i32) -> MsSqlBackend {
    MsSqlBackend::new(year_offset).expect("test MSSQL year offset must be valid")
}

fn mssql_backend_at(level: MsSqlDialectLevel, year_offset: i32) -> MsSqlBackend {
    mssql_backend(year_offset).with_dialect_level(level)
}

fn assert_backend_outcomes_match(
    source: &str,
    postgres: &Result<open_sdbl::query::CompiledQuery, open_sdbl::query::QueryDiagnostic>,
    mssql: &Result<open_sdbl::query::CompiledQuery, open_sdbl::query::QueryDiagnostic>,
) {
    match (postgres, mssql) {
        (Ok(postgres), Ok(mssql)) => {
            assert_eq!(
                postgres.columns.len(),
                mssql.columns.len(),
                "backend projection widths differ for {source}"
            );
            assert_eq!(
                postgres.deferred_presentations, mssql.deferred_presentations,
                "backend deferred presentations differ for {source}"
            );
            assert_eq!(
                kinds(postgres),
                kinds(mssql),
                "backend column kinds differ for {source}"
            );
        }
        (Err(postgres), Err(mssql)) => {
            assert_eq!(
                postgres.kind(),
                mssql.kind(),
                "backend diagnostics differ for {source}"
            );
            assert_eq!(
                (postgres.offset(), postgres.line(), postgres.column()),
                (mssql.offset(), mssql.line(), mssql.column()),
                "backend diagnostic positions differ for {source}"
            );
        }
        (postgres, mssql) => {
            panic!("backend outcomes differ for {source}: PostgreSQL={postgres:?}, MSSQL={mssql:?}")
        }
    }
}

fn assert_error_outcomes_match<Left, Right>(
    operation: &str,
    postgres: &Result<Left, open_sdbl::query::QueryDiagnostic>,
    mssql: &Result<Right, open_sdbl::query::QueryDiagnostic>,
) {
    match (postgres, mssql) {
        (Ok(_), Ok(_)) => {}
        (Err(postgres), Err(mssql)) => {
            assert_eq!(
                postgres.kind(),
                mssql.kind(),
                "backend diagnostics differ for {operation}"
            );
            assert_eq!(
                (postgres.offset(), postgres.line(), postgres.column()),
                (mssql.offset(), mssql.line(), mssql.column()),
                "backend diagnostic positions differ for {operation}"
            );
        }
        (postgres, mssql) => panic!(
            "backend outcomes differ for {operation}: PostgreSQL success={}, MSSQL success={}",
            postgres.is_ok(),
            mssql.is_ok()
        ),
    }
}

#[derive(Clone, Copy)]
enum SelectedBackend {
    Postgres,
    MsSql,
}

struct ParityPrepared {
    postgres: Prepared<PostgresBackend>,
    mssql: Prepared<MsSqlBackend>,
    selected: SelectedBackend,
}

impl ParityPrepared {
    fn presentation_request(&self) -> &open_sdbl::query::PresentationRequest {
        self.postgres.presentation_request()
    }

    fn compile(
        &self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
    ) -> Result<open_sdbl::query::CompiledQuery, open_sdbl::query::QueryDiagnostic> {
        let postgres = self.postgres.compile(snapshot, plans);
        let mssql = self.mssql.compile(snapshot, plans);
        assert_backend_outcomes_match("prepared query", &postgres, &mssql);
        match self.selected {
            SelectedBackend::Postgres => postgres,
            SelectedBackend::MsSql => mssql,
        }
    }
}

fn select_prepared_backend(
    postgres: Result<Prepared<PostgresBackend>, open_sdbl::query::QueryDiagnostic>,
    mssql: Result<Prepared<MsSqlBackend>, open_sdbl::query::QueryDiagnostic>,
    selected: SelectedBackend,
) -> Result<ParityPrepared, open_sdbl::query::QueryDiagnostic> {
    match (postgres, mssql) {
        (Ok(postgres), Ok(mssql)) => Ok(ParityPrepared {
            postgres,
            mssql,
            selected,
        }),
        (Err(postgres), Err(mssql)) => match selected {
            SelectedBackend::Postgres => Err(postgres),
            SelectedBackend::MsSql => Err(mssql),
        },
        _ => unreachable!("backend preparation parity was checked before selection"),
    }
}

macro_rules! for_each_backend {
    ($source:expr, $snapshot:expr $(,)?) => {{
        let source = $source;
        let snapshot = $snapshot;
        let postgres = compile_backend_generic(snapshot, PostgresBackend, source);
        let mssql = compile_backend_generic(snapshot, mssql_backend(0), source);
        assert_backend_outcomes_match(source, &postgres, &mssql);
        (postgres, mssql)
    }};
    (prepare $source:expr, $snapshot:expr $(,)?) => {{
        let source = $source;
        let snapshot = $snapshot;
        let postgres = QueryCompiler::new(snapshot, PostgresBackend).prepare(source);
        let mssql = QueryCompiler::new(snapshot, mssql_backend(0)).prepare(source);
        assert_error_outcomes_match(source, &postgres, &mssql);
        if let (Ok(postgres), Ok(mssql)) = (&postgres, &mssql) {
            assert_eq!(
                postgres.presentation_request(),
                mssql.presentation_request(),
                "backend presentation requests differ for {source}"
            );
        }
        (postgres, mssql)
    }};
    (presentation $snapshot:expr, $plan:expr, $references:expr, $year_offset:expr $(,)?) => {{
        let snapshot = $snapshot;
        let plan = $plan;
        let references = $references;
        let postgres = QueryCompiler::new(snapshot, PostgresBackend)
            .compile_presentation_lookup(plan, references);
        let mssql = QueryCompiler::new(snapshot, mssql_backend($year_offset))
            .compile_presentation_lookup(plan, references);
        assert_backend_outcomes_match("presentation lookup", &postgres, &mssql);
        (postgres, mssql)
    }};
}

macro_rules! postgres_compile {
    ($source:expr, $snapshot:expr $(,)?) => {
        for_each_backend!($source, $snapshot).0
    };
}

macro_rules! postgres_prepare {
    ($source:expr, $snapshot:expr $(,)?) => {{
        let (postgres, mssql) = for_each_backend!(prepare $source, $snapshot);
        select_prepared_backend(postgres, mssql, SelectedBackend::Postgres)
    }};
}

macro_rules! postgres_presentation_lookup {
    ($snapshot:expr, $plan:expr, $references:expr $(,)?) => {
        for_each_backend!(presentation $snapshot, $plan, $references, 0).0
    };
}

macro_rules! mssql_compile {
    ($source:expr, $snapshot:expr $(,)?) => {
        for_each_backend!($source, $snapshot).1
    };
}

macro_rules! mssql_compile_with_offset {
    ($source:expr, $snapshot:expr, $year_offset:expr $(,)?) => {
        QueryCompiler::new($snapshot, mssql_backend($year_offset)).compile($source)
    };
}

macro_rules! mssql_compile_with_level {
    ($source:expr, $snapshot:expr, $level:expr, $year_offset:expr $(,)?) => {
        QueryCompiler::new($snapshot, mssql_backend_at($level, $year_offset)).compile($source)
    };
}

macro_rules! mssql_prepare {
    ($source:expr, $snapshot:expr $(,)?) => {{
        let (postgres, mssql) = for_each_backend!(prepare $source, $snapshot);
        select_prepared_backend(postgres, mssql, SelectedBackend::MsSql)
    }};
}

macro_rules! mssql_presentation_lookup {
    ($snapshot:expr, $plan:expr, $references:expr, $year_offset:expr $(,)?) => {
        for_each_backend!(presentation $snapshot, $plan, $references, $year_offset).1
    };
}

#[test]
fn preserves_mssql_goldens_for_dialect_sensitive_features() {
    let cases = [
        (
            "slices",
            mssql_compile!(
                "SELECT l.Period, r.Period FROM InformationRegister.Prices.SliceFirst() l INNER JOIN InformationRegister.Prices.SliceLast() r ON l.ProbeAttribute = r.ProbeAttribute;",
                &information_register_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT [l].[_period] AS [Period], [r].[_period] AS [Period_2] FROM (SELECT [__slice_ranked].* FROM (SELECT [__slice_base].*, DENSE_RANK() OVER (PARTITION BY [__slice_base].[_fld54] ORDER BY [__slice_base].[_period] ASC) AS [__open_sdbl_slice_rank] FROM [_inforg53] AS [__slice_base]) AS [__slice_ranked] WHERE [__slice_ranked].[__open_sdbl_slice_rank] = 1) AS [l] INNER JOIN (SELECT [__slice_ranked].* FROM (SELECT [__slice_base].*, DENSE_RANK() OVER (PARTITION BY [__slice_base].[_fld54] ORDER BY [__slice_base].[_period] DESC) AS [__open_sdbl_slice_rank] FROM [_inforg53] AS [__slice_base]) AS [__slice_ranked] WHERE [__slice_ranked].[__open_sdbl_slice_rank] = 1) AS [r] ON [l].[_fld54] = [r].[_fld54]",
        ),
        (
            "turnovers",
            mssql_compile!(
                "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(\"2026-08-01\", \"2026-09-01\",, Номенклатура IS NOT NULL);",
                &accumulation_register_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT [__src].[_fld54] AS [Номенклатура], [__src].[_fld55] AS [КоличествоОборот] FROM (SELECT [__aggregate_base].[_fld54] AS [_fld54], SUM(CASE WHEN [__aggregate_base].[_recordkind] = 0 THEN [__aggregate_base].[_fld55] ELSE -[__aggregate_base].[_fld55] END) AS [_fld55] FROM [_accumrg53] AS [__aggregate_base] WHERE [__aggregate_base].[_active] = 0x01 AND ([__aggregate_base].[_period] >= N'2026-08-01') AND ([__aggregate_base].[_period] < N'2026-09-01') AND ([__aggregate_base].[_fld54] IS NOT NULL) GROUP BY [__aggregate_base].[_fld54]) AS [__src]",
        ),
        (
            "union",
            mssql_compile!(
                "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p WHERE p.Code = \"A\" UNION SELECT q.Code FROM Catalog.OpenSdblMetadataProbe q UNION ALL SELECT r.Code FROM Catalog.OpenSdblMetadataProbe r ORDER BY Code DESC;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT [p].[_code] AS [Code] FROM [_reference53] AS [p] WHERE ([p].[_code] = N'A') UNION SELECT [q].[_code] AS [Code] FROM [_reference53] AS [q] UNION ALL SELECT [r].[_code] AS [Code] FROM [_reference53] AS [r] ORDER BY 1 DESC",
        ),
        (
            "full_join",
            mssql_compile!(
                "SELECT l.Code, r.Date FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT * FROM (SELECT [l].[_code] AS [Code], [r].[_date_time] AS [Date] FROM [_reference53] AS [l] LEFT JOIN [_reference53] AS [r] ON [l].[_code] = [r].[_code] UNION ALL SELECT [l].[_code] AS [Code], [r].[_date_time] AS [Date] FROM [_reference53] AS [r] LEFT JOIN [_reference53] AS [l] ON [l].[_code] = [r].[_code] WHERE ([l].[_code] IS NULL)) AS [__full]",
        ),
        (
            "dereference",
            mssql_compile!(
                "SELECT Организация.Код FROM Catalog.OpenSdblMetadataProbe p;",
                &reference_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT [__ref1].[_code] AS [Организация.Код] FROM [_reference53] AS [p] LEFT JOIN [_reference57] AS [__ref1] ON [p].[_fld54] = [__ref1].[_idrref]",
        ),
        (
            "tabular",
            mssql_compile!(
                "SELECT Ссылка, НомерСтроки, Сумма FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
                &tabular_section_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT [__src].[_document53_idrref] AS [ID], [__src].[_lineno54] AS [LineNo], [__src].[_fld57] AS [Сумма] FROM [_document53_vt54X1] AS [__src]",
        ),
        (
            "aggregate",
            mssql_compile!(
                "SELECT COUNT(*) AS RowCount, SUM(ProbeAttribute) AS Total FROM Catalog.OpenSdblMetadataProbe;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT COUNT(*) AS [RowCount], SUM([__src].[_fld54]) AS [Total] FROM [_reference53] AS [__src]",
        ),
        (
            "top_in",
            mssql_compile!(
                "SELECT TOP 3 Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (\"A\", \"B\");",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT TOP (3) [__src].[_code] AS [Code] FROM [_reference53] AS [__src] WHERE ([__src].[_code] IN (N'A', N'B'))",
        ),
        (
            "value",
            mssql_compile!(
                "SELECT VALUE(Catalog.OpenSdblMetadataProbe.Утвержден);",
                &catalog_value_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT (SELECT [__open_sdbl_value].[_idrref] FROM [_reference53] AS [__open_sdbl_value] WHERE ([__open_sdbl_value].[_predefinedid] = 0xa3dae56fa2f94623445632b52e22ad88)) AS [column1]",
        ),
    ];
    for (name, actual, expected) in cases {
        assert_eq!(actual, expected, "MSSQL golden changed for {name}");
    }
}

#[test]
fn validates_mssql_year_offsets_at_backend_construction() {
    assert_eq!(MsSqlBackend::default().year_offset(), 0);
    assert_eq!(mssql_backend(10_000).year_offset(), 10_000);
    for year_offset in [-1, 10_001, i32::MIN, i32::MAX] {
        let error = MsSqlBackend::new(year_offset).unwrap_err();
        assert_eq!(error.year_offset(), year_offset);
    }
}

#[test]
fn compiles_through_one_backend_generic_api() {
    let snapshot = snapshot();
    let source = "SELECT TOP 1 Code FROM Catalog.OpenSdblMetadataProbe;";
    let (postgres, mssql) = for_each_backend!(source, &snapshot);
    let postgres = postgres.unwrap();
    let mssql = mssql.unwrap();

    assert!(postgres.sql.contains(" LIMIT 1"));
    assert!(mssql.sql.starts_with("SELECT TOP (1)"));
}

#[test]
fn rejects_pathologically_deep_query_expressions() {
    let snapshot = snapshot();
    let parentheses = format!("SELECT {}1{};", "(".repeat(5_000), ")".repeat(5_000));
    let error = postgres_compile!(&parentheses, &snapshot).unwrap_err();
    assert!(
        error
            .message()
            .contains("nesting depth exceeds limit of 128")
    );
    assert!(error.column() > 1);

    let unary = format!("SELECT {}1;", "-".repeat(5_000));
    let error = postgres_compile!(&unary, &snapshot).unwrap_err();
    assert!(
        error
            .message()
            .contains("nesting depth exceeds limit of 128")
    );
    assert!(error.column() > 1);

    let arithmetic = format!("SELECT {};", vec!["1"; 300_000].join("+"));
    let error = postgres_compile!(&arithmetic, &snapshot).unwrap_err();
    assert!(error.message().contains("limit of 4096 binary operators"));
    assert!(error.column() > 1);

    let predicate = vec!["Code = \"A\""; 5_000].join(" OR ");
    let logical = format!("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE {predicate};");
    let error = postgres_compile!(&logical, &snapshot).unwrap_err();
    assert!(error.message().contains("limit of 4096 binary operators"));
    assert!(error.column() > 1);

    let functions = format!(
        "SELECT {}DATETIME(2026, 1, 1){};",
        "BEGINOFPERIOD(".repeat(5_000),
        ", MONTH)".repeat(5_000)
    );
    let error = postgres_compile!(&functions, &snapshot).unwrap_err();
    assert!(
        error
            .message()
            .contains("nesting depth exceeds limit of 128")
    );
    assert!(error.column() > 1);
}

#[test]
fn compiles_the_binary_operator_budget_boundary_without_recursion() {
    let snapshot = snapshot();
    let arithmetic = format!("SELECT {};", vec!["1"; 4_097].join("+"));
    let compiled = postgres_compile!(&arithmetic, &snapshot).unwrap();
    assert_eq!(compiled.sql.matches(" + ").count(), 4_096);

    let predicate = vec!["Code = \"A\""; 2_048].join(" OR ");
    let logical =
        format!("SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE {predicate} AND TRUE;");
    let compiled = postgres_compile!(&logical, &snapshot).unwrap();
    assert_eq!(compiled.sql.matches(" OR ").count(), 2_047);
    assert_eq!(compiled.sql.matches(" AND ").count(), 1);
}

#[test]
fn compiles_native_mssql_projection_filter_and_limit() {
    let snapshot = mssql_snapshot();
    let compiled = mssql_compile!(
        "SELECT TOP 10 Code, ProbeAttribute FROM Catalog.OpenSdblMetadataProbe WHERE Code = \"\u{420}\u{430}\u{437}\u{43e}\u{432}\u{44b}\u{439}\";",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Code", "ProbeAttribute"]);
    assert_eq!(
        compiled.sql,
        "SELECT TOP (10) [__src].[_code] AS [Code], [__src].[_fld54] AS [ProbeAttribute] FROM [_reference53] AS [__src] WHERE ([__src].[_code] = N'\u{420}\u{430}\u{437}\u{43e}\u{432}\u{44b}\u{439}')"
    );
    assert!(!compiled.sql.contains("::"));
    assert!(!compiled.sql.contains(" LIMIT "));
    assert!(!compiled.sql.contains('"'));
}

#[test]
fn resolves_mssql_schema_table_names_with_the_shared_case_rule() {
    let snapshot = with_schema(mssql_snapshot(), |schema| {
        schema.tables[0].name = "rEfErEnCe53".to_owned();
    });

    let compiled =
        mssql_compile!("SELECT Code FROM Catalog.OpenSdblMetadataProbe;", &snapshot,).unwrap();
    assert!(compiled.sql.contains("FROM [_reference53] AS [__src]"));
    assert!(!compiled.sql.contains('"'));
}

#[test]
fn preserves_native_mssql_rowversion_projection() {
    for data_type in ["timestamp", "rowversion"] {
        let snapshot = with_live_tables(mssql_snapshot(), |tables| {
            tables[0].columns.push(LiveColumn {
                name: "_version".to_owned(),
                data_type: data_type.to_owned(),
            });
        });

        let compiled = mssql_compile!(
            "SELECT Version FROM Catalog.OpenSdblMetadataProbe;",
            &snapshot,
        )
        .unwrap();

        assert_eq!(labels(&compiled), ["Version"]);
        assert_eq!(
            compiled.sql,
            "SELECT [__src].[_version] AS [Version] FROM [_reference53] AS [__src]"
        );
    }
}

#[test]
fn compiles_binary_literals_for_each_sql_dialect() {
    let mssql = with_live_tables(mssql_snapshot(), |tables| {
        tables[0].columns.push(LiveColumn {
            name: "_version".to_owned(),
            data_type: "timestamp".to_owned(),
        });
    });
    let compiled = mssql_compile!(
        "SELECT Version FROM Catalog.OpenSdblMetadataProbe WHERE Version > 0x00000000000007D6;",
        &mssql,
    )
    .unwrap();
    assert!(
        compiled
            .sql
            .contains("([__src].[_version] > 0x00000000000007D6)")
    );

    let compiled = postgres_compile!(
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
    let postgres = postgres_compile!(
        "ВЫБРАТЬ ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус);",
        &snapshot,
    )
    .unwrap();
    assert!(
        postgres
            .sql
            .contains("decode('9022249e3a1ac4b94be8faddd2f8bde9', 'hex')")
    );

    let mssql = mssql_compile!(
        "SELECT VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус);",
        &snapshot,
    )
    .unwrap();
    assert!(mssql.sql.contains("0x9022249e3a1ac4b94be8faddd2f8bde9"));
}

#[test]
fn compiles_catalog_value_as_a_predefined_id_lookup() {
    let snapshot = catalog_value_snapshot();
    let postgres = postgres_compile!(
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

    let mssql = mssql_compile!(
        "SELECT VALUE(Catalog.OpenSdblMetadataProbe.ДополнительныеУсловияПоДоговору_Проверен);",
        &snapshot,
    )
    .unwrap();
    assert!(mssql.sql.contains("0xa161ed47a2787c5a437832a3f6fa6a92"));
    assert!(mssql.sql.contains("[_predefinedid]"));
}

#[test]
fn compiles_in_list_with_several_catalog_values() {
    let snapshot = catalog_value_snapshot();
    let query = "ВЫБРАТЬ Код ИЗ Справочник.OpenSdblMetadataProbe
        ГДЕ Ссылка В (
            ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.Утвержден),
            ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.ДополнительныеУсловияПоДоговору_Проверен)
        );";

    let postgres = postgres_compile!(query, &snapshot).unwrap();
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

    let mssql = mssql_compile!(query, &snapshot).unwrap();
    assert!(mssql.sql.contains("[__src].[_idrref] IN ("));
    assert!(mssql.sql.contains("0xa3dae56fa2f94623445632b52e22ad88"));
    assert!(mssql.sql.contains("0xa161ed47a2787c5a437832a3f6fa6a92"));
}

#[test]
fn compiles_in_lists_in_source_free_and_joined_queries() {
    let snapshot = snapshot();
    let source_free = postgres_compile!("SELECT 2 IN (1, 2);", &snapshot).unwrap();
    assert!(source_free.sql.contains("(2 IN (1, 2))"));

    let joined = mssql_compile!(
        "SELECT l.Code FROM Catalog.OpenSdblMetadataProbe l
         INNER JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code
         WHERE l.Code IN (\"Первый\", \"Второй\");",
        &mssql_snapshot(),
    )
    .unwrap();
    assert!(
        joined
            .sql
            .contains("([l].[_code] IN (N'Первый', N'Второй'))")
    );
}

#[test]
fn diagnoses_empty_and_malformed_in_lists() {
    let snapshot = snapshot();
    let empty = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN ();",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        empty
            .message()
            .contains("IN list must contain at least one expression")
    );

    let trailing = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code В (\"A\",);",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        trailing
            .message()
            .contains("expected expression after ',' in IN list")
    );

    let unclosed = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (\"A\";",
        &snapshot,
    )
    .unwrap_err();
    assert!(unclosed.message().contains("expected \")\""));
}

#[test]
fn diagnoses_invalid_value_kinds_paths_and_names() {
    let snapshot = catalog_value_snapshot();
    let error = postgres_compile!(
        "SELECT VALUE(Document.OpenSdblMetadataProbe.Утвержден);",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("only catalogs and enumerations"));

    let error = postgres_compile!(
        "SELECT VALUE(Catalog.OpenSdblMetadataProbe.Absent);",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("metadata value was not found"));

    let error = postgres_compile!("SELECT VALUE(Catalog.OnlyTwo);", &snapshot).unwrap_err();
    assert!(error.message().contains("expected \".\""));
}

#[test]
fn compiles_mssql_extension_tables_as_one_source_relation() {
    let snapshot = with_live_tables(mssql_snapshot(), |tables| {
        let mut extension = tables[0].clone();
        extension.name = "_reference53X1".to_owned();
        extension
            .columns
            .retain(|column| column.name != "_date_time");
        extension.columns.push(LiveColumn {
            name: "_extension_only".to_owned(),
            data_type: "nvarchar(10)".to_owned(),
        });
        tables.push(extension);
        let mut unrelated = tables[0].clone();
        unrelated.name = "_reference53Xother".to_owned();
        tables.push(unrelated);
    });

    let compiled = mssql_compile!(
        "SELECT Code, Date FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.contains("FROM (SELECT"));
    assert!(
        compiled
            .sql
            .contains("FROM [_reference53] UNION ALL SELECT")
    );
    assert!(compiled.sql.contains("NULL AS [_date_time]"));
    assert!(compiled.sql.contains("FROM [_reference53X1]"));
    assert!(!compiled.sql.contains("_extension_only"));
    assert!(!compiled.sql.contains("_reference53Xother"));
}

#[test]
fn compiles_mssql_presentation_from_a_source_with_extension_tables() {
    let snapshot = with_live_tables(reference_snapshot(), |tables| {
        let mut extension = tables[0].clone();
        extension.name = "_reference53X1".to_owned();
        tables.push(extension);
    });
    let prepared = mssql_prepare!(
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

    assert!(compiled.sql.contains("LEFT JOIN [_reference57]"));
    assert!(
        compiled
            .sql
            .contains("FROM [_reference53] UNION ALL SELECT")
    );
    assert!(compiled.sql.contains("FROM [_reference53X1]"));
    assert!(compiled.sql.contains("AS [__ref1] ON"));
}

#[test]
fn compiles_mssql_historical_balance_without_postgres_aggregate_syntax() {
    let snapshot = with_live_tables(accumulation_register_snapshot(), |tables| {
        for table in tables {
            for column in &mut table.columns {
                if column.name == "_active" {
                    column.data_type = "binary(1)".to_owned();
                }
            }
        }
    });
    let compiled = mssql_compile_with_offset!(
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
    assert!(!compiled.sql.contains('"'), "{}", compiled.sql);
}

#[test]
fn translates_mssql_year_offset_in_date_projection_and_filter() {
    let snapshot = mssql_snapshot();
    let compiled = mssql_compile_with_offset!(
        "SELECT Date FROM Catalog.OpenSdblMetadataProbe WHERE Date >= \"2026-09-01\";",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(
        compiled
            .sql
            .contains("DATEADD(year, -2000, [__src].[_date_time])")
    );
    assert!(compiled.sql.contains("DATEADD(year, 2000, N'2026-09-01')"));
}

#[test]
fn prepares_and_compiles_mssql_presentations() {
    let snapshot = reference_snapshot();
    let prepared = mssql_prepare!(
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
    assert!(compiled.sql.contains(" IS NULL THEN N'' ELSE "));
    assert!(!compiled.sql.contains("::bytea"));
}

#[test]
fn compiles_source_free_literals_and_scalar_presentations() {
    let snapshot = snapshot();
    let literal = postgres_compile!("SELECT 4;", &snapshot).unwrap();
    assert_eq!(labels(&literal), ["column1"]);
    assert_eq!(literal.sql, "SELECT 4 AS \"column1\"");

    let presentation = postgres_compile!("select представление(4);", &snapshot).unwrap();
    assert_eq!(labels(&presentation), ["представление"]);
    assert_eq!(presentation.sql, "SELECT (4)::text AS \"представление\"");

    let multiline = postgres_compile!("select\nпредставление(4);", &snapshot).unwrap();
    assert_eq!(multiline.sql, presentation.sql);
}

#[test]
fn applies_projection_aliases_and_diagnoses_a_missing_alias() {
    let snapshot = snapshot();
    let field = postgres_compile!(
        "SELECT Code AS ResultCode FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&field), ["ResultCode"]);
    assert!(field.sql.contains("AS \"ResultCode\""));

    let scalar = postgres_compile!("SELECT 2 + 2 КАК Результат;", &snapshot).unwrap();
    assert_eq!(labels(&scalar), ["Результат"]);
    assert_eq!(scalar.sql, "SELECT (2 + 2) AS \"Результат\"");

    let aggregate = postgres_compile!(
        "SELECT COUNT(*) AS RowCount FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&aggregate), ["RowCount"]);
    assert!(aggregate.sql.contains("COUNT(*) AS \"RowCount\""));

    let error = postgres_compile!(
        "SELECT Code AS FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("expected projection alias"));
}

#[test]
fn compiles_datetime_and_begin_of_period_for_postgres() {
    let snapshot = snapshot();
    let source_free = postgres_compile!(
        "SELECT DATETIME(2024, 2, 29, 12, 34, 56) AS Moment,
                BEGINOFPERIOD(DATETIME(2024, 8, 29, 12, 34, 56), MONTH) AS PeriodStart;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&source_free), ["Moment", "PeriodStart"]);
    assert!(
        source_free
            .sql
            .contains("TIMESTAMP '2024-02-29 12:34:56' AS \"Moment\"")
    );
    assert!(
        source_free
            .sql
            .contains("date_trunc('month', TIMESTAMP '2024-08-29 12:34:56') AS \"PeriodStart\"")
    );

    let source_backed = postgres_compile!(
        "ВЫБРАТЬ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ) КАК НачалоМесяца
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ Дата >= ДАТАВРЕМЯ(2026, 9, 2);",
        &snapshot,
    )
    .unwrap();
    assert!(
        source_backed
            .sql
            .contains("date_trunc('month', \"__src\".\"_date_time\") AS \"НачалоМесяца\""),
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
    let compiled = mssql_compile_with_offset!(
        "SELECT BEGINOFPERIOD(Date, MONTH) AS MonthStart
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Date >= DATETIME(2026, 9, 2, 10, 11, 12);",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "DATEADD(year, -2000, DATETIME2FROMPARTS(YEAR([__src].[_date_time]), MONTH([__src].[_date_time]), 1, 0, 0, 0, 0, 0)) AS [MonthStart]"
    ));
    assert!(
        compiled
            .sql
            .contains("DATEADD(year, 2000, CONVERT(datetime2, '2026-09-02T10:11:12', 126))")
    );

    let virtual_table = mssql_compile_with_offset!(
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
    let compiled = postgres_compile!(
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
            .contains("date_trunc('day', \"p\".\"_date_time\") AS \"StartDay\"")
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
        postgres_compile!(&query, &snapshot).unwrap();
        mssql_compile!(&query, &mssql_snapshot()).unwrap();
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
        let error = postgres_compile!(query, &snapshot).unwrap_err();
        assert!(error.message().contains(message), "{error}");
    }

    let error = mssql_compile_with_offset!(
        "SELECT DATETIME(9000, 1, 1) FROM Catalog.OpenSdblMetadataProbe;",
        &mssql_snapshot(),
        2000,
    )
    .unwrap_err();
    assert!(error.message().contains("outside 1..=9999"));

    let error = postgres_compile!(
        "SELECT BEGINOFPERIOD(Code, MONTH) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("must resolve to a date field"));
}

#[test]
fn compiles_source_free_arithmetic_and_rejects_fields_without_from() {
    let snapshot = snapshot();
    let compiled = postgres_compile!("SELECT 2 + 2, \"готово\";", &snapshot).unwrap();
    assert_eq!(labels(&compiled), ["column1", "column2"]);
    assert_eq!(
        compiled.sql,
        "SELECT (2 + 2) AS \"column1\", 'готово' AS \"column2\""
    );

    let field = postgres_compile!("SELECT Код;", &snapshot).unwrap_err();
    assert!(field.message().contains("requires FROM"));
    let wildcard = postgres_compile!("SELECT *;", &snapshot).unwrap_err();
    assert!(wildcard.message().contains("requires FROM"));
}

#[test]
fn compiles_count_all_and_distinct_field() {
    let snapshot = snapshot();
    let all = postgres_compile!(
        "select count(*) from Справочник.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&all), ["count"]);
    assert_eq!(
        all.sql,
        "SELECT COUNT(*) AS \"count\" FROM \"_reference53\" AS \"__src\""
    );

    let distinct = postgres_compile!(
        "ВЫБРАТЬ КОЛИЧЕСТВО(РАЗЛИЧНЫЕ Код) ИЗ Справочник.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&distinct), ["КОЛИЧЕСТВО"]);
    assert!(distinct.sql.contains("COUNT(DISTINCT \"__src\".\"_code\")"));
}

#[test]
fn bounds_count_aggregate_shapes() {
    let snapshot = snapshot();
    let source_free = postgres_compile!("SELECT COUNT(*);", &snapshot).unwrap();
    assert_eq!(source_free.sql, "SELECT COUNT(*) AS \"COUNT\"");

    let mixed = postgres_compile!(
        "SELECT COUNT(*), Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(mixed.message().contains("cannot be mixed"));

    let full = postgres_compile!(
        "SELECT COUNT(*) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(full.message().contains("transposed FULL JOIN"));
}

#[test]
fn compiles_sum_min_max_and_count_distinct_together() {
    let snapshot = snapshot();
    let compiled = postgres_compile!(
        "SELECT SUM(ProbeAttribute), МИНИМУМ(ProbeAttribute), MAX(ProbeAttribute), КОЛИЧЕСТВО(РАЗЛИЧНЫЕ ProbeAttribute) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&compiled), ["SUM", "МИНИМУМ", "MAX", "КОЛИЧЕСТВО"]);
    assert!(compiled.sql.contains("SUM(\"__src\".\"_fld54\")"));
    assert!(compiled.sql.contains("MIN(\"__src\".\"_fld54\")"));
    assert!(compiled.sql.contains("MAX(\"__src\".\"_fld54\")"));
    assert!(
        compiled
            .sql
            .contains("COUNT(DISTINCT \"__src\".\"_fld54\")")
    );
}

#[test]
fn rejects_wildcard_and_distinct_for_non_count_aggregates() {
    let snapshot = snapshot();
    let wildcard = postgres_compile!(
        "SELECT SUM(*) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("only by COUNT"));

    let distinct = postgres_compile!(
        "SELECT MIN(DISTINCT Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(distinct.message().contains("only by COUNT"));
}

#[test]
fn compiles_information_register_slice_last_by_config_dimensions() {
    let snapshot = information_register_snapshot();
    let compiled = postgres_compile!(
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

    let mssql = mssql_compile!(
        "SELECT ProbeAttribute, Period FROM InformationRegister.Prices.SliceLast();",
        &snapshot,
    )
    .unwrap();
    assert!(!mssql.sql.contains('"'), "{}", mssql.sql);
    assert!(mssql.sql.contains("AS [__open_sdbl_slice_rank]"));
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
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert_eq!(
        resolved.fields()[0].purpose,
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
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert_eq!(
        resolved.fields()[0].purpose,
        Some(ConfigFieldPurpose::AccumulationRegisterDimension)
    );
}

#[test]
fn applies_slice_last_parameters_before_and_where_after_ranking() {
    let snapshot = information_register_snapshot();
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
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
    let wrong_kind = postgres_compile!(
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
    let expression = postgres_compile!(
        "SELECT Period FROM InformationRegister.Prices.SliceLast(2 + 2);",
        &register,
    )
    .unwrap_err();
    assert!(expression.message().contains("scalar literal"));

    let parameter = postgres_compile!(
        "SELECT Period FROM InformationRegister.Prices.SliceLast(&Period);",
        &register,
    )
    .unwrap_err();
    assert_eq!(parameter.kind(), QueryDiagnosticKind::Parameter);
    assert!(parameter.message().contains("has no value"));
}

#[test]
fn compiles_information_register_slice_first_by_config_dimensions() {
    let snapshot = information_register_snapshot();
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
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
    let wrong_kind = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe.SliceFirst();",
        &catalog,
    )
    .unwrap_err();
    assert!(
        wrong_kind
            .message()
            .contains("SliceFirst is supported only for information registers")
    );

    let register = with_live_tables(information_register_snapshot(), |tables| {
        tables[0].columns.retain(|column| column.name != "_period");
    });
    let missing_period = postgres_compile!(
        "SELECT ProbeAttribute FROM InformationRegister.Prices.SliceFirst();",
        &register,
    )
    .unwrap_err();
    assert!(missing_period.message().contains("requires a live Period"));

    let register = information_register_snapshot();
    let parameter = postgres_compile!(
        "SELECT Period FROM InformationRegister.Prices.SliceFirst(&Period);",
        &register,
    )
    .unwrap_err();
    assert_eq!(parameter.kind(), QueryDiagnosticKind::Parameter);
    assert!(parameter.message().contains("has no value"));
}

#[test]
fn compiles_current_accumulation_register_balances() {
    let snapshot = accumulation_register_snapshot();
    let compiled = postgres_compile!(
        "SELECT Номенклатура, КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Номенклатура", "КоличествоОстаток"]);
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
    let compiled = postgres_compile!(
        "ВЫБРАТЬ КоличествоОстаток ИЗ РегистрНакопления.Остатки.Остатки(\"2026-09-01\", Номенклатура ЕСТЬ НЕ NULL) ГДЕ КоличествоОстаток > 0;",
        &snapshot,
    )
    .unwrap();

    let anchor = compiled
        .sql
        .find("MAX(CASE WHEN \"__anchor_totals\".\"_period\" <=")
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
    let compiled = postgres_compile!(
        "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(\"2026-08-01\", \"2026-09-01\",, Номенклатура IS NOT NULL);",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Номенклатура", "КоличествоОборот"]);
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
    let compiled = postgres_compile!(
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
    let wrong_kind = postgres_compile!(
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
    let periodicity = postgres_compile!(
        "SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(,,Day,);",
        &register,
    )
    .unwrap_err();
    assert!(
        periodicity
            .message()
            .contains("periodicity is not supported")
    );

    let resource_condition = postgres_compile!(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance(, Количество > 0);",
        &register,
    )
    .unwrap_err();
    assert!(resource_condition.message().contains("was not found"));

    let turnover_only = with_live_tables(accumulation_register_snapshot(), |tables| {
        tables[0]
            .columns
            .retain(|column| column.name != "_recordkind");
    });
    let balance = postgres_compile!(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &turnover_only,
    )
    .unwrap_err();
    assert!(balance.message().contains("turnover-only"));

    let base = accumulation_register_snapshot();
    let entries = base
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.alias != "AccumRgT")
        .map(|entry| format!("{{{},\"{}\",{}}}", entry.guid, entry.alias, entry.number))
        .collect::<Vec<_>>();
    let serialized = format!("{{{},{} }}", entries.len(), entries.join(","));
    let db_names = parse_db_names(&stored_deflate(serialized.as_bytes())).unwrap();
    let missing_mapping = resolve_metadata(
        db_names,
        base.descriptors().to_vec(),
        base.schema().clone(),
        base.live_tables().to_vec(),
    )
    .snapshot;
    let balance = postgres_compile!(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &missing_mapping,
    )
    .unwrap_err();
    assert!(balance.message().contains("AccumRgT entry"));
    let turnovers = postgres_compile!(
        "SELECT КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers();",
        &missing_mapping,
    )
    .unwrap();
    assert!(turnovers.sql.contains("_accumrg53"));
    assert!(!turnovers.sql.contains("_accumrgt56"));

    let missing_live_totals = with_live_tables(accumulation_register_snapshot(), |tables| {
        tables.retain(|table| table.name != "_accumrgt56");
    });
    let balance = postgres_compile!(
        "SELECT КоличествоОстаток FROM AccumulationRegister.Остатки.Balance();",
        &missing_live_totals,
    )
    .unwrap_err();
    assert!(balance.message().contains("is not live"));

    let missing_totals_resource = with_schema(accumulation_register_snapshot(), |schema| {
        schema.tables[1]
            .columns
            .retain(|column| column.name != "Fld55");
    });
    let balance = postgres_compile!(
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
    let prepared = postgres_prepare!(source, &snapshot).unwrap();
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
    let missing = postgres_compile!(source, &snapshot).unwrap_err();
    assert!(missing.message().contains("missing presentation plan"));

    let prepared = postgres_prepare!(source, &snapshot).unwrap();
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
        let prepared = postgres_prepare!(source, &snapshot).unwrap();
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

        let prepared = mssql_prepare!(source, &snapshot).unwrap();
        assert_eq!(
            prepared.presentation_request().targets.len(),
            expected_targets
        );
        let compiled = prepared.compile(&snapshot, &plans).unwrap();
        assert_eq!(compiled.sql.matches("LEFT JOIN").count(), expected_targets);
        if multiple {
            assert!(compiled.sql.contains("0x00000039"));
            assert!(compiled.sql.contains("0x0000003a"));
            assert!(compiled.sql.contains("CASE WHEN"));
        }
    }
}

#[test]
fn preserves_presentations_through_full_join_and_union_branches() {
    let snapshot = snapshot();
    let source = "SELECT REFPRESENTATION(l.Ссылка) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code UNION ALL SELECT REFPRESENTATION(Ссылка) FROM Catalog.OpenSdblMetadataProbe;";
    let prepared = postgres_prepare!(source, &snapshot).unwrap();
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
    let compiled = postgres_compile!(
        "ВЫБРАТЬ ПЕРВЫЕ 5 Код, ProbeAttribute ИЗ Справочник.OpenSdblMetadataProbe КАК p ГДЕ p.Код = \"A\" УПОРЯДОЧИТЬ ПО p.Код ВОЗР;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Code", "ProbeAttribute"]);
    assert_eq!(
        compiled.sql,
        "SELECT \"p\".\"_code\"::text AS \"Code\", \"p\".\"_fld54\" AS \"ProbeAttribute\" FROM \"_reference53\" AS \"p\" WHERE (\"p\".\"_code\" = 'A') ORDER BY \"p\".\"_code\" ASC LIMIT 5"
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
fn queryable_field_catalog_reflects_each_immutable_resolved_snapshot() {
    let snapshot = with_descriptors(snapshot(), |descriptors| {
        descriptors
            .iter_mut()
            .find(|descriptor| descriptor.resource_guid != descriptor.object_guid)
            .unwrap()
            .name = "RenamedAttribute".to_owned();
    });
    let object = &snapshot.objects()[0];
    let object_id = open_sdbl::metadata::ObjectId::from(&object.guid);
    let expected = queryable_fields(&snapshot, object).unwrap();
    let catalog = queryable_field_catalog(&snapshot);
    assert_eq!(catalog.get(&object_id), Some(&expected));
    assert!(
        catalog[&object_id]
            .iter()
            .any(|field| field.name == "RenamedAttribute")
    );

    let without_live = with_live_tables(snapshot, Vec::clear);
    assert!(!queryable_field_catalog(&without_live).contains_key(&object_id));
}

#[test]
fn prepared_queries_are_bound_to_their_resolved_snapshot() {
    let snapshot = snapshot();
    let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
        .prepare("SELECT Code FROM Catalog.OpenSdblMetadataProbe;")
        .unwrap();
    assert!(prepared.compile(&snapshot, &[]).is_ok());

    let changed = with_live_tables(snapshot.clone(), |tables| {
        tables[0].columns.push(LiveColumn {
            name: "_later_column".to_owned(),
            data_type: "bytea".to_owned(),
        });
    });
    let error = prepared.compile(&changed, &[]).unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::SnapshotMismatch);
}

#[test]
fn bounds_total_work_for_repeated_tabular_section_union_branches() {
    let branch = "SELECT Сумма FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений";
    let source = std::iter::repeat_n(branch, 12_000)
        .collect::<Vec<_>>()
        .join(" UNION ");
    let error = QueryCompiler::new(&tabular_section_snapshot(), PostgresBackend)
        .compile(&source)
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::WorkBudgetExceeded);
}

#[test]
fn extension_projection_scans_are_budgeted_once_after_the_field_cache_miss() {
    let snapshot = with_schema(snapshot(), |schema| {
        schema.tables.extend(
            (0..6_000).map(|index| schema_table(&format!("Unrelated{index}"), 0, Vec::new())),
        );
    });
    let snapshot = with_live_tables(snapshot, |tables| {
        tables.extend((0..6_000).map(|index| live_table(&format!("_unrelated{index}"), &[])));
    });
    let branch = "SELECT Code FROM Catalog.OpenSdblMetadataProbe";
    let source = std::iter::repeat_n(branch, 50)
        .collect::<Vec<_>>()
        .join(" UNION ALL ");

    QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(&source)
        .unwrap();
}

#[test]
fn unrelated_catalog_tables_do_not_consume_the_work_budget() {
    let snapshot = with_schema(snapshot(), |schema| {
        schema.tables.extend(
            (0..9_000).map(|index| schema_table(&format!("Unrelated{index}"), 0, Vec::new())),
        );
    });
    let snapshot = with_live_tables(snapshot, |tables| {
        tables.extend((0..9_000).map(|index| live_table(&format!("_unrelated{index}"), &[])));
    });

    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe;")
        .unwrap();
    assert_eq!(labels(&compiled), ["Code"]);
}

#[test]
fn accepts_a_wide_legitimate_union_within_the_work_budget() {
    let snapshot = with_live_tables(snapshot(), |tables| {
        tables[0].columns.extend((0..160).map(|index| LiveColumn {
            name: format!("_extra{index}"),
            data_type: "bytea".to_owned(),
        }));
    });
    let branch = "SELECT * FROM Catalog.OpenSdblMetadataProbe";
    let source = std::iter::repeat_n(branch, 100)
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let (postgres, mssql) = for_each_backend!(&source, &snapshot);
    assert_eq!(postgres.unwrap().columns.len(), 164);
    assert_eq!(mssql.unwrap().columns.len(), 164);
}

#[test]
fn rejects_parameters_and_unsupported_clauses_before_sql_generation() {
    let snapshot = snapshot();
    let parameter = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Code;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(parameter.kind(), QueryDiagnosticKind::Parameter);
    assert!(parameter.message().contains("has no value"));

    let unsupported = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe ДЛЯ ИЗМЕНЕНИЯ;",
        &snapshot,
    )
    .unwrap_err();
    assert!(unsupported.message().contains("unsupported query syntax"));
}

#[test]
fn compiles_english_distinct_wildcard_and_real_document_date_spelling() {
    let snapshot = snapshot();
    let compiled = postgres_compile!(
        "SELECT DISTINCT * FROM Catalog.OpenSdblMetadataProbe ORDER BY Date DESC;",
        &snapshot,
    )
    .unwrap();

    assert!(compiled.sql.starts_with("SELECT DISTINCT "));
    assert!(compiled.sql.contains("\"_date_time\" AS \"Date\""));
    assert!(
        compiled
            .sql
            .ends_with("ORDER BY \"__src\".\"_date_time\" DESC")
    );
    assert!(
        compiled
            .columns
            .iter()
            .any(|column| column.label == "ProbeAttribute")
    );
}

#[test]
fn compiles_russian_descending_order() {
    let snapshot = snapshot();
    let compiled = postgres_compile!(
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
    let snapshot = snapshot();
    let missing = postgres_compile!(
        "SELECT Missing FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(missing.line(), 1);
    assert!(missing.column() > 1);
    assert_eq!(missing.kind(), QueryDiagnosticKind::UnknownField);
    assert!(missing.message().contains("was not found"));

    let ambiguous = find_metadata_object(&ambiguous_object_snapshot(), "Duplicate").unwrap_err();
    assert_eq!(ambiguous.kind(), QueryDiagnosticKind::AmbiguousObject);
    assert!(ambiguous.message().contains("ambiguous"));
}

#[test]
fn exposes_typed_diagnostic_sources_and_metadata_token_positions() {
    use std::error::Error as _;

    let snapshot = snapshot();
    let lexical = postgres_compile!("SELECT @;", &snapshot).unwrap_err();
    assert_eq!(lexical.kind(), QueryDiagnosticKind::Lex);
    assert!(lexical.source().is_some());

    let lookup =
        postgres_compile!("SELECT VALUE(Catalog.DoesNotExist.Value);", &snapshot,).unwrap_err();
    assert_eq!(lookup.kind(), QueryDiagnosticKind::UnknownObject);
    assert!(lookup.source().is_some());

    let unknown =
        postgres_compile!("SELECT Code\nFROM Catalog.DoesNotExist;", &snapshot,).unwrap_err();
    assert_eq!(unknown.kind(), QueryDiagnosticKind::UnknownObject);
    assert_eq!(unknown.line(), 2);
    assert_eq!(unknown.column(), 14);

    let eof_source = "SELECT Code\nFROM ";
    let eof = postgres_compile!(eof_source, &snapshot).unwrap_err();
    assert_eq!(eof.kind(), QueryDiagnosticKind::Syntax);
    assert_eq!(eof.offset(), eof_source.len());
    assert_eq!(eof.line(), 2);
    assert_eq!(eof.column(), 6);

    let presentation_named_field = postgres_compile!(
        "SELECT Представление FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(
        presentation_named_field.kind(),
        QueryDiagnosticKind::UnknownField
    );

    let unsupported = postgres_compile!("SELECT &Parameter;", &snapshot).unwrap_err();
    assert_eq!(unsupported.kind(), QueryDiagnosticKind::Parameter);

    let unknown_value = postgres_compile!(
        "SELECT VALUE(Catalog.OpenSdblMetadataProbe.DoesNotExist);",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(unknown_value.kind(), QueryDiagnosticKind::UnknownValue);
}

#[test]
fn covers_syntax_ambiguity_liveness_and_presentation_diagnostic_kinds() {
    let syntax_source = "SELECT Code FROM";
    let syntax = postgres_compile!(syntax_source, &snapshot()).unwrap_err();
    assert_eq!(syntax.kind(), QueryDiagnosticKind::Syntax);
    assert_eq!(syntax.offset(), syntax_source.len());

    let ambiguous_object = postgres_compile!(
        "SELECT ID FROM Catalog.Duplicate;",
        &ambiguous_object_snapshot(),
    )
    .unwrap_err();
    assert_eq!(
        ambiguous_object.kind(),
        QueryDiagnosticKind::AmbiguousObject
    );

    let ambiguous_field = postgres_compile!(
        "SELECT DuplicateField FROM Catalog.OpenSdblMetadataProbe;",
        &ambiguous_field_snapshot(),
    )
    .unwrap_err();
    assert_eq!(ambiguous_field.kind(), QueryDiagnosticKind::AmbiguousField);

    let not_live_snapshot = with_live_tables(snapshot(), Vec::clear);
    let not_live = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &not_live_snapshot,
    )
    .unwrap_err();
    assert_eq!(not_live.kind(), QueryDiagnosticKind::NotLive);

    let snapshot = snapshot();
    let source = "SELECT REFPRESENTATION(Ссылка) FROM Catalog.OpenSdblMetadataProbe;";
    let missing_plan = postgres_compile!(source, &snapshot).unwrap_err();
    assert_eq!(missing_plan.kind(), QueryDiagnosticKind::PresentationPlan);

    let prepared = postgres_prepare!(source, &snapshot).unwrap();
    let object = prepared.presentation_request().targets[0].object;
    let code = FieldId::Standard(StandardFieldId::Code);
    let invalid_plan = prepared
        .compile(
            &snapshot,
            &[PresentationPlan {
                object,
                fields: vec![code, code],
                expression: PresentationExpression::Field(code),
            }],
        )
        .unwrap_err();
    assert_eq!(invalid_plan.kind(), QueryDiagnosticKind::PresentationPlan);
}

#[test]
fn reports_resolution_mismatches_without_dropping_unknown_columns() {
    let base = snapshot();
    let clean = resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert!(clean.report.is_empty());

    let mut schema = base.schema().clone();
    schema.anomalies.push(SchemaAnomaly {
        table: "Reference53".to_owned(),
        detail: "columns count is not an unsigned integer".to_owned(),
    });
    schema.tables[0].columns.push(SchemaColumn {
        name: "FutureColumn".to_owned(),
        types: vec![ColumnType {
            tag: "FUTURE".to_owned(),
            reference_target: None,
        }],
    });
    let mut live_tables = base.live_tables().to_vec();
    live_tables[0].columns.push(LiveColumn {
        name: "_futurecolumn".to_owned(),
        data_type: "bytea".to_owned(),
    });
    live_tables.push(LiveTable {
        name: "_live_only".to_owned(),
        columns: Vec::new(),
        indexes: Vec::new(),
    });
    live_tables[0].indexes.clear();
    let resolved = resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        live_tables,
    );

    assert!(resolved.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::UnknownColumnTag { tag, .. } if tag == "FUTURE"
    )));
    assert!(resolved.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::InvalidSchemaDeclaration { detail, .. }
            if detail.contains("columns count")
    )));
    assert!(resolved.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::TableNotDeclared { table } if table == "_live_only"
    )));
    assert!(
        resolved
            .report
            .findings()
            .iter()
            .any(|finding| matches!(finding, ResolutionFinding::IndexMismatch { .. }))
    );
    let compiled = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &resolved.snapshot,
    )
    .unwrap();
    assert_eq!(labels(&compiled), ["Code"]);
    assert!(
        queryable_fields(&resolved.snapshot, &resolved.snapshot.objects()[0])
            .unwrap()
            .iter()
            .any(|field| field.schema_name == "FutureColumn")
    );

    let without_live = resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        base.schema().clone(),
        Vec::new(),
    );
    assert!(
        without_live
            .report
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                ResolutionFinding::TableNotLive { table } if table == "_Reference53"
            ))
    );

    let without_descriptor = resolve_metadata(
        base.db_names().clone(),
        Vec::new(),
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert!(
        without_descriptor
            .report
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                ResolutionFinding::DescriptorMissing { table, .. } if table == "_Reference53"
            ))
    );

    let mut duplicated_descriptors = base.descriptors().to_vec();
    duplicated_descriptors.push(base.descriptors()[0].clone());
    let duplicated = resolve_metadata(
        base.db_names().clone(),
        duplicated_descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    );
    assert!(duplicated.report.findings().iter().any(|finding| matches!(
        finding,
        ResolutionFinding::DuplicateGuid { guid }
            if guid == &base.descriptors()[0].object_guid
    )));
}

#[test]
fn emits_unique_utf8_safe_output_labels_at_each_dialect_limit() {
    let snapshot = snapshot();
    let postgres_prefix = "Я".repeat(40);
    let postgres = postgres_compile!(
        &format!(
            "SELECT Code AS {postgres_prefix}А, Date AS {postgres_prefix}Б FROM Catalog.OpenSdblMetadataProbe;"
        ),
        &snapshot,
    )
    .unwrap();
    assert_ne!(postgres.columns[0], postgres.columns[1]);
    assert!(labels(&postgres).iter().all(|label| label.len() <= 63));
    assert!(
        labels(&postgres)
            .iter()
            .all(|label| postgres.sql.contains(&format!("AS \"{label}\"")))
    );

    let mssql_prefix = "Я".repeat(130);
    let mssql = mssql_compile!(
        &format!(
            "SELECT Code AS {mssql_prefix}А, Date AS {mssql_prefix}Б FROM Catalog.OpenSdblMetadataProbe;"
        ),
        &mssql_snapshot(),
    )
    .unwrap();
    assert_ne!(mssql.columns[0], mssql.columns[1]);
    assert!(
        labels(&mssql)
            .iter()
            .all(|label| label.encode_utf16().count() <= 128)
    );
    assert!(
        labels(&mssql)
            .iter()
            .all(|label| mssql.sql.contains(&format!("AS [{label}]")))
    );
}

#[test]
fn resolves_only_authoritative_enumeration_value_descriptors() {
    let base = enumeration_value_snapshot();
    let owner = base
        .objects()
        .iter()
        .find(|object| object.kind == Some(MetadataKind::Enumeration))
        .unwrap()
        .guid
        .clone();
    let form_guid = guid("03bd775a-e0a1-4205-82ce-6068e73ad134");
    let mut descriptors = base.descriptors().to_vec();
    descriptors.push(descriptor(&owner, &form_guid, "ListForm"));
    let resolved = resolve_metadata(
        base.db_names().clone(),
        descriptors,
        base.schema().clone(),
        base.live_tables().to_vec(),
    );

    assert!(resolved.values().iter().any(|value| value.name == "Статус"));
    assert!(
        !resolved
            .values()
            .iter()
            .any(|value| value.name == "ListForm")
    );
}

#[test]
fn expands_a_compound_projection_and_rejects_it_in_predicates() {
    let snapshot = with_live_tables(snapshot(), |tables| {
        let table = &mut tables[0];
        table.columns.retain(|column| column.name != "_fld54");
        table.columns.extend([
            LiveColumn {
                name: "_fld54_rtref".to_owned(),
                data_type: "bytea".to_owned(),
            },
            LiveColumn {
                name: "_fld54_rrref".to_owned(),
                data_type: "bytea".to_owned(),
            },
        ]);
    });

    let (compiled, mssql) = for_each_backend!(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    );
    let compiled = compiled.unwrap();
    assert_eq!(labels(&compiled), ["ProbeAttribute"]);
    assert!(compiled.sql.contains(
        "(\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") AS \"ProbeAttribute\""
    ));
    assert!(matches!(
        &compiled.columns[0].kind,
        ColumnKind::Reference {
            runtime_typed: true,
            ..
        }
    ));
    assert!(
        mssql
            .unwrap()
            .sql
            .contains("([__src].[_fld54_rtref] + [__src].[_fld54_rrref]) AS [ProbeAttribute]")
    );

    let aliased = postgres_compile!(
        "SELECT ProbeAttribute AS Value FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&aliased), ["Value"]);

    let error = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute IS NULL;",
        &snapshot,
    )
    .unwrap_err();
    assert!(error.message().contains("compound field"));
}

#[test]
fn dereferences_a_reference_property_with_one_reused_left_join() {
    let snapshot = reference_snapshot();
    let compiled = postgres_compile!(
        "SELECT Организация.Код FROM Catalog.OpenSdblMetadataProbe WHERE Организация.Код = \"A\" ORDER BY Организация.Код;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Организация.Код"]);
    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 1);
    assert_eq!(
        compiled.sql,
        "SELECT \"__ref1\".\"_code\"::text AS \"Организация.Код\" FROM \"_reference53\" AS \"__src\" LEFT JOIN \"_reference57\" AS \"__ref1\" ON \"__src\".\"_fld54\" = \"__ref1\".\"_idrref\" WHERE (\"__ref1\".\"_code\" = 'A') ORDER BY \"__ref1\".\"_code\" ASC"
    );
}

#[test]
fn supports_a_qualified_reference_path_and_rejects_non_references() {
    let snapshot = reference_snapshot();
    let explicit = postgres_compile!(
        "ВЫБРАТЬ d.Организация.Код ИЗ Справочник.OpenSdblMetadataProbe КАК d;",
        &snapshot,
    )
    .unwrap();
    let implicit = postgres_compile!(
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
    assert_eq!(labels(&implicit), ["Организация.Code"]);

    let error = postgres_compile!(
        "SELECT Code.Value FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("no unique SchemaStorage reference target")
    );

    let deep = postgres_compile!(
        "SELECT d.Организация.Ссылка.Код FROM Catalog.OpenSdblMetadataProbe AS d;",
        &snapshot,
    )
    .unwrap_err();
    assert!(deep.message().contains("deeper than one hop"));

    let collision = postgres_compile!(
        "SELECT __ref1.Организация.Код FROM Catalog.OpenSdblMetadataProbe AS __ref1;",
        &snapshot,
    )
    .unwrap();
    assert!(collision.sql.contains("AS \"__ref2\" ON"));
}

#[test]
fn leaves_where_and_order_clauses_after_an_unaliased_source() {
    let snapshot = snapshot();
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p WHERE p.Code = \"A\"
         ОБЪЕДИНИТЬ
         ВЫБРАТЬ q.Код ИЗ Справочник.OpenSdblMetadataProbe КАК q
         UNION ALL
         SELECT r.Code FROM Catalog.OpenSdblMetadataProbe AS r
         ORDER BY Code DESC;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Code"]);
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
    let compiled = postgres_compile!(
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
    let logical_mismatch = postgres_compile!(
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

    let local_order = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe ORDER BY Code
         UNION SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(local_order.message().contains("unsupported query syntax"));

    let missing_order_field = postgres_compile!(
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
    let snapshot = with_live_tables(snapshot(), |tables| {
        let table = &mut tables[0];
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
    });

    let mismatch = postgres_compile!(
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
        let compiled = postgres_compile!(&query, &snapshot).unwrap();
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
        postgres_compile!(query, &snapshot).unwrap(),
        mssql_compile!(query, &snapshot).unwrap(),
    ] {
        let on = compiled.sql.split_once(" ON ").unwrap().1;
        let (left_code, left_id) = if compiled.sql.contains("[l]") {
            ("[l].[_code] = [r].[_code] AND ", "([l].[_idrref] IN (")
        } else {
            (
                "\"l\".\"_code\" = \"r\".\"_code\" AND ",
                "(\"l\".\"_idrref\" IN (",
            )
        };
        assert!(on.starts_with(left_code));
        assert!(on.contains(left_id));
        assert!(on.contains("a3dae56fa2f94623445632b52e22ad88"));
        assert!(on.contains("a161ed47a2787c5a437832a3f6fa6a92"));
        assert!(on.contains(if compiled.sql.contains("[l]") {
            " AND ([l].[_code] <> "
        } else {
            " AND (\"l\".\"_code\" <> "
        }));
    }
}

#[test]
fn keeps_additional_full_join_predicates_in_both_on_clauses() {
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
        "SELECT Организация.Код, t.Code
         FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap();

    assert_eq!(labels(&compiled), ["Организация.Код", "Code"]);
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
    let compiled = postgres_compile!(
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

    let mssql = mssql_compile!(
        "SELECT l.Code, r.Date
         FROM Catalog.OpenSdblMetadataProbe l
         FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code
         UNION ALL SELECT Code, Date FROM Catalog.OpenSdblMetadataProbe;",
        &mssql_snapshot(),
    )
    .unwrap();
    assert!(!mssql.sql.contains('"'), "{}", mssql.sql);
    assert!(mssql.sql.contains("AS [__full]"));
}

#[test]
fn rejects_unsafe_or_ambiguous_join_shapes() {
    let snapshot = reference_snapshot();
    let wildcard = postgres_compile!(
        "SELECT * FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("wildcard projection"));

    let inequality = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code > t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        inequality
            .message()
            .contains("top-level direct-field equality")
    );

    let nested_anchor = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t
         ON p.Code = t.Code OR p.Code <> t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        nested_anchor
            .message()
            .contains("top-level direct-field equality")
    );

    let same_alias = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации p ON p.Code = p.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(same_alias.message().contains("distinct aliases"));

    let deep_condition = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Организация.Код.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(deep_condition.message().contains("deeper than one hop"));

    let full_join_condition = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         FULL JOIN Catalog.Организации t ON p.Организация.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(
        full_join_condition
            .message()
            .contains("FULL JOIN condition supports direct fields only")
    );

    let ambiguous = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(ambiguous.message().contains("ambiguous in JOIN sources"));
}

#[test]
fn rejects_an_ambiguous_schema_reference_target() {
    let snapshot = with_schema(reference_snapshot(), |schema| {
        let source_field = schema.tables[0]
            .columns
            .iter_mut()
            .find(|column| column.name == "Fld54")
            .unwrap();
        source_field.types.push(ColumnType {
            tag: "R".to_owned(),
            reference_target: Some("Reference58".to_owned()),
        });
    });

    let error = postgres_compile!(
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
    let compiled = postgres_compile!(
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
        labels(&compiled),
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

    let direct = postgres_compile!(
        "SELECT Ссылка, НомерСтроки, Сумма
         FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&direct), ["ID", "LineNo", "Сумма"]);
    assert!(direct.sql.contains("FROM \"_document53_vt54X1\""));
    assert!(direct.sql.contains("\"_document53_idrref\" AS \"ID\""));
    assert!(direct.sql.contains("\"_lineno54\" AS \"LineNo\""));
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

    let postgres = postgres_prepare!(source, &snapshot).unwrap();
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

    let mssql = mssql_prepare!(source, &snapshot).unwrap();
    assert_eq!(mssql.presentation_request().targets.len(), 1);
    let mssql = mssql.compile(&snapshot, &[plan]).unwrap();
    assert_dereferenced_presentation_joins(&mssql.sql);
}

#[test]
fn reuses_a_dereference_join_only_when_the_complete_key_matches() {
    let snapshot = dereferenced_presentation_snapshot();
    let source = "SELECT
            строки.ЦФО.Сам_БизнесРегион,
            REFPRESENTATION(строки.ЦФО.Сам_БизнесРегион)
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";
    let prepared = postgres_prepare!(source, &snapshot).unwrap();
    let object = prepared.presentation_request().targets[0].object;
    let id = FieldId::Standard(StandardFieldId::Id);
    let plan = PresentationPlan {
        object,
        fields: vec![id],
        expression: PresentationExpression::Field(id),
    };
    let compiled = prepared
        .compile(&snapshot, std::slice::from_ref(&plan))
        .unwrap();

    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 2);
    assert!(compiled.sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref1\" ON \"строки\".\"_fld55\" = \"__right_ref1\".\"_idrref\""
    ));
    assert!(compiled.sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref2\" ON \"__right_ref1\".\"_fld63\" = \"__right_ref2\".\"_idrref\""
    ));

    let prepared = mssql_prepare!(source, &snapshot).unwrap();
    let compiled = prepared.compile(&snapshot, &[plan]).unwrap();
    assert_eq!(compiled.sql.matches(" LEFT JOIN ").count(), 2);
    assert!(compiled.sql.contains(
        "LEFT JOIN [_reference62] AS [__right_ref1] ON [строки].[_fld55] = [__right_ref1].[_idrref]"
    ));
    assert!(compiled.sql.contains(
        "LEFT JOIN [_reference62] AS [__right_ref2] ON [__right_ref1].[_fld63] = [__right_ref2].[_idrref]"
    ));
}

#[test]
fn presents_a_scalar_from_a_dereferenced_join_alias_without_a_plan() {
    let snapshot = tabular_section_snapshot();
    let source = "SELECT PRESENTATION(строки.ЦФО.Сам_БизнесРегион)
         FROM InformationRegister.бит_СтатусыОбъектов AS статусы
         INNER JOIN Document.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений AS строки
         ON статусы.Объект = строки.Ссылка;";
    let prepared = postgres_prepare!(source, &snapshot).unwrap();
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

    let postgres = postgres_prepare!(source, &snapshot).unwrap();
    assert!(postgres.presentation_request().targets.is_empty());
    let postgres = postgres.compile(&snapshot, &[]).unwrap();
    assert_eq!(postgres.deferred_presentations, [0]);
    assert!(postgres.sql.contains(
        "(\"__right_ref1\".\"_fld59_rtref\" || \"__right_ref1\".\"_fld59_rrref\") AS \"Договор\""
    ));
    assert_eq!(
        postgres.columns[0].kind,
        ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        }
    );
    assert!(postgres.sql.ends_with(" LIMIT 10"));
    assert_eq!(postgres.sql.matches(" LEFT JOIN ").count(), 1);

    let mssql = mssql_prepare!(source, &snapshot).unwrap();
    let mssql = mssql.compile(&snapshot, &[]).unwrap();
    assert_eq!(mssql.deferred_presentations, [0]);
    assert!(
        mssql.sql.contains(
            "([__right_ref1].[_fld59_rtref] + [__right_ref1].[_fld59_rrref]) AS [Договор]"
        )
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

    let postgres = postgres_presentation_lookup!(&snapshot, &plan, &references).unwrap();
    assert!(postgres.deferred_presentations.is_empty());
    assert!(postgres.sql.contains(
        "WHERE \"__presentation_target\".\"_idrref\" IN (decode('11111111111111111111111111111111', 'hex'), decode('22222222222222222222222222222222', 'hex'))"
    ));

    let mssql = mssql_presentation_lookup!(&snapshot, &plan, &references, 2000).unwrap();
    assert!(mssql.sql.contains(
        "WHERE [__presentation_target].[_idrref] IN (0x11111111111111111111111111111111, 0x22222222222222222222222222222222)"
    ));

    let (postgres, mssql) = for_each_backend!(presentation & snapshot, &plan, &[], 0);
    assert_eq!(
        postgres.unwrap_err().kind(),
        QueryDiagnosticKind::PresentationBatch
    );
    assert_eq!(
        mssql.unwrap_err().kind(),
        QueryDiagnosticKind::PresentationBatch
    );
}

#[test]
fn enforces_presentation_batch_boundaries_for_both_backends() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let object = snapshot.object_id_by_database_type(62).unwrap();
    let id = FieldId::Standard(StandardFieldId::Id);
    let plan = PresentationPlan {
        object,
        fields: vec![id],
        expression: PresentationExpression::Field(id),
    };

    let empty = postgres_presentation_lookup!(&snapshot, &plan, &[]).unwrap_err();
    assert_eq!(empty.kind(), QueryDiagnosticKind::PresentationBatch);

    for count in [1, 1_024] {
        let references = (0..count)
            .map(|number| (number as u128).to_be_bytes())
            .collect::<Vec<_>>();
        let (postgres, mssql) = for_each_backend!(presentation & snapshot, &plan, &references, 0);
        let postgres = postgres.unwrap();
        let mssql = mssql.unwrap();
        assert_eq!(labels(&postgres), ["__reference", "__presentation"]);
        assert_eq!(labels(&mssql), ["__reference", "__presentation"]);
        assert_eq!(postgres.sql.matches("decode('").count(), count);
        assert_eq!(mssql.sql.matches("0x").count(), count);
    }

    let references = (0..1_025_u128).map(u128::to_be_bytes).collect::<Vec<_>>();
    let (postgres, mssql) = for_each_backend!(presentation & snapshot, &plan, &references, 0);
    for oversized in [postgres.unwrap_err(), mssql.unwrap_err()] {
        assert_eq!(oversized.kind(), QueryDiagnosticKind::PresentationBatch);
        assert!(oversized.message().contains("exceeds 1,024"));
    }
}

fn assert_dereferenced_presentation_joins(sql: &str) {
    let normalized = sql.replace(['[', ']'], "\"");
    assert_eq!(normalized.matches(" LEFT JOIN ").count(), 4, "{sql}");
    assert!(normalized.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref1\" ON \"строки\".\"_fld55\" = \"__right_ref1\".\"_idrref\""
    ));
    assert!(normalized.contains(
        "LEFT JOIN \"_document53\" AS \"__right_ref2\" ON \"строки\".\"_document53_idrref\" = \"__right_ref2\".\"_idrref\""
    ));
    assert!(normalized.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref3\" ON \"__right_ref2\".\"_fld59\" = \"__right_ref3\".\"_idrref\""
    ));
    assert!(normalized.contains(
        "LEFT JOIN \"_reference62\" AS \"__right_ref4\" ON \"__right_ref1\".\"_fld63\" = \"__right_ref4\".\"_idrref\""
    ));
}

#[test]
fn diagnoses_a_tabular_section_missing_from_schema_storage() {
    let snapshot = with_schema(tabular_section_snapshot(), |schema| {
        schema
            .tables
            .retain(|table| table.name != "Document53_VT54X1");
    });

    let error = postgres_compile!(
        "SELECT Сумма
         FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
        &snapshot,
    )
    .unwrap_err();

    assert!(error.message().contains("absent from SchemaStorage"));
    assert!(error.message().contains("_Document53_VT54"));
    assert_eq!(error.kind(), QueryDiagnosticKind::Metadata);
}

#[test]
fn reports_structured_column_kinds_for_fields_scalars_and_aggregates() {
    let snapshot = snapshot();
    let object = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();

    let fields = postgres_compile!(
        "SELECT Ссылка, Code, Date, ProbeAttribute FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        kinds(&fields),
        [
            &ColumnKind::Reference {
                targets: vec![object],
                runtime_typed: false,
            },
            &ColumnKind::String { length: Some(9) },
            &ColumnKind::DateTime,
            &ColumnKind::Binary { length: None },
        ]
    );
    assert!(fields.sql.contains("\"__src\".\"_idrref\" AS \"ID\""));
    assert!(fields.sql.contains("\"__src\".\"_code\"::text AS \"Code\""));
    assert!(fields.sql.contains("\"__src\".\"_date_time\" AS \"Date\""));

    let scalars = postgres_compile!(
        "SELECT 4, \"text\", TRUE, NULL, DATETIME(2024, 1, 1), 2 + 2, 1 < 2;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        kinds(&scalars),
        [
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::String { length: None },
            &ColumnKind::Boolean,
            &ColumnKind::Null,
            &ColumnKind::DateTime,
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::Boolean,
        ]
    );

    let aggregates = postgres_compile!(
        "SELECT COUNT(*), MIN(Date), MAX(Code), SUM(Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        kinds(&aggregates),
        [
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::DateTime,
            &ColumnKind::String { length: Some(9) },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
        ]
    );
    assert!(!aggregates.sql.contains("::text"));
}

#[test]
fn diagnoses_union_kind_mismatches_and_accepts_null_branches() {
    let snapshot = snapshot();
    let (postgres, mssql) = for_each_backend!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe
         ОБЪЕДИНИТЬ ВСЕ
         SELECT Date FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    );
    let error = postgres.unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert_eq!((error.line(), error.column()), (2, 10));
    assert!(error.message().contains("DateTime"));
    assert!(mssql.is_err());

    let nullable = postgres_compile!("SELECT NULL UNION ALL SELECT 4;", &snapshot).unwrap();
    assert_eq!(
        kinds(&nullable),
        [&ColumnKind::Number {
            precision: None,
            scale: None,
        }]
    );

    let reference_with_null = postgres_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe UNION ALL SELECT NULL;",
        &snapshot,
    )
    .unwrap();
    assert!(matches!(
        &reference_with_null.columns[0].kind,
        ColumnKind::Reference { .. }
    ));
}

#[test]
fn compiles_reference_uuid_in_both_dialects() {
    let snapshot = reference_snapshot();
    let (postgres, mssql) = for_each_backend!(
        "ВЫБРАТЬ УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Ссылка) КАК Идентификатор, UUID(Организация) AS Owner
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ UUID(Ссылка) = \"d2f8bde9-fadd-4be8-9022-249e3a1ac4b9\";",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["Идентификатор", "Owner"]);
    assert_eq!(kinds(&postgres), [&ColumnKind::Uuid, &ColumnKind::Uuid]);
    assert!(postgres.sql.contains(
        "encode(substring(\"__src\".\"_idrref\" from 13 for 4) || substring(\"__src\".\"_idrref\" from 11 for 2) || substring(\"__src\".\"_idrref\" from 9 for 2) || substring(\"__src\".\"_idrref\" from 1 for 8), 'hex')::uuid AS \"Идентификатор\""
    ));
    assert!(
        postgres
            .sql
            .contains("substring(\"__src\".\"_fld54\" from 13 for 4)")
    );
    assert!(
        postgres
            .sql
            .contains("'hex')::uuid = 'd2f8bde9-fadd-4be8-9022-249e3a1ac4b9')")
    );

    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "CAST(SUBSTRING([__src].[_idrref], 16, 1) + SUBSTRING([__src].[_idrref], 15, 1) + SUBSTRING([__src].[_idrref], 14, 1) + SUBSTRING([__src].[_idrref], 13, 1) + SUBSTRING([__src].[_idrref], 12, 1) + SUBSTRING([__src].[_idrref], 11, 1) + SUBSTRING([__src].[_idrref], 10, 1) + SUBSTRING([__src].[_idrref], 9, 1) + SUBSTRING([__src].[_idrref], 1, 8) AS uniqueidentifier) AS [Идентификатор]"
    ));
    assert!(!mssql.sql.contains("::"));
}

#[test]
fn decodes_uuid_of_dereferenced_and_compound_references() {
    let dereferenced = postgres_compile!(
        "SELECT UUID(Организация.Ссылка), Организация.Код FROM Catalog.OpenSdblMetadataProbe;",
        &reference_snapshot(),
    )
    .unwrap();
    assert_eq!(dereferenced.sql.matches(" LEFT JOIN ").count(), 1);
    assert!(
        dereferenced
            .sql
            .contains("substring(\"__ref1\".\"_idrref\" from 13 for 4)")
    );
    assert_eq!(dereferenced.columns[0].kind, ColumnKind::Uuid);

    let compound_snapshot = with_live_tables(snapshot(), |tables| {
        let table = &mut tables[0];
        table.columns.retain(|column| column.name != "_fld54");
        table
            .columns
            .extend(["_fld54_rtref", "_fld54_rrref"].map(|name| LiveColumn {
                name: name.to_owned(),
                data_type: "bytea".to_owned(),
            }));
    });
    let compound = postgres_compile!(
        "SELECT UUID(ProbeAttribute) FROM Catalog.OpenSdblMetadataProbe;",
        &compound_snapshot,
    )
    .unwrap();
    assert!(
        compound
            .sql
            .contains("substring(\"__src\".\"_fld54_rrref\" from 13 for 4)")
    );
    assert!(!compound.sql.contains("_fld54_rtref"));
}

#[test]
fn diagnoses_invalid_uuid_arguments() {
    let snapshot = snapshot();
    let non_reference = postgres_compile!(
        "SELECT UUID(Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(non_reference.kind(), QueryDiagnosticKind::Syntax);
    assert_eq!((non_reference.line(), non_reference.column()), (1, 8));
    assert!(non_reference.message().contains("reference field"));

    let literal = postgres_compile!(
        "SELECT UUID(4) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(literal.kind(), QueryDiagnosticKind::Syntax);

    let two_arguments = postgres_compile!(
        "SELECT UUID(Ссылка, Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(two_arguments.kind(), QueryDiagnosticKind::Syntax);

    let source_free = postgres_compile!("SELECT UUID(Ссылка);", &snapshot).unwrap_err();
    assert_eq!(source_free.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(source_free.message().contains("requires FROM"));

    // The keyword still works as an ordinary alias.
    let alias = postgres_compile!(
        "SELECT Code AS UUID FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&alias), ["UUID"]);
}

fn inflated_snapshot(extra_tables: usize) -> MetadataSnapshot {
    let base = universal_dereferenced_presentation_snapshot();
    let mut schema = base.schema().clone();
    let mut live = base.live_tables().to_vec();
    let template = schema.tables[0].clone();
    for offset in 0..extra_tables {
        let number = 10_000 + u32::try_from(offset).unwrap();
        let mut table = template.clone();
        table.name = format!("Reference{number}");
        table.number = number;
        schema.tables.push(table);
        live.push(LiveTable {
            name: format!("_reference{number}"),
            columns: vec![LiveColumn {
                name: "_idrref".to_owned(),
                data_type: "bytea".to_owned(),
            }],
            indexes: Vec::new(),
        });
    }
    resolve_metadata(
        base.db_names().clone(),
        base.descriptors().to_vec(),
        schema,
        live,
    )
    .snapshot
}

#[test]
fn compilation_work_does_not_scale_with_information_base_size() {
    let snapshot = inflated_snapshot(20_000);
    let source = "ВЫБРАТЬ ПЕРВЫЕ 10
        ПредставлениеСсылки(строки.ЦФО) КАК ЦФО,
        ПредставлениеСсылки(строки.Ссылка.ДоговорКонтрагента) КАК Договор,
        строки.СуммаБезНДС КАК СуммаБезНДС,
        строки.Период КАК Период,
        ПредставлениеСсылки(строки.ЦФО.Сам_БизнесРегион) КАК Город
        ИЗ РегистрСведений.бит_СтатусыОбъектов КАК статусы
        ВНУТРЕННЕЕ СОЕДИНЕНИЕ Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК строки
        ПО статусы.Объект = строки.Ссылка
        ГДЕ строки.Сумма > 0;";
    let (postgres, mssql) = for_each_backend!(prepare source, &snapshot);
    let prepared = postgres.unwrap();
    assert!(mssql.is_ok());
    let plans = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| {
            let id = FieldId::Standard(StandardFieldId::Id);
            PresentationPlan {
                object: target.object,
                fields: vec![id],
                expression: PresentationExpression::Field(id),
            }
        })
        .collect::<Vec<_>>();
    let compiled = prepared.compile(&snapshot, &plans).unwrap();
    assert_eq!(compiled.columns.len(), 5);
    assert_eq!(compiled.deferred_presentations, [1]);
}

#[test]
fn indexes_live_and_schema_tables_including_extension_variants() {
    let snapshot = with_live_tables(snapshot(), |tables| {
        tables.push(LiveTable {
            name: "_reference53X1".to_owned(),
            columns: vec![LiveColumn {
                name: "_fld99".to_owned(),
                data_type: "bytea".to_owned(),
            }],
            indexes: Vec::new(),
        });
    });
    assert_eq!(
        snapshot
            .live_table("_Reference53")
            .map(|table| table.name.as_str()),
        Some("_reference53")
    );
    assert!(snapshot.live_table("_reference99").is_none());
    assert_eq!(
        snapshot
            .extension_live_tables("_reference53")
            .map(|table| table.name.as_str())
            .collect::<Vec<_>>(),
        ["_reference53X1"]
    );
    assert!(
        snapshot
            .extension_live_tables("_reference53X1")
            .next()
            .is_none()
    );
    assert_eq!(
        snapshot
            .schema_table("_reference53")
            .map(|table| table.number),
        snapshot
            .schema()
            .table("_reference53")
            .map(|table| table.number)
    );
    assert_eq!(
        snapshot.object_id_by_physical_table("Reference53"),
        snapshot.object_id_by_physical_table("_reference53")
    );
}

#[test]
fn preserves_sql_server_2008_goldens_for_begin_of_period() {
    let snapshot = mssql_snapshot();
    let value = "[__src].[_date_time]";
    let base = "CONVERT(datetime2, '00010101', 112)";
    let day = format!("CONVERT(datetime2, CONVERT(date, {value}))");
    let cases = [
        (
            "МИНУТА",
            format!("DATEADD(minute, DATEDIFF(minute, {day}, {value}), {day})"),
        ),
        (
            "ЧАС",
            format!("DATEADD(hour, DATEDIFF(hour, {day}, {value}), {day})"),
        ),
        ("ДЕНЬ", day.clone()),
        (
            "НЕДЕЛЯ",
            format!(
                "DATEADD(day, -(((DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, {value})) % 7) + 7) % 7), {day})"
            ),
        ),
        (
            "ДЕКАДА",
            format!(
                "DATEADD(day, CASE WHEN DAY({value}) <= 10 THEN 0 WHEN DAY({value}) <= 20 THEN 10 ELSE 20 END, DATEADD(month, DATEDIFF(month, {base}, {value}), {base}))"
            ),
        ),
        (
            "МЕСЯЦ",
            format!("DATEADD(month, DATEDIFF(month, {base}, {value}), {base})"),
        ),
        (
            "КВАРТАЛ",
            format!("DATEADD(quarter, DATEDIFF(quarter, {base}, {value}), {base})"),
        ),
        (
            "ПОЛУГОДИЕ",
            format!("DATEADD(month, (DATEDIFF(month, {base}, {value}) / 6) * 6, {base})"),
        ),
        (
            "ГОД",
            format!("DATEADD(year, DATEDIFF(year, {base}, {value}), {base})"),
        ),
    ];
    for (period, expected) in cases {
        let source = format!(
            "ВЫБРАТЬ НАЧАЛОПЕРИОДА(Дата, {period}) КАК Начало ИЗ Справочник.OpenSdblMetadataProbe;"
        );
        let legacy =
            mssql_compile_with_level!(&source, &snapshot, MsSqlDialectLevel::Sql2008, 0).unwrap();
        assert_eq!(
            legacy.sql,
            format!("SELECT {expected} AS [Начало] FROM [_reference53] AS [__src]"),
            "SQL Server 2008 golden changed for {period}"
        );
        assert!(!legacy.sql.contains("DATETIME2FROMPARTS"));

        let offset =
            mssql_compile_with_level!(&source, &snapshot, MsSqlDialectLevel::Sql2008, 2000)
                .unwrap();
        assert!(
            offset
                .sql
                .starts_with(&format!("SELECT DATEADD(year, -2000, {expected}) AS")),
            "year offset must wrap the 2008 rendering for {period}: {}",
            offset.sql
        );
        assert_eq!(kinds(&legacy), [&ColumnKind::DateTime]);
    }
}

#[test]
fn dialect_levels_differ_only_where_newer_functions_were_used() {
    let sources = [
        (
            snapshot(),
            "ВЫБРАТЬ ПЕРВЫЕ 5 Ссылка, Code, UUID(Ссылка), Date ИЗ Справочник.OpenSdblMetadataProbe ГДЕ Date > ДАТАВРЕМЯ(2024, 1, 1) УПОРЯДОЧИТЬ ПО Code;",
        ),
        (
            reference_snapshot(),
            "SELECT Организация.Код, ПРЕДСТАВЛЕНИЕ(4) FROM Catalog.OpenSdblMetadataProbe UNION ALL SELECT Code, ПРЕДСТАВЛЕНИЕ(5) FROM Catalog.OpenSdblMetadataProbe;",
        ),
        (
            snapshot(),
            "SELECT COUNT(*), SUM(Code) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;",
        ),
    ];
    for (snapshot, source) in &sources {
        let modern = QueryCompiler::new(snapshot, mssql_backend(2000))
            .compile(source)
            .map(|compiled| compiled.sql);
        let legacy =
            QueryCompiler::new(snapshot, mssql_backend_at(MsSqlDialectLevel::Sql2008, 2000))
                .compile(source)
                .map(|compiled| compiled.sql);
        assert_eq!(modern.ok(), legacy.ok(), "levels diverge for {source}");
    }

    let with_period = "SELECT BEGINOFPERIOD(Date, MONTH) FROM Catalog.OpenSdblMetadataProbe;";
    let modern = mssql_compile!(with_period, &mssql_snapshot()).unwrap();
    assert!(modern.sql.contains("DATETIME2FROMPARTS"));
    assert_eq!(
        QueryCompiler::new(&mssql_snapshot(), mssql_backend(0))
            .backend()
            .dialect_level(),
        MsSqlDialectLevel::Sql2012
    );
}

#[test]
fn compiles_scalar_casts_on_both_dialects() {
    let snapshot = snapshot();
    let (postgres, mssql) = for_each_backend!(
        "ВЫБРАТЬ ВЫРАЗИТЬ(Code КАК СТРОКА(10)) КАК Короткий, CAST(Code AS NUMBER(15, 2)) AS Число,
                ВЫРАЗИТЬ(Code КАК БУЛЕВО) КАК Флаг, ВЫРАЗИТЬ(Date КАК ДАТА) КАК Момент,
                ВЫРАЗИТЬ(Code КАК СТРОКА) КАК Полный, ВЫРАЗИТЬ(Code КАК СТРОКА(5000)) КАК Большой,
                ВЫРАЗИТЬ(Code КАК ЧИСЛО) КАК ЧислоПоУмолчанию
         ИЗ Справочник.OpenSdblMetadataProbe;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::String { length: Some(10) },
            &ColumnKind::Number {
                precision: Some(15),
                scale: Some(2),
            },
            &ColumnKind::Boolean,
            &ColumnKind::DateTime,
            &ColumnKind::String { length: None },
            &ColumnKind::String { length: Some(5000) },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
        ]
    );
    assert!(
        postgres
            .sql
            .contains("substring(\"__src\".\"_code\"::text from 1 for 10) AS \"Короткий\"")
    );
    assert!(
        postgres
            .sql
            .contains("\"__src\".\"_code\"::numeric(15, 2) AS \"Число\"")
    );
    assert!(
        postgres
            .sql
            .contains("\"__src\".\"_code\"::boolean AS \"Флаг\"")
    );
    assert!(
        postgres
            .sql
            .contains("\"__src\".\"_date_time\"::timestamp AS \"Момент\"")
    );
    assert!(
        postgres
            .sql
            .contains("\"__src\".\"_code\"::text AS \"Полный\"")
    );
    assert!(
        postgres
            .sql
            .contains("\"__src\".\"_code\"::numeric AS \"ЧислоПоУмолчанию\"")
    );

    let mssql = mssql.unwrap();
    assert!(
        mssql
            .sql
            .contains("CONVERT(nvarchar(10), [__src].[_code]) AS [Короткий]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(numeric(15, 2), [__src].[_code]) AS [Число]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(bit, [__src].[_code]) AS [Флаг]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(datetime2, [__src].[_date_time]) AS [Момент]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(nvarchar(max), [__src].[_code]) AS [Полный]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(nvarchar(max), [__src].[_code]) AS [Большой]")
    );
    assert!(
        mssql
            .sql
            .contains("CONVERT(numeric(38, 10), [__src].[_code]) AS [ЧислоПоУмолчанию]")
    );

    let offset = mssql_compile_with_offset!(
        "SELECT CAST(Date AS DATE) AS Момент FROM Catalog.OpenSdblMetadataProbe;",
        &mssql_snapshot(),
        2000,
    )
    .unwrap();
    assert!(
        offset
            .sql
            .contains("DATEADD(year, -2000, CONVERT(datetime2, [__src].[_date_time])) AS [Момент]")
    );

    let source_free = postgres_compile!("SELECT ВЫРАЗИТЬ(4 КАК СТРОКА(3));", &snapshot).unwrap();
    assert_eq!(
        source_free.sql,
        "SELECT substring(4::text from 1 for 3) AS \"column1\""
    );
    assert_eq!(
        kinds(&source_free),
        [&ColumnKind::String { length: Some(3) }]
    );
}

#[test]
fn narrows_references_with_and_without_dereference() {
    let snapshot = universal_dereferenced_presentation_snapshot();
    let catalog = snapshot
        .object_id(MetadataKind::Catalog, "ЦентрыФинансовойОтветственности")
        .unwrap();
    let (postgres, mssql) = for_each_backend!(
        "ВЫБРАТЬ ВЫРАЗИТЬ(ДоговорКонтрагента КАК Справочник.ЦентрыФинансовойОтветственности) КАК Договор
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert!(postgres.sql.contains(
        "CASE WHEN \"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex') THEN \"__src\".\"_fld59_rrref\" END AS \"Договор\""
    ));
    assert_eq!(
        postgres.columns[0].kind,
        ColumnKind::Reference {
            targets: vec![catalog],
            runtime_typed: false,
        }
    );
    assert!(mssql.unwrap().sql.contains(
        "CASE WHEN [__src].[_fld59_rtref] = 0x0000003e THEN [__src].[_fld59_rrref] END AS [Договор]"
    ));

    let dereferenced = postgres_compile!(
        "ВЫБРАТЬ ВЫРАЗИТЬ(ДоговорКонтрагента КАК Справочник.ЦентрыФинансовойОтветственности).Сам_БизнесРегион КАК Регион
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
        &snapshot,
    )
    .unwrap();
    assert!(dereferenced.sql.contains(
        "LEFT JOIN \"_reference62\" AS \"__ref1\" ON \"__src\".\"_fld59_rrref\" = \"__ref1\".\"_idrref\" AND \"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex')"
    ));
    assert!(
        dereferenced
            .sql
            .contains("\"__ref1\".\"_fld63\" AS \"Регион\"")
    );
    assert_eq!(dereferenced.sql.matches(" LEFT JOIN ").count(), 1);

    let fixed = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Организация КАК Справочник.Организации).Код AS Код, Организация.Код AS Тот_же FROM Catalog.OpenSdblMetadataProbe;",
        &reference_snapshot(),
    )
    .unwrap();
    assert!(fixed.sql.contains("\"__ref1\".\"_code\"::text AS \"Код\""));
    assert!(
        fixed
            .sql
            .contains("\"__ref1\".\"_code\"::text AS \"Тот_же\"")
    );
    assert_eq!(fixed.sql.matches(" LEFT JOIN ").count(), 1);
    assert!(!fixed.sql.contains("_rtref"));
}

#[test]
fn diagnoses_invalid_casts() {
    let snapshot = snapshot();
    let mismatch = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Организация КАК Справочник.OpenSdblMetadataProbe) FROM Catalog.OpenSdblMetadataProbe;",
        &reference_snapshot(),
    )
    .unwrap_err();
    assert_eq!(mismatch.kind(), QueryDiagnosticKind::Syntax);
    assert!(mismatch.message().contains("cannot hold"));

    let non_reference = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Code КАК Справочник.OpenSdblMetadataProbe) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(non_reference.kind(), QueryDiagnosticKind::Syntax);

    let unknown_target = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Code КАК ФЛОАТ) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(
        unknown_target.kind(),
        QueryDiagnosticKind::UnsupportedFeature
    );
    assert_eq!((unknown_target.line(), unknown_target.column()), (1, 26));

    let too_many = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Code КАК СТРОКА(1, 2)) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(too_many.kind(), QueryDiagnosticKind::Syntax);

    let deep = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Организация КАК Справочник.Организации).Ссылка.Код FROM Catalog.OpenSdblMetadataProbe;",
        &reference_snapshot(),
    )
    .unwrap_err();
    assert_eq!(deep.kind(), QueryDiagnosticKind::UnsupportedFeature);

    let source_free = postgres_compile!(
        "SELECT ВЫРАЗИТЬ(Code КАК Справочник.OpenSdblMetadataProbe);",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(source_free.kind(), QueryDiagnosticKind::UnsupportedFeature);

    let alias = postgres_compile!(
        "SELECT Code AS CAST FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(labels(&alias), ["CAST"]);
}

#[test]
fn renders_boolean_predicates_as_comparisons_on_mssql() {
    let snapshot = with_live_tables(snapshot(), |tables| {
        tables[0].columns.push(LiveColumn {
            name: "_fld77".to_owned(),
            data_type: "boolean".to_owned(),
        });
    });
    let (postgres, mssql) = for_each_backend!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Fld77 AND NOT ВЫРАЗИТЬ(Code КАК БУЛЕВО) OR TRUE;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert!(postgres.sql.ends_with(
        "WHERE ((\"__src\".\"_fld77\" AND (NOT \"__src\".\"_code\"::boolean)) OR TRUE)"
    ));
    let mssql = mssql.unwrap();
    assert!(mssql.sql.ends_with(
        "WHERE ((([__src].[_fld77] = 0x01) AND (NOT (CONVERT(bit, [__src].[_code]) = 0x01))) OR (1 = 1))"
    ));

    let projected = mssql_compile!(
        "SELECT Fld77 FROM Catalog.OpenSdblMetadataProbe WHERE Fld77;",
        &snapshot,
    )
    .unwrap();
    assert!(projected.sql.contains("SELECT [__src].[_fld77] AS [Fld77]"));
    assert!(projected.sql.ends_with("WHERE ([__src].[_fld77] = 0x01)"));

    let joined = mssql_compile!(
        "SELECT l.Code FROM Catalog.OpenSdblMetadataProbe l INNER JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code AND r.Fld77;",
        &snapshot,
    )
    .unwrap();
    assert!(joined.sql.contains("AND ([r].[_fld77] = 0x01)"));
}

fn boolean_snapshot() -> MetadataSnapshot {
    with_live_tables(snapshot(), |tables| {
        tables[0].columns.push(LiveColumn {
            name: "_fld77".to_owned(),
            data_type: "boolean".to_owned(),
        });
    })
}

#[test]
fn compiles_case_expressions_in_projections_and_predicates() {
    let snapshot = boolean_snapshot();
    let (postgres, mssql) = for_each_backend!(
        "ВЫБРАТЬ ВЫБОР КОГДА Fld77 ТОГДА \"Да\" КОГДА Code = \"A\" ТОГДА \"A\" ИНАЧЕ \"Нет\" КОНЕЦ КАК Статус,
                CASE WHEN Fld77 THEN 1 END AS Флаг
         ИЗ Справочник.OpenSdblMetadataProbe
         ГДЕ ВЫБОР КОГДА Fld77 ТОГДА ИСТИНА ИНАЧЕ ЛОЖЬ КОНЕЦ;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["Статус", "Флаг"]);
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::String { length: None },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
        ]
    );
    assert!(postgres.sql.contains(
        "CASE WHEN \"__src\".\"_fld77\" THEN 'Да' WHEN (\"__src\".\"_code\" = 'A') THEN 'A' ELSE 'Нет' END AS \"Статус\""
    ));
    assert!(
        postgres
            .sql
            .contains("CASE WHEN \"__src\".\"_fld77\" THEN 1 END AS \"Флаг\"")
    );
    assert!(
        postgres
            .sql
            .ends_with("WHERE CASE WHEN \"__src\".\"_fld77\" THEN TRUE ELSE FALSE END")
    );

    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "CASE WHEN ([__src].[_fld77] = 0x01) THEN N'Да' WHEN ([__src].[_code] = N'A') THEN N'A' ELSE N'Нет' END AS [Статус]"
    ));
    assert!(
        mssql.sql.ends_with(
            "WHERE (CASE WHEN ([__src].[_fld77] = 0x01) THEN 0x01 ELSE 0x00 END = 0x01)"
        )
    );

    let source_free = mssql_compile!(
        "SELECT CASE WHEN TRUE THEN 1 ELSE 2 END AS N, ISNULL(NULL, \"x\") AS S;",
        &snapshot,
    )
    .unwrap();
    assert!(
        source_free
            .sql
            .contains("CASE WHEN (1 = 1) THEN 1 ELSE 2 END AS [N]")
    );
    assert!(source_free.sql.contains("COALESCE(NULL, N'x') AS [S]"));
    assert_eq!(
        kinds(&source_free),
        [
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::String { length: None },
        ]
    );
}

#[test]
fn corrects_case_dates_once_on_mssql() {
    let snapshot = boolean_snapshot();
    let compiled = mssql_compile_with_offset!(
        "SELECT ВЫБОР КОГДА Fld77 ТОГДА Date ИНАЧЕ ДАТАВРЕМЯ(2024, 1, 1) КОНЕЦ AS D,
                ЕСТЬNULL(Date, ДАТАВРЕМЯ(1, 1, 1)) AS E,
                НАЧАЛОПЕРИОДА(ЕСТЬNULL(Date, ДАТАВРЕМЯ(1, 1, 1)), МЕСЯЦ) AS P
         FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
        2000
    )
    .unwrap();
    assert_eq!(
        kinds(&compiled),
        [
            &ColumnKind::DateTime,
            &ColumnKind::DateTime,
            &ColumnKind::DateTime
        ]
    );
    assert!(compiled.sql.contains(
        "DATEADD(year, -2000, CASE WHEN ([__src].[_fld77] = 0x01) THEN [__src].[_date_time] ELSE DATEADD(year, 2000, CONVERT(datetime2, '2024-01-01T00:00:00', 126)) END) AS [D]"
    ));
    assert!(compiled.sql.contains(
        "DATEADD(year, -2000, COALESCE([__src].[_date_time], DATEADD(year, 2000, CONVERT(datetime2, '0001-01-01T00:00:00', 126)))) AS [E]"
    ));
    assert!(
        compiled
            .sql
            .contains("DATEADD(year, -2000, DATETIME2FROMPARTS(YEAR(COALESCE(")
    );
    assert_eq!(compiled.sql.matches("DATEADD(year, -2000").count(), 3);
}

#[test]
fn widens_references_in_case_isnull_and_union() {
    let snapshot = presentation_reference_snapshot(true);
    let probe = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let field_targets = match &postgres_compile!(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap()
    .columns[0]
        .kind
    {
        ColumnKind::Reference { targets, .. } => targets.clone(),
        other => panic!("unexpected field kind {other:?}"),
    };
    assert_eq!(field_targets.len(), 2);
    let mut targets = field_targets.clone();
    targets.push(probe);
    targets.sort();
    let widened = ColumnKind::Reference {
        targets: targets.clone(),
        runtime_typed: true,
    };

    let (postgres, mssql) = for_each_backend!(
        "SELECT CASE WHEN Ссылка ЕСТЬ NULL THEN ProbeAttribute ELSE Ссылка END AS Any,
                ЕСТЬNULL(ProbeAttribute, Ссылка) AS Fallback,
                ЕСТЬNULL(Ссылка, Ссылка) AS Same
         FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(
        kinds(&postgres),
        [
            &widened,
            &widened,
            &ColumnKind::Reference {
                targets: vec![probe],
                runtime_typed: false,
            },
        ]
    );
    assert!(postgres.sql.contains(
        "CASE WHEN (\"__src\".\"_idrref\" IS NULL) THEN (\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") ELSE (decode('00000035', 'hex') || \"__src\".\"_idrref\") END AS \"Any\""
    ));
    assert!(postgres.sql.contains(
        "COALESCE((\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\"), (decode('00000035', 'hex') || \"__src\".\"_idrref\")) AS \"Fallback\""
    ));
    assert!(
        postgres
            .sql
            .contains("COALESCE(\"__src\".\"_idrref\", \"__src\".\"_idrref\") AS \"Same\"")
    );
    assert!(
        mssql
            .unwrap()
            .sql
            .contains("THEN ([__src].[_fld54_rtref] + [__src].[_fld54_rrref]) ELSE (0x00000035 + [__src].[_idrref]) END AS [Any]")
    );

    let union = postgres_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         UNION ALL SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe
         UNION ALL SELECT NULL;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(kinds(&union), [&widened]);
    assert!(
        union
            .sql
            .contains("(decode('00000035', 'hex') || \"__src\".\"_idrref\") AS \"ID\"")
    );
    assert!(union.sql.contains(
        "(\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") AS \"ProbeAttribute\""
    ));

    let same = postgres_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         UNION ALL SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        kinds(&same),
        [&ColumnKind::Reference {
            targets: vec![probe],
            runtime_typed: false,
        }]
    );
    assert!(!same.sql.contains("decode('00000035', 'hex')"));
}

#[test]
fn diagnoses_incompatible_conditional_branches() {
    let snapshot = snapshot();
    let error = postgres_compile!(
        "SELECT CASE WHEN Code = \"A\" THEN 1 ELSE \"x\" END FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert_eq!((error.line(), error.column()), (1, 41));
    assert!(error.message().contains("kinds differ"));

    let missing = postgres_compile!(
        "SELECT CASE END FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(missing.kind(), QueryDiagnosticKind::Syntax);

    let one_argument = postgres_compile!(
        "SELECT ЕСТЬNULL(Code) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(one_argument.kind(), QueryDiagnosticKind::Syntax);

    let mismatch = postgres_compile!(
        "SELECT ISNULL(Code, 5) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(mismatch.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn compiles_like_predicates_with_escape_and_negation() {
    let snapshot = snapshot();
    let (postgres, mssql) = for_each_backend!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe
         WHERE Code ПОДОБНО \"[0-9]%\" И Code НЕ ПОДОБНО \"%\\_%\" СПЕЦСИМВОЛ \"\\\" OR Code LIKE Code;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert!(postgres.sql.ends_with(
        "WHERE (((\"__src\".\"_code\" LIKE '[0-9]%') AND (NOT (\"__src\".\"_code\" LIKE '%\\_%' ESCAPE '\\'))) OR (\"__src\".\"_code\" LIKE \"__src\".\"_code\"))"
    ));
    let mssql = mssql.unwrap();
    assert!(mssql.sql.ends_with(
        "WHERE ((([__src].[_code] LIKE N'[0-9]%') AND (NOT ([__src].[_code] LIKE N'%\\_%' ESCAPE N'\\'))) OR ([__src].[_code] LIKE [__src].[_code]))"
    ));

    let in_case = postgres_compile!(
        "SELECT CASE WHEN Code LIKE \"A%\" THEN 1 ELSE 0 END AS Flag FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert!(
        in_case
            .sql
            .contains("CASE WHEN (\"__src\".\"_code\" LIKE 'A%') THEN 1 ELSE 0 END AS \"Flag\"")
    );

    let projected = postgres_compile!(
        "SELECT Code LIKE \"A%\" FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(projected.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(projected.message().contains("predicate positions"));

    let non_string = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Date LIKE \"2024%\";",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(non_string.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(non_string.message().contains("must be strings"));
}

#[test]
fn aggregates_arbitrary_scalar_expressions() {
    let snapshot = boolean_snapshot();
    let (postgres, mssql) = for_each_backend!(
        "SELECT SUM(CASE WHEN Fld77 THEN 1 ELSE 0 END) AS S,
                COUNT(DISTINCT BEGINOFPERIOD(Date, MONTH)) AS C,
                MAX(ISNULL(Code, \"\")) AS M
         FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["S", "C", "M"]);
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::String { length: Some(9) },
        ]
    );
    assert!(
        postgres
            .sql
            .contains("SUM(CASE WHEN \"__src\".\"_fld77\" THEN 1 ELSE 0 END) AS \"S\"")
    );
    assert!(
        postgres
            .sql
            .contains("COUNT(DISTINCT date_trunc('month', \"__src\".\"_date_time\")) AS \"C\"")
    );
    assert!(
        postgres
            .sql
            .contains("MAX(COALESCE(\"__src\".\"_code\", '')) AS \"M\"")
    );
    assert!(
        mssql
            .unwrap()
            .sql
            .contains("SUM(CASE WHEN ([__src].[_fld77] = 0x01) THEN 1 ELSE 0 END) AS [S]")
    );

    let reference = postgres_compile!(
        "SELECT MAX(ЕСТЬNULL(ProbeAttribute, Ссылка)) AS R FROM Catalog.OpenSdblMetadataProbe;",
        &presentation_reference_snapshot(true),
    )
    .unwrap();
    assert!(
        reference
            .sql
            .contains("MAX(COALESCE((\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\"), (decode('00000035', 'hex') || \"__src\".\"_idrref\"))) AS \"R\"")
    );
    assert!(matches!(
        &reference.columns[0].kind,
        ColumnKind::Reference {
            runtime_typed: true,
            ..
        }
    ));

    let nested = postgres_compile!(
        "SELECT SUM(SUM(Code)) FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(nested.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(nested.message().contains("only as projections"));

    let filtered = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE SUM(Code) > 1;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(filtered.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

use open_sdbl::query::{CompileOptions, ParameterDate, ParameterValue, QueryParameter};

fn compile_with_parameters<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    source: &str,
    parameters: &[QueryParameter],
) -> Result<open_sdbl::query::CompiledQuery, open_sdbl::query::QueryDiagnostic> {
    QueryCompiler::new(snapshot, backend)
        .compile_with(source, &CompileOptions::new().parameters(parameters))
}

fn number(unscaled: i128, scale: u8) -> ParameterValue {
    ParameterValue::Number { unscaled, scale }
}

fn date(year: u16, month: u8, day: u8) -> ParameterValue {
    ParameterValue::Date(ParameterDate::new(year, month, day, 0, 0, 0).unwrap())
}

#[test]
fn compiles_scalar_parameters_on_both_dialects() {
    let snapshot = boolean_snapshot();
    let parameters = [
        QueryParameter::new("Строка", ParameterValue::String("A'B".to_owned())),
        QueryParameter::new("Начало", date(2024, 1, 1)),
        QueryParameter::new("Флаг", ParameterValue::Boolean(true)),
        QueryParameter::new("Число", number(1550, 2)),
        QueryParameter::new("Пусто", ParameterValue::Null),
    ];
    let source = "SELECT &Число AS N, &строка AS S, &Начало AS D, &Пусто AS Z
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Code = &Строка AND Date >= &Начало AND &Флаг AND Fld77 = &Флаг;";
    let postgres =
        compile_with_parameters(&snapshot, PostgresBackend, source, &parameters).unwrap();
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::Number {
                precision: None,
                scale: Some(2),
            },
            &ColumnKind::String { length: None },
            &ColumnKind::DateTime,
            &ColumnKind::Null,
        ]
    );
    assert!(postgres.sql.starts_with(
        "SELECT 15.50 AS \"N\", 'A''B' AS \"S\", TIMESTAMP '2024-01-01 00:00:00' AS \"D\", NULL AS \"Z\" FROM"
    ));
    assert!(postgres.sql.ends_with(
        "WHERE ((((\"__src\".\"_code\" = 'A''B') AND (\"__src\".\"_date_time\" >= TIMESTAMP '2024-01-01 00:00:00')) AND TRUE) AND (\"__src\".\"_fld77\" = TRUE))"
    ));

    let mssql =
        compile_with_parameters(&snapshot, mssql_backend(2000), source, &parameters).unwrap();
    assert_eq!(kinds(&mssql), kinds(&postgres));
    assert!(mssql.sql.contains(
        "DATEADD(year, -2000, DATEADD(year, 2000, CONVERT(datetime2, '2024-01-01T00:00:00', 126))) AS [D]"
    ));
    assert!(mssql.sql.ends_with(
        "WHERE (((([__src].[_code] = N'A''B') AND ([__src].[_date_time] >= DATEADD(year, 2000, CONVERT(datetime2, '2024-01-01T00:00:00', 126)))) AND (0x01 = 0x01)) AND ([__src].[_fld77] = 0x01))"
    ));

    let source_free = compile_with_parameters(
        &snapshot,
        mssql_backend(2000),
        "SELECT &Начало AS D, CASE WHEN &Флаг THEN 1 ELSE 0 END AS F;",
        &[
            QueryParameter::new("Начало", date(2024, 1, 1)),
            QueryParameter::new("Флаг", ParameterValue::Boolean(false)),
        ],
    )
    .unwrap();
    assert!(
        source_free
            .sql
            .contains("CONVERT(datetime2, '2024-01-01T00:00:00', 126) AS [D]")
    );
    assert!(
        source_free
            .sql
            .contains("CASE WHEN (0x00 = 0x01) THEN 1 ELSE 0 END AS [F]")
    );
    assert!(!source_free.sql.contains("DATEADD"));
}

#[test]
fn compiles_reference_parameters_lists_and_output_format_literals() {
    let snapshot = presentation_reference_snapshot(true);
    let compiled = postgres_compile!(
        "SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    let ColumnKind::Reference { targets, .. } = &compiled.columns[0].kind else {
        panic!("reference field expected");
    };
    let target = targets[0];
    let probe = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let id = [0x11; 16];
    let other = [0x22; 16];
    let reference = ParameterValue::Reference { object: target, id };
    let list = ParameterValue::List(vec![
        reference.clone(),
        ParameterValue::Reference {
            object: target,
            id: other,
        },
    ]);
    let mut payload = 0x39u32.to_be_bytes().to_vec();
    payload.extend_from_slice(&id);

    let parameters = [
        QueryParameter::new("Ссылка", reference.clone()),
        QueryParameter::new("Список", list),
        QueryParameter::new("Payload", ParameterValue::Binary(payload.clone())),
        QueryParameter::new("Пустой", ParameterValue::List(Vec::new())),
    ];
    let source = "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         WHERE ProbeAttribute = &Ссылка OR ProbeAttribute <> &Ссылка OR ProbeAttribute IN (&Список)
            OR ProbeAttribute = &Payload OR Ссылка IN (&Пустой);";
    let postgres =
        compile_with_parameters(&snapshot, PostgresBackend, source, &parameters).unwrap();
    let hex_id = "11".repeat(16);
    let hex_other = "22".repeat(16);
    assert!(postgres.sql.contains(&format!(
        "((\"__src\".\"_fld54_rtref\" = decode('00000039', 'hex')) AND (\"__src\".\"_fld54_rrref\" = decode('{hex_id}', 'hex')))"
    )));
    assert!(postgres.sql.contains(&format!(
        "(NOT ((\"__src\".\"_fld54_rtref\" = decode('00000039', 'hex')) AND (\"__src\".\"_fld54_rrref\" = decode('{hex_id}', 'hex'))))"
    )));
    assert!(postgres.sql.contains(&format!(
        "(((\"__src\".\"_fld54_rtref\" = decode('00000039', 'hex')) AND (\"__src\".\"_fld54_rrref\" = decode('{hex_id}', 'hex'))) OR ((\"__src\".\"_fld54_rtref\" = decode('00000039', 'hex')) AND (\"__src\".\"_fld54_rrref\" = decode('{hex_other}', 'hex'))))"
    )));
    assert!(postgres.sql.ends_with("OR FALSE)"));
    let mssql = compile_with_parameters(&snapshot, mssql_backend(0), source, &parameters).unwrap();
    assert!(mssql.sql.contains(&format!(
        "(([__src].[_fld54_rtref] = 0x00000039) AND ([__src].[_fld54_rrref] = 0x{hex_id}))"
    )));
    assert!(mssql.sql.ends_with("OR (1 = 0))"));

    let single = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe WHERE Ссылка = &Ссылка OR Ссылка IN (&Список);",
        &[
            QueryParameter::new(
                "Ссылка",
                ParameterValue::Reference { object: probe, id },
            ),
            QueryParameter::new(
                "Список",
                ParameterValue::List(vec![
                    ParameterValue::Reference { object: probe, id },
                    ParameterValue::Reference { object: probe, id: other },
                ]),
            ),
        ],
    )
    .unwrap();
    assert!(single.sql.ends_with(&format!(
        "WHERE ((\"__src\".\"_idrref\" = decode('{hex_id}', 'hex')) OR (\"__src\".\"_idrref\" IN (decode('{hex_id}', 'hex'), decode('{hex_other}', 'hex'))))"
    )));

    let pasted = postgres_compile!(
        &format!(
            "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = 0x00000039{} AND Ссылка = 0x{};",
            hex_id.to_uppercase(),
            hex_other.to_uppercase()
        ),
        &snapshot,
    )
    .unwrap();
    assert!(pasted.sql.ends_with(&format!(
        "WHERE (((\"__src\".\"_fld54_rtref\" = decode('00000039', 'hex')) AND (\"__src\".\"_fld54_rrref\" = decode('{hex_id}', 'hex'))) AND (\"__src\".\"_idrref\" = decode('{hex_other}', 'hex')))"
    )));

    let narrow = postgres_compile!(
        &format!(
            "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = 0x{hex_id};"
        ),
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(narrow.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(narrow.message().contains("20-byte"));
    assert_eq!((narrow.line(), narrow.column()), (1, 73));

    let wide = postgres_compile!(
        &format!(
            "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe WHERE Ссылка = 0x00000039{hex_id};"
        ),
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(wide.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(wide.message().contains("16-byte"));

    let binary16 = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe WHERE ProbeAttribute = &B;",
        &[QueryParameter::new(
            "B",
            ParameterValue::Binary(vec![0x11; 16]),
        )],
    )
    .unwrap_err();
    assert_eq!(binary16.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn diagnoses_parameter_binding_failures() {
    let snapshot = snapshot();
    let missing = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Код;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(missing.kind(), QueryDiagnosticKind::Parameter);
    assert_eq!((missing.line(), missing.column()), (1, 61));

    let unused = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe;",
        &[QueryParameter::new("Лишний", ParameterValue::Null)],
    )
    .unwrap_err();
    assert_eq!(unused.kind(), QueryDiagnosticKind::Parameter);
    assert!(unused.message().contains("never referenced"));

    let duplicate = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT &X;",
        &[
            QueryParameter::new("X", ParameterValue::Null),
            QueryParameter::new("x", ParameterValue::Null),
        ],
    )
    .unwrap_err();
    assert_eq!(duplicate.kind(), QueryDiagnosticKind::Parameter);

    let misplaced = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Список;",
        &[QueryParameter::new(
            "Список",
            ParameterValue::List(vec![number(1, 0)]),
        )],
    )
    .unwrap_err();
    assert_eq!(misplaced.kind(), QueryDiagnosticKind::Parameter);
    assert!(misplaced.message().contains("operand of IN"));

    let nested = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (&Список);",
        &[QueryParameter::new(
            "Список",
            ParameterValue::List(vec![ParameterValue::List(Vec::new())]),
        )],
    )
    .unwrap_err();
    assert_eq!(nested.kind(), QueryDiagnosticKind::Parameter);

    let with_null = compile_with_parameters(
        &snapshot,
        PostgresBackend,
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (&Список, \"C\");",
        &[QueryParameter::new(
            "Список",
            ParameterValue::List(vec![
                ParameterValue::String("A".to_owned()),
                ParameterValue::Null,
            ]),
        )],
    )
    .unwrap();
    assert!(
        with_null
            .sql
            .ends_with("WHERE (\"__src\".\"_code\" IN ('A', NULL, 'C'))")
    );

    let prepared = for_each_backend!(
        prepare "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code = &Код;",
        &snapshot,
    );
    let prepared = prepared.0.unwrap();
    assert!(prepared.presentation_request().targets.is_empty());
    let bound = prepared
        .compile_with(
            &snapshot,
            &CompileOptions::new().parameters(&[QueryParameter::new(
                "Код",
                ParameterValue::String("X".to_owned()),
            )]),
        )
        .unwrap();
    assert!(bound.sql.ends_with("WHERE (\"__src\".\"_code\" = 'X')"));
    assert!(matches!(
        prepared.compile(&snapshot, &[]),
        Err(error) if error.kind() == QueryDiagnosticKind::Parameter
    ));
}

#[test]
fn compiles_empty_references_for_reference_kinds() {
    let snapshot = snapshot();
    let probe = snapshot
        .object_id(MetadataKind::Catalog, "OpenSdblMetadataProbe")
        .unwrap();
    let (postgres, mssql) = for_each_backend!(
        "SELECT ЗНАЧЕНИЕ(Справочник.OpenSdblMetadataProbe.ПустаяСсылка) AS Empty
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Ссылка = VALUE(Catalog.OpenSdblMetadataProbe.EmptyRef);",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(
        kinds(&postgres),
        [&ColumnKind::Reference {
            targets: vec![probe],
            runtime_typed: false,
        }]
    );
    let zeros = "00".repeat(16);
    assert!(
        postgres
            .sql
            .contains(&format!("decode('{zeros}', 'hex') AS \"Empty\""))
    );
    assert!(postgres.sql.ends_with(&format!(
        "WHERE (\"__src\".\"_idrref\" = decode('{zeros}', 'hex'))"
    )));
    assert!(
        mssql
            .unwrap()
            .sql
            .ends_with(&format!("WHERE ([__src].[_idrref] = 0x{zeros})"))
    );

    let pair = universal_dereferenced_presentation_snapshot();
    let guarded = postgres_compile!(
        "SELECT Ссылка FROM Документ.бит_ДополнительныеУсловияПоДоговору
         WHERE ДоговорКонтрагента = ЗНАЧЕНИЕ(Справочник.ЦентрыФинансовойОтветственности.ПустаяСсылка);",
        &pair,
    )
    .unwrap();
    assert!(guarded.sql.ends_with(&format!(
        "WHERE ((\"__src\".\"_fld59_rtref\" = decode('0000003e', 'hex')) AND (\"__src\".\"_fld59_rrref\" = decode('{zeros}', 'hex')))"
    )));

    let register = postgres_compile!(
        "SELECT ЗНАЧЕНИЕ(РегистрСведений.Prices.ПустаяСсылка);",
        &information_register_snapshot(),
    )
    .unwrap_err();
    assert_eq!(register.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(register.message().contains("no empty reference"));
}

#[test]
fn accepts_date_parameters_as_virtual_table_periods() {
    let register = information_register_snapshot();
    let parameters = [
        QueryParameter::new("Период", date(2026, 3, 1)),
        QueryParameter::new("Значение", ParameterValue::Binary(vec![0x11; 16])),
    ];
    let compiled = compile_with_parameters(
        &register,
        mssql_backend(2000),
        "SELECT Period FROM InformationRegister.Prices.SliceLast(&Период, ProbeAttribute = &Значение);",
        &parameters,
    )
    .unwrap();
    assert!(
        compiled
            .sql
            .contains("<= DATEADD(year, 2000, CONVERT(datetime2, '2026-03-01T00:00:00', 126))")
    );
    assert!(compiled.sql.contains(&format!("= 0x{}", "11".repeat(16))));

    let wrong_kind = compile_with_parameters(
        &register,
        PostgresBackend,
        "SELECT Period FROM InformationRegister.Prices.SliceLast(&Период);",
        &[QueryParameter::new("Период", number(1, 0))],
    )
    .unwrap_err();
    assert_eq!(wrong_kind.kind(), QueryDiagnosticKind::Parameter);

    let turnovers = compile_with_parameters(
        &accumulation_register_snapshot(),
        PostgresBackend,
        "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(&Начало, &Конец);",
        &[
            QueryParameter::new("Начало", date(2026, 8, 1)),
            QueryParameter::new("Конец", date(2026, 9, 1)),
        ],
    )
    .unwrap();
    assert!(turnovers.sql.contains(">= TIMESTAMP '2026-08-01 00:00:00'"));
    assert!(turnovers.sql.contains("< TIMESTAMP '2026-09-01 00:00:00'"));
}

#[test]
fn compiles_grouped_branches_with_having_and_ordering() {
    let snapshot = boolean_snapshot();
    let (postgres, mssql) = for_each_backend!(
        "SELECT Code, COUNT(*) AS N, SUM(CASE WHEN Fld77 THEN 1 ELSE 0 END) AS Flags,
                CASE WHEN COUNT(*) > 1 THEN \"many\" ELSE \"one\" END AS Label
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Fld77
         GROUP BY Code
         HAVING COUNT(*) > 1 AND MAX(Date) > DATETIME(2024, 1, 1)
         ORDER BY N DESC, Code;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["Code", "N", "Flags", "Label"]);
    assert!(postgres.sql.contains(
        "WHERE \"__src\".\"_fld77\" GROUP BY \"__src\".\"_code\" HAVING ((COUNT(*) > 1) AND (MAX(\"__src\".\"_date_time\") > TIMESTAMP '2024-01-01 00:00:00')) ORDER BY 2 DESC, 1 ASC"
    ));
    assert!(
        postgres
            .sql
            .contains("CASE WHEN (COUNT(*) > 1) THEN 'many' ELSE 'one' END AS \"Label\"")
    );
    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "WHERE ([__src].[_fld77] = 0x01) GROUP BY [__src].[_code] HAVING ((COUNT(*) > 1) AND (MAX([__src].[_date_time]) > CONVERT(datetime2, '2024-01-01T00:00:00', 126))) ORDER BY 2 DESC, 1 ASC"
    ));

    let only_having = postgres_compile!(
        "SELECT COUNT(*) FROM Catalog.OpenSdblMetadataProbe HAVING COUNT(*) > 0;",
        &snapshot,
    )
    .unwrap();
    assert!(only_having.sql.ends_with("HAVING (COUNT(*) > 0)"));
    assert!(!only_having.sql.contains("GROUP BY"));
}

#[test]
fn groups_reference_keys_by_every_physical_member() {
    let snapshot = presentation_reference_snapshot(true);
    let compiled = postgres_compile!(
        "SELECT ProbeAttribute, COUNT(*) AS N FROM Catalog.OpenSdblMetadataProbe GROUP BY ProbeAttribute;",
        &snapshot,
    )
    .unwrap();
    assert!(compiled.sql.contains(
        "(\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") AS \"ProbeAttribute\""
    ));
    assert!(
        compiled
            .sql
            .ends_with("GROUP BY \"__src\".\"_fld54_rtref\", \"__src\".\"_fld54_rrref\"")
    );
    assert!(matches!(
        &compiled.columns[0].kind,
        ColumnKind::Reference {
            runtime_typed: true,
            ..
        }
    ));
}

#[test]
fn groups_by_dereferenced_keys_aliases_and_expressions() {
    let reference = reference_snapshot();
    let dereferenced = postgres_compile!(
        "SELECT Организация.Код AS Код, ПРЕДСТАВЛЕНИЕ(Организация.Код) AS Текст, COUNT(*) AS N
         FROM Catalog.OpenSdblMetadataProbe
         GROUP BY Организация.Код;",
        &reference,
    )
    .unwrap();
    assert_eq!(dereferenced.sql.matches(" LEFT JOIN ").count(), 1);
    assert!(
        dereferenced
            .sql
            .ends_with("GROUP BY \"__ref1\".\"_code\", (\"__ref1\".\"_code\")::text")
    );

    let snapshot = snapshot();
    let by_alias = postgres_compile!(
        "SELECT НАЧАЛОПЕРИОДА(Date, МЕСЯЦ) AS Месяц, COUNT(*) AS N
         FROM Catalog.OpenSdblMetadataProbe GROUP BY Месяц ORDER BY Месяц;",
        &snapshot,
    )
    .unwrap();
    assert!(
        by_alias
            .sql
            .ends_with("GROUP BY date_trunc('month', \"__src\".\"_date_time\") ORDER BY 1 ASC")
    );

    let by_expression = postgres_compile!(
        "SELECT НАЧАЛОПЕРИОДА(Date, МЕСЯЦ) AS Месяц, COUNT(*) AS N
         FROM Catalog.OpenSdblMetadataProbe GROUP BY beginofperiod(date, month);",
        &snapshot,
    )
    .unwrap();
    assert!(
        by_expression
            .sql
            .ends_with("GROUP BY date_trunc('month', \"__src\".\"_date_time\")")
    );

    let unprojected_key = postgres_compile!(
        "SELECT COUNT(*) AS N FROM Catalog.OpenSdblMetadataProbe GROUP BY Code, Date;",
        &snapshot,
    )
    .unwrap();
    assert!(
        unprojected_key
            .sql
            .ends_with("GROUP BY \"__src\".\"_code\", \"__src\".\"_date_time\"")
    );

    let joined = postgres_compile!(
        "SELECT l.Code, COUNT(*) AS N
         FROM Catalog.OpenSdblMetadataProbe l INNER JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code
         GROUP BY l.Code;",
        &snapshot,
    )
    .unwrap();
    assert!(joined.sql.ends_with("GROUP BY \"l\".\"_code\""));

    let unioned = postgres_compile!(
        "SELECT Code, COUNT(*) AS N FROM Catalog.OpenSdblMetadataProbe GROUP BY Code
         UNION ALL SELECT Code, COUNT(*) FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(unioned.sql.matches("GROUP BY").count(), 2);
}

#[test]
fn diagnoses_invalid_grouping() {
    let snapshot = snapshot();
    let ungrouped = postgres_compile!(
        "SELECT Code, Date, COUNT(*) FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(ungrouped.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert_eq!((ungrouped.line(), ungrouped.column()), (1, 14));
    assert!(ungrouped.message().contains("grouped or aggregated"));

    let in_where = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE COUNT(*) > 1 GROUP BY Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(in_where.message().contains("only as projections"));

    let aggregate_key = postgres_compile!(
        "SELECT Code, COUNT(*) AS N FROM Catalog.OpenSdblMetadataProbe GROUP BY Code, N;",
        &snapshot,
    )
    .unwrap_err();
    assert!(aggregate_key.message().contains("aggregate projection"));

    let nested = postgres_compile!(
        "SELECT Code, SUM(COUNT(*)) FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(nested.message().contains("only as projections"));

    let wildcard = postgres_compile!(
        "SELECT * FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("GROUP BY"));

    let source_free = postgres_compile!("SELECT 1 GROUP BY 1;", &snapshot).unwrap_err();
    assert!(source_free.message().contains("requires FROM"));

    let full = postgres_compile!(
        "SELECT l.Code, COUNT(*) FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code GROUP BY l.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(full.message().contains("FULL JOIN"));

    let unknown_key = postgres_compile!(
        "SELECT COUNT(*) FROM Catalog.OpenSdblMetadataProbe GROUP BY Nothing;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(unknown_key.kind(), QueryDiagnosticKind::UnknownField);

    let mixed = postgres_compile!(
        "SELECT Code, CASE WHEN COUNT(*) > 1 THEN 1 ELSE 0 END FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap_err();
    assert!(mixed.message().contains("without GROUP BY"));
}

#[test]
fn chains_several_joins_in_source_order() {
    let snapshot = tabular_section_snapshot();
    let source = "SELECT Док.Ссылка AS Документ, строки.НомерСтроки AS Строка, цфо.Сам_БизнесРегион AS Регион
         FROM Документ.бит_ДополнительныеУсловияПоДоговору КАК Док
         INNER JOIN Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений КАК строки
             ON строки.Ссылка = Док.Ссылка
         LEFT JOIN Справочник.ЦентрыФинансовойОтветственности КАК цфо
             ON цфо.Ссылка = строки.ЦФО AND Док.Ссылка ЕСТЬ НЕ NULL
         WHERE строки.НомерСтроки > 0
         ORDER BY Строка DESC;";
    let (postgres, mssql) = for_each_backend!(source, &snapshot);
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["Документ", "Строка", "Регион"]);
    assert!(postgres.sql.contains(
        "FROM \"_document53\" AS \"Док\" INNER JOIN \"_document53_vt54X1\" AS \"строки\" ON \"строки\".\"_document53_idrref\" = \"Док\".\"_idrref\" LEFT JOIN \"_reference62\" AS \"цфо\" ON \"цфо\".\"_idrref\" = \"строки\".\"_fld55\" AND (\"Док\".\"_idrref\" IS NOT NULL) WHERE (\"строки\".\"_lineno54\" > 0) ORDER BY 2 DESC"
    ));
    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "FROM [_document53] AS [Док] INNER JOIN [_document53_vt54X1] AS [строки] ON [строки].[_document53_idrref] = [Док].[_idrref] LEFT JOIN [_reference62] AS [цфо] ON [цфо].[_idrref] = [строки].[_fld55] AND ([Док].[_idrref] IS NOT NULL) WHERE ([строки].[_lineno54] > 0) ORDER BY 2 DESC"
    ));
}

#[test]
fn joins_the_same_object_under_several_aliases_with_dereference() {
    let snapshot = reference_snapshot();
    let compiled = postgres_compile!(
        "SELECT a.Code, b.Code AS Second, c.Организация.Код AS Owner
         FROM Catalog.OpenSdblMetadataProbe a
         RIGHT JOIN Catalog.OpenSdblMetadataProbe b ON a.Code = b.Code
         LEFT JOIN Catalog.OpenSdblMetadataProbe c ON c.Code = a.Code AND c.Code = b.Code
         ORDER BY Second;",
        &snapshot,
    )
    .unwrap();
    assert!(compiled.sql.contains(
        "FROM \"_reference53\" AS \"a\" RIGHT JOIN \"_reference53\" AS \"b\" ON \"a\".\"_code\" = \"b\".\"_code\" LEFT JOIN \"_reference53\" AS \"c\" ON \"c\".\"_code\" = \"a\".\"_code\" AND \"c\".\"_code\" = \"b\".\"_code\" LEFT JOIN \"_reference57\" AS \"__join3_ref1\" ON \"c\".\"_fld54\" = \"__join3_ref1\".\"_idrref\""
    ));
    assert!(
        compiled
            .sql
            .contains("\"__join3_ref1\".\"_code\"::text AS \"Owner\"")
    );
    assert!(compiled.sql.ends_with("ORDER BY 2 ASC"));

    let unaliased = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe
         JOIN Catalog.Организации ON OpenSdblMetadataProbe.Code = Организации.Code
         JOIN Catalog.Организации о ON о.Code = Организации.Code;",
        &snapshot,
    );
    assert_eq!(
        unaliased.unwrap_err().kind(),
        QueryDiagnosticKind::AmbiguousField
    );

    let defaults = postgres_compile!(
        "SELECT бит_ДополнительныеУсловияПоДоговору.Ссылка
         FROM Документ.бит_ДополнительныеУсловияПоДоговору
         JOIN РегистрСведений.бит_СтатусыОбъектов
             ON бит_СтатусыОбъектов.Объект = бит_ДополнительныеУсловияПоДоговору.Ссылка
         JOIN Справочник.ЦентрыФинансовойОтветственности
             ON ЦентрыФинансовойОтветственности.Ссылка = бит_ДополнительныеУсловияПоДоговору.ДоговорКонтрагента;",
        &tabular_section_snapshot(),
    )
    .unwrap();
    assert!(
        defaults
            .sql
            .contains("AS \"__left\" INNER JOIN \"_inforg60\" AS \"__right\" ON")
    );
    assert!(defaults.sql.contains(
        "INNER JOIN \"_reference62\" AS \"__join3\" ON \"__join3\".\"_idrref\" = \"__left\".\"_fld59\""
    ));
}

#[test]
fn diagnoses_invalid_join_chains() {
    let snapshot = snapshot();
    let forward = postgres_compile!(
        "SELECT a.Code FROM Catalog.OpenSdblMetadataProbe a
         JOIN Catalog.OpenSdblMetadataProbe b ON a.Code = c.Code
         JOIN Catalog.OpenSdblMetadataProbe c ON c.Code = a.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(forward.kind(), QueryDiagnosticKind::UnsupportedFeature);
    assert!(forward.message().contains("joined later"));
    assert_eq!((forward.line(), forward.column()), (2, 61));

    let no_anchor = postgres_compile!(
        "SELECT a.Code FROM Catalog.OpenSdblMetadataProbe a
         JOIN Catalog.OpenSdblMetadataProbe b ON a.Code = b.Code
         JOIN Catalog.OpenSdblMetadataProbe c ON a.Code = b.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(no_anchor.message().contains("earlier source"));

    let full = postgres_compile!(
        "SELECT a.Code FROM Catalog.OpenSdblMetadataProbe a
         FULL JOIN Catalog.OpenSdblMetadataProbe b ON a.Code = b.Code
         JOIN Catalog.OpenSdblMetadataProbe c ON c.Code = a.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(full.message().contains("only join"));
    assert_eq!((full.line(), full.column()), (2, 10));

    let duplicate_alias = postgres_compile!(
        "SELECT a.Code FROM Catalog.OpenSdblMetadataProbe a
         JOIN Catalog.OpenSdblMetadataProbe b ON a.Code = b.Code
         JOIN Catalog.OpenSdblMetadataProbe a ON a.Code = b.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(duplicate_alias.message().contains("distinct aliases"));
}

#[test]
fn compiles_nested_sources_with_grouping_joins_and_dereference() {
    let snapshot = snapshot();
    let (postgres, mssql) = for_each_backend!(
        "SELECT Т.К AS Код, Т.N AS Сумма, c.Date
         FROM (SELECT Code AS К, COUNT(*) AS N FROM Catalog.OpenSdblMetadataProbe GROUP BY Code) AS Т
         INNER JOIN Catalog.OpenSdblMetadataProbe c ON c.Code = Т.К
         WHERE Т.N > 1
         ORDER BY Сумма DESC;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert_eq!(labels(&postgres), ["Код", "Сумма", "Date"]);
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::String { length: Some(9) },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
            &ColumnKind::DateTime,
        ]
    );
    assert!(postgres.sql.contains(
        "FROM (SELECT \"__src\".\"_code\"::text AS \"К\", COUNT(*) AS \"N\" FROM \"_reference53\" AS \"__src\" GROUP BY \"__src\".\"_code\") AS \"Т\" INNER JOIN \"_reference53\" AS \"c\" ON \"c\".\"_code\" = \"Т\".\"К\" WHERE (\"Т\".\"N\" > 1) ORDER BY 2 DESC"
    ));
    assert!(
        postgres
            .sql
            .starts_with("SELECT \"Т\".\"К\" AS \"Код\", \"Т\".\"N\" AS \"Сумма\"")
    );
    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "FROM (SELECT [__src].[_code] AS [К], COUNT(*) AS [N] FROM [_reference53] AS [__src] GROUP BY [__src].[_code]) AS [Т] INNER JOIN"
    ));

    let dereferenced = postgres_compile!(
        "SELECT Т.ID.Code AS Код, Т.ID AS Ссылка
         FROM (SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe) КАК Т;",
        &snapshot,
    )
    .unwrap();
    assert!(dereferenced.sql.contains(
        "FROM (SELECT \"__src\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"__src\") AS \"Т\" LEFT JOIN \"_reference53\" AS \"__ref1\" ON \"Т\".\"ID\" = \"__ref1\".\"_idrref\""
    ));
    assert!(
        dereferenced
            .sql
            .contains("\"__ref1\".\"_code\"::text AS \"Код\"")
    );
    assert!(matches!(
        &dereferenced.columns[1].kind,
        ColumnKind::Reference {
            runtime_typed: false,
            ..
        }
    ));

    let first_n = mssql_compile_with_offset!(
        "SELECT Т.Date, Т.Текст FROM (SELECT TOP 10 Date, ПРЕДСТАВЛЕНИЕ(Code) AS Текст FROM Catalog.OpenSdblMetadataProbe ORDER BY Date DESC) AS Т;",
        &snapshot,
        2000
    )
    .unwrap();
    assert!(first_n.sql.contains(
        "FROM (SELECT TOP (10) [__src].[_date_time] AS [Date], CONVERT(nvarchar(max), [__src].[_code]) AS [Текст] FROM [_reference53] AS [__src] ORDER BY [__src].[_date_time] DESC) AS [Т]"
    ));
    assert!(
        first_n.sql.starts_with(
            "SELECT DATEADD(year, -2000, [Т].[Date]) AS [Date], [Т].[Текст] AS [Текст]"
        )
    );
    assert_eq!(first_n.sql.matches("DATEADD").count(), 1);

    let payload = postgres_compile!(
        "SELECT ПРЕДСТАВЛЕНИЕССЫЛКИ(Т.ProbeAttribute) AS Текст, Т.ProbeAttribute
         FROM (SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe) AS Т;",
        &presentation_reference_snapshot(true),
    )
    .unwrap();
    assert_eq!(payload.deferred_presentations, [0]);
    assert!(
        payload
            .sql
            .starts_with("SELECT \"Т\".\"ProbeAttribute\" AS \"Текст\"")
    );
}

#[test]
fn compiles_in_subqueries_with_reference_widening() {
    let snapshot = snapshot();
    let scalar = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe
         WHERE Code NOT IN (SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Date IS NULL)
           AND Ссылка В (SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe)
           AND Code НЕ В (\"1\", \"2\");",
        &snapshot,
    )
    .unwrap();
    assert!(postgres_sql_ends_with(
        &scalar.sql,
        "WHERE (((NOT (\"__src\".\"_code\" IN (SELECT \"__src\".\"_code\"::text AS \"Code\" FROM \"_reference53\" AS \"__src\" WHERE (\"__src\".\"_date_time\" IS NULL)))) AND (\"__src\".\"_idrref\" IN (SELECT \"__src\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"__src\"))) AND (NOT (\"__src\".\"_code\" IN ('1', '2'))))"
    ));

    let pairs = presentation_reference_snapshot(true);
    let widened_inner = postgres_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         WHERE ProbeAttribute IN (SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe);",
        &pairs,
    )
    .unwrap();
    assert!(widened_inner.sql.ends_with(
        "WHERE ((\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") IN (SELECT (decode('00000035', 'hex') || \"__in\".\"ID\") FROM (SELECT \"__src\".\"_idrref\" AS \"ID\" FROM \"_reference53\" AS \"__src\") AS \"__in\"))"
    ));

    let widened_outer = postgres_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         WHERE Ссылка IN (SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe);",
        &pairs,
    )
    .unwrap();
    assert!(widened_outer.sql.ends_with(
        "WHERE ((decode('00000035', 'hex') || \"__src\".\"_idrref\") IN (SELECT (\"__src\".\"_fld54_rtref\" || \"__src\".\"_fld54_rrref\") AS \"ProbeAttribute\" FROM \"_reference53\" AS \"__src\"))"
    ));

    let both_payload = mssql_compile!(
        "SELECT Ссылка FROM Catalog.OpenSdblMetadataProbe
         WHERE ProbeAttribute NOT IN (SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe);",
        &pairs,
    )
    .unwrap();
    assert!(both_payload.sql.ends_with(
        "WHERE (NOT (([__src].[_fld54_rtref] + [__src].[_fld54_rrref]) IN (SELECT ([__src].[_fld54_rtref] + [__src].[_fld54_rrref]) AS [ProbeAttribute] FROM [_reference53] AS [__src])))"
    ));
}

fn postgres_sql_ends_with(sql: &str, suffix: &str) -> bool {
    sql.ends_with(suffix)
}

#[test]
fn diagnoses_invalid_nested_queries() {
    let snapshot = snapshot();
    let correlated = postgres_compile!(
        "SELECT o.Code FROM Catalog.OpenSdblMetadataProbe o
         WHERE o.Code IN (SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Date = o.Date);",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(correlated.kind(), QueryDiagnosticKind::UnknownField);
    assert_eq!(correlated.line(), 2);

    let two_columns = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (SELECT Code, Date FROM Catalog.OpenSdblMetadataProbe);",
        &snapshot,
    )
    .unwrap_err();
    assert!(two_columns.message().contains("exactly one column"));

    let mismatch = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (SELECT Date FROM Catalog.OpenSdblMetadataProbe);",
        &snapshot,
    )
    .unwrap_err();
    assert!(mismatch.message().contains("not compatible"));

    let ordered = postgres_compile!(
        "SELECT Т.Code FROM (SELECT Code FROM Catalog.OpenSdblMetadataProbe ORDER BY Code) AS Т;",
        &snapshot,
    )
    .unwrap_err();
    assert!(ordered.message().contains("requires TOP"));

    let wildcard = postgres_compile!(
        "SELECT Т.Code FROM (SELECT * FROM Catalog.OpenSdblMetadataProbe) AS Т;",
        &snapshot,
    )
    .unwrap_err();
    assert!(wildcard.message().contains("'*'"));

    let unaliased = postgres_compile!(
        "SELECT Code FROM (SELECT Code FROM Catalog.OpenSdblMetadataProbe);",
        &snapshot,
    )
    .unwrap_err();
    assert_eq!(unaliased.kind(), QueryDiagnosticKind::Syntax);

    let deferred = postgres_compile!(
        "SELECT Т.Текст FROM (SELECT ПРЕДСТАВЛЕНИЕССЫЛКИ(ДоговорКонтрагента) AS Текст FROM Документ.бит_ДополнительныеУсловияПоДоговору) AS Т;",
        &universal_dereferenced_presentation_snapshot(),
    )
    .unwrap_err();
    assert!(deferred.message().contains("deferred"));

    let runtime_typed = postgres_compile!(
        "SELECT Т.ProbeAttribute.Code FROM (SELECT ProbeAttribute FROM Catalog.OpenSdblMetadataProbe) AS Т;",
        &presentation_reference_snapshot(true),
    )
    .unwrap_err();
    assert_eq!(runtime_typed.kind(), QueryDiagnosticKind::Metadata);

    let mut deep = String::from("SELECT Code FROM Catalog.OpenSdblMetadataProbe");
    for level in 0..17 {
        deep = format!("SELECT Code FROM ({deep}) AS T{level}");
    }
    let too_deep = postgres_compile!(&format!("{deep};"), &snapshot).unwrap_err();
    assert_eq!(too_deep.kind(), QueryDiagnosticKind::TooDeep);
    assert!(too_deep.message().contains("nested query depth"));

    let mut ok = String::from("SELECT Code FROM Catalog.OpenSdblMetadataProbe");
    for level in 0..16 {
        ok = format!("SELECT Code FROM ({ok}) AS T{level}");
    }
    assert!(postgres_compile!(&format!("{ok};"), &snapshot).is_ok());
}

use open_sdbl::query::{TempTable, TempTablesManager};

/// Compiles batches on both backends against separate managers and checks
/// that the backends stay in step statement by statement.
struct BatchSession {
    postgres: TempTablesManager,
    mssql: TempTablesManager,
}

type BatchOutcome =
    Result<Option<open_sdbl::query::CompiledQuery>, open_sdbl::query::QueryDiagnostic>;

impl BatchSession {
    fn new() -> Self {
        Self {
            postgres: TempTablesManager::new(),
            mssql: TempTablesManager::new(),
        }
    }

    fn compile(
        &mut self,
        snapshot: &MetadataSnapshot,
        source: &str,
    ) -> (BatchOutcome, BatchOutcome) {
        self.compile_with(snapshot, source, &CompileOptions::new())
    }

    fn compile_with(
        &mut self,
        snapshot: &MetadataSnapshot,
        source: &str,
        options: &CompileOptions<'_>,
    ) -> (BatchOutcome, BatchOutcome) {
        let postgres = QueryCompiler::new(snapshot, PostgresBackend).compile_batch(
            source,
            options,
            &mut self.postgres,
        );
        let mssql = QueryCompiler::new(snapshot, mssql_backend(0)).compile_batch(
            source,
            options,
            &mut self.mssql,
        );
        assert_error_outcomes_match(source, &postgres, &mssql);
        if let (Ok(postgres), Ok(mssql)) = (&postgres, &mssql) {
            assert_eq!(
                postgres.as_ref().map(|query| labels(query)),
                mssql.as_ref().map(|query| labels(query)),
                "backend batch labels differ for {source}"
            );
        }
        (postgres, mssql)
    }

    fn tables(&self) -> Vec<TempTable<'_>> {
        self.postgres.tables().collect()
    }
}

fn postgres_batch(session: &mut BatchSession, snapshot: &MetadataSnapshot, source: &str) -> String {
    session
        .compile(snapshot, source)
        .0
        .unwrap()
        .expect("batch produces a statement")
        .sql
}

#[test]
fn compiles_temporary_tables_as_common_table_expressions() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();

    let placed = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК К, КОЛИЧЕСТВО(*) КАК N ПОМЕСТИТЬ Обороты
             ИЗ Catalog.OpenSdblMetadataProbe СГРУППИРОВАТЬ ПО Code;",
        )
        .0
        .unwrap()
        .expect("a placement statement returns its row count");
    assert_eq!(labels(&placed), ["Количество"]);
    assert_eq!(
        kinds(&placed),
        [&ColumnKind::Number {
            precision: None,
            scale: None,
        }]
    );
    assert_eq!(
        placed.sql,
        "WITH \"vt1\" AS (SELECT \"__src\".\"_code\"::text AS \"К\", COUNT(*) AS \"N\" FROM \"_reference53\" AS \"__src\" GROUP BY \"__src\".\"_code\") SELECT COUNT(*) AS \"Количество\" FROM \"vt1\" AS \"__placed\""
    );

    let (postgres, mssql) = session.compile(
        &snapshot,
        "ВЫБРАТЬ Т.К КАК Код, Т.N КАК Итог ИЗ Обороты КАК Т ГДЕ Т.N > 1;",
    );
    let postgres = postgres.unwrap().expect("a query statement returns rows");
    assert_eq!(labels(&postgres), ["Код", "Итог"]);
    assert_eq!(
        kinds(&postgres),
        [
            &ColumnKind::String { length: Some(9) },
            &ColumnKind::Number {
                precision: None,
                scale: None,
            },
        ]
    );
    assert!(postgres.sql.starts_with(
        "WITH \"vt1\" AS (SELECT \"__src\".\"_code\"::text AS \"К\", COUNT(*) AS \"N\" FROM \"_reference53\" AS \"__src\" GROUP BY \"__src\".\"_code\") SELECT \"Т\".\"К\" AS \"Код\""
    ));
    assert!(
        postgres
            .sql
            .ends_with("FROM \"vt1\" AS \"Т\" WHERE (\"Т\".\"N\" > 1)")
    );
    let mssql = mssql.unwrap().unwrap();
    assert!(
        mssql
            .sql
            .starts_with("WITH [vt1] AS (SELECT [__src].[_code] AS [К], COUNT(*) AS [N]")
    );
    assert!(mssql.sql.ends_with("FROM [vt1] AS [Т] WHERE ([Т].[N] > 1)"));

    let joined = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Т.Итог ИЗ (ВЫБРАТЬ Х.N КАК Итог ИЗ Обороты КАК Х) КАК Т
         ВНУТРЕННЕЕ СОЕДИНЕНИЕ Catalog.OpenSdblMetadataProbe КАК c ПО c.Code = Т.Итог;",
    );
    assert!(joined.contains("FROM (SELECT \"Х\".\"N\" AS \"Итог\" FROM \"vt1\" AS \"Х\") AS \"Т\" INNER JOIN \"_reference53\" AS \"c\""));

    // The table name qualifies its own fields when no alias is given.
    let unaliased = postgres_batch(&mut session, &snapshot, "ВЫБРАТЬ Обороты.К ИЗ Обороты;");
    assert!(unaliased.ends_with("SELECT \"Обороты\".\"К\" AS \"К\" FROM \"vt1\" AS \"Обороты\""));

    // A membership subquery reads the same CTE.
    let membership = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Code ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code В (ВЫБРАТЬ Т.К ИЗ Обороты КАК Т);",
    );
    assert!(membership.contains("IN (SELECT \"Т\".\"К\" AS \"К\" FROM \"vt1\" AS \"Т\")"));
}

#[test]
fn emits_only_the_temporary_tables_a_statement_reaches() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    let sql = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ Первая ИЗ Catalog.OpenSdblMetadataProbe;
         ВЫБРАТЬ Date КАК Д ПОМЕСТИТЬ Вторая ИЗ Catalog.OpenSdblMetadataProbe;
         ВЫБРАТЬ Т.Д ИЗ Вторая КАК Т;",
    );
    assert!(sql.starts_with("WITH \"vt2\" AS ("));
    assert_eq!(sql.matches(" AS (").count(), 1);
    assert!(!sql.contains("vt1"));
    assert_eq!(
        session
            .tables()
            .iter()
            .map(TempTable::name)
            .collect::<Vec<_>>(),
        ["Первая", "Вторая"]
    );

    // A chain keeps the CTEs it reads, in definition order.
    let chained = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Т.Имя КАК Имя ПОМЕСТИТЬ Третья ИЗ Первая КАК Т;
         ВЫБРАТЬ Х.Имя ИЗ Третья КАК Х;",
    );
    assert!(chained.starts_with("WITH \"vt1\" AS ("));
    assert!(
        chained.contains(", \"vt3\" AS (SELECT \"Т\".\"Имя\" AS \"Имя\" FROM \"vt1\" AS \"Т\")")
    );
    assert!(!chained.contains("vt2"));
}

#[test]
fn appends_rows_through_a_union_all_definition() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Наименование ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap();

    let appended = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Другое ДОБАВИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = \"A\";",
        )
        .0
        .unwrap()
        .expect("an append statement returns its row count");
    assert_eq!(labels(&appended), ["Количество"]);
    // Only the appended rows are counted, so the base table is not read.
    assert!(!appended.sql.contains("vt2"));
    assert!(appended.sql.contains(
        "SELECT COUNT(*) AS \"Количество\" FROM (SELECT \"__src\".\"_code\"::text AS \"Другое\""
    ));

    let read = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Т.Наименование ИЗ ВТ КАК Т;",
    );
    assert!(read.contains(
        ", \"vt2\" AS (SELECT \"Наименование\" FROM \"vt1\" UNION ALL SELECT \"__src\".\"_code\"::text AS \"Другое\""
    ));
    assert!(read.ends_with("FROM \"vt2\" AS \"Т\""));

    // A second append chains onto the previous definition.
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Третье ДОБАВИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = \"B\";",
        )
        .0
        .unwrap();
    let chained = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Т.Наименование ИЗ ВТ КАК Т;",
    );
    assert!(chained.contains("\"vt3\" AS (SELECT \"Наименование\" FROM \"vt2\" UNION ALL"));
    assert!(chained.ends_with("FROM \"vt3\" AS \"Т\""));
    assert_eq!(chained.matches(" AS (").count(), 3);

    // The table keeps the labels of its first definition.
    assert_eq!(
        session.tables()[0]
            .columns()
            .iter()
            .map(|column| column.label.as_str())
            .collect::<Vec<_>>(),
        ["Наименование"]
    );
}

#[test]
fn drops_and_redefines_temporary_tables() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap();

    let dropped = session.compile(&snapshot, "УНИЧТОЖИТЬ ВТ;").0.unwrap();
    assert!(dropped.is_none(), "a drop statement produces no SQL");
    assert!(session.tables().is_empty());

    let missing = session
        .compile(&snapshot, "ВЫБРАТЬ Т.Имя ИЗ ВТ КАК Т;")
        .0
        .unwrap_err();
    assert_eq!(missing.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(missing.message().contains("does not exist"));

    let redefined = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Date КАК Д ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;
         ВЫБРАТЬ Т.Д ИЗ ВТ КАК Т;",
    );
    assert!(redefined.starts_with("WITH \"vt2\" AS (SELECT \"__src\".\"_date_time\" AS \"Д\""));
    assert_eq!(session.tables().len(), 1);
}

#[test]
fn accepts_index_clauses_without_generating_indexes() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    let plain = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Code КАК Код, Date КАК Д ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe
         ИНДЕКСИРОВАТЬ ПО Код, Д УНИКАЛЬНО;",
    );
    assert!(!plain.to_uppercase().contains("INDEX"));

    let sets = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ Code КАК Код, Date КАК Д ПОМЕСТИТЬ ВТ2 ИЗ Catalog.OpenSdblMetadataProbe
         ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код, Д) УНИКАЛЬНО, (Д));",
    );
    assert!(!sets.to_uppercase().contains("INDEX"));

    let ordered = postgres_batch(
        &mut session,
        &snapshot,
        "ВЫБРАТЬ ПЕРВЫЕ 5 Code КАК Код ПОМЕСТИТЬ ВТ3 ИЗ Catalog.OpenSdblMetadataProbe
         УПОРЯДОЧИТЬ ПО Код ИНДЕКСИРОВАТЬ ПО Код;",
    );
    assert!(ordered.contains("LIMIT 5"));

    let unknown_field = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Код ПОМЕСТИТЬ ВТ4 ИЗ Catalog.OpenSdblMetadataProbe
             ИНДЕКСИРОВАТЬ ПО Date;",
        )
        .0
        .unwrap_err();
    assert_eq!(unknown_field.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(
        unknown_field
            .message()
            .contains("is not in the selection list")
    );
    assert_eq!(unknown_field.line(), 2);
}

#[test]
fn keeps_temporary_tables_across_batches_and_failures() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    let parameters = [QueryParameter::new(
        "Код",
        ParameterValue::String("A".to_owned()),
    )];
    session
        .compile_with(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe ГДЕ Code = &Код;",
            &CompileOptions::new().parameters(&parameters),
        )
        .0
        .unwrap();

    // The definition froze the parameter, so reading it needs no values.
    let read = postgres_batch(&mut session, &snapshot, "ВЫБРАТЬ Т.Имя ИЗ ВТ КАК Т;");
    assert!(read.contains("WHERE (\"__src\".\"_code\" = 'A')"));

    let before = session
        .tables()
        .iter()
        .map(|table| table.name().to_owned())
        .collect::<Vec<_>>();
    let failed = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ Другая ИЗ Catalog.OpenSdblMetadataProbe;
             ВЫБРАТЬ Т.Нет ИЗ Другая КАК Т;",
        )
        .0
        .unwrap_err();
    assert_eq!(failed.kind(), QueryDiagnosticKind::UnknownField);
    assert_eq!(
        session
            .tables()
            .iter()
            .map(|table| table.name().to_owned())
            .collect::<Vec<_>>(),
        before,
        "a failed batch must not change the manager"
    );
}

#[test]
fn compiles_temporary_table_references_and_dates() {
    let snapshot = snapshot();
    let mut manager = TempTablesManager::new();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);
    compiler
        .compile_batch(
            "ВЫБРАТЬ Ссылка КАК Ссылка ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap();
    assert!(matches!(
        manager.tables().next().unwrap().columns()[0].kind,
        ColumnKind::Reference {
            runtime_typed: false,
            ..
        }
    ));

    let source = "ВЫБРАТЬ Т.Ссылка.Code КАК Код ИЗ ВТ КАК Т;";
    let prepared = compiler.prepare_with(source, &manager).unwrap();
    assert!(prepared.presentation_request().targets.is_empty());
    let dereferenced = prepared
        .compile_batch(&snapshot, &CompileOptions::new(), &mut manager)
        .unwrap()
        .unwrap();
    assert!(dereferenced.sql.contains(
        "FROM \"vt1\" AS \"Т\" LEFT JOIN \"_reference53\" AS \"__ref1\" ON \"Т\".\"Ссылка\" = \"__ref1\".\"_idrref\""
    ));

    // A runtime-typed column read from a temporary table is presented by
    // the application, exactly as a derived-source payload column is.
    let payload_snapshot = presentation_reference_snapshot(true);
    let payload_compiler = QueryCompiler::new(&payload_snapshot, PostgresBackend);
    let mut payload_manager = TempTablesManager::new();
    payload_compiler
        .compile_batch(
            "ВЫБРАТЬ ProbeAttribute КАК Объект ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut payload_manager,
        )
        .unwrap();
    let payload = payload_compiler
        .compile_batch(
            "ВЫБРАТЬ ПРЕДСТАВЛЕНИЕССЫЛКИ(Т.Объект) КАК Текст ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut payload_manager,
        )
        .unwrap()
        .unwrap();
    assert_eq!(payload.deferred_presentations, [0]);
    assert!(
        payload
            .sql
            .contains("SELECT \"Т\".\"Объект\" AS \"Текст\" FROM \"vt1\" AS \"Т\"")
    );

    // MSSQL corrects the year offset once, in the final projection only.
    let mssql = QueryCompiler::new(&snapshot, mssql_backend(2000));
    let mut offset_manager = TempTablesManager::new();
    mssql
        .compile_batch(
            "ВЫБРАТЬ Date КАК Д ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut offset_manager,
        )
        .unwrap();
    let dated = mssql
        .compile_batch(
            "ВЫБРАТЬ Т.Д ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut offset_manager,
        )
        .unwrap()
        .unwrap();
    assert_eq!(dated.sql.matches("DATEADD").count(), 1);
    assert!(
        dated
            .sql
            .contains("SELECT DATEADD(year, -2000, [Т].[Д]) AS [Д] FROM [vt1] AS [Т]")
    );
}

#[test]
fn diagnoses_temporary_table_failures() {
    let snapshot = snapshot();
    let mut session = BatchSession::new();
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap();

    let duplicate = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert_eq!(duplicate.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(duplicate.message().contains("already exists"));

    let widths = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя, Date КАК Д ДОБАВИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert_eq!(widths.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(widths.message().contains("projects 2 columns"));

    let kind_mismatch = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Date КАК Имя ДОБАВИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert_eq!(kind_mismatch.kind(), QueryDiagnosticKind::TemporaryTable);

    let missing_append = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ДОБАВИТЬ Нет ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert_eq!(missing_append.kind(), QueryDiagnosticKind::TemporaryTable);

    let missing_drop = session.compile(&snapshot, "УНИЧТОЖИТЬ Нет;").0.unwrap_err();
    assert_eq!(missing_drop.kind(), QueryDiagnosticKind::TemporaryTable);

    let wildcard = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ * ПОМЕСТИТЬ ВТ5 ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert!(wildcard.message().contains("'*'"));

    let ordered = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ6 ИЗ Catalog.OpenSdblMetadataProbe УПОРЯДОЧИТЬ ПО Имя;",
        )
        .0
        .unwrap_err();
    assert!(ordered.message().contains("requires TOP"));

    let nested_into = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Т.Имя ИЗ (ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ7 ИЗ Catalog.OpenSdblMetadataProbe) КАК Т;",
        )
        .0
        .unwrap_err();
    assert_eq!(nested_into.kind(), QueryDiagnosticKind::Syntax);
    assert!(nested_into.message().contains("INTO is not allowed"));

    let union_into = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Code КАК Имя ИЗ Catalog.OpenSdblMetadataProbe
             ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ8 ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap_err();
    assert_eq!(union_into.kind(), QueryDiagnosticKind::Syntax);

    let statements = (0..65).map(|_| "ВЫБРАТЬ 1").collect::<Vec<_>>().join("; ");
    let too_many = session
        .compile(&snapshot, &format!("{statements};"))
        .0
        .unwrap_err();
    assert_eq!(too_many.kind(), QueryDiagnosticKind::WorkBudgetExceeded);
    assert!(too_many.message().contains("64 statements"));
}

#[test]
fn bounds_batches_compiled_without_a_manager() {
    let snapshot = snapshot();
    let compiler = QueryCompiler::new(&snapshot, PostgresBackend);

    let batch = compiler
        .compile(
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;
             ВЫБРАТЬ Т.Имя ИЗ ВТ КАК Т;",
        )
        .unwrap();
    assert!(batch.sql.starts_with("WITH \"vt1\" AS ("));

    let no_rows = compiler
        .compile(
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;
             УНИЧТОЖИТЬ ВТ;",
        )
        .unwrap_err();
    assert_eq!(no_rows.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(no_rows.message().contains("returns no rows"));

    // A manager filled by one dialect refuses another.
    let mut manager = TempTablesManager::new();
    compiler
        .compile_batch(
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap();
    let other_dialect = QueryCompiler::new(&snapshot, mssql_backend(0))
        .compile_batch(
            "ВЫБРАТЬ Т.Имя ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap_err();
    assert_eq!(other_dialect.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(other_dialect.message().contains("another SQL dialect"));

    let other_snapshot = QueryCompiler::new(&reference_snapshot(), PostgresBackend)
        .compile_batch(
            "ВЫБРАТЬ Т.Имя ИЗ ВТ КАК Т;",
            &CompileOptions::new(),
            &mut manager,
        )
        .unwrap_err();
    assert_eq!(other_snapshot.kind(), QueryDiagnosticKind::SnapshotMismatch);

    manager.clear();
    assert!(manager.is_empty());
    assert!(!manager.contains("ВТ"));

    // A manager holds a bounded number of definitions.
    let mut crowded = TempTablesManager::new();
    let mut definitions = 0;
    let overflow = loop {
        let source = format!(
            "ВЫБРАТЬ Code КАК Имя ПОМЕСТИТЬ Т{definitions} ИЗ Catalog.OpenSdblMetadataProbe;"
        );
        match compiler.compile_batch(&source, &CompileOptions::new(), &mut crowded) {
            Ok(_) => definitions += 1,
            Err(error) => break error,
        }
        assert!(
            definitions <= 256,
            "the definition bound must stop the loop"
        );
    };
    assert_eq!(definitions, 256);
    assert_eq!(overflow.kind(), QueryDiagnosticKind::TemporaryTable);
    assert!(overflow.message().contains("256 definitions"));
}

#[test]
fn widens_reference_equalities_in_join_conditions() {
    let snapshot = presentation_reference_snapshot(true);
    let mut session = BatchSession::new();
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ ProbeAttribute КАК Объект ПОМЕСТИТЬ ВТ ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap();

    // A fixed 16-byte reference is widened to its own payload before it is
    // compared with the 20-byte column of the temporary table.
    let (postgres, mssql) = session.compile(
        &snapshot,
        "ВЫБРАТЬ Д.Ссылка ИЗ Catalog.OpenSdblMetadataProbe КАК Д
         ЛЕВОЕ СОЕДИНЕНИЕ ВТ КАК Т ПО Д.Ссылка = Т.Объект;",
    );
    let postgres = postgres.unwrap().unwrap();
    assert!(postgres.sql.contains(
        "LEFT JOIN \"vt1\" AS \"Т\" ON (decode('00000035', 'hex') || \"Д\".\"_idrref\") = \"Т\".\"Объект\""
    ));
    let mssql = mssql.unwrap().unwrap();
    assert!(
        mssql
            .sql
            .contains("LEFT JOIN [vt1] AS [Т] ON (0x00000035 + [Д].[_idrref]) = [Т].[Объект]")
    );

    // A composite field is concatenated instead of failing on its missing
    // unique SchemaStorage target.
    let composite = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ П.Ссылка ИЗ Catalog.OpenSdblMetadataProbe КАК П
             ВНУТРЕННЕЕ СОЕДИНЕНИЕ ВТ КАК Т ПО П.ProbeAttribute = Т.Объект;",
        )
        .0
        .unwrap()
        .unwrap();
    assert!(composite.sql.contains(
        "INNER JOIN \"vt1\" AS \"Т\" ON (\"П\".\"_fld54_rtref\" || \"П\".\"_fld54_rrref\") = \"Т\".\"Объект\""
    ));

    // Two payload columns compare directly.
    session
        .compile(
            &snapshot,
            "ВЫБРАТЬ ProbeAttribute КАК Объект ПОМЕСТИТЬ ВТ2 ИЗ Catalog.OpenSdblMetadataProbe;",
        )
        .0
        .unwrap();
    let payloads = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ А.Объект ИЗ ВТ КАК А ВНУТРЕННЕЕ СОЕДИНЕНИЕ ВТ2 КАК Б ПО А.Объект = Б.Объект;",
        )
        .0
        .unwrap()
        .unwrap();
    assert!(
        payloads
            .sql
            .contains("INNER JOIN \"vt2\" AS \"Б\" ON \"А\".\"Объект\" = \"Б\".\"Объект\"")
    );

    // Two composite fields compare member by member.
    let members = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ А.Ссылка ИЗ Catalog.OpenSdblMetadataProbe КАК А
             ВНУТРЕННЕЕ СОЕДИНЕНИЕ Catalog.OpenSdblMetadataProbe КАК Б
             ПО А.ProbeAttribute = Б.ProbeAttribute;",
        )
        .0
        .unwrap()
        .unwrap();
    assert!(members.sql.contains(
        "ON (\"А\".\"_fld54_rrref\" = \"Б\".\"_fld54_rrref\" AND \"А\".\"_fld54_rtref\" = \"Б\".\"_fld54_rtref\")"
    ));

    // A transposed FULL JOIN anchors on the widened expression.
    let full = session
        .compile(
            &snapshot,
            "ВЫБРАТЬ Д.Ссылка ИЗ Catalog.OpenSdblMetadataProbe КАК Д
             ПОЛНОЕ СОЕДИНЕНИЕ ВТ КАК Т ПО Д.Ссылка = Т.Объект;",
        )
        .0
        .unwrap()
        .unwrap();
    assert!(full.sql.contains("UNION ALL"));
    assert!(
        full.sql
            .contains("(decode('00000035', 'hex') || \"Д\".\"_idrref\") IS NULL")
    );
}

#[test]
fn dereferences_references_inside_join_conditions() {
    let snapshot = reference_snapshot();

    // The dereference join is grouped with its own source so that the ON
    // clause of the later join can address it.
    let (postgres, mssql) = for_each_backend!(
        "ВЫБРАТЬ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Организация.Code = t.Code;",
        &snapshot,
    );
    let postgres = postgres.unwrap();
    assert!(postgres.sql.contains(
        "FROM (\"_reference53\" AS \"p\" LEFT JOIN \"_reference57\" AS \"__left_ref1\" ON \"p\".\"_fld54\" = \"__left_ref1\".\"_idrref\") LEFT JOIN \"_reference57\" AS \"t\" ON \"__left_ref1\".\"_code\" = \"t\".\"_code\""
    ));
    let mssql = mssql.unwrap();
    assert!(mssql.sql.contains(
        "FROM ([_reference53] AS [p] LEFT JOIN [_reference57] AS [__left_ref1] ON [p].[_fld54] = [__left_ref1].[_idrref]) LEFT JOIN [_reference57] AS [t] ON"
    ));

    // A dereference used only as an extra predicate keeps the anchor.
    let extra = postgres_compile!(
        "ВЫБРАТЬ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t
         ПО p.Code = t.Code И p.Организация.Code = \"A\";",
        &snapshot,
    )
    .unwrap();
    assert!(
        extra
            .sql
            .contains("LEFT JOIN \"_reference57\" AS \"__left_ref1\" ON")
    );
    assert!(extra.sql.contains("(\"__left_ref1\".\"_code\" = 'A')"));

    // A dereference of an earlier source inside a later condition groups
    // that earlier source.
    let earlier = postgres_compile!(
        "ВЫБРАТЬ p.Code ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Code = t.Code
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК u ПО p.Организация.Code = u.Code;",
        &snapshot,
    )
    .unwrap();
    assert!(earlier.sql.contains(
        "FROM (\"_reference53\" AS \"p\" LEFT JOIN \"_reference57\" AS \"__left_ref1\" ON"
    ));
    assert!(earlier.sql.contains(
        "LEFT JOIN \"_reference57\" AS \"u\" ON \"__left_ref1\".\"_code\" = \"u\".\"_code\""
    ));

    // A dereference shared by a projection and a condition is planned once.
    let shared = postgres_compile!(
        "ВЫБРАТЬ p.Организация.Code КАК Орг ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Организация.Code = t.Code;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(shared.sql.matches("__left_ref1").count(), 4);
    assert!(
        shared
            .sql
            .contains("FROM (\"_reference53\" AS \"p\" LEFT JOIN")
    );

    // Statements whose conditions use no dereference keep the flat form.
    let flat = postgres_compile!(
        "ВЫБРАТЬ p.Организация.Code КАК Орг ИЗ Catalog.OpenSdblMetadataProbe КАК p
         ЛЕВОЕ СОЕДИНЕНИЕ Catalog.Организации КАК t ПО p.Code = t.Code;",
        &snapshot,
    )
    .unwrap();
    assert!(
        flat.sql
            .contains("FROM \"_reference53\" AS \"p\" LEFT JOIN \"_reference57\" AS \"t\" ON")
    );
    assert!(flat.sql.ends_with("LEFT JOIN \"_reference57\" AS \"__left_ref1\" ON \"p\".\"_fld54\" = \"__left_ref1\".\"_idrref\""));
}
