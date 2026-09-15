//! Tests of the `mssql live` module.

use crate::args::{ConnectionOptions, MsSqlConnection};
use crate::auth::pgpass::{Credentials, EnvironmentSecret};
use crate::db::mssql::MsSqlSession;
use open_sdbl::metadata::{FieldId, MetadataSnapshot, StandardFieldId};
use open_sdbl::query::{
    CompiledQuery, MsSqlBackend, PresentationExpression, PresentationPlan, QueryCompiler,
};
use zeroize::Zeroizing;

fn compile_mssql_test_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    year_offset: i32,
) -> CompiledQuery {
    let backend = MsSqlBackend::new(year_offset).expect("test MSSQL year offset must be valid");
    let prepared = QueryCompiler::new(snapshot, backend)
        .prepare(source)
        .unwrap();
    let plans = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| {
            let description = FieldId::Standard(StandardFieldId::Description);
            let code = FieldId::Standard(StandardFieldId::Code);
            PresentationPlan {
                object: target.object,
                fields: vec![description, code],
                expression: PresentationExpression::Concat(vec![
                    PresentationExpression::Field(description),
                    PresentationExpression::Literal(" (".to_owned()),
                    PresentationExpression::Field(code),
                    PresentationExpression::Literal(")".to_owned()),
                ]),
            }
        })
        .collect::<Vec<_>>();
    prepared.compile(snapshot, &plans).unwrap()
}

fn mssql_test_connection() -> MsSqlConnection {
    let user = std::env::var("OPEN_SDBL_MSSQL_TEST_USER")
        .expect("OPEN_SDBL_MSSQL_TEST_USER must name a SELECT-only SQL login");
    MsSqlConnection {
        options: ConnectionOptions {
            host: std::env::var("OPEN_SDBL_MSSQL_TEST_HOST")
                .unwrap_or_else(|_| "192.168.122.222".to_owned()),
            port: std::env::var("OPEN_SDBL_MSSQL_TEST_PORT")
                .map_or(1433, |value| value.parse().expect("invalid test port")),
            database: std::env::var("OPEN_SDBL_MSSQL_TEST_DATABASE")
                .unwrap_or_else(|_| "demo".to_owned()),
            user,
            socks5_proxy: None,
        },
        trust_server_certificate: std::env::var_os("OPEN_SDBL_MSSQL_TEST_TRUST_CERTIFICATE")
            .is_some(),
        trust_ca_file: None,
        dialect_level: None,
    }
}

fn mssql_test_credentials() -> Credentials {
    Credentials {
        postgres: EnvironmentSecret::Missing,
        mssql: EnvironmentSecret::Present(Zeroizing::new(
            std::env::var("MSSQL_PASSWORD").expect("MSSQL_PASSWORD is required"),
        )),
        socks5: EnvironmentSecret::Missing,
    }
}

#[tokio::test]
#[ignore = "requires OPEN_SDBL_MSSQL_TEST_USER, MSSQL_PASSWORD, and a live 1C database"]
async fn reads_metadata_from_the_mssql_demo_database() {
    let mut session = MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
        .await
        .unwrap();
    let snapshot = session.metadata().await.unwrap();
    assert!(!snapshot.objects().is_empty());
    assert!(!snapshot.live_tables().is_empty());
    session.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires OPEN_SDBL_MSSQL_TEST_* pointing at a SQL Server 2008 R2 base"]
async fn compiles_begin_of_period_on_a_sql_server_2008_base() {
    let mut session = MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
        .await
        .unwrap();
    let backend = session.backend();
    assert_eq!(
        backend.dialect_level(),
        open_sdbl::query::MsSqlDialectLevel::Sql2008,
        "{}",
        session.server_description()
    );
    let snapshot = session.metadata().await.unwrap();
    let document = snapshot
        .objects()
        .iter()
        .find(|object| {
            object.kind == Some(open_sdbl::metadata::MetadataKind::Document)
                && object.live
                && object.name.is_some()
        })
        .expect("a base has at least one live document");
    let compiled = QueryCompiler::new(&snapshot, backend)
        .compile(&format!(
            "ВЫБРАТЬ ПЕРВЫЕ 3 НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ), НАЧАЛОПЕРИОДА(Дата, ДЕКАДА) ИЗ Документ.{};",
            document.name.as_deref().unwrap()
        ))
        .unwrap();
    assert!(!compiled.sql.contains("DATETIME2FROMPARTS"));
    let rows = session
        .query(&compiled.sql, compiled.columns.len())
        .await
        .unwrap();
    assert!(rows.len() <= 3);
    session.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires OPEN_SDBL_MSSQL_TEST_* pointing at a platform 8.2 base without PartNo"]
async fn reads_metadata_from_a_legacy_mssql_base_without_part_numbers() {
    let mut session = MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
        .await
        .unwrap();
    let layout = session.storage_layout().await.unwrap();
    assert!(
        !layout.config_parts,
        "the configured base has PartNo: {layout:?}"
    );
    assert!(!layout.params_parts);
    assert!(!layout.extension_store);
    assert!(!layout.extension_restructure);
    assert!(layout.schema_storage);

    let backend = session.backend();
    let snapshot = session.metadata().await.unwrap();
    assert!(!snapshot.objects().is_empty());
    let catalog = snapshot
        .objects()
        .iter()
        .find(|object| {
            object.kind == Some(open_sdbl::metadata::MetadataKind::Catalog)
                && object.live
                && object.name.is_some()
        })
        .expect("a legacy base has at least one live catalog");
    let compiled = QueryCompiler::new(&snapshot, backend)
        .compile(&format!(
            "ВЫБРАТЬ ПЕРВЫЕ 1 Ссылка, УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Ссылка) ИЗ Справочник.{};",
            catalog.name.as_deref().unwrap()
        ))
        .unwrap();
    let rows = session
        .query(&compiled.sql, compiled.columns.len())
        .await
        .unwrap();
    assert!(rows.len() <= 1);
    session.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires the MSSQL demo database and its _ДемоЗаказПокупателя document"]
async fn reads_native_rowversion_from_the_mssql_demo_database() {
    let mut session = MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
        .await
        .unwrap();
    let backend = session.backend();
    let snapshot = session.metadata().await.unwrap();
    let compiled = QueryCompiler::new(&snapshot, backend)
        .compile(
            "SELECT Version FROM Документ._ДемоЗаказПокупателя WHERE Version > 0x00000000000007D6;",
        )
        .unwrap();

    assert!(
        compiled
            .sql
            .contains("\"__src\".\"_Version\" AS \"Version\"")
    );
    assert!(
        !compiled
            .sql
            .contains("CONVERT(nvarchar(max), \"__src\".\"_Version\")")
    );
    assert!(
        compiled
            .sql
            .contains("(\"__src\".\"_Version\" > 0x00000000000007D6)")
    );
    let rows = session
        .query(&compiled.sql, compiled.columns.len())
        .await
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let version = row[0].render();
        assert!(version.as_ref() > "0x00000000000007D6");
        assert_eq!(version.len(), 18);
    }
    session.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires the MSSQL demo database and its _Reference18X1 extension table"]
async fn reads_dereferences_and_presents_the_mssql_demo_extension_table() {
    let mut session = MsSqlSession::connect(&mssql_test_connection(), &mssql_test_credentials())
        .await
        .unwrap();
    let snapshot = session.metadata().await.unwrap();
    let direct = compile_mssql_test_query(
        "SELECT TOP 3 ID, Code, Description FROM Catalog._ДемоНоменклатура;",
        &snapshot,
        session.backend().year_offset(),
    );
    assert!(direct.sql.contains("FROM \"_Reference18X1\""));
    let direct_rows = session
        .query(&direct.sql, direct.columns.len())
        .await
        .unwrap();
    assert_eq!(direct_rows.len(), 3);
    assert!(direct_rows.iter().all(|row| {
        row[2]
            .as_text()
            .is_some_and(|description| !description.trim().is_empty())
    }));

    let dereference = compile_mssql_test_query(
        "SELECT TOP 3 Номенклатура.Наименование FROM РегистрНакопления._ДемоОстаткиТоваровВМестахХранения.Остатки();",
        &snapshot,
        session.backend().year_offset(),
    );
    assert!(dereference.sql.contains("FROM \"_Reference18X1\""));
    let dereference_rows = session
        .query(&dereference.sql, dereference.columns.len())
        .await
        .unwrap();
    assert_eq!(dereference_rows.len(), 3);
    assert!(dereference_rows.iter().all(|row| {
        row[0]
            .as_text()
            .is_some_and(|description| !description.trim().is_empty())
    }));

    let presentations = compile_mssql_test_query(
        "SELECT Номенклатура, ПредставлениеСсылки(Номенклатура), Представление(Номенклатура), КоличествоОстаток FROM РегистрНакопления._ДемоОстаткиТоваровВМестахХранения.Остатки();",
        &snapshot,
        session.backend().year_offset(),
    );
    assert!(presentations.sql.contains("FROM \"_Reference18X1\""));
    let presentation_rows = session
        .query(&presentations.sql, presentations.columns.len())
        .await
        .unwrap();
    assert!(!presentation_rows.is_empty());
    assert!(presentation_rows.iter().all(|row| {
        let reference = row[1].as_text();
        let value = row[2].as_text();
        reference == value
            && reference.is_some_and(|presentation| {
                !presentation.trim().is_empty() && presentation != " ()"
            })
    }));
    session.close().await.unwrap();
}
