use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn console_help_accepts_piped_input_without_connecting() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["console", "--help"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\\help\n\\q\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("console postgres --host HOST"));
    assert!(stdout.contains("--socks5-proxy HOST:PORT"));
    assert!(stdout.contains("PGPASSWORD, PGPASSFILE, or $HOME/.pgpass"));
}

#[test]
fn console_validates_provider_and_required_options_before_connecting() {
    let unsupported = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["console", "sqlite"])
        .output()
        .unwrap();
    assert!(!unsupported.status.success());
    assert!(
        String::from_utf8(unsupported.stderr)
            .unwrap()
            .contains("unsupported console provider")
    );

    let incomplete = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["console", "postgres", "--host", "db"])
        .output()
        .unwrap();
    assert!(!incomplete.status.success());
    assert!(
        String::from_utf8(incomplete.stderr)
            .unwrap()
            .contains("--host, --database, and --user are required")
    );
}

#[test]
fn repl_remains_a_compatibility_alias() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["repl", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("console postgres --host HOST")
    );
}
