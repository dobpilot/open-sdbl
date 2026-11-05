//! Raw protocol exchanges with the gateway over an in-memory stream.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use open_sdbl::password::{PasswordState, SourceProfile};
use open_sdbl::query::{ParameterValue, QueryParameter, SessionParameters};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use super::*;

pub(crate) struct Raw {
    pub(crate) stream: DuplexStream,
}

pub(crate) type Frame = (u8, Vec<u8>);

impl Raw {
    pub(crate) async fn startup(&mut self, version: u32, parameters: &[(&str, &str)]) {
        let mut body = version.to_be_bytes().to_vec();
        for (name, value) in parameters {
            body.extend_from_slice(name.as_bytes());
            body.push(0);
            body.extend_from_slice(value.as_bytes());
            body.push(0);
        }
        body.push(0);
        self.packet(&body).await;
    }

    pub(crate) async fn packet(&mut self, body: &[u8]) {
        let length = u32::try_from(body.len() + 4).unwrap();
        self.stream.write_all(&length.to_be_bytes()).await.unwrap();
        self.stream.write_all(body).await.unwrap();
    }

    pub(crate) async fn send(&mut self, code: u8, body: &[u8]) {
        self.stream.write_all(&[code]).await.unwrap();
        let length = u32::try_from(body.len() + 4).unwrap();
        self.stream.write_all(&length.to_be_bytes()).await.unwrap();
        self.stream.write_all(body).await.unwrap();
    }

    pub(crate) async fn query(&mut self, text: &str) -> Vec<Frame> {
        let mut body = text.as_bytes().to_vec();
        body.push(0);
        self.send(b'Q', &body).await;
        self.until_ready().await
    }

    pub(crate) async fn byte(&mut self) -> u8 {
        let mut byte = [0];
        self.stream.read_exact(&mut byte).await.unwrap();
        byte[0]
    }

    /// The next message, or `None` when the gateway closed.
    pub(crate) async fn read(&mut self) -> Option<Frame> {
        let mut header = [0_u8; 5];
        tokio::time::timeout(Duration::from_secs(20), self.stream.read_exact(&mut header))
            .await
            .expect("the gateway answers in time")
            .ok()?;
        let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        let mut body = vec![0; length - 4];
        self.stream.read_exact(&mut body).await.ok()?;
        Some((header[0], body))
    }

    pub(crate) async fn until_ready(&mut self) -> Vec<Frame> {
        let mut frames = Vec::new();
        while let Some(frame) = self.read().await {
            let ready = frame.0 == b'Z';
            frames.push(frame);
            if ready {
                break;
            }
        }
        frames
    }

    /// Reads to the end of the stream.
    pub(crate) async fn rest(&mut self) -> Vec<Frame> {
        let mut frames = Vec::new();
        while let Some(frame) = self.read().await {
            frames.push(frame);
        }
        frames
    }

    pub(crate) async fn password(&mut self, password: &str) {
        let mut body = password.as_bytes().to_vec();
        body.push(0);
        self.send(b'p', &body).await;
    }

    /// Starts up as `user` and answers the password request.
    pub(crate) async fn login(&mut self, user: &str, password: &str) -> Vec<Frame> {
        self.startup(0x0003_0000, &[("user", user), ("database", "unf")])
            .await;
        let request = self.read().await.expect("a password request");
        assert_eq!(request, (b'R', 3_i32.to_be_bytes().to_vec()));
        self.password(password).await;
        let mut frames = Vec::new();
        while let Some(frame) = self.read().await {
            let last = matches!(frame.0, b'Z' | b'E');
            frames.push(frame);
            if last {
                break;
            }
        }
        frames
    }
}

pub(crate) fn fields(body: &[u8]) -> HashMap<u8, String> {
    let mut fields = HashMap::new();
    let mut rest = body;
    while let Some((&code, tail)) = rest.split_first() {
        if code == 0 {
            break;
        }
        let end = tail.iter().position(|byte| *byte == 0).unwrap();
        fields.insert(code, String::from_utf8(tail[..end].to_vec()).unwrap());
        rest = &tail[end + 1..];
    }
    fields
}

pub(crate) fn error_of(frames: &[Frame]) -> HashMap<u8, String> {
    let (_, body) = frames
        .iter()
        .find(|(code, _)| *code == b'E')
        .unwrap_or_else(|| panic!("no error in {:?}", codes(frames)));
    fields(body)
}

pub(crate) fn codes(frames: &[Frame]) -> String {
    frames.iter().map(|(code, _)| char::from(*code)).collect()
}

/// The text values of the DataRows.
pub(crate) fn rows(frames: &[Frame]) -> Vec<Vec<Option<String>>> {
    frames
        .iter()
        .filter(|(code, _)| *code == b'D')
        .map(|(_, body)| {
            let count = i16::from_be_bytes([body[0], body[1]]);
            let mut offset = 2;
            (0..count)
                .map(|_| {
                    let length = i32::from_be_bytes(body[offset..offset + 4].try_into().unwrap());
                    offset += 4;
                    if length < 0 {
                        return None;
                    }
                    let length = usize::try_from(length).unwrap();
                    let value =
                        String::from_utf8_lossy(&body[offset..offset + length]).into_owned();
                    offset += length;
                    Some(value)
                })
                .collect()
        })
        .collect()
}

/// The (name, type OID, modifier) of each column of the RowDescription.
pub(crate) fn columns(frames: &[Frame]) -> Vec<(String, u32, i32)> {
    let (_, body) = frames.iter().find(|(code, _)| *code == b'T').unwrap();
    let count = i16::from_be_bytes([body[0], body[1]]);
    let mut offset = 2;
    (0..count)
        .map(|_| {
            let end = offset + body[offset..].iter().position(|byte| *byte == 0).unwrap();
            let name = String::from_utf8(body[offset..end].to_vec()).unwrap();
            offset = end + 1 + 6;
            let oid = u32::from_be_bytes(body[offset..offset + 4].try_into().unwrap());
            let modifier = i32::from_be_bytes(body[offset + 6..offset + 10].try_into().unwrap());
            offset += 4 + 2 + 4 + 2;
            (name, oid, modifier)
        })
        .collect()
}

pub(crate) fn tags(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .filter(|(code, _)| *code == b'C')
        .map(|(_, body)| String::from_utf8(body[..body.len() - 1].to_vec()).unwrap())
        .collect()
}

pub(crate) async fn logged_in(harness: &Harness) -> Raw {
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login(USER, PASSWORD).await;
    assert_eq!(frames.last().unwrap().0, b'Z', "{}", codes(&frames));
    raw
}

const SELECT: &str = "ВЫБРАТЬ Категория, Количество ИЗ РегистрНакопления.ЗамерОстатков";

#[tokio::test]
async fn ssl_and_gssenc_requests_without_tls_answer_n() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.packet(&80_877_103_u32.to_be_bytes()).await;
    assert_eq!(raw.byte().await, b'N');
    raw.packet(&80_877_104_u32.to_be_bytes()).await;
    assert_eq!(raw.byte().await, b'N');
    let frames = raw.login(USER, PASSWORD).await;
    assert_eq!(frames.last().unwrap(), &(b'Z', b"I".to_vec()));
}

#[tokio::test]
async fn unsupported_protocol_is_refused() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0002_0000, &[("user", USER)]).await;
    let frames = raw.rest().await;
    assert_eq!(codes(&frames), "E");
    assert_eq!(error_of(&frames)[&b'C'], "0A000");
}

#[tokio::test]
async fn newer_minor_version_is_negotiated_down() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0003_0002, &[("user", USER), ("_pq_.unknown", "1")])
        .await;
    let (code, body) = raw.read().await.unwrap();
    assert_eq!(code, b'v');
    assert_eq!(&body[..8], &[0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(&body[8..], b"_pq_.unknown\0");
    assert_eq!(raw.read().await.unwrap().0, b'R');
}

#[tokio::test]
async fn plain_connection_from_the_network_never_asks_for_a_password() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect_from("192.0.2.10:5000".parse().unwrap()),
    };
    raw.startup(0x0003_0000, &[("user", USER)]).await;
    let frames = raw.rest().await;
    assert_eq!(codes(&frames), "E");
    assert_eq!(error_of(&frames)[&b'C'], "28000");
}

#[tokio::test]
async fn explicitly_allowed_network_connection_requests_a_password() {
    let harness = Harness::new(Options {
        insecure_loopback: false,
        insecure_network: true,
        ..Options::default()
    });
    let mut raw = Raw {
        stream: harness.connect_from("192.0.2.10:5000".parse().unwrap()),
    };
    raw.startup(0x0003_0000, &[("user", USER)]).await;
    let (code, body) = raw.read().await.unwrap();
    assert_eq!(code, b'R');
    assert_eq!(body.as_slice(), &[0, 0, 0, 3]);
    raw.password(PASSWORD).await;
    let (code, body) = raw.read().await.unwrap();
    assert_eq!(code, b'R');
    assert_eq!(body.as_slice(), &[0, 0, 0, 0]);
}

#[tokio::test]
async fn loopback_without_tls_needs_the_operator_flag() {
    let harness = Harness::new(Options {
        insecure_loopback: false,
        ..Options::default()
    });
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0003_0000, &[("user", USER)]).await;
    let frames = raw.rest().await;
    assert_eq!(error_of(&frames)[&b'C'], "28000");
}

#[tokio::test]
async fn startup_needs_a_user_and_utf8() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0003_0000, &[("database", "unf")]).await;
    assert_eq!(error_of(&raw.rest().await)[&b'C'], "28000");
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(
        0x0003_0000,
        &[("user", USER), ("client_encoding", "WIN1251")],
    )
    .await;
    assert_eq!(error_of(&raw.rest().await)[&b'C'], "22023");
}

#[tokio::test]
async fn correct_password_reports_the_startup_parameters() {
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    raw.startup(0x0003_0000, &[("user", USER), ("application_name", "psql")])
        .await;
    raw.read().await.unwrap();
    raw.password(PASSWORD).await;
    let frames = raw.until_ready().await;
    assert_eq!(frames[0], (b'R', 0_i32.to_be_bytes().to_vec()));
    let statuses = frames
        .iter()
        .filter(|(code, _)| *code == b'S')
        .map(|(_, body)| {
            let text = String::from_utf8(body.clone()).unwrap();
            let mut parts = text.split('\0');
            (
                parts.next().unwrap().to_owned(),
                parts.next().unwrap().to_owned(),
            )
        })
        .collect::<HashMap<_, _>>();
    assert!(statuses["server_version"].starts_with("13.0 (open-sdbl "));
    assert_eq!(statuses["client_encoding"], "UTF8");
    assert_eq!(statuses["server_encoding"], "UTF8");
    assert_eq!(statuses["DateStyle"], "ISO, MDY");
    assert_eq!(statuses["integer_datetimes"], "on");
    assert_eq!(statuses["standard_conforming_strings"], "on");
    assert_eq!(statuses["TimeZone"], "UTC");
    assert_eq!(statuses["application_name"], "psql");
    let key = frames.iter().find(|(code, _)| *code == b'K').unwrap();
    assert_eq!(key.1.len(), 8);
    assert_eq!(frames.last().unwrap(), &(b'Z', b"I".to_vec()));
    let events = harness.event_lines();
    assert!(events.iter().any(|line| line.contains("Authenticated")));
    // A user name is case-insensitive, as the platform compares it.
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login(&USER.to_uppercase(), PASSWORD).await;
    assert_eq!(frames.last().unwrap().0, b'Z');
}

/// The 28P01 answer of one login attempt.
async fn failed(harness: &Harness, user: &str, password: &str) -> HashMap<u8, String> {
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login(user, password).await;
    assert_eq!(codes(&frames), "E", "{user}");
    assert!(raw.read().await.is_none(), "the connection closes");
    error_of(&frames)
}

#[tokio::test]
async fn every_failure_is_the_same_authentication_error() {
    let mut answers = Vec::new();
    let harness = Harness::new(Options::default());
    answers.push(failed(&harness, USER, "wrong").await);
    answers.push(failed(&harness, "Нет такого", PASSWORD).await);
    // Only H2 matches the uppercase variant.
    answers.push(failed(&harness, USER, "PASS123!").await);
    // A synthetic source cannot authenticate a production client.
    answers.push(failed(&harness, "EDIEvents", PASSWORD).await);
    // A user whose record has no confirmed mapping.
    answers.push(failed(&harness, "Администратор", PASSWORD).await);
    let legacy_with_id_zero = format!("{DEMO},0,");
    let legacy_with_unknown_id = format!("{DEMO},7,abc");
    for (stored, source, state, password) in [
        (
            Box::leak(legacy_with_id_zero.into_boxed_str()) as &str,
            SourceProfile::PublicStoredValue8327,
            PasswordState::Unknown,
            PASSWORD,
        ),
        (
            Box::leak(legacy_with_unknown_id.into_boxed_str()),
            SourceProfile::PublicStoredValue8327,
            PasswordState::Unknown,
            PASSWORD,
        ),
        (
            DEMO,
            SourceProfile::Unconfirmed,
            PasswordState::Unknown,
            PASSWORD,
        ),
        (
            "",
            SourceProfile::PublicStoredValue8327,
            PasswordState::Unknown,
            "",
        ),
        (
            "2jmj7l5rSw0yVb/vlWAYkK/YBwk=,2jmj7l5rSw0yVb/vlWAYkK/YBwk=",
            SourceProfile::PublicStoredValue8327,
            PasswordState::NotSet,
            "",
        ),
        (
            "!,!",
            SourceProfile::PublicStoredValue8327,
            PasswordState::Unknown,
            PASSWORD,
        ),
    ] {
        let harness = Harness::new(Options {
            stored,
            source,
            state,
            ..Options::default()
        });
        answers.push(failed(&harness, USER, password).await);
    }
    let disabled = Harness::new(Options {
        access: Some(fixture::store_with(&fixture::snapshot(), true, |user| {
            user.standard_authentication = false;
        })),
        ..Options::default()
    });
    answers.push(failed(&disabled, USER, PASSWORD).await);
    for answer in &answers {
        assert_eq!(answer, &answers[0]);
    }
    assert_eq!(answers[0][&b'C'], "28P01");
    assert_eq!(answers[0][&b'M'], "password authentication failed");
    let events = harness.event_lines().join("\n");
    assert!(events.contains("AuthenticationFailed"));
    assert!(!events.contains("wrong") && !events.contains(PASSWORD));
}

#[tokio::test]
async fn failures_are_padded_to_the_configured_delay() {
    let harness = Harness::new(Options {
        limits: GatewayLimits {
            authentication_failure_delay: Duration::from_millis(300),
            ..GatewayLimits::default()
        },
        ..Options::default()
    });
    for user in [USER, "Нет такого"] {
        let mut raw = Raw {
            stream: harness.connect(),
        };
        raw.startup(0x0003_0000, &[("user", user)]).await;
        raw.read().await.unwrap();
        let started = Instant::now();
        raw.password("wrong").await;
        raw.rest().await;
        assert!(started.elapsed() >= Duration::from_millis(300), "{user}");
    }
}

/// The stored values of the fixture vectors for `PaSs123!`.
fn extended_values() -> Vec<String> {
    include_str!("../../../../tests/fixtures/password/vectors.tsv")
        .lines()
        .filter(|line| {
            ["sha256", "sha512", "pbkdf2_hmac_sha256"]
                .iter()
                .any(|tag| line.starts_with(&format!("{tag}-match-public-stored-value-8.3.27\t")))
        })
        .map(|line| line.split('\t').nth(5).unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn stored_password_from_the_record_authenticates() {
    // "Администратор" is not in the harness password map, so the gateway
    // takes the UserData path: it reads the stored password from the
    // decoded v8users record. In this fixture that record carries the
    // empty-password SHA-1, so the empty password logs in and any other
    // is refused — the measured v8users.Data layout, end to end.
    let harness = Harness::new(Options::default());
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login("Администратор", "").await;
    assert_eq!(
        frames.last().unwrap(),
        &(b'Z', b"I".to_vec()),
        "{}",
        codes(&frames)
    );

    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login("Администратор", "x").await;
    assert_eq!(codes(&frames), "E");
    assert_eq!(error_of(&frames)[&b'C'], "28P01");
}

#[tokio::test]
async fn extended_profiles_authenticate_like_legacy() {
    let values = extended_values();
    assert_eq!(values.len(), 3);
    for value in values {
        let harness = Harness::new(Options {
            stored: Box::leak(value.into_boxed_str()),
            ..Options::default()
        });
        logged_in(&harness).await;
    }
}

#[tokio::test]
async fn oversized_message_closes_the_connection() {
    let harness = Harness::new(Options {
        limits: GatewayLimits {
            max_message_bytes: 64,
            ..Options::default().limits
        },
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    let mut body = "x".repeat(100).into_bytes();
    body.push(0);
    raw.send(b'Q', &body).await;
    let frames = raw.rest().await;
    assert_eq!(error_of(&frames)[&b'C'], "08P01");
}

#[tokio::test]
async fn simple_query_streams_rows_with_native_types() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw.query(SELECT).await;
    assert_eq!(codes(&frames), "TDDDCZ");
    assert_eq!(
        columns(&frames),
        [
            ("Категория".to_owned(), 1043, 16),
            ("Количество".to_owned(), 1700, (15 << 16 | 2) + 4),
        ]
    );
    assert_eq!(
        rows(&frames)[1],
        [Some("К1".to_owned()), Some("1.50".to_owned())]
    );
    assert_eq!(tags(&frames), ["SELECT 3"]);
    assert_eq!(harness.statements().len(), 1);
}

#[tokio::test]
async fn sql_text_is_never_forwarded() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw.query("SELECT * FROM _Reference42").await;
    assert_eq!(codes(&frames), "EZ");
    assert!(harness.statements().is_empty());
}

#[tokio::test]
async fn syntax_error_carries_the_character_position() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw
        .query("ВЫБРАТЬ ИЗ РегистрНакопления.ЗамерОстатков")
        .await;
    let error = error_of(&frames);
    assert_eq!(error[&b'C'], "42601");
    assert_eq!(error[&b'P'], "9");
    assert!(harness.statements().is_empty());
}

#[tokio::test]
async fn simple_query_parameter_without_a_value_is_42p02() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw.query(&format!("{SELECT} ГДЕ Категория = &Дата")).await;
    let error = error_of(&frames);
    assert_eq!(error[&b'C'], "42P02");
    assert!(error[&b'M'].contains("Дата"), "{}", error[&b'M']);
}

#[tokio::test]
async fn denied_target_and_field_are_42501_before_any_sql() {
    let harness = Harness::new(Options {
        read: false,
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    let frames = raw.query(SELECT).await;
    assert_eq!(error_of(&frames)[&b'C'], "42501");
    // A standard field no measurement proves readable.
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw
        .query("ВЫБРАТЬ Регистратор ИЗ РегистрНакопления.ЗамерОстатков")
        .await;
    let error = error_of(&frames);
    assert_eq!(error[&b'C'], "42501", "{}", error[&b'M']);
    assert!(harness.statements().is_empty());
}

#[tokio::test]
async fn temporary_tables_live_for_the_connection() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw
        .query(
            "ВЫБРАТЬ Категория КАК К ПОМЕСТИТЬ ВТ ИЗ РегистрНакопления.ЗамерОстатков; ВЫБРАТЬ К ИЗ ВТ",
        )
        .await;
    assert_eq!(codes(&frames), "CTDDDCZ");
    assert_eq!(tags(&frames), ["SELECT 0", "SELECT 3"]);
    let frames = raw.query("ВЫБРАТЬ К ИЗ ВТ").await;
    assert_eq!(tags(&frames), ["SELECT 3"]);
    let frames = raw.query("УНИЧТОЖИТЬ ВТ").await;
    assert_eq!(codes(&frames), "CZ");
    assert_eq!(tags(&frames), ["DROP TABLE"]);
    let frames = raw.query("ВЫБРАТЬ К ИЗ ВТ").await;
    assert_eq!(codes(&frames), "EZ");
    // DISCARD ALL forgets them too.
    raw.query("ВЫБРАТЬ Категория КАК К ПОМЕСТИТЬ ВТ ИЗ РегистрНакопления.ЗамерОстатков")
        .await;
    assert_eq!(tags(&raw.query("DISCARD ALL").await), ["DISCARD ALL"]);
    assert_eq!(codes(&raw.query("ВЫБРАТЬ К ИЗ ВТ").await), "EZ");
}

#[tokio::test]
async fn chatter_is_answered_without_the_database() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw.query("SET application_name = 'DBeaver'").await;
    assert_eq!(codes(&frames), "SCZ");
    assert_eq!(frames[0].1, b"application_name\0DBeaver\0");
    let frames = raw.query("SELECT version()").await;
    assert!(
        rows(&frames)[0][0]
            .as_deref()
            .unwrap()
            .contains("open-sdbl")
    );
    let frames = raw.query("SHOW application_name").await;
    assert_eq!(rows(&frames), [[Some("DBeaver".to_owned())]]);
    let frames = raw.query("SHOW transaction_isolation").await;
    assert_eq!(rows(&frames), [[Some("read committed".to_owned())]]);
    let frames = raw.query("select current_schema()").await;
    assert_eq!(rows(&frames), [[Some("public".to_owned())]]);
    let frames = raw.query("SELECT 1").await;
    assert_eq!(columns(&frames)[0].1, 23);
    assert_eq!(error_of(&raw.query("SHOW nonsense").await)[&b'C'], "42704");
    assert_eq!(
        error_of(&raw.query("START TRANSACTION READ WRITE").await)[&b'C'],
        "25006"
    );
    assert_eq!(
        error_of(&raw.query("SET client_encoding = 'LATIN1'").await)[&b'C'],
        "22023"
    );
    assert_eq!(codes(&raw.query("").await), "IZ");
    assert_eq!(codes(&raw.query("-- nothing").await), "IZ");
    assert_eq!(tags(&raw.query("RESET application_name").await), ["RESET"]);
    assert!(harness.statements().is_empty());
}

#[tokio::test]
async fn transaction_block_reads_in_one_read_only_transaction() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let frames = raw.query("BEGIN").await;
    assert_eq!(frames.last().unwrap(), &(b'Z', b"T".to_vec()));
    raw.query(SELECT).await;
    let frames = raw.query(SELECT).await;
    assert_eq!(frames.last().unwrap(), &(b'Z', b"T".to_vec()));
    let frames = raw.query("COMMIT").await;
    assert_eq!(tags(&frames), ["COMMIT"]);
    assert_eq!(frames.last().unwrap(), &(b'Z', b"I".to_vec()));
    assert_eq!(harness.db_events(), ["open", "begin", "commit"]);
    // An error fails the block until it ends.
    raw.query("BEGIN").await;
    let frames = raw.query("ВЫБРАТЬ ИЗ").await;
    assert_eq!(frames.last().unwrap(), &(b'Z', b"E".to_vec()));
    let frames = raw.query(SELECT).await;
    assert_eq!(error_of(&frames)[&b'C'], "25P02");
    let frames = raw.query("ROLLBACK").await;
    assert_eq!(tags(&frames), ["ROLLBACK"]);
    assert_eq!(frames.last().unwrap(), &(b'Z', b"I".to_vec()));
}

#[tokio::test]
async fn operator_session_parameters_win() {
    let mut operator = SessionParameters::new();
    operator.set(QueryParameter::new(
        "ТекущийПользователь",
        ParameterValue::String("operator".to_owned()),
    ));
    let harness = Harness::new(Options {
        operator,
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    let frames = raw
        .query(&format!("{SELECT} ГДЕ Категория = &ТекущийПользователь"))
        .await;
    assert_eq!(codes(&frames).chars().last(), Some('Z'));
    let sql = harness.statements().join("\n");
    assert!(
        sql.contains("operator") && !sql.contains("derived"),
        "{sql}"
    );
}

#[tokio::test]
async fn cancel_request_with_the_key_ends_the_query() {
    let harness = Harness::new(Options {
        delay: Some(Duration::from_secs(30)),
        ..Options::default()
    });
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login(USER, PASSWORD).await;
    let key = frames
        .iter()
        .find(|(code, _)| *code == b'K')
        .unwrap()
        .1
        .clone();
    let mut body = SELECT.as_bytes().to_vec();
    body.push(0);
    raw.send(b'Q', &body).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    // An unknown key is ignored.
    let mut wrong = Raw {
        stream: harness.connect(),
    };
    let mut packet = 80_877_102_u32.to_be_bytes().to_vec();
    packet.extend_from_slice(&key[..4]);
    packet.extend_from_slice(&[0, 0, 0, 0]);
    wrong.packet(&packet).await;
    assert!(wrong.rest().await.is_empty());
    let mut cancel = Raw {
        stream: harness.connect(),
    };
    let mut packet = 80_877_102_u32.to_be_bytes().to_vec();
    packet.extend_from_slice(&key);
    cancel.packet(&packet).await;
    let frames = raw.until_ready().await;
    assert_eq!(error_of(&frames)[&b'C'], "57014");
    assert!(harness.db_events().contains(&"cancel"));
    // The connection stays usable.
    assert_eq!(codes(&raw.query("SELECT 1").await), "TDCZ");
}

#[tokio::test]
async fn idle_and_shutdown_end_connections() {
    let harness = Harness::new(Options {
        limits: GatewayLimits {
            idle_timeout: Some(Duration::from_millis(100)),
            ..Options::default().limits
        },
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    assert_eq!(error_of(&raw.rest().await)[&b'C'], "57P05");
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    harness.stop();
    assert_eq!(error_of(&raw.rest().await)[&b'C'], "57P01");
}

/// Parse, Bind and Execute of the unnamed statement with a row limit.
async fn extended(raw: &mut Raw, text: &str, max_rows: i32) -> Vec<Frame> {
    let mut parse = vec![0];
    parse.extend_from_slice(text.as_bytes());
    parse.extend_from_slice(&[0, 0, 0]);
    raw.send(b'P', &parse).await;
    raw.send(b'B', &[0, 0, 0, 0, 0, 0, 0, 0]).await;
    let mut execute = vec![0];
    execute.extend_from_slice(&max_rows.to_be_bytes());
    raw.send(b'E', &execute).await;
    raw.send(b'S', &[]).await;
    raw.until_ready().await
}

#[tokio::test]
async fn held_rows_beyond_the_ceiling_fail_the_portal() {
    let harness = Harness::new(Options {
        responder: register_rows(50),
        portal_buffer_rows: 10,
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    let frames = extended(&mut raw, SELECT, 1).await;
    assert_eq!(error_of(&frames)[&b'C'], "54000");
    // Within the ceiling the portal suspends.
    let frames = extended(&mut raw, SELECT, 45).await;
    assert_eq!(codes(&frames).matches('D').count(), 45);
    assert!(codes(&frames).ends_with("sZ"), "{}", codes(&frames));
}

#[tokio::test]
async fn extended_errors_skip_to_sync() {
    let harness = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let mut parse = vec![0];
    parse.extend_from_slice("ВЫБРАТЬ ИЗ".as_bytes());
    parse.extend_from_slice(&[0, 0, 0]);
    raw.send(b'P', &parse).await;
    raw.send(b'B', &[0, 0, 0, 0, 0, 0, 0, 0]).await;
    raw.send(b'E', &[0, 0, 0, 0, 0]).await;
    raw.send(b'S', &[]).await;
    let frames = raw.until_ready().await;
    assert_eq!(codes(&frames), "EZ");
    assert_eq!(error_of(&frames)[&b'C'], "42601");
}

#[tokio::test]
async fn the_same_query_on_an_mssql_base() {
    let backend = open_sdbl::query::MsSqlBackend::new(2000).unwrap();
    let harness = Harness::new(Options {
        dialect: open_sdbl_db::DatabaseDialect::MsSql { backend },
        responder: Arc::new(|sql: &str, columns: usize| {
            register_rows(3)(&sql.to_lowercase(), columns)
        }),
        ..Options::default()
    });
    let postgres = Harness::new(Options::default());
    let mut raw = logged_in(&harness).await;
    let mssql = raw.query(SELECT).await;
    let mut raw = logged_in(&postgres).await;
    let expected = raw.query(SELECT).await;
    assert_eq!(codes(&mssql), codes(&expected));
    assert_eq!(columns(&mssql), columns(&expected));
    assert_eq!(rows(&mssql), rows(&expected));
    let sql = harness.statements().join("\n");
    assert!(sql.contains("FROM [_accumrg1]"), "{sql}");
}

#[tokio::test]
async fn role_restriction_is_expanded_into_the_sql() {
    let snapshot = fixture::snapshot();
    let harness = Harness::new(Options {
        access: Some(fixture::restricted_store(
            &snapshot,
            "ГДЕ Категория = \"Разрешено\"",
        )),
        ..Options::default()
    });
    let mut raw = logged_in(&harness).await;
    let frames = raw.query(SELECT).await;
    assert_eq!(
        codes(&frames).chars().last(),
        Some('Z'),
        "{:?}",
        error_of(&frames)
    );
    let sql = harness.statements().join("\n");
    assert!(sql.contains("Разрешено"), "{sql}");
}

#[tokio::test]
async fn serve_bounds_connections_and_stops_on_shutdown() {
    let harness = Harness::new(Options {
        limits: GatewayLimits {
            max_connections: 1,
            ..Options::default().limits
        },
        ..Options::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let shared = Arc::clone(&harness.shared);
    let server = tokio::spawn(crate::serve_with(listener, shared, async {
        let _ = stopped.await;
    }));
    let mut config = tokio_postgres::Config::new();
    config
        .host("127.0.0.1")
        .port(address.port())
        .user(USER)
        .password(PASSWORD);
    let (client, connection) = config.connect(tokio_postgres::NoTls).await.unwrap();
    let connection = tokio::spawn(connection);
    assert_eq!(client.simple_query("SELECT 1").await.unwrap().len(), 3);
    let Err(refused) = config.connect(tokio_postgres::NoTls).await else {
        panic!("a second connection is over the limit");
    };
    assert_eq!(
        refused.code(),
        Some(&tokio_postgres::error::SqlState::TOO_MANY_CONNECTIONS)
    );
    stop.send(()).unwrap();
    // The idle connection is told the server is shutting down.
    let _ = connection.await;
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("serve returns after shutdown")
        .unwrap()
        .unwrap();
    assert!(client.is_closed());
}
