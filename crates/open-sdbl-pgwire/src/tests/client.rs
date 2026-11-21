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
    use super::{DEFERRED_PRESENTATION_QUERY as query, DEFERRED_PRESENTATION_VALUE as expected};
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let harness = deferred_presentation_harness(dialect);
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

#[tokio::test]
async fn enumeration_presentations_are_text_in_both_protocols_and_backends() {
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let snapshot = support::enumeration_value_snapshot();
        let base = fixture::snapshot();
        let mut access = AccessStore::new(&snapshot);
        access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
        let harness = Harness::new(Options {
            snapshot: Some(snapshot),
            access: Some(access),
            dialect,
            unrestricted_access: true,
            responder: Arc::new(|sql, columns| {
                assert!(sql.contains("9022249e3a1ac4b94be8faddd2f8bde9"), "{sql}");
                assert!(sql.contains("Статус"), "{sql}");
                Ok(vec![vec![Cell::Text("Статус".into()); columns]])
            }),
            ..Options::default()
        });
        let client = connect(&harness).await;
        let query = "SELECT PRESENTATION(Ссылка), REFPRESENTATION(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов";
        let messages = client.simple_query(query).await.unwrap();
        let row = messages
            .iter()
            .find_map(|message| match message {
                SimpleQueryMessage::Row(row) => Some(row),
                _ => None,
            })
            .unwrap();
        assert_eq!(row.get(0), Some("Статус"));
        assert_eq!(row.get(1), Some("Статус"));
        let statement = client.prepare(query).await.unwrap();
        assert!(
            statement
                .columns()
                .iter()
                .all(|column| column.type_() == &Type::TEXT)
        );
        let row = client.query_one(&statement, &[]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), "Статус");
        assert_eq!(row.get::<_, String>(1), "Статус");
        harness.stop();
    }
}

#[tokio::test]
async fn deferred_enumeration_values_resolve_using_the_metadata_plan() {
    let value: [u8; 16] = [
        0x90, 0x22, 0x24, 0x9e, 0x3a, 0x1a, 0xc4, 0xb9, 0x4b, 0xe8, 0xfa, 0xdd, 0xd2, 0xf8, 0xbd,
        0xe9,
    ];
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let snapshot = support::enumeration_value_snapshot();
        let base = fixture::snapshot();
        let mut access = AccessStore::new(&snapshot);
        access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
        let harness = Harness::new(Options {
            snapshot: Some(snapshot),
            access: Some(access),
            dialect,
            unrestricted_access: true,
            responder: Arc::new(move |sql, _| {
                if sql.contains("__presentation_target") {
                    assert!(sql.contains("Статус"), "{sql}");
                    Ok(vec![vec![
                        Cell::Bytes(value.to_vec()),
                        Cell::Text("Статус".into()),
                    ]])
                } else {
                    let mut payload = 99_u32.to_be_bytes().to_vec();
                    payload.extend_from_slice(&value);
                    Ok(vec![vec![Cell::Bytes(payload)]])
                }
            }),
            ..Options::default()
        });
        let client = connect(&harness).await;
        for function in ["PRESENTATION", "REFPRESENTATION"] {
            for from in ["", " FROM Enumeration.бит_ВидыСтатусовОбъектов"] {
                let query = format!(
                    "SELECT {function}(VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус)){from}"
                );
                let statement = client.prepare(&query).await.unwrap();
                assert_eq!(statement.columns()[0].type_(), &Type::TEXT);
                let row = client.query_one(&statement, &[]).await.unwrap();
                assert_eq!(row.get::<_, String>(0), "Статус");
            }
        }
        harness.stop();
    }
}

#[tokio::test]
async fn enumeration_presentation_does_not_bypass_denied_target() {
    use open_sdbl::metadata::{Guid, MetadataKind};
    let snapshot = support::enumeration_value_snapshot();
    let object = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let base = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, false);
    rights.objects[0].object = snapshot.object_by_id(object).unwrap().guid.clone();
    access.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights);
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let query = "SELECT PRESENTATION(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов";
    let error = client.query(query, &[]).await.unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn selected_enumeration_reference_is_text_but_uuid_stays_uuid() {
    let value: [u8; 16] = [
        0x90, 0x22, 0x24, 0x9e, 0x3a, 0x1a, 0xc4, 0xb9, 0x4b, 0xe8, 0xfa, 0xdd, 0xd2, 0xf8, 0xbd,
        0xe9,
    ];
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: open_sdbl::query::MsSqlBackend::default(),
        },
    ] {
        let snapshot = support::enumeration_value_snapshot();
        let base = fixture::snapshot();
        let mut access = AccessStore::new(&snapshot);
        access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
        let harness = Harness::new(Options {
            snapshot: Some(snapshot),
            access: Some(access),
            dialect,
            unrestricted_access: true,
            responder: Arc::new(move |sql, _| {
                if sql.contains("__presentation_target") {
                    return Ok(vec![vec![
                        Cell::Bytes(value.to_vec()),
                        Cell::Text("Статус".into()),
                    ]]);
                }
                assert!(sql.contains("DISTINCT"), "{sql}");
                let row = |reference, presentation: &str| {
                    vec![
                        reference,
                        Cell::Uuid([7; 16]),
                        Cell::Text(presentation.into()),
                        Cell::Text(presentation.into()),
                        Cell::Text("Enumeration".into()),
                    ]
                };
                Ok(vec![
                    row(Cell::Bytes(value.to_vec()), "Статус"),
                    row(Cell::Bytes(vec![0; 16]), ""),
                    row(Cell::Bytes(vec![0xff; 16]), ""),
                    row(Cell::Null, ""),
                ])
            }),
            ..Options::default()
        });
        let client = connect(&harness).await;
        let query = "SELECT DISTINCT Ссылка, UUID(Ссылка), PRESENTATION(Ссылка), REFPRESENTATION(Ссылка), VALUETYPE(Ссылка) FROM Enumeration.бит_ВидыСтатусовОбъектов";
        let simple = client.simple_query(query).await.unwrap();
        let rows: Vec<_> = simple
            .iter()
            .filter_map(|message| match message {
                SimpleQueryMessage::Row(row) => Some(row),
                _ => None,
            })
            .collect();
        assert_eq!(rows[0].get(0), Some("Статус"));
        assert_eq!(rows[0].get(2), Some("Статус"));
        assert_eq!(rows[0].get(3), Some("Статус"));
        assert_eq!(rows[1].get(0), Some(""));
        assert_eq!(rows[2].get(0), Some(""));
        assert_eq!(rows[3].get(0), None);
        let statement = client.prepare(query).await.unwrap();
        assert_eq!(statement.columns()[0].type_(), &Type::TEXT);
        assert_eq!(statement.columns()[1].type_(), &Type::UUID);
        let rows = client.query(&statement, &[]).await.unwrap();
        assert_eq!(rows[0].get::<_, String>(0), "Статус");
        assert_eq!(rows[0].get::<_, String>(2), "Статус");
        assert_eq!(rows[0].get::<_, String>(3), "Статус");
        assert_eq!(rows[1].get::<_, String>(0), "");
        assert_eq!(rows[2].get::<_, String>(0), "");
        assert_eq!(rows[3].get::<_, Option<String>>(0), None);
        let metadata = client.query_one(
            "SELECT data_type FROM information_schema.columns WHERE table_schema = 'Перечисление' AND table_name = 'бит_ВидыСтатусовОбъектов' AND column_name = 'Ссылка'",
            &[],
        ).await.unwrap();
        assert_eq!(metadata.get::<_, String>(0), "text");
        assert!(
            harness
                .statements()
                .iter()
                .any(|sql| sql.contains("__presentation_target"))
        );
        harness.stop();
    }
}

#[tokio::test]
async fn selected_enum_presentation_respects_denied_target() {
    use open_sdbl::metadata::{Guid, MetadataKind};
    let snapshot = support::enumeration_value_snapshot();
    let enumeration = snapshot
        .object_id(MetadataKind::Enumeration, "бит_ВидыСтатусовОбъектов")
        .unwrap();
    let fixture = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&fixture, true).users().unwrap().to_vec());
    let mut rights = fixture::rights(&fixture, false);
    rights.objects[0].object = snapshot.object_by_id(enumeration).unwrap().guid.clone();
    access.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights);
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        responder: Arc::new(|sql, _| {
            if sql.contains("__presentation_target") {
                assert!(sql.contains("WHERE FALSE"), "{sql}");
                Ok(vec![])
            } else {
                Ok(vec![vec![Cell::Bytes(vec![1; 16])]])
            }
        }),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let row = client
        .query_one(
            "SELECT VALUE(Enumeration.бит_ВидыСтатусовОбъектов.Статус)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "");
    assert!(
        harness
            .statements()
            .iter()
            .any(|sql| sql.contains("WHERE FALSE"))
    );
    harness.stop();
}

#[tokio::test]
async fn selected_enumerations_keep_streaming_beyond_the_buffer_limit() {
    let snapshot = support::enumeration_value_snapshot();
    let base = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    let reference = vec![
        0x90, 0x22, 0x24, 0x9e, 0x3a, 0x1a, 0xc4, 0xb9, 0x4b, 0xe8, 0xfa, 0xdd, 0xd2, 0xf8, 0xbd,
        0xe9,
    ];
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        unrestricted_access: true,
        responder: Arc::new(move |sql, _| {
            if sql.contains("__presentation_target") {
                Ok(vec![vec![
                    Cell::Bytes(reference.clone()),
                    Cell::Text("Статус".into()),
                ]])
            } else {
                Ok((0..1050)
                    .map(|_| vec![Cell::Bytes(reference.clone())])
                    .collect())
            }
        }),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let rows = client
        .query(
            "SELECT Ссылка FROM Enumeration.бит_ВидыСтатусовОбъектов",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1050);
    assert!(rows.iter().all(|row| row.get::<_, String>(0) == "Статус"));
    harness.stop();
}

#[tokio::test]
async fn metadata_inspection_serves_native_resources_to_restricted_users() {
    use crate::inspection::fixture::{FORM, OWNER, TEMPLATE, index};

    let harness = Harness::new(Options {
        read: false,
        portal_buffer_rows: 1,
        ..Options::default()
    });
    let snapshot = fixture::snapshot();
    let access = fixture::store(&snapshot, false);
    harness.state.replace(
        GatewayState::from_parts(snapshot, access, DatabaseDialect::Postgres)
            .with_inspection(index()),
    );
    let mut client = connect(&harness).await;
    let list = client
        .simple_query("SELECT * FROM onec_find_metadata();")
        .await
        .unwrap();
    assert!(list.iter().any(
        |message| matches!(message, SimpleQueryMessage::Row(row) if row.get(0) == Some(FORM))
    ));
    let filter = client
        .query(
            "select * from onec_find_metadata($1)",
            &[&"дополнительноеописание.модуль"],
        )
        .await
        .unwrap();
    assert_eq!(filter.len(), 1);
    assert_eq!(filter[0].get::<_, &str>("metadata_path"), FORM);
    let statement = client
        .prepare("select onec_show_metadata($1)")
        .await
        .unwrap();
    assert_eq!(statement.params(), &[Type::TEXT]);
    let row = client.query_one(&statement, &[&FORM]).await.unwrap();
    let source = row.get::<_, &str>(0);
    let expected =
        include_str!("../../../../tests/fixtures/data-processor-header2/form-profile49.bsl");
    assert_eq!(source, expected.trim_start_matches('\u{feff}'));
    let xml = client.query_one(&statement, &[&TEMPLATE]).await.unwrap();
    let xml = roxmltree::Document::parse(xml.get(0)).unwrap();
    let exported =
        include_str!("../../../../tests/fixtures/pgwire-metadata-inspection/dcs.designer.xml");
    let exported = roxmltree::Document::parse(exported).unwrap();
    let variant = |doc: &roxmltree::Document<'_>| {
        doc.descendants()
            .find(|node| node.has_tag_name("settingsVariant"))
            .and_then(|node| node.first_element_child())
            .and_then(|node| node.text())
            .unwrap()
            .to_owned()
    };
    assert_eq!(variant(&xml), variant(&exported));
    let module = format!("{OWNER}.МодульОбъекта.bsl");
    let row = client.query_one(&statement, &[&module]).await.unwrap();
    assert!(row.get::<_, &str>(0).contains("КонецФункции"));
    let processing = client.query_one(&statement, &[&OWNER]).await.unwrap();
    assert_eq!(processing.get::<_, &str>(0), row.get::<_, &str>(0));

    let html = format!("{OWNER}.Макеты.КакУстановитьИлиОбновитьВерсиюПлатформы1СПредприятие");
    let err = client.query(&statement, &[&html]).await.unwrap_err();
    assert_eq!(err.code(), Some(&SqlState::FEATURE_NOT_SUPPORTED));
    let absent = "metadata'); SELECT * FROM _Reference42; --";
    let err = client.query(&statement, &[&absent]).await.unwrap_err();
    assert_eq!(err.code().unwrap().code(), "42704");
    let err = client
        .simple_query("SELECT * FROM onec_find_metadata('it''s safe'); SELECT 1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Some(&SqlState::SYNTAX_ERROR));

    let transaction = client.transaction().await.unwrap();
    let find = transaction
        .prepare("SELECT * FROM onec_find_metadata($1)")
        .await
        .unwrap();
    let portal = transaction
        .bind(&find, &[&"дополнительноеописание"])
        .await
        .unwrap();
    let first = transaction.query_portal(&portal, 1).await.unwrap();
    let rest = transaction.query_portal(&portal, 0).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(rest.len(), 1);
    assert_ne!(first[0].get::<_, &str>(0), rest[0].get::<_, &str>(0));
    let row = transaction.query_one(&statement, &[&FORM]).await.unwrap();
    assert_eq!(row.get::<_, &str>(0), source);
    transaction.commit().await.unwrap();

    let mut bad = config();
    bad.password("wrong");
    assert!(bad.connect_raw(harness.connect(), NoTls).await.is_err());
    assert!(harness.statements().is_empty());
}

const SALES_OBJECT_LINK: &str =
    "e1cib/data/Документ.РеализацияТоваровУслуг?ref=90c202d78b8d8e9911f1d77c464243d8";

#[tokio::test]
async fn dcs_queries_from_xml_over_both_pgwire_protocols() {
    const CONTENT: &str = r#"
    <d:dataSource><d:name>Источник</d:name></d:dataSource>
    <d:dataSet xsi:type="d:DataSetQuery">
        <d:name>Остатки</d:name>
        <d:query>  ВЫБРАТЬ &amp;Период
<![CDATA[ГДЕ Сумма < 0]]><!-- split text --> И Код = "А"</d:query>
    </d:dataSet>
    <d:dataSet xsi:type="d:DataSetObject"><d:name>Объект</d:name></d:dataSet>
    <d:dataSet xsi:type="d:DataSetUnion">
        <d:name>Объединение</d:name>
        <d:item xsi:type="d:DataSetQuery"><d:name>Заказы</d:name><d:query>ВЫБРАТЬ 2</d:query></d:item>
        <d:item xsi:type="d:DataSetQuery"><d:query/></d:item>
    </d:dataSet>"#;
    const NS: &str = r#"xmlns:d="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#;
    let expected = [
        (
            Some("Остатки"),
            "  ВЫБРАТЬ &Период\nГДЕ Сумма < 0 И Код = \"А\"",
        ),
        (Some("Заказы"), "ВЫБРАТЬ 2"),
        (None, ""),
    ];
    let harness = Harness::new(Options {
        read: false,
        portal_buffer_rows: 2,
        ..Options::default()
    });
    let mut client = connect(&harness).await;
    let exported =
        format!("\u{feff}<d:DataCompositionSchema {NS}>{CONTENT}</d:DataCompositionSchema>");
    let simple = client
        .simple_query(&format!("SELECT * FROM onec_dcs_queries('{exported}');"))
        .await
        .unwrap();
    assert_eq!(
        simple
            .iter()
            .filter_map(|message| match message {
                SimpleQueryMessage::Row(row) => Some((row.get("name"), row.get("query").unwrap())),
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected
    );

    let native = format!(
        r#"<SchemaFile xmlns=""><d:dataCompositionSchema {NS}>{CONTENT}{}</d:dataCompositionSchema></SchemaFile>"#,
        " ".repeat(9000)
    );
    let statement = client
        .prepare("SELECT * FROM onec_dcs_queries($1)")
        .await
        .unwrap();
    assert_eq!(statement.params(), &[Type::TEXT]);
    assert_eq!(
        statement
            .columns()
            .iter()
            .map(|column| (column.name(), column.type_()))
            .collect::<Vec<_>>(),
        [("name", &Type::TEXT), ("query", &Type::TEXT)]
    );
    let transaction = client.transaction().await.unwrap();
    let portal = transaction.bind(&statement, &[&native]).await.unwrap();
    let first = transaction.query_portal(&portal, 1).await.unwrap();
    let rest = transaction.query_portal(&portal, 0).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(rest.len(), 2);
    assert_eq!(
        first
            .iter()
            .chain(&rest)
            .map(|row| (row.get::<_, Option<&str>>(0), row.get::<_, &str>(1)))
            .collect::<Vec<_>>(),
        expected
    );
    transaction.commit().await.unwrap();
    assert!(
        client
            .query(&statement, &[&None::<&str>])
            .await
            .unwrap()
            .is_empty()
    );
    for (xml, code) in [
        ("<broken>", "2200M"),
        ("<!DOCTYPE root SYSTEM 'file:///a'><root/>", "2200M"),
        ("<root/>", "22023"),
    ] {
        let error = client.query(&statement, &[&xml]).await.unwrap_err();
        assert_eq!(error.code().unwrap().code(), code);
    }
    let excessive_depth = format!("{}{}", "<element>".repeat(129), "</element>".repeat(129));
    let error = client
        .query(&statement, &[&excessive_depth])
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "54000");
    assert!(harness.statements().is_empty());
}

#[tokio::test]
async fn object_link_returns_document_attributes_in_both_protocols_and_dialects() {
    use open_sdbl::query::{
        CompileOptions, LogicalResultValue, MsSqlBackend, PostgresBackend, QueryCompiler,
    };
    use serde_json::Value;

    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/buh");
    let snapshot = support::demo_resolved_at(&base).snapshot;
    let mut operator = SessionParameters::new();
    for field in snapshot.separators() {
        if let Some(constant) = field
            .separation
            .as_ref()
            .and_then(|separation| separation.conditional_separation.clone())
        {
            operator.set_separation_condition(constant, false);
        }
    }
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::MsSql {
            backend: MsSqlBackend::default(),
        },
    ] {
        let lookup = crate::object::Lookup::resolve(&snapshot, SALES_OBJECT_LINK).unwrap();
        let parameters = [lookup.reference];
        let options = CompileOptions::new()
            .parameters(&parameters)
            .session(&operator);
        let plan = match dialect {
            DatabaseDialect::Postgres => {
                QueryCompiler::new(&snapshot, PostgresBackend).compile_with(&lookup.query, &options)
            }
            DatabaseDialect::MsSql { backend } => {
                QueryCompiler::new(&snapshot, backend).compile_with(&lookup.query, &options)
            }
        }
        .unwrap();
        let projection = plan
            .logical_projection
            .as_ref()
            .expect("exact sales document projection");
        let mut cells = vec![Cell::Null; plan.columns.len()];
        for (name, cell) in [
            (
                "Ссылка",
                Cell::Bytes(crate::values::decode_hex("90c202d78b8d8e9911f1d77c464243d8").unwrap()),
            ),
            (
                "Date",
                Cell::DateTime(open_sdbl_db::DateTimeParts {
                    year: 2026,
                    month: 11,
                    day: 20,
                    hour: 12,
                    minute: 30,
                    second: 0,
                }),
            ),
            ("Number", Cell::Text("Тест-001".into())),
            (
                "Комментарий",
                Cell::Text("Первая строка\n\"Вторая строка\"".into()),
            ),
            ("Posted", Cell::Bool(true)),
            ("СуммаДокумента", Cell::Number("9007199254740993.25".into())),
        ] {
            let field = projection
                .iter()
                .find(|field| field.name == name)
                .unwrap_or_else(|| panic!("missing logical field {name}"));
            let LogicalResultValue::Scalar { column } = field.value else {
                panic!("{name} is a scalar attribute");
            };
            cells[column] = cell;
        }
        let expected_count = projection.len();
        let harness = Harness::new(Options {
            snapshot: Some(snapshot.clone()),
            access: Some(fixture::store(&fixture::snapshot(), true)),
            dialect,
            operator: operator.clone(),
            unrestricted_access: true,
            responder: Arc::new(move |sql, width| {
                let sql = sql.to_ascii_lowercase();
                assert!(
                    sql.contains("_document1009") && sql.contains("_idrref"),
                    "{sql}"
                );
                assert!(!sql.contains("e1cib") && !sql.contains("_document1009_vt"));
                if sql.contains("90c202d78b8d8e9911f1d77c464243d8") {
                    assert_eq!(cells.len(), width);
                    Ok(vec![cells.clone()])
                } else {
                    Ok(Vec::new())
                }
            }),
            ..Options::default()
        });
        let client = connect(&harness).await;
        let statement = client.prepare("SELECT onec_get_object($1)").await.unwrap();
        assert_eq!(statement.params(), &[Type::TEXT]);
        assert_eq!(statement.columns()[0].type_(), &Type::JSONB);
        let data: Value = client
            .query_one(&statement, &[&SALES_OBJECT_LINK])
            .await
            .unwrap()
            .get(0);
        assert_eq!(data["Ссылка"], SALES_OBJECT_LINK);
        assert_eq!(data["Номер"], "Тест-001");
        assert_eq!(data["Дата"], "2026-11-20 12:30:00");
        assert_eq!(data["Комментарий"], "Первая строка\n\"Вторая строка\"");
        assert_eq!(data["Проведен"], true);
        assert_eq!(
            data["СуммаДокумента"],
            "9007199254740993.25".parse::<Value>().unwrap()
        );
        assert_eq!(data["Организация"], Value::Null);
        assert!(!data.as_object().unwrap().contains_key("Товары"));
        assert_eq!(data.as_object().unwrap().len(), expected_count);
        let simple = client
            .simple_query(&format!("SELECT onec_get_object('{SALES_OBJECT_LINK}')"))
            .await
            .unwrap();
        let row = simple
            .iter()
            .find_map(|message| match message {
                SimpleQueryMessage::Row(row) => row.get(0),
                _ => None,
            })
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(row).unwrap(), data);

        let missing = SALES_OBJECT_LINK.replace("90c2", "90c3");
        let row = client.query_one(&statement, &[&missing]).await.unwrap();
        assert_eq!(row.get::<_, Option<Value>>(0), None);

        let before = harness.statements().len();
        for bad in [
            format!("https://other.example/base#{SALES_OBJECT_LINK}"),
            format!("{SALES_OBJECT_LINK}&ref=0011"),
            SALES_OBJECT_LINK.replace("Документ.", "Документ.%27"),
        ] {
            let error = client.query(&statement, &[&bad]).await.unwrap_err();
            assert_eq!(error.code(), Some(&SqlState::INVALID_PARAMETER_VALUE));
        }
        assert_eq!(harness.statements().len(), before);
        harness.stop();
    }
}

#[tokio::test]
async fn object_link_respects_document_rights_and_row_restrictions() {
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
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights.clone());
    let harness = Harness::new(Options {
        snapshot: Some(snapshot.clone()),
        access: Some(access.clone()),
        responder: Arc::new(|sql, _| {
            assert!(
                sql.contains("_document1") && sql.contains("__restricted") && sql.contains("NOT"),
                "{sql}"
            );
            Ok(Vec::new())
        }),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let statement = client.prepare("SELECT onec_get_object($1)").await.unwrap();
    let link = "e1cib/data/Документ.ПриходнаяНакладная?ref=90c202d78b8d8e9911f1d77c464243d8";
    let error = client.query(&statement, &[&link]).await.unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
    assert!(harness.statements().is_empty());

    // Exercise row filtering with the verified standard reference attribute.
    let mut schema = snapshot.schema().clone();
    schema
        .tables
        .iter_mut()
        .find(|t| t.name == "Document1")
        .unwrap()
        .columns
        .retain(|column| column.name == "ID");
    let mut live = snapshot.live_tables().to_vec();
    live.iter_mut()
        .find(|t| t.name == "_document1")
        .unwrap()
        .columns
        .retain(|column| column.name == "_idrref");
    let snapshot = open_sdbl::metadata::resolve_metadata(
        snapshot.db_names().clone(),
        snapshot.descriptors().to_vec(),
        schema,
        live,
    )
    .snapshot;
    harness.state.replace(GatewayState::from_parts(
        snapshot.clone(),
        access.clone(),
        DatabaseDialect::Postgres,
    ));
    let row = client.query_one(&statement, &[&link]).await.unwrap();
    assert_eq!(row.get::<_, Option<serde_json::Value>>(0), None);
    assert_eq!(harness.statements().len(), 1);

    rights.objects[0].rights[0].allowed = false;
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights);
    harness.state.replace(GatewayState::from_parts(
        snapshot,
        access,
        DatabaseDialect::Postgres,
    ));
    let error = client.query(&statement, &[&link]).await.unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
    assert_eq!(harness.statements().len(), 1);
    harness.stop();
}

#[tokio::test]
async fn object_link_returns_the_active_value_of_a_composite_attribute() {
    let snapshot = support::mixed_composite_snapshot();
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(fixture::store(&fixture::snapshot(), true)),
        unrestricted_access: true,
        responder: Arc::new(|_, width| {
            assert_eq!(width, 4);
            Ok(vec![vec![
                Cell::Bytes(vec![1; 16]),
                Cell::Bytes(vec![open_sdbl::query::TypeValue::String.tag()]),
                Cell::Text("Активная строка".into()),
                Cell::Bytes(vec![2; 16]),
            ]])
        }),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let value: serde_json::Value = client
        .query_one(
            "SELECT onec_get_object($1)",
            &[&"e1cib/data/Catalog.OpenSdblMetadataProbe?ref=01010101010101010101010101010101"],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(value["ProbeAttribute"], "Активная строка");
    assert_eq!(value.as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn report_variant_composes_authorized_attribute_reads_with_native_schema() {
    use crate::inspection::fixture::{REPORT_PATH, REPORT_SCHEMA, report_index};

    const VARIANT: &str =
        "e1cib/data/Справочник.ВариантыОтчетов?ref=835574d02b7dfd8c11e4b5b86e3446c5";
    const ATTRIBUTE_PATH: &str = "Отчет.ПолноеИмя";
    const PROPERTY: &str = "ОсновнаяСхемаКомпоновкиДанных";
    const LOOKUP: &str =
        "SELECT onec_show_metadata(onec_get_metadata_ref(onec_get_attribute_text($1,$2),$3))";

    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/buh");
    let snapshot = support::demo_resolved_at(&base).snapshot;
    let mut operator = SessionParameters::new();
    for field in snapshot.separators() {
        if let Some(constant) = field
            .separation
            .as_ref()
            .and_then(|separation| separation.conditional_separation.clone())
        {
            operator.set_separation_condition(constant, false);
        }
    }
    let base = fixture::snapshot();
    let mut rights = fixture::rights(&base, true);
    rights.objects[0].object = "0adf02da-940a-4bfc-afa5-59605494a169".parse().unwrap();
    let mut identifier = rights.objects[0].clone();
    identifier.object = "4062674a-4a81-494d-82bc-4984cd7c7903".parse().unwrap();
    rights.objects.push(identifier);
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&base, true).users().unwrap().to_vec());
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights.clone());
    let harness = Harness::new(Options {
        snapshot: Some(snapshot.clone()),
        access: Some(access.clone()),
        operator,
        responder: Arc::new(|sql, width| {
            let sql = sql.to_ascii_lowercase();
            assert!(!sql.contains("e1cib/") && !sql.contains("onec_"), "{sql}");
            if sql.contains("_reference87") {
                assert_eq!(width, 3, "{sql}");
                assert!(
                    sql.contains("_fld2751_rtref") && sql.contains("_fld2751_rrref"),
                    "{sql}"
                );
                if !sql.contains("835574d02b7dfd8c11e4b5b86e3446c5") {
                    return Ok(Vec::new());
                }
                let mut reference = 221_u32.to_be_bytes().to_vec();
                reference
                    .extend(crate::values::decode_hex("835574d02b7dfd8c11e4b5b860f11146").unwrap());
                Ok(vec![vec![
                    Cell::Bytes(vec![open_sdbl::query::TypeValue::TAG_REFERENCE]),
                    Cell::Text(String::new()),
                    Cell::Bytes(reference),
                ]])
            } else {
                assert_eq!(width, 1, "{sql}");
                assert!(
                    sql.contains("_reference221") && sql.contains("_fld6140"),
                    "{sql}"
                );
                assert!(sql.contains("835574d02b7dfd8c11e4b5b860f11146"), "{sql}");
                Ok(vec![vec![Cell::Text(REPORT_PATH.into())]])
            }
        }),
        ..Options::default()
    });
    harness.state.replace(
        GatewayState::from_parts(snapshot.clone(), access.clone(), DatabaseDialect::Postgres)
            .with_inspection(report_index()),
    );
    let client = connect(&harness).await;

    let statement = client.prepare(LOOKUP).await.unwrap();
    assert_eq!(statement.params(), &[Type::TEXT, Type::TEXT, Type::TEXT]);
    assert_eq!(statement.columns()[0].type_(), &Type::TEXT);
    let row = client
        .query_one(&statement, &[&VARIANT, &ATTRIBUTE_PATH, &PROPERTY])
        .await
        .unwrap();
    let xml = row.get::<_, &str>(0);
    assert!(
        roxmltree::Document::parse(xml)
            .unwrap()
            .descendants()
            .any(|node| node.has_tag_name("dataCompositionSchema"))
    );
    assert_eq!(harness.statements().len(), 2);

    // The native fixture's main schema only defines settings variants.
    let datasets = client.prepare(
        "SELECT * FROM onec_dcs_queries(onec_show_metadata(onec_get_metadata_ref(onec_get_attribute_text($1,$2),$3)))"
    ).await.unwrap();
    assert!(
        client
            .query(&datasets, &[&VARIANT, &ATTRIBUTE_PATH, &PROPERTY])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(harness.statements().len(), 4);

    let result = client.simple_query(&format!("SELECT onec_get_metadata_ref(onec_get_attribute_text('{VARIANT}','{ATTRIBUTE_PATH}'),'{PROPERTY}')")).await.unwrap();
    assert!(result.iter().any(|message| matches!(message, SimpleQueryMessage::Row(row) if row.get(0) == Some(REPORT_SCHEMA))));
    assert_eq!(harness.statements().len(), 6);

    let missing = VARIANT.replace("46c5", "46c6");
    let row = client
        .query_one(&statement, &[&missing, &ATTRIBUTE_PATH, &PROPERTY])
        .await
        .unwrap();
    assert_eq!(row.get::<_, Option<&str>>(0), None);
    assert_eq!(harness.statements().len(), 7);

    assert!(
        client
            .query(&datasets, &[&missing, &ATTRIBUTE_PATH, &PROPERTY])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(harness.statements().len(), 8);

    let invalid = client
        .simple_query(&format!(
            "SELECT onec_show_metadata(onec_get_object('{VARIANT}'))"
        ))
        .await
        .unwrap_err();
    assert_eq!(invalid.code(), Some(&SqlState::DATATYPE_MISMATCH));
    assert_eq!(harness.statements().len(), 8);

    let mssql = DatabaseDialect::MsSql {
        backend: open_sdbl::query::MsSqlBackend::default(),
    };
    harness.state.replace(
        GatewayState::from_parts(snapshot.clone(), access.clone(), mssql)
            .with_inspection(report_index()),
    );
    let row = client
        .query_one(&statement, &[&VARIANT, &ATTRIBUTE_PATH, &PROPERTY])
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), xml);
    assert_eq!(harness.statements().len(), 10);

    rights.objects[1].rights[0].allowed = false;
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights);
    harness.state.replace(
        GatewayState::from_parts(snapshot, access, DatabaseDialect::Postgres)
            .with_inspection(report_index()),
    );
    let error = client
        .query(&statement, &[&VARIANT, &ATTRIBUTE_PATH, &PROPERTY])
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
    assert_eq!(harness.statements().len(), 11);
    harness.stop();
}

#[tokio::test]
async fn a_backing_session_startup_failure_does_not_stop_the_listener() {
    let harness = Harness::new(Options::default());
    harness.journal.lock().unwrap().fail_open_once = true;
    let failure = config()
        .connect_raw(harness.connect(), NoTls)
        .await
        .err()
        .unwrap();
    let failure = failure.as_db_error().unwrap();
    assert_eq!(failure.code(), &SqlState::CONNECTION_FAILURE);
    assert_eq!(failure.message(), "the gateway cannot reach the database");
    assert!(failure.detail().is_none());

    let client = connect(&harness).await;
    assert!(!client.simple_query("SELECT 1").await.unwrap().is_empty());
}
