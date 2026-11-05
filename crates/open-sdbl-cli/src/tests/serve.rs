use super::*;

fn strings(arguments: &[&str]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
}

fn usage_text(result: Result<(ServeOptions, Vec<String>), CliError>) -> String {
    match result {
        Err(CliError::Usage(message)) => message,
        other => panic!("expected a usage error, got {other:?}"),
    }
}

#[test]
fn defaults_and_passthrough() {
    let (options, rest) =
        split_serve_options(strings(&["postgres", "--host", "db", "--user=reader"])).unwrap();
    assert_eq!(options.listen, DEFAULT_LISTEN.parse().unwrap());
    assert_eq!(options.tls_cert, None);
    assert!(!options.allow_insecure_loopback && !options.verbose_errors);
    assert_eq!(
        rest,
        strings(&["postgres", "--host", "db", "--user=reader"])
    );
}

#[test]
fn gateway_options_are_taken_out() {
    let (options, rest) = split_serve_options(strings(&[
        "mssql",
        "--listen=0.0.0.0:6543",
        "--tls-cert",
        "c.pem",
        "--tls-key=k.pem",
        "--session-param",
        "ОбластьДанныхОсновныеДанные=5",
        "--session-param=&Флаг=ИСТИНА",
        "--allow-insecure-loopback",
        "--verbose-errors",
        "--host",
        "db",
    ]))
    .unwrap();
    assert_eq!(options.listen, "0.0.0.0:6543".parse().unwrap());
    assert_eq!(options.tls_cert.as_deref(), Some("c.pem"));
    assert_eq!(options.tls_key.as_deref(), Some("k.pem"));
    assert_eq!(
        options.session_parameters,
        [
            ("ОбластьДанныхОсновныеДанные".to_owned(), "5".to_owned()),
            ("Флаг".to_owned(), "ИСТИНА".to_owned())
        ]
    );
    assert!(options.allow_insecure_loopback && options.verbose_errors);
    assert_eq!(rest, strings(&["mssql", "--host", "db"]));
}

#[test]
fn network_listen_without_tls_is_refused() {
    for listen in ["0.0.0.0:5433", "192.0.2.1:5433", "[::]:5433"] {
        let message = usage_text(split_serve_options(strings(&[
            "postgres", "--listen", listen,
        ])));
        assert!(message.contains("without TLS"), "{message}");
    }
    // Even the insecure-loopback flag does not allow it.
    let message = usage_text(split_serve_options(strings(&[
        "postgres",
        "--listen",
        "0.0.0.0:5433",
        "--allow-insecure-loopback",
    ])));
    assert!(message.contains("without TLS"));
    // Loopback addresses need no TLS.
    for listen in ["127.0.0.1:5433", "[::1]:5433"] {
        split_serve_options(strings(&["postgres", "--listen", listen])).unwrap();
    }
}

#[test]
fn malformed_serve_options_are_usage_errors() {
    for arguments in [
        &["postgres", "--listen", "nowhere"][..],
        &["postgres", "--listen"],
        &["postgres", "--tls-cert", "c.pem"],
        &["postgres", "--tls-key", "k.pem"],
        &["postgres", "--session-param", "НетЗнака"],
        &["postgres", "--session-param", "=1"],
        &["postgres", "--session-param", "Имя="],
        &["postgres", "--verbose-errors=yes"],
        &["postgres", "--tls-cert", "--tls-key"],
    ] {
        usage_text(split_serve_options(strings(arguments)));
    }
}

#[test]
fn tls_files_are_validated() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../open-sdbl-pgwire/tests/fixtures"
    );
    let cert = format!("{directory}/gateway-cert.pem");
    let key = format!("{directory}/gateway-key.pem");
    load_tls(&cert, &key).unwrap();
    assert!(matches!(load_tls(&key, &key), Err(CliError::Data(_))));
    assert!(matches!(load_tls(&cert, &cert), Err(CliError::Data(_))));
    assert!(matches!(
        load_tls("/nonexistent/cert.pem", &key),
        Err(CliError::Io(_, _))
    ));
}

#[test]
fn config_uses_existing_validation() {
    let args = super::config_arguments(
        r#"
provider = "postgres"
host = "db"
database = "base"
user = "reader"
port = 5432
allow_insecure_loopback = true
session_params = ["X=1"]
"#,
    )
    .unwrap();
    let (options, rest) = split_serve_options(args).unwrap();
    assert!(options.allow_insecure_loopback);
    assert_eq!(options.session_parameters, vec![("X".into(), "1".into())]);
    assert!(
        crate::args::parse_connection(&mut rest.into_iter(), "serve", &mut Vec::new())
            .unwrap()
            .is_some()
    );
    assert!(super::config_arguments("provider = 'postgres'\npassword = 'secret'").is_err());
    assert!(super::config_arguments("provider = 'postgres'\nport = '5432'").is_err());
    let args = super::config_arguments("provider = 'postgres'\nlisten = '0.0.0.0:5433'").unwrap();
    assert!(split_serve_options(args).is_err());
}
