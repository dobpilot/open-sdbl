//! Tests of the `args` module.

use super::{
    BackingDatabase, DatabaseConnection, MsSqlTransportSecurity, PostgresSslMode, parse_connection,
    select_postgres_sslmode,
};
use crate::error::CliError;

#[test]
fn parses_inline_values_and_rejects_option_like_values() {
    let mut inline = [
        "postgres",
        "--host=db",
        "--database=test",
        "--user=reader",
        "--sslmode=verify-full",
    ]
    .into_iter()
    .map(str::to_owned);
    let parsed = parse_connection(&mut inline, "console").unwrap();
    assert!(matches!(parsed.connection, DatabaseConnection::Postgres(_)));

    let mut invalid = ["postgres", "--host", "--database=test"]
        .into_iter()
        .map(str::to_owned);
    assert!(
        parse_connection(&mut invalid, "console")
            .unwrap_err()
            .to_string()
            .contains("option-like")
    );
}

/// The proxy credentials reach the connection options that
/// `net::socks5` later reads, so the flag is parsed here.
#[test]
fn carries_socks5_proxy_credentials_into_connection_options() {
    let mut arguments = [
        "mssql",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--socks5-user",
        "proxy-reader",
        "--socks5-proxy",
        "proxy.example:1080",
    ]
    .into_iter()
    .map(str::to_owned);
    let connection = parse_connection(&mut arguments, "console")
        .unwrap()
        .connection;
    let DatabaseConnection::MsSql(connection) = connection else {
        panic!("expected MSSQL connection");
    };
    assert_eq!(
        connection
            .options
            .socks5_proxy
            .as_ref()
            .and_then(|proxy| proxy.username.as_deref()),
        Some("proxy-reader")
    );
}

#[test]
fn selects_secure_postgres_ssl_modes_with_flag_precedence() {
    assert_eq!(
        select_postgres_sslmode(None, None).unwrap(),
        PostgresSslMode::VerifyFull
    );
    assert_eq!(
        select_postgres_sslmode(None, Some("verify-ca")).unwrap(),
        PostgresSslMode::VerifyCa
    );
    assert_eq!(
        select_postgres_sslmode(Some(PostgresSslMode::Require), Some("invalid")).unwrap(),
        PostgresSslMode::Require
    );
    assert!(select_postgres_sslmode(None, Some("prefer")).is_err());
}

#[test]
fn refuses_postgres_plaintext_without_explicit_opt_in() {
    let base = [
        "postgres",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--sslmode",
        "disable",
    ];
    let mut refused = base.into_iter().map(str::to_owned);
    let error = parse_connection(&mut refused, "console").unwrap_err();
    assert!(matches!(error, CliError::PostgresPlaintextOptInRequired));

    let mut accepted = base
        .into_iter()
        .chain(["--insecure-plaintext"])
        .map(str::to_owned);
    let connection = parse_connection(&mut accepted, "console")
        .unwrap()
        .connection;
    let DatabaseConnection::Postgres(connection) = connection else {
        panic!("expected PostgreSQL connection");
    };
    assert_eq!(connection.sslmode, PostgresSslMode::Disable);
}

#[test]
fn accepts_inline_option_values_and_rejects_option_like_values() {
    let mut inline = ["postgres", "--host=db", "--database=test", "--user=reader"]
        .into_iter()
        .map(str::to_owned);
    let connection = parse_connection(&mut inline, "console").unwrap().connection;
    let DatabaseConnection::Postgres(connection) = connection else {
        panic!("expected PostgreSQL connection");
    };
    assert_eq!(connection.options.host, "db");

    let mut option_like = ["postgres", "--host", "--database=test", "--user=reader"]
        .into_iter()
        .map(str::to_owned);
    let error = parse_connection(&mut option_like, "console").unwrap_err();
    assert!(error.to_string().contains("option-like"));
}

#[test]
fn parses_mssql_provider_defaults_and_explicit_tls_exception() {
    let mut arguments = [
        "mssql",
        "--host",
        "192.168.122.222",
        "--database",
        "demo",
        "--user",
        "reader",
        "--trust-server-certificate",
    ]
    .into_iter()
    .map(str::to_owned);
    let parsed = parse_connection(&mut arguments, "metadata").unwrap();
    assert_eq!(parsed.mssql_transport, MsSqlTransportSecurity::Tls);
    assert!(
        parsed
            .transport_warning()
            .unwrap()
            .contains("certificate and hostname are unverified")
    );
    let DatabaseConnection::MsSql(connection) = parsed.connection else {
        panic!("expected MSSQL connection");
    };
    assert_eq!(connection.options.host, "192.168.122.222");
    assert_eq!(connection.options.port, 1433);
    assert_eq!(connection.options.database, "demo");
    assert!(connection.trust_server_certificate);
}

#[test]
fn parses_private_ca_files_for_both_database_providers() {
    let mut arguments = [
        "mssql",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--trust-ca-file",
        "company-ca.pem",
    ]
    .into_iter()
    .map(str::to_owned);
    let connection = parse_connection(&mut arguments, "metadata")
        .unwrap()
        .connection;
    let DatabaseConnection::MsSql(connection) = connection else {
        panic!("expected MSSQL connection");
    };
    assert_eq!(connection.trust_ca_file.as_deref(), Some("company-ca.pem"));

    let mut arguments = [
        "postgres",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--trust-ca-file",
        "company-ca.pem",
    ]
    .into_iter()
    .map(str::to_owned);
    let connection = parse_connection(&mut arguments, "metadata")
        .unwrap()
        .connection;
    let DatabaseConnection::Postgres(connection) = connection else {
        panic!("expected PostgreSQL connection");
    };
    assert_eq!(connection.trust_ca_file.as_deref(), Some("company-ca.pem"));

    let mut arguments = [
        "postgres",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--sslmode",
        "require",
        "--trust-ca-file",
        "company-ca.pem",
    ]
    .into_iter()
    .map(str::to_owned);
    let error = parse_connection(&mut arguments, "metadata").unwrap_err();
    assert!(error.to_string().contains("requires PostgreSQL --sslmode"));
}

#[test]
fn rejects_mssql_only_tls_flag_for_postgres() {
    let mut arguments = [
        "postgres",
        "--host",
        "db",
        "--database",
        "test",
        "--user",
        "reader",
        "--trust-server-certificate",
    ]
    .into_iter()
    .map(str::to_owned);
    let error = parse_connection(&mut arguments, "console").unwrap_err();
    assert!(error.to_string().contains("unknown console option"));
}

#[test]
fn rejects_password_bearing_command_line_flags() {
    for option in ["--password", "--db-password", "--socks5-password"] {
        let mut arguments = [
            "postgres",
            "--host",
            "db",
            "--database",
            "test",
            "--user",
            "reader",
            option,
            "secret",
        ]
        .into_iter()
        .map(str::to_owned);
        let error = parse_connection(&mut arguments, "console").unwrap_err();
        assert!(error.to_string().contains("unknown console option"));
    }
}

#[test]
fn canonical_transport_modes_agree_on_plaintext_and_reject_mixed_policy() {
    fn parse(provider: &str, tls_arguments: &[&str]) -> Result<BackingDatabase, CliError> {
        let mut args = [
            provider,
            "--host",
            "db",
            "--database",
            "base",
            "--user",
            "reader",
        ]
        .into_iter()
        .chain(tls_arguments.iter().copied())
        .map(str::to_owned);
        parse_connection(&mut args, "serve")
    }

    for provider in ["postgres", "mssql"] {
        let plaintext = parse(provider, &["--tls-mode", "disable", "--tls-mode=disable"]).unwrap();
        assert!(plaintext.transport_warning().unwrap().contains("no TLS"));
        match plaintext.connection {
            DatabaseConnection::Postgres(connection) => {
                assert_eq!(connection.sslmode, PostgresSslMode::Disable)
            }
            DatabaseConnection::MsSql(_) => {
                assert_eq!(plaintext.mssql_transport, MsSqlTransportSecurity::Plaintext)
            }
        }
        assert!(
            parse(
                provider,
                &["--tls-mode", "disable", "--tls-mode", "require"]
            )
            .is_err()
        );
        assert!(
            parse(
                provider,
                &["--tls-mode", "require", "--trust-ca-file", "ca.pem"]
            )
            .is_err()
        );
    }
    assert!(
        parse(
            "postgres",
            &["--tls-mode", "verify-full", "--sslmode", "verify-full"]
        )
        .is_err()
    );
    assert!(
        parse(
            "mssql",
            &["--tls-mode", "require", "--trust-server-certificate"]
        )
        .is_err()
    );
    assert!(parse("mssql", &["--tls-mode", "verify-ca"]).is_err());
}
