//! A real `tokio-postgres` client against the gateway.

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use tokio_postgres::error::{ErrorPosition, SqlState};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type, to_sql_checked};
use tokio_postgres::{Client, Config, NoTls, SimpleQueryMessage};

use super::protocol::Raw;
use super::*;

const SELECT: &str = "ВЫБРАТЬ Категория, Количество ИЗ РегистрНакопления.ЗамерОстатков";

fn config() -> Config {
    let mut config = Config::new();
    config
        .user(USER)
        .password(PASSWORD)
        .dbname("unf")
        .application_name("tests");
    config
}

async fn connect(harness: &Harness) -> Client {
    let (client, connection) = config()
        .connect_raw(harness.connect(), NoTls)
        .await
        .expect("the gateway accepts the client");
    tokio::spawn(connection);
    client
}

#[tokio::test]
async fn section_owner_reference_query_uses_normal_rights_and_rls() {
    use open_sdbl::metadata::Guid;
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let snapshot = section_reference::snapshot();
        let base = fixture::snapshot();
        let mut access = AccessStore::new(&snapshot);
        access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
        let mut rights = fixture::rights(&base, true);
        rights.objects[0].object = section_reference::OWNER.parse().unwrap();
        let mut restriction = fixture::sample_restriction();
        restriction.condition =
            "ГДЕ Ссылка <> ЗНАЧЕНИЕ(Документ.ПриходнаяНакладная.ПустаяСсылка)".into();
        restriction.fields.clear();
        rights.objects[0].rights[0].restrictions = vec![restriction];
        access.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights.clone());
        let harness = Harness::new(Options {
            snapshot: Some(snapshot),
            access: Some(access),
            dialect,
            responder: Arc::new(|sql, width| {
                let mut row = vec![
                    Cell::Uuid([1; 16]),
                    Cell::Text("Приходная накладная 123 от 2026-10-26".into()),
                ];
                if sql.to_lowercase().contains("_fld3") {
                    row.push(Cell::Bytes(vec![2; 16]));
                }
                row.resize(width, Cell::Bool(false));
                Ok(vec![row])
            }),
            ..Options::default()
        });
        let client = connect(&harness).await;
        for text in [section_reference::QUERY, section_reference::ITEM_QUERY] {
            let result = client.simple_query(text).await.unwrap();
            let rows = result
                .iter()
                .filter_map(|m| {
                    if let SimpleQueryMessage::Row(r) = m {
                        Some(r)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].get(0), Some("01010101-0101-0101-0101-010101010101"));
            assert_eq!(
                rows[0].get(1),
                Some("Приходная накладная 123 от 2026-10-26")
            );
            if text == section_reference::ITEM_QUERY {
                assert_eq!(rows[0].get(2), Some("\\x02020202020202020202020202020202"));
            }
        }
        let columns = client.query("SELECT column_name FROM information_schema.columns WHERE table_name='ПриходнаяНакладная.Запасы'", &[]).await.unwrap();
        let names = columns
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect::<Vec<_>>();
        assert!(names.contains(&"Номенклатура".into()), "{names:?}");
        assert!(names.contains(&"LineNo".into()), "{names:?}");
        let sql = harness.statements().join("\n").to_lowercase();
        assert!(sql.contains("_document1_vt2"), "{sql}");
        assert!(sql.contains("_document1") && sql.contains("where"), "{sql}");
        assert!(sql.contains(" in (") || sql.contains("exists"), "{sql}");
        let snapshot = section_reference::snapshot();
        let mut denied = AccessStore::new(&snapshot);
        denied.set_users(fixture::store(&base, true).users().unwrap().to_vec());
        rights.objects[0].rights[0].allowed = false;
        denied.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights);
        harness
            .state
            .replace(GatewayState::from_parts(snapshot, denied, dialect));
        let before = harness.statements().len();
        let error = client
            .simple_query(section_reference::ITEM_QUERY)
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
        assert_eq!(harness.statements().len(), before);
        harness.stop();
    }
}

#[tokio::test]
#[ignore = "requires local psql and a loopback listener"]
async fn psql_section_owner_reference_query() {
    let snapshot = section_reference::snapshot();
    let base = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, true);
    rights.objects[0].object = section_reference::OWNER.parse().unwrap();
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights);
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        responder: Arc::new(|sql, width| {
            let mut row = vec![
                Cell::Uuid([1; 16]),
                Cell::Text("Приходная накладная 123".into()),
            ];
            if sql.to_lowercase().contains("_fld3") {
                row.push(Cell::Bytes(vec![2; 16]));
            }
            assert_eq!(width, row.len());
            Ok(vec![row])
        }),
        ..Options::default()
    });
    let stdout = super::catalog::psql_script(
        &harness,
        &format!(
            "{}\n{}\n",
            section_reference::QUERY,
            section_reference::ITEM_QUERY
        ),
    )
    .await;
    assert!(
        stdout.contains("01010101-0101-0101-0101-010101010101"),
        "{stdout}"
    );
    assert!(stdout.contains("Приходная накладная 123"), "{stdout}");
    assert!(
        stdout.contains("\\x02020202020202020202020202020202"),
        "{stdout}"
    );
    assert!(
        harness
            .statements()
            .iter()
            .any(|sql| sql.contains("_document1_vt2"))
    );
    harness.stop();
}

/// A `numeric`, decoded here independently of the gateway's encoder.
#[derive(Debug, PartialEq)]
struct Numeric(String);

impl<'a> FromSql<'a> for Numeric {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let word = |index: usize| u16::from_be_bytes([raw[index * 2], raw[index * 2 + 1]]);
        let digits = usize::from(word(0));
        let weight = i64::from(word(1) as i16);
        let negative = word(2) == 0x4000;
        let scale = u32::from(word(3));
        let mut value: i128 = 0;
        for index in 0..digits {
            value = value * 10_000 + i128::from(word(4 + index));
        }
        // value × 10000^(weight − digits + 1), expressed with `scale` decimals.
        let exponent = 4 * (weight - i64::try_from(digits).unwrap() + 1) + i64::from(scale);
        let unscaled = if exponent >= 0 {
            value * 10_i128.pow(u32::try_from(exponent).unwrap())
        } else {
            value / 10_i128.pow(u32::try_from(-exponent).unwrap())
        };
        let text = unscaled.to_string();
        let scale = scale as usize;
        let padded = format!("{text:0>width$}", width = scale + 1);
        let (integer, fraction) = padded.split_at(padded.len() - scale);
        let sign = if negative { "-" } else { "" };
        Ok(Self(if scale == 0 {
            format!("{sign}{integer}")
        } else {
            format!("{sign}{integer}.{fraction}")
        }))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

/// Raw bytes declared as `jsonb`, which the gateway cannot bind.
#[derive(Debug)]
struct Jsonb;

impl ToSql for Jsonb {
    fn to_sql(
        &self,
        _: &Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(b"\x01{}");
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::JSONB
    }

    to_sql_checked!();
}

#[tokio::test]
async fn simple_query_through_tokio_postgres() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let messages = client.simple_query(SELECT).await.unwrap();
    let rows = messages
        .iter()
        .filter_map(|message| match message {
            SimpleQueryMessage::Row(row) => Some((
                row.get(0).unwrap().to_owned(),
                row.get(1).unwrap().to_owned(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            ("К0".to_owned(), "0.50".to_owned()),
            ("К1".to_owned(), "1.50".to_owned()),
            ("К2".to_owned(), "2.50".to_owned())
        ]
    );
    assert!(matches!(
        messages.last(),
        Some(SimpleQueryMessage::CommandComplete(3))
    ));
}

#[tokio::test]
async fn prepared_statement_is_described_and_decoded_in_binary() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let statement = client
        .prepare(&format!("{SELECT} ГДЕ Категория <> &Исключить"))
        .await
        .unwrap();
    assert_eq!(statement.params(), [Type::TEXT]);
    let types = statement
        .columns()
        .iter()
        .map(|column| (column.name().to_owned(), column.type_().clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        [
            ("Категория".to_owned(), Type::VARCHAR),
            ("Количество".to_owned(), Type::NUMERIC)
        ]
    );
    let rows = client.query(&statement, &[&"нет"]).await.unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].get::<_, String>(0), "К2");
    assert_eq!(rows[2].get::<_, Numeric>(1), Numeric("2.50".to_owned()));
    let sql = harness.statements().join("\n");
    assert!(sql.contains("нет"), "{sql}");
}

#[tokio::test]
async fn dollar_slots_follow_first_occurrence_of_sdbl_names() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let statement = client
        .prepare_typed(
            &format!(
                "{SELECT} ГДЕ Количество >= &Порог И Категория = &Кат И Количество < &Порог + 10"
            ),
            &[Type::INT4, Type::VARCHAR],
        )
        .await
        .unwrap();
    assert_eq!(statement.params(), [Type::INT4, Type::VARCHAR]);
    client.query(&statement, &[&5_i32, &"К1"]).await.unwrap();
    let sql = harness.statements().join("\n");
    assert!(sql.contains(">= 5") && sql.contains("'К1'"), "{sql}");
}

#[tokio::test]
async fn date_reference_list_and_unsupported_parameters() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let statement = client
        .prepare_typed(
            &format!(
                "{SELECT} ГДЕ &Начало >= ДАТАВРЕМЯ(2000, 1, 1) И &Склад = &Склад И Категория В (&Коды)"
            ),
            &[Type::TIMESTAMP, Type::BYTEA, Type::TEXT_ARRAY],
        )
        .await
        .unwrap();
    let start = std::time::UNIX_EPOCH + Duration::from_secs(1_704_067_200);
    client
        .query(&statement, &[&start, &vec![7_u8; 16], &vec!["К1", "К2"]])
        .await
        .unwrap();
    let sql = harness.statements().join("\n");
    assert!(sql.contains("2024-01-01"), "{sql}");
    assert!(sql.contains("07070707070707070707070707070707"), "{sql}");
    assert!(sql.contains("'К1'") && sql.contains("'К2'"), "{sql}");
    let statement = client
        .prepare_typed(&format!("{SELECT} ГДЕ Категория = &Кат"), &[Type::JSONB])
        .await
        .unwrap();
    let error = client.query(&statement, &[&Jsonb]).await.unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::DATATYPE_MISMATCH));
}

#[tokio::test]
async fn portal_with_a_row_limit_suspends() {
    let harness = Harness::new(Options {
        responder: register_rows(250),
        ..Options::default()
    });
    let mut client = connect(&harness).await;
    let transaction = client.transaction().await.unwrap();
    let statement = transaction.prepare(SELECT).await.unwrap();
    let portal = transaction.bind(&statement, &[]).await.unwrap();
    let mut counts = Vec::new();
    for _ in 0..3 {
        counts.push(transaction.query_portal(&portal, 100).await.unwrap().len());
    }
    assert_eq!(counts, [100, 100, 50]);
    transaction.commit().await.unwrap();
    assert_eq!(harness.db_events(), ["open", "begin", "commit"]);
}

#[tokio::test]
async fn syntax_error_position_reaches_the_client() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let error = client
        .simple_query("ВЫБРАТЬ ИЗ РегистрНакопления.ЗамерОстатков")
        .await
        .unwrap_err();
    let error = error.as_db_error().unwrap();
    assert_eq!(error.code(), &SqlState::SYNTAX_ERROR);
    assert_eq!(error.position(), Some(&ErrorPosition::Original(9)));
}

#[tokio::test]
async fn dbeaver_connection_test() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    client
        .simple_query("SET application_name = 'DBeaver'")
        .await
        .unwrap();
    let rows = client.query("SELECT version()", &[]).await.unwrap();
    assert!(rows[0].get::<_, String>(0).contains("open-sdbl"));
    let one = client.query_one("SELECT 1", &[]).await.unwrap();
    assert_eq!(one.get::<_, i32>(0), 1);
}

#[tokio::test]
async fn datagrip_bootstrap_reports_configured_database_and_stable_server_time() {
    let harness = Harness::new(Options::default());
    let mut cfg = config();
    cfg.dbname(USER);
    let (client, connection) = cfg.connect_raw(harness.connect(), NoTls).await.unwrap();
    tokio::spawn(connection);
    let ssl = "select ssl from pg_stat_ssl where pid = pg_backend_pid()";
    assert!(!client.query_one(ssl, &[]).await.unwrap().get::<_, bool>(0));
    let epoch = "select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time";
    let row = client.query_one(epoch, &[]).await.unwrap();
    assert_eq!(row.columns()[0].type_(), &Type::NUMERIC);
    assert_eq!(row.get::<_, Numeric>(0), Numeric("1700000000".into()));
    let other = connect(&harness).await;
    assert_eq!(
        other
            .query_one(epoch, &[])
            .await
            .unwrap()
            .get::<_, Numeric>(0),
        Numeric("1700000000".into())
    );
    let rows = client
        .query(
            include_str!("../../tests/fixtures/catalog-corpus/datagrip-list-databases.sql"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(1), "unf");
    assert_eq!(rows[0].get::<_, Option<String>>(2), None);
    for sql in [
        "SELECT current_database()",
        "SELECT current_catalog",
        "SELECT catalog_name FROM information_schema.schemata LIMIT 1",
        "SELECT table_catalog FROM information_schema.tables LIMIT 1",
    ] {
        assert_eq!(
            client
                .query_one(sql, &[])
                .await
                .unwrap()
                .get::<_, String>(0),
            "unf",
            "{sql}"
        );
    }
    let snapshot = fixture::snapshot();
    let access = fixture::store(&snapshot, true);
    harness.state.replace(GatewayState::from_parts(
        snapshot,
        access,
        DatabaseDialect::Postgres,
    ));
    let text = client.simple_query(epoch).await.unwrap();
    assert!(
        text.iter()
            .any(|m| matches!(m,SimpleQueryMessage::Row(r) if r.get(0)==Some("1700000000")))
    );
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn jdbc_string_probes_use_local_text_results_and_preserve_error_recovery() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let before = harness.db_events();
    for (sql, expected) in [
        ("SELECT 'keep alive'", "keep alive"),
        ("SELECT 'Hello'", "Hello"),
        ("SELECT ''", ""),
        (
            "SELECT 'I''m alive; -- /* Привет' AS \"KeepAlive\"",
            "I'm alive; -- /* Привет",
        ),
    ] {
        let messages = client.simple_query(sql).await.unwrap();
        let row = messages
            .iter()
            .find_map(|message| match message {
                SimpleQueryMessage::Row(row) => Some(row),
                _ => None,
            })
            .unwrap();
        assert_eq!(row.get(0), Some(expected));
        // tokio-postgres requests a binary result from the prepared path.
        let statement = client.prepare(sql).await.unwrap();
        assert_eq!(statement.columns()[0].type_(), &Type::TEXT);
        let row = client.query_one(&statement, &[]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), expected);
    }
    assert_eq!(harness.db_events(), before);
    assert!(harness.statements().is_empty());
    client.batch_execute("BEGIN").await.unwrap();
    let error = client
        .simple_query("SELECT 'alive' FROM _Reference42")
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::SYNTAX_ERROR));
    assert_eq!(
        error.as_db_error().unwrap().position(),
        Some(&ErrorPosition::Original(8))
    );
    let error = client
        .simple_query("SELECT 'keep alive'")
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::IN_FAILED_SQL_TRANSACTION));
    client.batch_execute("ROLLBACK").await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT 'recovered'", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "recovered"
    );
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn cancel_token_cancels_the_running_query() {
    let harness = Arc::new(Harness::new(Options {
        delay: Some(Duration::from_secs(30)),
        ..Options::default()
    }));
    let client = connect(&harness).await;
    let token = client.cancel_token();
    let running = tokio::spawn(async move { client.simple_query(SELECT).await.map(|_| client) });
    tokio::time::sleep(Duration::from_millis(100)).await;
    token
        .cancel_query_raw(harness.connect(), NoTls)
        .await
        .unwrap();
    let error = running.await.unwrap().expect_err("the query is cancelled");
    assert_eq!(error.code(), Some(&SqlState::QUERY_CANCELED));
}

#[tokio::test]
async fn refreshed_state_applies_to_new_statements() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    client.simple_query(SELECT).await.unwrap();
    let snapshot = fixture::snapshot();
    let access = fixture::store(&snapshot, false);
    harness.state.replace(GatewayState::from_parts(
        snapshot,
        access,
        open_sdbl_db::DatabaseDialect::Postgres,
    ));
    let error = client.simple_query(SELECT).await.unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
}

/// Accepts any server certificate: the test identity is self-signed.
#[derive(Debug)]
struct AnyCertificate(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub(crate) fn server_tls() -> Arc<rustls::ServerConfig> {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let certificates = CertificateDer::pem_file_iter(format!("{directory}/gateway-cert.pem"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(format!("{directory}/gateway-key.pem")).unwrap();
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .unwrap(),
    )
}

#[tokio::test]
async fn tls_is_negotiated_when_the_client_requires_it() {
    use tokio_postgres::tls::MakeTlsConnect;
    let harness = Harness::new(Options {
        tls: Some(server_tls()),
        insecure_loopback: false,
        ..Options::default()
    });
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let client_config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
        .with_no_client_auth();
    let mut make = tokio_postgres_rustls::MakeRustlsConnect::new(client_config);
    let tls = <tokio_postgres_rustls::MakeRustlsConnect as MakeTlsConnect<
        tokio::io::DuplexStream,
    >>::make_tls_connect(&mut make, "localhost")
    .unwrap();
    let mut config = config();
    config.ssl_mode(tokio_postgres::config::SslMode::Require);
    let (client, connection) = config.connect_raw(harness.connect(), tls).await.unwrap();
    tokio::spawn(connection);
    assert_eq!(client.simple_query(SELECT).await.unwrap().len(), 5);
    assert!(
        client
            .query_one(
                "select ssl from pg_stat_ssl where pid = pg_backend_pid()",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    // Without TLS the same gateway asks for no password at all.
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0003_0000, &[("user", USER)]).await;
    assert_eq!(super::protocol::error_of(&raw.rest().await)[&b'C'], "28000");
}

#[tokio::test]
async fn deferred_reference_presentation_is_utf8_text_for_mssql_and_postgres() {
    use open_sdbl::metadata::{DbNames, resolve_metadata};
    let register = support::universal_dereferenced_presentation_snapshot();
    let document = section_reference::snapshot();
    let mut entries = register.db_names().entries().to_vec();
    entries.extend_from_slice(document.db_names().entries());
    let mut descriptors = register.descriptors().to_vec();
    descriptors.extend_from_slice(document.descriptors());
    let mut schema = register.schema().clone();
    schema.tables.extend(document.schema().tables.clone());
    let mut live = register.live_tables().to_vec();
    live.extend_from_slice(document.live_tables());
    let snapshot =
        resolve_metadata(DbNames::from_entries(entries), descriptors, schema, live).snapshot;
    let mut access = AccessStore::new(&snapshot);
    let base = fixture::snapshot();
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    let expected = "Экземпляр бюджета 000000001 от 2023-12-14 14:57:01";
    let query = "ВЫБРАТЬ ПЕРВЫЕ 10 ПредставлениеСсылки(Д.ДоговорКонтрагента) ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д";
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let harness = Harness::new(Options {
            snapshot: Some(snapshot.clone()),
            access: Some(access.clone()),
            dialect,
            unrestricted_access: true,
            responder: Arc::new(move |sql, _width| {
                if sql.contains("__presentation_target") {
                    Ok(vec![vec![
                        Cell::Bytes(vec![1; 16]),
                        Cell::Text(expected.into()),
                    ]])
                } else {
                    let mut payload = 1_u32.to_be_bytes().to_vec();
                    payload.extend([1; 16]);
                    Ok(vec![vec![Cell::Bytes(payload)]])
                }
            }),
            ..Options::default()
        });
        let mut raw = super::protocol::logged_in(&harness).await;
        let frames = raw.query(query).await;
        assert_eq!(super::protocol::columns(&frames)[0].1, 25);
        assert_eq!(
            super::protocol::rows(&frames)[0][0].as_deref(),
            Some(expected)
        );
        let client = connect(&harness).await;
        let simple = client.simple_query(query).await.unwrap();
        let row = simple
            .iter()
            .find_map(|message| match message {
                SimpleQueryMessage::Row(row) => Some(row),
                _ => None,
            })
            .unwrap();
        assert_eq!(row.get(0), Some(expected));
        let statement = client.prepare(query).await.unwrap();
        assert_eq!(statement.columns()[0].type_(), &Type::TEXT);
        let reference = client
            .prepare("ВЫБРАТЬ Д.Ссылка ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д")
            .await
            .unwrap();
        assert_eq!(reference.columns()[0].type_(), &Type::BYTEA);
        let row = client.query_one(&statement, &[]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), expected);
        harness.stop();
    }
}
