mod support;

use support::*;

use open_sdbl::metadata::{
    ColumnType, ConfigFieldPurpose, FieldId, LiveColumn, LiveTable, LookupError, MetadataKind,
    MetadataSnapshot, ResolutionFinding, SchemaAnomaly, SchemaColumn, StandardFieldId,
    parse_config_descriptors, parse_db_names, resolve_metadata,
};
use open_sdbl::query::{
    Backend, MsSqlBackend, PostgresBackend, Prepared, PresentationExpression, PresentationPlan,
    QueryCompiler, QueryDiagnosticKind, find_metadata_object, queryable_field_catalog,
    queryable_fields,
};

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
            "SELECT CONVERT(nvarchar(max), [l].[_period]) AS [Period], CONVERT(nvarchar(max), [r].[_period]) AS [Period_2] FROM (SELECT [__slice_ranked].* FROM (SELECT [__slice_base].*, DENSE_RANK() OVER (PARTITION BY [__slice_base].[_fld54] ORDER BY [__slice_base].[_period] ASC) AS [__open_sdbl_slice_rank] FROM [_inforg53] AS [__slice_base]) AS [__slice_ranked] WHERE [__slice_ranked].[__open_sdbl_slice_rank] = 1) AS [l] INNER JOIN (SELECT [__slice_ranked].* FROM (SELECT [__slice_base].*, DENSE_RANK() OVER (PARTITION BY [__slice_base].[_fld54] ORDER BY [__slice_base].[_period] DESC) AS [__open_sdbl_slice_rank] FROM [_inforg53] AS [__slice_base]) AS [__slice_ranked] WHERE [__slice_ranked].[__open_sdbl_slice_rank] = 1) AS [r] ON [l].[_fld54] = [r].[_fld54]",
        ),
        (
            "turnovers",
            mssql_compile!(
                "SELECT Номенклатура, КоличествоОборот FROM AccumulationRegister.Остатки.Turnovers(\"2026-08-01\", \"2026-09-01\",, Номенклатура IS NOT NULL);",
                &accumulation_register_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), [__src].[_fld54]) AS [Номенклатура], CONVERT(nvarchar(max), [__src].[_fld55]) AS [КоличествоОборот] FROM (SELECT [__aggregate_base].[_fld54] AS [_fld54], SUM(CASE WHEN [__aggregate_base].[_recordkind] = 0 THEN [__aggregate_base].[_fld55] ELSE -[__aggregate_base].[_fld55] END) AS [_fld55] FROM [_accumrg53] AS [__aggregate_base] WHERE [__aggregate_base].[_active] = 0x01 AND ([__aggregate_base].[_period] >= N'2026-08-01') AND ([__aggregate_base].[_period] < N'2026-09-01') AND ([__aggregate_base].[_fld54] IS NOT NULL) GROUP BY [__aggregate_base].[_fld54]) AS [__src]",
        ),
        (
            "union",
            mssql_compile!(
                "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p WHERE p.Code = \"A\" UNION SELECT q.Date FROM Catalog.OpenSdblMetadataProbe q UNION ALL SELECT r.ProbeAttribute FROM Catalog.OpenSdblMetadataProbe r ORDER BY Code DESC;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), [p].[_code]) AS [Code] FROM [_reference53] AS [p] WHERE ([p].[_code] = N'A') UNION SELECT CONVERT(nvarchar(max), [q].[_date_time]) AS [Date] FROM [_reference53] AS [q] UNION ALL SELECT CONVERT(nvarchar(max), [r].[_fld54]) AS [ProbeAttribute] FROM [_reference53] AS [r] ORDER BY 1 DESC",
        ),
        (
            "full_join",
            mssql_compile!(
                "SELECT l.Code, r.Date FROM Catalog.OpenSdblMetadataProbe l FULL JOIN Catalog.OpenSdblMetadataProbe r ON l.Code = r.Code;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT * FROM (SELECT CONVERT(nvarchar(max), [l].[_code]) AS [Code], CONVERT(nvarchar(max), [r].[_date_time]) AS [Date] FROM [_reference53] AS [l] LEFT JOIN [_reference53] AS [r] ON [l].[_code] = [r].[_code] UNION ALL SELECT CONVERT(nvarchar(max), [l].[_code]) AS [Code], CONVERT(nvarchar(max), [r].[_date_time]) AS [Date] FROM [_reference53] AS [r] LEFT JOIN [_reference53] AS [l] ON [l].[_code] = [r].[_code] WHERE ([l].[_code] IS NULL)) AS [__full]",
        ),
        (
            "dereference",
            mssql_compile!(
                "SELECT Организация.Код FROM Catalog.OpenSdblMetadataProbe p;",
                &reference_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), [__ref1].[_code]) AS [Организация.Код] FROM [_reference53] AS [p] LEFT JOIN [_reference57] AS [__ref1] ON [p].[_fld54] = [__ref1].[_idrref]",
        ),
        (
            "tabular",
            mssql_compile!(
                "SELECT Ссылка, НомерСтроки, Сумма FROM Документ.бит_ДополнительныеУсловияПоДоговору.ГрафикНачислений;",
                &tabular_section_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), [__src].[_document53_idrref]) AS [ID], CONVERT(nvarchar(max), [__src].[_lineno54]) AS [LineNo], CONVERT(nvarchar(max), [__src].[_fld57]) AS [Сумма] FROM [_document53_vt54X1] AS [__src]",
        ),
        (
            "aggregate",
            mssql_compile!(
                "SELECT COUNT(*) AS RowCount, SUM(ProbeAttribute) AS Total FROM Catalog.OpenSdblMetadataProbe;",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), COUNT(*)) AS [RowCount], CONVERT(nvarchar(max), SUM([__src].[_fld54])) AS [Total] FROM [_reference53] AS [__src]",
        ),
        (
            "top_in",
            mssql_compile!(
                "SELECT TOP 3 Code FROM Catalog.OpenSdblMetadataProbe WHERE Code IN (\"A\", \"B\");",
                &snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT TOP (3) CONVERT(nvarchar(max), [__src].[_code]) AS [Code] FROM [_reference53] AS [__src] WHERE ([__src].[_code] IN (N'A', N'B'))",
        ),
        (
            "value",
            mssql_compile!(
                "SELECT VALUE(Catalog.OpenSdblMetadataProbe.Утвержден);",
                &catalog_value_snapshot(),
            )
            .unwrap()
            .sql,
            "SELECT CONVERT(nvarchar(max), (SELECT [__open_sdbl_value].[_idrref] FROM [_reference53] AS [__open_sdbl_value] WHERE ([__open_sdbl_value].[_predefinedid] = 0xa3dae56fa2f94623445632b52e22ad88))) AS [column1]",
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

    assert_eq!(compiled.columns, ["Code", "ProbeAttribute"]);
    assert_eq!(
        compiled.sql,
        "SELECT TOP (10) CONVERT(nvarchar(max), [__src].[_code]) AS [Code], CONVERT(varchar(max), [__src].[_fld54], 1) AS [ProbeAttribute] FROM [_reference53] AS [__src] WHERE ([__src].[_code] = N'\u{420}\u{430}\u{437}\u{43e}\u{432}\u{44b}\u{439}')"
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

        assert_eq!(compiled.columns, ["Version"]);
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
    assert_eq!(literal.columns, ["column1"]);
    assert_eq!(literal.sql, "SELECT (4)::text AS \"column1\"");

    let presentation = postgres_compile!("select представление(4);", &snapshot).unwrap();
    assert_eq!(presentation.columns, ["представление"]);
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
    assert_eq!(field.columns, ["ResultCode"]);
    assert!(field.sql.contains("AS \"ResultCode\""));

    let scalar = postgres_compile!("SELECT 2 + 2 КАК Результат;", &snapshot).unwrap();
    assert_eq!(scalar.columns, ["Результат"]);
    assert_eq!(scalar.sql, "SELECT ((2 + 2))::text AS \"Результат\"");

    let aggregate = postgres_compile!(
        "SELECT COUNT(*) AS RowCount FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(aggregate.columns, ["RowCount"]);
    assert!(aggregate.sql.contains("COUNT(*)::text AS \"RowCount\""));

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
    assert_eq!(source_free.columns, ["Moment", "PeriodStart"]);
    assert!(
        source_free
            .sql
            .contains("(TIMESTAMP '2024-02-29 12:34:56')::text AS \"Moment\"")
    );
    assert!(source_free.sql.contains(
        "(date_trunc('month', TIMESTAMP '2024-08-29 12:34:56'))::text AS \"PeriodStart\""
    ));

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
    let compiled = mssql_compile_with_offset!(
        "SELECT BEGINOFPERIOD(Date, MONTH) AS MonthStart
         FROM Catalog.OpenSdblMetadataProbe
         WHERE Date >= DATETIME(2026, 9, 2, 10, 11, 12);",
        &snapshot,
        2000,
    )
    .unwrap();

    assert!(compiled.sql.contains(
        "CONVERT(nvarchar(max), DATEADD(year, -2000, DATETIME2FROMPARTS(YEAR([__src].[_date_time]), MONTH([__src].[_date_time]), 1, 0, 0, 0, 0, 0))) AS [MonthStart]"
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
    assert_eq!(compiled.columns, ["column1", "column2"]);
    assert_eq!(
        compiled.sql,
        "SELECT ((2 + 2))::text AS \"column1\", ('готово')::text AS \"column2\""
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
    assert_eq!(all.columns, ["count"]);
    assert_eq!(
        all.sql,
        "SELECT COUNT(*)::text AS \"count\" FROM \"_reference53\" AS \"__src\""
    );

    let distinct = postgres_compile!(
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
    let source_free = postgres_compile!("SELECT COUNT(*);", &snapshot).unwrap();
    assert_eq!(source_free.sql, "SELECT COUNT(*)::text AS \"COUNT\"");

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
    assert!(parameter.message().contains("parameters are not supported"));
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
    assert!(parameter.message().contains("parameters are not supported"));
}

#[test]
fn compiles_current_accumulation_register_balances() {
    let snapshot = accumulation_register_snapshot();
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
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
    let compiled = postgres_compile!(
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
fn extension_projection_catalog_scans_cannot_escape_the_work_budget() {
    let snapshot = with_schema(snapshot(), |schema| {
        schema.tables.extend(
            (0..9_000).map(|index| schema_table(&format!("Unrelated{index}"), 0, Vec::new())),
        );
    });
    let snapshot = with_live_tables(snapshot, |tables| {
        tables.extend((0..9_000).map(|index| live_table(&format!("_unrelated{index}"), &[])));
    });

    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("SELECT Code FROM Catalog.OpenSdblMetadataProbe;")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::WorkBudgetExceeded);
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
    assert!(parameter.message().contains("parameters are not supported"));

    let unsupported = postgres_compile!(
        "SELECT Code FROM Catalog.OpenSdblMetadataProbe GROUP BY Code;",
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
    assert_eq!(unsupported.kind(), QueryDiagnosticKind::UnsupportedFeature);

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
    assert_eq!(compiled.columns, ["Code"]);
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
    assert!(postgres.columns.iter().all(|label| label.len() <= 63));
    assert!(
        postgres
            .columns
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
        mssql
            .columns
            .iter()
            .all(|label| label.encode_utf16().count() <= 128)
    );
    assert!(
        mssql
            .columns
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
                name: "_fld54_tref".to_owned(),
                data_type: "bytea".to_owned(),
            },
            LiveColumn {
                name: "_fld54_rrref".to_owned(),
                data_type: "bytea".to_owned(),
            },
        ]);
    });

    let compiled = postgres_compile!(
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

    let aliased = postgres_compile!(
        "SELECT ProbeAttribute AS Value FROM Catalog.OpenSdblMetadataProbe;",
        &snapshot,
    )
    .unwrap();
    assert_eq!(aliased.columns, ["Value_TRef", "Value_RRRef"]);

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
    assert_eq!(implicit.columns, ["Организация.Code"]);

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
            .contains("top-level cross-source field equality")
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
            .contains("top-level cross-source field equality")
    );

    let same_alias = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации p ON p.Code = p.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(same_alias.message().contains("distinct aliases"));

    let reference_condition = postgres_compile!(
        "SELECT p.Code FROM Catalog.OpenSdblMetadataProbe p
         LEFT JOIN Catalog.Организации t ON p.Организация.Code = t.Code;",
        &snapshot,
    )
    .unwrap_err();
    assert!(reference_condition.message().contains("direct fields only"));

    let additional_reference_condition = postgres_compile!(
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

    let direct = postgres_compile!(
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

    let mssql = mssql_prepare!(source, &snapshot).unwrap();
    let mssql = mssql.compile(&snapshot, &[]).unwrap();
    assert_eq!(mssql.deferred_presentations, [0]);
    assert!(
        mssql
            .sql
            .contains("CONVERT(varchar(max), [__right_ref1].[_fld59_rtref], 2)")
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
        assert_eq!(postgres.columns, ["__reference", "__presentation"]);
        assert_eq!(mssql.columns, ["__reference", "__presentation"]);
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
