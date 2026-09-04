use std::process::Command;

#[test]
fn metadata_help_and_required_options_are_reported() {
    let help = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["metadata", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("metadata postgres --host HOST"));
    assert!(help.contains("--socks5-proxy HOST:PORT"));
    assert!(help.contains("PGPASSWORD, PGPASSFILE, or $HOME/.pgpass"));
    assert!(!help.contains("--psql"));

    let invalid = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["metadata", "postgres", "--host", "db"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("--host, --database, and --user are required")
    );
}

#[test]
fn invalid_socks5_proxy_is_rejected_before_connecting() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "metadata",
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--socks5-proxy",
            "2001:db8::1:1080",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("IPv6 addresses must be enclosed in brackets")
    );
}

#[test]
fn obsolete_psql_option_is_rejected_before_connecting() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "metadata",
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            "--psql",
            "/bin/false",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unknown metadata option \"--psql\"")
    );
}

#[test]
fn plaintext_opt_in_error_preserves_help_line_breaks() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "console",
            "postgres",
            "--sslmode",
            "disable",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "error[OPEN_SDBL_CLI_PG_PLAINTEXT_OPT_IN_REQUIRED]: plaintext PostgreSQL transport requires explicit confirmation\n\
cause: --sslmode disable turns off encryption and certificate verification\n\
help: add --insecure-plaintext to accept plaintext, or remove --sslmode disable to use verify-full\n\
note: --socks5-proxy routes traffic but does not provide PostgreSQL transport security\n"
    );
}
