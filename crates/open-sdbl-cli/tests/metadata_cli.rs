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
