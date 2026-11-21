use std::process::Command;

#[test]
fn metadata_help_and_required_options_are_reported() {
    let help = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args(["metadata", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("metadata <postgres|mssql> --host HOST"));
    assert!(help.contains("--socks5-proxy HOST:PORT"));
    assert!(help.contains("--mssql-dialect LEVEL"));
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
            .contains("missing required options --database, --user")
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
help: add --insecure-plaintext, use --tls-mode disable, or remove --sslmode disable to use verify-full\n\
note: --socks5-proxy routes traffic but does not provide PostgreSQL transport security\n"
    );
}

#[test]
fn scoped_help_precedes_files_and_listener_validation() {
    let binary = env!("CARGO_BIN_EXE_open-sdbl");
    for (args, syntax) in [
        (
            &["serve", "postgres", "--listen", "0.0.0.0:5433", "--help"][..],
            "serve postgres --host HOST",
        ),
        (
            &["metadata", "--config", "missing.toml", "-h"][..],
            "metadata <postgres|mssql>",
        ),
        (
            &["console", "mssql", "--help"][..],
            "console mssql --host HOST",
        ),
        (&[][..], "<command> [options]"),
    ] {
        let output = Command::new(binary).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert!(output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains(syntax) && help.contains("\nExample\n"));
        assert_eq!(help.matches("\nExample\n").count(), 1);
        assert!(!help.contains('\x1b'));
    }
}

#[test]
fn runtime_and_argument_diagnostics_use_stderr() {
    let binary = env!("CARGO_BIN_EXE_open-sdbl");
    for (args, problem) in [
        (
            &["metadata", "postgres", "--host", "db"][..],
            "missing required options --database, --user",
        ),
        (&["serve", "postgres", "--wat"][..], "unknown serve option"),
        (&["serve", "postgres", "--listen"][..], "missing value"),
        (
            &["metadata", "--config", "missing-test-config.toml"][..],
            "cannot read configuration",
        ),
    ] {
        let output = Command::new(binary).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains(problem), "{diagnostic}");
        assert!(!diagnostic.contains("\nExample\n"));
    }
}
