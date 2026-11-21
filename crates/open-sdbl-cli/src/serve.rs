//! `open-sdbl serve postgres|mssql`: the PostgreSQL wire-protocol gateway.
//!
//! This module is the host side: it parses the command line, reads the
//! TLS files, logs gateway events to standard error and handles signals.
//! The protocol lives in `open-sdbl-pgwire`.

use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;

use open_sdbl::query::{QueryParameter, SessionParameters};
use open_sdbl_db::{
    Credentials, DatabaseConnection, EnvironmentSecret, Limits, MsSqlConnection,
    MsSqlTransportSecurity, PostgresConnection,
};
use open_sdbl_pgwire::{GatewayConfig, GatewayEvent, GatewayEvents, GatewayState, StateHandle};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

use crate::args::parse_connection;
use crate::auth::pgpass::resolve_credentials;
use crate::config::expand_config;
use crate::error::CliError;
use crate::params::parse_parameter_literal;
use crate::progress::{MetadataProgress, log, observe};

#[cfg(test)]
#[path = "tests/serve.rs"]
mod tests;

/// The default listen address.
pub(crate) const DEFAULT_LISTEN: &str = "127.0.0.1:5433";

/// The options `serve` takes besides those of `console`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServeOptions {
    pub(crate) listen: SocketAddr,
    pub(crate) catalog_memory_limit_bytes: usize,
    pub(crate) tls_cert: Option<String>,
    pub(crate) tls_key: Option<String>,
    pub(crate) session_parameters: Vec<(String, String)>,
    pub(crate) allow_insecure_loopback: bool,
    pub(crate) allow_insecure_network: bool,
    pub(crate) verbose_errors: bool,
    pub(crate) unrestricted_access: bool,
    pub(crate) debug_authentication: bool,
    pub(crate) debug_authentication_secrets: bool,
}

fn usage(message: impl std::fmt::Display) -> CliError {
    CliError::Usage(message.to_string())
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
        catalog_memory_limit_bytes: 64 * 1024 * 1024,
        tls_cert: None,
        tls_key: None,
        session_parameters: Vec::new(),
        allow_insecure_loopback: false,
        allow_insecure_network: false,
        verbose_errors: false,
        unrestricted_access: false,
        debug_authentication: false,
        debug_authentication_secrets: false,
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
            "--catalog-memory-limit-mib" => {
                let text = value(option)?;
                options.catalog_memory_limit_bytes = text
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0)
                    .and_then(|n| n.checked_mul(1024 * 1024))
                    .ok_or_else(|| {
                        usage(format!(
                            "invalid catalog memory limit {text:?}: expected a positive MiB value"
                        ))
                    })?;
            }
            "--tls-cert" => options.tls_cert = Some(value(option)?),
            "--tls-key" => options.tls_key = Some(value(option)?),
            "--session-param" => {
                let text = value(option)?;
                let (name, literal) = text
                    .split_once('=')
                    .ok_or_else(|| usage("--session-param expects NAME=LITERAL"))?;
                let name = name.trim().trim_start_matches('&');
                if name.is_empty() || literal.trim().is_empty() {
                    return Err(usage("--session-param expects NAME=LITERAL"));
                }
                options
                    .session_parameters
                    .push((name.to_owned(), literal.trim().to_owned()));
            }
            "--allow-insecure-loopback"
            | "--allow-insecure-network"
            | "--verbose-errors"
            | "--debug-authentication"
            | "--debug-authentication-secrets"
            | "--disable-rls-check" => {
                if inline.is_some() {
                    return Err(usage(format!("option {option:?} does not take a value")));
                }
                if option == "--verbose-errors" {
                    options.verbose_errors = true;
                } else if option == "--disable-rls-check" {
                    options.unrestricted_access = true;
                } else if option == "--debug-authentication-secrets" {
                    options.debug_authentication_secrets = true;
                } else if option == "--debug-authentication" {
                    options.debug_authentication = true;
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

/// Gateway events on standard error, including secrets only when explicitly enabled.
struct StderrEvents;

fn peer_text(peer: Option<SocketAddr>) -> String {
    peer.map_or_else(|| "-".to_owned(), |peer| peer.to_string())
}

impl GatewayEvents for StderrEvents {
    fn event(&self, event: GatewayEvent<'_>) {
        let level = match event {
            GatewayEvent::ConnectionError { .. } => "ERROR",
            GatewayEvent::Refused { .. }
            | GatewayEvent::AuthenticationFailed { .. }
            | GatewayEvent::AuthenticationSecrets { .. }
            | GatewayEvent::PlaintextAuthenticated { .. }
            | GatewayEvent::SessionParameterUnread { .. } => "WARN",
            _ => "INFO",
        };
        let line = match event {
            GatewayEvent::PlaintextAuthenticated { peer, user } => {
                format!(
                    "no TLS for incoming client {} as user {user:?}",
                    peer_text(peer)
                )
            }
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
            GatewayEvent::AuthenticationDiagnostic { peer, user, reason } => format!(
                "authentication diagnostic {} for user {user:?}: {reason}",
                peer_text(peer)
            ),
            GatewayEvent::AuthenticationSecrets {
                peer,
                user,
                password,
                stored,
            } => format!(
                "SENSITIVE authentication {} user {user:?} supplied_password={password:?} stored_value={stored:?}",
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
        log(level, line);
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

async fn acquire_state(
    connection: &DatabaseConnection,
    credentials: &Credentials,
    limits: Limits,
    unrestricted: bool,
    mssql_transport: MsSqlTransportSecurity,
) -> Result<GatewayState, open_sdbl_db::DbError> {
    let mut progress = MetadataProgress::start(true, false);
    let result = observe(GatewayState::acquire_with_transport(
        connection,
        credentials,
        limits,
        &mut progress,
        unrestricted,
        mssql_transport,
    ))
    .await;
    progress.outcome(&result);
    result
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
    unrestricted_access: bool,
    mssql_transport: MsSqlTransportSecurity,
) {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut hangup) = signal(SignalKind::hangup()) else {
        log(
            "WARN",
            "cannot listen for SIGHUP; metadata refresh is unavailable",
        );
        return std::future::pending().await;
    };
    while hangup.recv().await.is_some() {
        log("INFO", "SIGHUP received; reloading gateway metadata");
        match acquire_state(
            &connection,
            &credentials,
            Limits::default(),
            unrestricted_access,
            mssql_transport,
        )
        .await
        {
            Ok(state) => {
                let users = state.user_count();
                handle.replace(state);
                log("INFO", format_args!("metadata refreshed ({users} users)"));
            }
            Err(error) => log(
                "ERROR",
                format_args!("refresh failed; serving the previous metadata: {error}"),
            ),
        }
    }
    std::future::pending().await
}

#[cfg(not(unix))]
async fn refresh_on_hangup(
    _: Arc<StateHandle>,
    _: DatabaseConnection,
    _: Credentials,
    _: bool,
    _: MsSqlTransportSecurity,
) {
    std::future::pending().await
}

pub(crate) async fn serve(
    arguments: impl Iterator<Item = String>,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let (options, rest) = split_serve_options(expand_config(arguments.collect(), true)?)?;
    let backing = parse_connection(&mut rest.into_iter(), "serve")?;
    if let Some(warning) = backing.transport_warning() {
        log("WARN", warning);
    }
    let mssql_transport = backing.mssql_transport;
    let connection = backing.connection;
    if options.unrestricted_access {
        log(
            "WARN",
            "unrestricted access enabled; authenticated users can read all supported tables without role restrictions",
        );
    }
    if options.debug_authentication_secrets {
        log(
            "WARN",
            "sensitive authentication logging enabled; passwords and stored hashes will be logged",
        );
    }
    if options.allow_insecure_network {
        log(
            "WARN",
            "no TLS required from incoming clients; passwords may be sent in plaintext",
        );
    } else if options.allow_insecure_loopback {
        log(
            "WARN",
            "no TLS required from loopback clients; passwords may be sent in plaintext",
        );
    }
    let tls = match (&options.tls_cert, &options.tls_key) {
        (Some(cert), Some(key)) => Some(load_tls(cert, key)?),
        _ => None,
    };
    if tls.is_none() && !options.allow_insecure_loopback && !options.allow_insecure_network {
        log(
            "WARN",
            "no TLS and no --allow-insecure-loopback; every client will be refused before sending a password",
        );
    }
    let credentials = resolve_credentials(&connection, credentials)?;
    let limits = Limits::default();
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    let state = tokio::select! {
        biased;
        () = &mut shutdown => {
            log("INFO", "Serve stopped");
            return Ok(());
        }
        result = acquire_state(&connection, &credentials, limits, options.unrestricted_access, mssql_transport) => result?,
    };
    let mut session_parameters = SessionParameters::new();
    for (name, literal) in &options.session_parameters {
        let value = parse_parameter_literal(literal, state.snapshot())
            .map_err(|_| usage(format!("invalid SDBL literal for --session-param {name:?}")))?;
        session_parameters.set(QueryParameter::new(name, value));
    }
    let users = state.user_count();
    if options.unrestricted_access {
        log(
            "INFO",
            format_args!("{users} users read; role rights are not loaded"),
        );
    } else {
        log(
            "INFO",
            format_args!(
                "{users} users read; {} roles carry no rights resource",
                state.unreadable_role_count()
            ),
        );
    }
    let listener = TcpListener::bind(options.listen)
        .await
        .map_err(|error| CliError::Io(format!("cannot listen on {}", options.listen), error))?;
    let address = listener
        .local_addr()
        .map_err(|error| CliError::Io("cannot read the listen address".to_owned(), error))?;
    let listener_mode = if tls.is_some() { "TLS" } else { "no TLS" };
    log(
        "INFO",
        format_args!("serve: listening on {address} ({listener_mode})"),
    );
    let handle = Arc::new(StateHandle::new(state));
    let refresh_connection = copy_connection(&connection);
    let refresh_credentials = copy_credentials(&credentials);
    let refresh_handle = Arc::clone(&handle);
    let refresh = move || {
        refresh_on_hangup(
            refresh_handle,
            refresh_connection,
            refresh_credentials,
            options.unrestricted_access,
            mssql_transport,
        )
    };
    let mut config = GatewayConfig::new(connection, credentials);
    config.limits = limits;
    config.gateway_limits.catalog_allocation_bytes = options.catalog_memory_limit_bytes;
    config.tls = tls;
    config.session_parameters = session_parameters;
    config.allow_insecure_loopback = options.allow_insecure_loopback;
    config.allow_insecure_network = options.allow_insecure_network;
    config.verbose_errors = options.verbose_errors;
    config.unrestricted_access = options.unrestricted_access;
    config.debug_authentication = options.debug_authentication;
    config.debug_authentication_secrets = options.debug_authentication_secrets;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let serving = open_sdbl_pgwire::serve_with_transport(
        listener,
        config,
        handle,
        Arc::new(StderrEvents),
        async {
            let _ = stopped.await;
        },
        mssql_transport,
    );
    tokio::pin!(serving);
    let outcome = {
        let refresh = refresh();
        tokio::pin!(refresh);
        tokio::select! {
            biased;
            () = &mut shutdown => None,
            result = &mut serving => Some(result),
            () = &mut refresh => None,
        }
    };
    let result = match outcome {
        Some(result) => result,
        None => {
            let _ = stop.send(());
            serving.await
        }
    };
    result.map_err(|error| CliError::Io("the gateway stopped".to_owned(), error))?;
    log("INFO", "Serve stopped");
    Ok(())
}
