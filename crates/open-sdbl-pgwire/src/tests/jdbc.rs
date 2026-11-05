//! Real pgjdbc connection checks; the driver is supplied explicitly, never downloaded.
use super::*;
use std::process::{Command, Stdio};
use std::time::Instant;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Java 11+, OPEN_SDBL_PGJDBC_JAR and a loopback listener"]
async fn pgjdbc_connection_and_keep_alive() {
    let jar = std::env::var_os("OPEN_SDBL_PGJDBC_JAR").expect("OPEN_SDBL_PGJDBC_JAR is required");
    let harness = Harness::new(Options::default());
    let server = super::psql::Server::start(&harness).await;
    for mode in ["simple", "extended"] {
        let jar = jar.clone();
        let url = format!("jdbc:postgresql://127.0.0.1:{}/EDIEvents", server.port());
        let output = tokio::task::spawn_blocking(move || {
            let mut child =
                Command::new(std::env::var_os("OPEN_SDBL_JAVA").unwrap_or_else(|| "java".into()))
                    .args(["--class-path".as_ref(), jar.as_os_str()])
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/fixtures/jdbc/ConnectionProbe.java"
                    ))
                    .args([url.as_str(), mode])
                    .arg(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/fixtures/catalog-corpus/datagrip-list-schemas.sql"
                    ))
                    .env("OPEN_SDBL_JDBC_USER", USER)
                    .env("OPEN_SDBL_JDBC_PASSWORD", PASSWORD)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("Java is installed");
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut expired = false;
            while child.try_wait().unwrap().is_none() {
                if Instant::now() >= deadline {
                    expired = true;
                    child.kill().unwrap();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let output = child.wait_with_output().unwrap();
            assert!(!expired, "JDBC probe exceeded 30 seconds");
            output
        })
        .await
        .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(&format!("PASS {mode}:")), "{stdout}");
        assert!(stdout.contains("PASS DataGrip ListSchemas:"), "{stdout}");
        eprint!("{stdout}");
    }
    assert!(
        harness.statements().is_empty(),
        "probes execute no database SQL"
    );
    assert_eq!(
        harness.db_events(),
        ["open", "begin", "rollback", "open", "begin", "rollback"]
    );
    harness.stop();
}
