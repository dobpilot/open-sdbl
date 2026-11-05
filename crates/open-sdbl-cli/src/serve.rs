//! `open-sdbl serve postgres|mssql`: the PostgreSQL wire-protocol gateway.
//!
//! This module is the host side: it parses the command line, reads the
//! TLS files, logs gateway events to standard error and handles signals.
//! The protocol lives in `open-sdbl-pgwire`.

use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use open_sdbl::query::{QueryParameter, SessionParameters};
use open_sdbl_db::{
    Credentials, DatabaseConnection, EnvironmentSecret, Limits, MsSqlConnection, PostgresConnection,
};
use open_sdbl_pgwire::{GatewayConfig, GatewayEvent, GatewayEvents, GatewayState, StateHandle};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

use crate::args::{HELP, parse_connection};
use crate::auth::pgpass::resolve_credentials;
use crate::error::CliError;
use crate::output::escape_field;
use crate::params::parse_parameter_literal;
use crate::progress::MetadataProgress;

#[cfg(test)]
#[path = "tests/serve.rs"]
mod tests;

/// The default listen address.
pub(crate) const DEFAULT_LISTEN: &str = "127.0.0.1:5433";

/// The options `serve` takes besides those of `console`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServeOptions {
    pub(crate) listen: SocketAddr,
    pub(crate) tls_cert: Option<String>,
    pub(crate) tls_key: Option<String>,
    pub(crate) session_parameters: Vec<(String, String)>,
    pub(crate) allow_insecure_loopback: bool,
    pub(crate) allow_insecure_network: bool,
    pub(crate) verbose_errors: bool,
}

fn usage(message: impl std::fmt::Display) -> CliError {
    CliError::Usage(format!("{message}\n\n{HELP}"))
}

/// Takes the gateway options out of `arguments`; the rest are the
/// connection options of `console`, in order.
///
/// A plain network listener requires explicit opt-in before connecting to the database.
pub(crate) fn split_serve_options(
    arguments: Vec<String>,
) -> Result<(ServeOptions, Vec<String>), CliError> {
    let mut options = ServeOptions {
        listen: DEFAULT_LISTEN.parse().expect("the default address parses"),
        tls_cert: None,
        tls_key: None,
        session_parameters: Vec::new(),
        allow_insecure_loopback: false,
        allow_insecure_network: false,
        verbose_errors: false,
    };
    let mut rest = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let (option, inline) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(option, value)| {
                (option, Some(value.to_owned()))
            });
        let mut value = |option: &str| -> Result<String, CliError> {
            let value = inline
                .clone()
                .or_else(|| arguments.next())
                .ok_or_else(|| usage(format!("missing value for {option:?}")))?;
            if value.is_empty() || value.starts_with('-') {
                return Err(usage(format!(
                    "invalid option-like or empty value {value:?} for {option:?}"
                )));
            }
            Ok(value)
        };
        match option {
            "--listen" => {
                let text = value(option)?;
                options.listen = text
                    .parse()
                    .map_err(|_| usage(format!("invalid listen address {text:?}")))?;
            }
            "--tls-cert" => options.tls_cert = Some(value(option)?),
            "--tls-key" => options.tls_key = Some(value(option)?),
            "--session-param" => {
                let text = value(option)?;
                let (name, literal) = text.split_once('=').ok_or_else(|| {
                    usage(format!(
                        "--session-param expects NAME=LITERAL, got {text:?}"
                    ))
                })?;
                let name = name.trim().trim_start_matches('&');
                if name.is_empty() || literal.trim().is_empty() {
                    return Err(usage(format!(
                        "--session-param expects NAME=LITERAL, got {text:?}"
                    )));
                }
                options
                    .session_parameters
                    .push((name.to_owned(), literal.trim().to_owned()));
            }
            "--allow-insecure-loopback" | "--allow-insecure-network" | "--verbose-errors" => {
                if inline.is_some() {
                    return Err(usage(format!("option {option:?} does not take a value")));
                }
                if option == "--verbose-errors" {
                    options.verbose_errors = true;
                } else if option == "--allow-insecure-network" {
                    options.allow_insecure_network = true;
                } else {
                    options.allow_insecure_loopback = true;
                }
            }
            _ => rest.push(argument),
        }
    }
    match (&options.tls_cert, &options.tls_key) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(usage("--tls-cert and --tls-key go together"));
        }
        (None, None) if !options.listen.ip().is_loopback() && !options.allow_insecure_network => {
            return Err(usage(format!(
                "refusing to listen on {} without TLS: passwords would cross the network in clear text; pass --tls-cert and --tls-key, listen on a loopback address, or pass --allow-insecure-network",
                options.listen
            )));
        }
        _ => {}
    }
    Ok((options, rest))
}

/// Convert the configuration to the existing command-line representation so
/// both entry points share all connection and security validation.
fn config_arguments(text: &str) -> Result<Vec<String>, CliError> {
    let table: toml::Table = toml::from_str(text)
        .map_err(|error| usage(format!("invalid TOML configuration: {error}")))?;
    let mut args = vec![
        table
            .get("provider")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| usage("configuration requires a string provider"))?
            .to_owned(),
    ];
    for (key, value) in &table {
        let option = format!("--{}", key.replace('_', "-"));
        match key.as_str() {
            "provider" => {}
            "host" | "database" | "user" | "sslmode" | "trust_ca_file" | "socks5_proxy"
            | "socks5_user" | "mssql_dialect" | "listen" | "tls_cert" | "tls_key" => {
                let value = value
                    .as_str()
                    .ok_or_else(|| usage(format!("{key} must be a string")))?;
                args.extend([option, value.to_owned()]);
            }
            "port" => {
                let value = value
                    .as_integer()
                    .ok_or_else(|| usage("port must be an integer"))?;
                args.extend([option, value.to_string()]);
            }
            "insecure_plaintext"
            | "trust_server_certificate"
            | "allow_insecure_loopback"
            | "allow_insecure_network"
            | "verbose_errors" => {
                if value
                    .as_bool()
                    .ok_or_else(|| usage(format!("{key} must be a boolean")))?
                {
                    args.push(option);
                }
            }
            "session_params" => {
                let values = value
                    .as_array()
                    .ok_or_else(|| usage("session_params must be an array of strings"))?;
                for value in values {
                    args.extend([
                        "--session-param".to_owned(),
                        value
                            .as_str()
                            .ok_or_else(|| usage("session_params must contain strings"))?
                            .to_owned(),
                    ]);
                }
            }
            _ => return Err(usage(format!("unknown configuration field {key:?}"))),
        }
    }
    Ok(args)
}

fn expand_config(arguments: Vec<String>) -> Result<Vec<String>, CliError> {
    if arguments.first().is_some_and(|arg| arg == "--config") {
        if arguments.len() != 2 {
            return Err(usage(
                "--config requires a path and cannot be combined with other options",
            ));
        }
        let path = &arguments[1];
        let text = fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read configuration {path:?}"), error))?;
        config_arguments(&text)
    } else {
        Ok(arguments)
    }
}

/// Reads a PEM certificate chain and private key into a TLS configuration.
pub(crate) fn load_tls(cert: &str, key: &str) -> Result<Arc<rustls::ServerConfig>, CliError> {
    let chain = fs::read(cert)
        .map_err(|error| CliError::Io(format!("cannot read TLS certificate {cert:?}"), error))?;
    let certificates = CertificateDer::pem_slice_iter(&chain)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| CliError::Data(format!("invalid TLS certificate {cert:?}: {error}")))?;
    if certificates.is_empty() {
        return Err(CliError::Data(format!(
            "TLS certificate file {cert:?} holds no certificate"
        )));
    }
    let key_bytes = Zeroizing::new(
        fs::read(key)
            .map_err(|error| CliError::Io(format!("cannot read TLS key {key:?}"), error))?,
    );
    let private_key = PrivateKeyDer::from_pem_slice(&key_bytes)
        .map_err(|error| CliError::Data(format!("invalid TLS key {key:?}: {error}")))?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| CliError::Data(format!("cannot configure TLS: {error}")))?
    .with_no_client_auth()
    .with_single_cert(certificates, private_key)
    .map_err(|error| {
        CliError::Data(format!("the TLS certificate and key do not match: {error}"))
    })?;
    Ok(Arc::new(config))
}

/// Gateway events on standard error; no event carries a password.
struct StderrEvents;

fn peer_text(peer: Option<SocketAddr>) -> String {
    peer.map_or_else(|| "-".to_owned(), |peer| peer.to_string())
}

impl GatewayEvents for StderrEvents {
    fn event(&self, event: GatewayEvent<'_>) {
        let line = match event {
            GatewayEvent::Connected { .. } | GatewayEvent::Disconnected { .. } => return,
            GatewayEvent::Refused { peer, reason } => {
                format!("refused {}: {reason}", peer_text(peer))
            }
            GatewayEvent::Authenticated {
                peer,
                user,
                application_name,
            } => format!(
                "connection {} as user {user:?} ({application_name:?})",
                peer_text(peer)
            ),
            GatewayEvent::AuthenticationFailed { peer, user } => format!(
                "authentication failed {} for user {user:?}",
                peer_text(peer)
            ),
            GatewayEvent::SessionParameterUnread { peer, user, reason } => format!(
                "session parameter unread for user {user:?} {}: {reason}",
                peer_text(peer)
            ),
            GatewayEvent::ConnectionError { peer, message } => {
                format!("connection {}: {message}", peer_text(peer))
            }
            _ => return,
        };
        eprintln!("serve: {}", escape_field(&line));
    }
}

/// A copy of a connection description, for the refresh task.
fn copy_connection(connection: &DatabaseConnection) -> DatabaseConnection {
    match connection {
        DatabaseConnection::Postgres(connection) => {
            DatabaseConnection::Postgres(PostgresConnection {
                options: connection.options.clone(),
                sslmode: connection.sslmode,
                trust_ca_file: connection.trust_ca_file.clone(),
            })
        }
        DatabaseConnection::MsSql(connection) => DatabaseConnection::MsSql(MsSqlConnection {
            options: connection.options.clone(),
            trust_server_certificate: connection.trust_server_certificate,
            trust_ca_file: connection.trust_ca_file.clone(),
            dialect_level: connection.dialect_level,
        }),
    }
}

fn copy_credentials(credentials: &Credentials) -> Credentials {
    let copy = |secret: &EnvironmentSecret| secret.clone();
    Credentials {
        postgres: copy(&credentials.postgres),
        mssql: copy(&credentials.mssql),
        socks5: copy(&credentials.socks5),
    }
}

/// Completes on Ctrl-C, or on `SIGTERM` on Unix.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

/// Re-reads metadata, users and rights on every `SIGHUP`; never completes.
///
/// Reading metadata is not `Send`, so this runs beside the gateway in the
/// command's own future rather than on a task of its own.
#[cfg(unix)]
async fn refresh_on_hangup(
    handle: Arc<StateHandle>,
    connection: DatabaseConnection,
    credentials: Credentials,
) {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut hangup) = signal(SignalKind::hangup()) else {
        eprintln!("warning: cannot listen for SIGHUP; metadata refresh is unavailable");
        return std::future::pending().await;
    };
    while hangup.recv().await.is_some() {
        eprintln!("serve: SIGHUP received; reading metadata, users and rights again");
        match GatewayState::acquire(
            &connection,
            &credentials,
            Limits::default(),
            &mut MetadataProgress::new(),
        )
        .await
        {
            Ok(state) => {
                let users = state.user_count();
                handle.replace(state);
                eprintln!("serve: metadata refreshed ({users} users)");
            }
            Err(error) => eprintln!(
                "serve: refresh failed, the previous metadata stays: {}",
                escape_field(&error.to_string())
            ),
        }
    }
    std::future::pending().await
}

#[cfg(not(unix))]
async fn refresh_on_hangup(_: Arc<StateHandle>, _: DatabaseConnection, _: Credentials) {
    std::future::pending().await
}

pub(crate) async fn serve(
    arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let (options, rest) = split_serve_options(expand_config(arguments.collect())?)?;
    let Some(connection) = parse_connection(&mut rest.into_iter(), "serve", output)? else {
        return Ok(());
    };
    if options.allow_insecure_network {
        eprintln!(
            "warning: insecure network connections enabled; client passwords may be sent without TLS"
        );
    }
    let tls = match (&options.tls_cert, &options.tls_key) {
        (Some(cert), Some(key)) => Some(load_tls(cert, key)?),
        _ => None,
    };
    if tls.is_none() && !options.allow_insecure_loopback && !options.allow_insecure_network {
        eprintln!(
            "warning: no TLS and no --allow-insecure-loopback: every client will be refused before it sends a password"
        );
    }
    let credentials = resolve_credentials(&connection, credentials)?;
    let limits = Limits::default();
    let state = GatewayState::acquire(
        &connection,
        &credentials,
        limits,
        &mut MetadataProgress::new(),
    )
    .await?;
    let mut session_parameters = SessionParameters::new();
    for (name, literal) in &options.session_parameters {
        let value = parse_parameter_literal(literal, state.snapshot())
            .map_err(|error| usage(format!("invalid --session-param {name}: {error}")))?;
        session_parameters.set(QueryParameter::new(name, value));
    }
    eprintln!(
        "serve: {} users read; {} roles carry no rights resource",
        state.user_count(),
        state.unreadable_role_count()
    );
    eprintln!(
        "serve: passwords are checked against the stored value read from each v8users record; a user without a readable stored password cannot log in (28P01)"
    );
    let listener = TcpListener::bind(options.listen)
        .await
        .map_err(|error| CliError::Io(format!("cannot listen on {}", options.listen), error))?;
    let address = listener
        .local_addr()
        .map_err(|error| CliError::Io("cannot read the listen address".to_owned(), error))?;
    eprintln!(
        "serve: listening on {address} ({})",
        if tls.is_some() { "TLS" } else { "no TLS" }
    );
    let handle = Arc::new(StateHandle::new(state));
    let refresh = refresh_on_hangup(
        Arc::clone(&handle),
        copy_connection(&connection),
        copy_credentials(&credentials),
    );
    let mut config = GatewayConfig::new(connection, credentials);
    config.limits = limits;
    config.tls = tls;
    config.session_parameters = session_parameters;
    config.allow_insecure_loopback = options.allow_insecure_loopback;
    config.allow_insecure_network = options.allow_insecure_network;
    config.verbose_errors = options.verbose_errors;
    let serving = open_sdbl_pgwire::serve(
        listener,
        config,
        handle,
        Arc::new(StderrEvents),
        shutdown_signal(),
    );
    let result = tokio::select! {
        result = serving => result,
        () = refresh => Ok(()),
    };
    result.map_err(|error| CliError::Io("the gateway stopped".to_owned(), error))?;
    eprintln!("serve: stopped");
    Ok(())
}
