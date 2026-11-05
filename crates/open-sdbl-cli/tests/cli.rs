use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn lexes_standard_input() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["lex", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all("ВЫБРАТЬ &Имя".as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "1:1\tKEYWORD(SELECT)\tВЫБРАТЬ\n1:9\tPARAMETER\t&Имя\n"
    );
}

#[test]
fn returns_a_positional_diagnostic() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["lex", "-"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\"unfinished")
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("1:1: unterminated string literal")
    );
}

#[test]
fn lexes_a_named_file() {
    let path = std::env::temp_dir().join(format!("open-sdbl-{}.sdbl", std::process::id()));
    std::fs::write(&path, "SELECT Code").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["lex", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "1:1\tKEYWORD(SELECT)\tSELECT\n1:8\tIDENTIFIER\tCode\n"
    );
}

#[test]
fn closed_stdout_pipe_is_quiet_success() {
    let path =
        std::env::temp_dir().join(format!("open-sdbl-broken-pipe-{}.sdbl", std::process::id()));
    std::fs::write(&path, "SELECT ".repeat(10_000)).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["lex", path.to_str().unwrap()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    std::fs::remove_file(path).unwrap();

    assert!(status.success());
}

#[test]
fn warns_when_mssql_certificate_verification_is_disabled() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "metadata",
            "mssql",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--trust-server-certificate",
            "--trust-ca-file",
            "company-ca.pem",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("warning: --trust-server-certificate disables MSSQL certificate"));
    assert!(stderr.contains("mutually exclusive"));
}

#[test]
fn serve_refuses_a_network_listener_without_tls_before_connecting() {
    let started = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "serve",
            "postgres",
            // An address nobody answers on: the command must not try it.
            "--host",
            "192.0.2.1",
            "--database",
            "unf",
            "--user",
            "reader",
            "--listen",
            "0.0.0.0:5433",
        ])
        .env_remove("PGPASSWORD")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("without TLS"), "{stderr}");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn serve_argument_errors_are_usage_errors() {
    for arguments in [
        &["serve"][..],
        &["serve", "oracle"],
        &["serve", "postgres", "--host", "db"],
        &[
            "serve",
            "postgres",
            "--host",
            "db",
            "--database",
            "unf",
            "--user",
            "u",
            "--tls-cert",
            "c.pem",
        ],
        &[
            "serve",
            "postgres",
            "--host",
            "db",
            "--database",
            "unf",
            "--user",
            "u",
            "--unknown",
            "x",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["serve", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--tls-cert") && help.contains("serve postgres|mssql"));
}

#[test]
fn connection_commands_accept_gateway_config_and_validate_connection() {
    let path = std::env::temp_dir().join(format!("open-sdbl-config-{}.toml", std::process::id()));
    std::fs::write(
        &path,
        r#"
provider = "mssql"
host = "db"
database = "erp"
user = "reader"
trust_server_certificate = true
trust_ca_file = "ca.pem"
listen = "0.0.0.0:5433"
allow_insecure_network = true
session_params = ["X=1"]
"#,
    )
    .unwrap();
    for command in ["console", "metadata", "repl"] {
        let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
            .arg(command)
            .arg("--config")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("mutually exclusive"), "{command}: {stderr}");
        let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
            .arg(command)
            .arg("--config")
            .arg(&path)
            .arg("--host")
            .arg("other")
            .output()
            .unwrap();
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("cannot be combined")
        );
    }
    std::fs::remove_file(path).unwrap();
}
