//! Real psql, bounded child processes and a loopback bridge to the fake engine.
use super::*;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

pub(super) struct Server {
    port: u16,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) async fn start(harness: &Harness) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let harness = harness.clone();
        let task = tokio::spawn(async move {
            let mut bridges = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut tcp, _) = accepted.unwrap();
                        let mut backend = harness.connect();
                        bridges.spawn(async move {
                            // psql can close immediately after a client error.
                            let _ = tokio::io::copy_bidirectional(&mut tcp, &mut backend).await;
                        });
                    }
                    _ = bridges.join_next(), if !bridges.is_empty() => {}
                }
            }
        });
        Self { port, task }
    }

    pub(super) async fn run(&self, arguments: &[&str], script: Option<&str>) -> Output {
        let port = self.port;
        let arguments = arguments
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>();
        let script = script.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let mut child =
                Command::new(std::env::var_os("OPEN_SDBL_PSQL").unwrap_or_else(|| "psql".into()))
                    .args([
                        "-X",
                        "-w",
                        "-h",
                        "127.0.0.1",
                        "-p",
                        &port.to_string(),
                        "-U",
                        USER,
                        "-d",
                        "catalog_test",
                        "-P",
                        "pager=off",
                    ])
                    .args(arguments)
                    .env("PGPASSWORD", PASSWORD)
                    .env("PGSSLMODE", "disable")
                    .env("PGCONNECT_TIMEOUT", "5")
                    .env("PGAPPNAME", "pgwire-psql-test")
                    .stdin(if script.is_some() {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    })
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("psql is installed (or set OPEN_SDBL_PSQL)");
            let input = script.map(|script| {
                let mut stdin = child.stdin.take().unwrap();
                std::thread::spawn(move || stdin.write_all(script.as_bytes()))
            });
            // Drain both pipes concurrently so verbose diagnostics cannot block
            // the child before the deadline. Corpus queries return no DB rows.
            fn drain(stream: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
                std::thread::spawn(move || {
                    let mut bytes = Vec::new();
                    stream
                        .take(8 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(
                        bytes.len() <= 8 * 1024 * 1024,
                        "psql output exceeds test bound"
                    );
                    bytes
                })
            }
            let stdout = drain(child.stdout.take().unwrap());
            let stderr = drain(child.stderr.take().unwrap());
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut expired = false;
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    expired = true;
                    let _ = child.kill();
                    break child.wait().unwrap();
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            if let Some(input) = input {
                let _ = input.join().unwrap();
            }
            let output = Output {
                status,
                stdout: stdout.join().unwrap(),
                stderr: stderr.join().unwrap(),
            };
            assert!(
                !expired,
                "psql exceeded 30 seconds: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output
        })
        .await
        .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) fn state(output: &Output) -> String {
    if output.status.success() {
        return "OK".into();
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(
        output.status.code(),
        Some(2),
        "psql connection failed: {stderr}"
    );
    stderr
        .lines()
        .find_map(|line| {
            let (_, rest) = line.split_once("ERROR:")?;
            let code = rest.split_whitespace().next()?;
            (code.len() == 5
                && code
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
            .then(|| code.into())
        })
        .unwrap_or_else(|| panic!("psql did not report a SQLSTATE: {stderr}"))
}

#[tokio::test]
#[ignore = "requires psql and a loopback listener"]
async fn psql_errors_recover_and_describe_result_types() {
    let harness = Harness::new(Options::default());
    let server = Server::start(&harness).await;
    let output = server
        .run(
            &["-A", "-t", "-v", "VERBOSITY=sqlstate", "-f", "-"],
            Some(
                r#"
\set ON_ERROR_STOP off
ВЫБРАТЬ ИЗ;
\echo syntax=:SQLSTATE
ВЫБРАТЬ НеизвестноеПоле ИЗ РегистрНакопления.ЗамерОстатков;
\echo column=:SQLSTATE
ВЫБРАТЬ Категория, Количество ИЗ РегистрНакопления.ЗамерОстатков;
\echo recovered=:SQLSTATE
ВЫБРАТЬ Категория, Количество ИЗ РегистрНакопления.ЗамерОстатков
\gdesc
"#,
            ),
        )
        .await;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for expected in [
        "syntax=42601",
        "column=42703",
        "recovered=00000",
        "К0|0.50",
        "К2|2.50",
        "Категория|character varying(12)",
        "Количество|numeric(15,2)",
    ] {
        assert!(stdout.contains(expected), "missing {expected}:\n{stdout}");
    }
    assert_eq!(
        harness.statements().len(),
        1,
        "Describe must not execute a query"
    );
    harness.stop();
}

#[tokio::test]
#[ignore = "requires psql and a loopback listener"]
async fn psql_temporary_tables_are_connection_local() {
    let ordinary = register_rows(3);
    let harness = Harness::new(Options {
        responder: Arc::new(move |sql, width| {
            if sql.contains("__placed") {
                assert_eq!(width, 1);
                Ok(vec![vec![Cell::Number("3".into())]])
            } else {
                ordinary(sql, width)
            }
        }),
        ..Options::default()
    });
    let server = Server::start(&harness).await;
    let output = server
        .run(
            &["-A", "-t", "-v", "ON_ERROR_STOP=1", "-f", "-"],
            Some(
                r#"
ВЫБРАТЬ Категория ПОМЕСТИТЬ ВТ_psql ИЗ РегистрНакопления.ЗамерОстатков;
ВЫБРАТЬ Категория ИЗ ВТ_psql;
ВЫБРАТЬ Категория ИЗ ВТ_psql;
"#,
            ),
        )
        .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("К0")
            .count(),
        2,
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let before = harness.statements().len();
    let output = server
        .run(
            &[
                "-v",
                "VERBOSITY=sqlstate",
                "-c",
                "ВЫБРАТЬ Категория ИЗ ВТ_psql",
            ],
            None,
        )
        .await;
    assert_eq!(state(&output), "42P01");
    assert_eq!(
        harness.statements().len(),
        before,
        "a foreign connection cannot execute a local temp table"
    );
    harness.stop();
}

#[tokio::test]
#[ignore = "requires psql and a loopback listener"]
async fn psql_read_refusal_never_executes_sql() {
    let harness = Harness::new(Options {
        read: false,
        ..Options::default()
    });
    let server = Server::start(&harness).await;
    let output = server
        .run(
            &[
                "-v",
                "VERBOSITY=sqlstate",
                "-c",
                "ВЫБРАТЬ Категория ИЗ РегистрНакопления.ЗамерОстатков",
            ],
            None,
        )
        .await;
    assert_eq!(state(&output), "42501");
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
#[ignore = "requires psql with bind support and a loopback listener"]
async fn psql_extended_named_parameter_binding() {
    let harness = Harness::new(Options {
        responder: Arc::new(|sql, width| {
            assert_eq!(width, 1);
            assert!(
                sql.contains("К1"),
                "bound Unicode value must enter generated SQL: {sql}"
            );
            Ok(vec![vec![Cell::Text("К1".into())]])
        }),
        ..Options::default()
    });
    let server = Server::start(&harness).await;
    let output = server
        .run(
            &["-A", "-t", "-v", "ON_ERROR_STOP=1", "-f", "-"],
            Some(
                r#"
ВЫБРАТЬ Категория ИЗ РегистрНакопления.ЗамерОстатков ГДЕ Категория = &Имя
\bind 'К1'
\g
"#,
            ),
        )
        .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("К1"));
    assert_eq!(harness.statements().len(), 1);
    harness.stop();
}
