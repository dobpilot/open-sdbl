//! A PostgreSQL wire-protocol gateway that accepts SDBL.
//!
//! Any libpq client — `psql`, JDBC and ODBC drivers, DBeaver, `psycopg` —
//! connects to the gateway as to PostgreSQL 13 and sends 1C query text
//! instead of SQL. The gateway authenticates the client as an
//! information-base user, compiles every statement in
//! [`RestrictionMode::Restricted`](open_sdbl::query::RestrictionMode::Restricted)
//! with that user's rights, runs the generated SELECT on the 1C base
//! (PostgreSQL or Microsoft SQL Server) and streams the rows back with
//! native column types. Catalog-introspection SQL is evaluated locally over
//! readable metadata, with no database I/O. The recorded psql 13/18 corpus
//! defines the currently verified client subset.
//!
//! The package owns the listening socket, client connections and database
//! sessions. It reads no environment variable, file or command line and
//! writes nothing to a terminal: the caller supplies the listener, the
//! [`GatewayConfig`] (connection, credentials, limits, parsed TLS
//! configuration, operator session parameters) and a [`GatewayEvents`]
//! sink, and stops the gateway through the shutdown future.
//!
//! ```no_run
//! # async fn example(config: open_sdbl_pgwire::GatewayConfig) -> std::io::Result<()> {
//! use std::sync::Arc;
//! use open_sdbl_db::NoProgress;
//! use open_sdbl_pgwire::{GatewayState, NoEvents, StateHandle, serve};
//!
//! let state = GatewayState::acquire(
//!     &config.connection,
//!     &config.credentials,
//!     config.limits,
//!     &mut NoProgress,
//! )
//! .await
//! .map_err(std::io::Error::other)?;
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:5433").await?;
//! serve(
//!     listener,
//!     config,
//!     Arc::new(StateHandle::new(state)),
//!     Arc::new(NoEvents),
//!     std::future::pending(),
//! )
//! .await
//! # }
//! ```
//!
//! # Authentication
//!
//! The startup `user` names the information-base user. Only a primary
//! `MATCH` of the password against a confirmed production
//! `StoredPasswordValue` of that user, with standard authentication on,
//! permits login; every other outcome is the same `28P01`. No confirmed
//! mapping from a `v8users` record to its public stored value exists (see
//! [`open_sdbl::metadata::UserData::verify_password`]), so against a real
//! base every login currently fails closed.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cancel;
mod catalog;
mod chatter;
pub mod codec;
mod connection;
mod engine;
mod errors;
mod state;
mod values;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use open_sdbl::query::SessionParameters;
use open_sdbl_db::{Credentials, DatabaseConnection, Limits};
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

pub use state::{GatewayState, StateHandle};

use crate::cancel::CancelRegistry;
use crate::engine::{DbEngine, Engine};

/// Bounds on what clients may make the gateway hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayLimits {
    /// Concurrent client connections; more are refused with `53300`.
    pub max_connections: usize,
    /// Time from accepting a connection to the end of authentication.
    pub startup_timeout: Duration,
    /// Time a connection may wait for its next message; `None` waits
    /// forever.
    pub idle_timeout: Option<Duration>,
    /// The largest frontend message accepted; a larger one is `08P01`.
    pub max_message_bytes: usize,
    /// Every failed authentication is answered no sooner than this after
    /// the password arrived, so that timing does not tell an unknown user
    /// from a wrong password.
    pub authentication_failure_delay: Duration,
}

impl Default for GatewayLimits {
    /// 64 connections, 30 seconds to authenticate, no idle limit, 16 MiB
    /// messages, and failures answered after half a second.
    fn default() -> Self {
        Self {
            max_connections: 64,
            startup_timeout: Duration::from_secs(30),
            idle_timeout: None,
            max_message_bytes: 16 * 1024 * 1024,
            authentication_failure_delay: Duration::from_millis(500),
        }
    }
}

/// Everything a gateway needs besides its listener.
pub struct GatewayConfig {
    /// The base sessions are opened against.
    pub connection: DatabaseConnection,
    /// The gateway's own database secrets.
    pub credentials: Credentials,
    /// The limits of each database session; `portal_buffer_rows` bounds
    /// held-back rows and `query_timeout` each statement.
    pub limits: Limits,
    /// The TLS configuration; `None` answers `SSLRequest` with `N`.
    pub tls: Option<Arc<rustls::ServerConfig>>,
    /// Session parameters the operator binds for every client; they win
    /// over what is derived for the user, and clients cannot change them.
    pub session_parameters: SessionParameters,
    /// Whether a loopback client may send its password without TLS.
    pub allow_insecure_loopback: bool,
    /// Whether clients on any network may send passwords without TLS.
    pub allow_insecure_network: bool,
    /// Whether database error messages are sent to clients as details.
    pub verbose_errors: bool,
    /// The bounds on client connections.
    pub gateway_limits: GatewayLimits,
}

impl GatewayConfig {
    /// A configuration with no TLS, no operator parameters and default
    /// limits.
    #[must_use]
    pub fn new(connection: DatabaseConnection, credentials: Credentials) -> Self {
        Self {
            connection,
            credentials,
            limits: Limits::default(),
            tls: None,
            session_parameters: SessionParameters::new(),
            allow_insecure_loopback: false,
            allow_insecure_network: false,
            verbose_errors: false,
            gateway_limits: GatewayLimits::default(),
        }
    }
}

/// Something that happened on the gateway, for the caller to log.
///
/// No event carries a password or a stored hash.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum GatewayEvent<'a> {
    /// A client connected.
    Connected {
        /// The client address.
        peer: Option<SocketAddr>,
    },
    /// A client was refused before authentication.
    Refused {
        /// The client address.
        peer: Option<SocketAddr>,
        /// Why.
        reason: &'a str,
    },
    /// A client authenticated.
    Authenticated {
        /// The client address.
        peer: Option<SocketAddr>,
        /// The information-base user.
        user: &'a str,
        /// The client's `application_name`.
        application_name: &'a str,
    },
    /// A client failed to authenticate.
    AuthenticationFailed {
        /// The client address.
        peer: Option<SocketAddr>,
        /// The user name the client sent.
        user: &'a str,
    },
    /// A session parameter of an authenticated user could not be read;
    /// restrictions that need it refuse their statements.
    SessionParameterUnread {
        /// The client address.
        peer: Option<SocketAddr>,
        /// The user.
        user: &'a str,
        /// What could not be read, and why.
        reason: &'a str,
    },
    /// A connection ended with an error.
    ConnectionError {
        /// The client address.
        peer: Option<SocketAddr>,
        /// What went wrong.
        message: &'a str,
    },
    /// A client disconnected.
    Disconnected {
        /// The client address.
        peer: Option<SocketAddr>,
    },
}

/// Receives [`GatewayEvent`]s.
pub trait GatewayEvents: Send + Sync {
    /// Called for each event, from the connection's task.
    fn event(&self, event: GatewayEvent<'_>);
}

/// Discards every event.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEvents;

impl GatewayEvents for NoEvents {
    fn event(&self, _event: GatewayEvent<'_>) {}
}

/// What all connections of one gateway share.
pub(crate) struct Shared<E: Engine> {
    pub(crate) database_name: String,
    pub(crate) startup_time: i64,
    pub(crate) engine: E,
    pub(crate) state: Arc<StateHandle>,
    pub(crate) tls: Option<tokio_rustls::TlsAcceptor>,
    pub(crate) session_parameters: SessionParameters,
    pub(crate) allow_insecure_loopback: bool,
    pub(crate) allow_insecure_network: bool,
    pub(crate) verbose_errors: bool,
    pub(crate) limits: GatewayLimits,
    pub(crate) portal_buffer_rows: usize,
    pub(crate) registry: Arc<CancelRegistry>,
    pub(crate) events: Arc<dyn GatewayEvents>,
}

/// Serves libpq clients on `listener` until `shutdown` completes.
///
/// After `shutdown` no connection is accepted; idle connections are told
/// the server is shutting down (`57P01`), and the function returns once
/// running statements have ended and every connection has closed.
///
/// # Errors
///
/// Returns an error only when accepting fails in a way that persists.
pub async fn serve(
    listener: TcpListener,
    config: GatewayConfig,
    state: Arc<StateHandle>,
    events: Arc<dyn GatewayEvents>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let database_name = match &config.connection {
        DatabaseConnection::Postgres(c) => c.options.database.clone(),
        DatabaseConnection::MsSql(c) => c.options.database.clone(),
    };
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let startup_time = i64::try_from(
        elapsed
            .as_secs()
            .saturating_add(u64::from(elapsed.subsec_nanos() >= 500_000_000)),
    )
    .unwrap_or(i64::MAX);
    let shared = Arc::new(Shared {
        database_name,
        startup_time,
        engine: DbEngine {
            connection: config.connection,
            credentials: config.credentials,
            limits: config.limits,
        },
        state,
        tls: config.tls.map(tokio_rustls::TlsAcceptor::from),
        session_parameters: config.session_parameters,
        allow_insecure_loopback: config.allow_insecure_loopback,
        allow_insecure_network: config.allow_insecure_network,
        verbose_errors: config.verbose_errors,
        limits: config.gateway_limits,
        portal_buffer_rows: config.limits.portal_buffer_rows,
        registry: Arc::default(),
        events,
    });
    serve_with(listener, shared, shutdown).await
}

pub(crate) async fn serve_with<E: Engine>(
    listener: TcpListener,
    shared: Arc<Shared<E>>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let (stop, stopped) = watch::channel(false);
    let permits = Arc::new(Semaphore::new(shared.limits.max_connections));
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            accepted = listener.accept() => {
                let (socket, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        shared.events.event(GatewayEvent::ConnectionError {
                            peer: None,
                            message: &format!("accept failed: {error}"),
                        });
                        // A transient failure (too many open files) must
                        // not spin the loop.
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let _ = socket.set_nodelay(true);
                let shared = Arc::clone(&shared);
                let stopped = stopped.clone();
                match Arc::clone(&permits).try_acquire_owned() {
                    Ok(permit) => {
                        tasks.spawn(async move {
                            connection::run(shared, socket, Some(peer), stopped).await;
                            drop(permit);
                        });
                    }
                    Err(_) => {
                        tasks.spawn(async move {
                            connection::refuse_busy(&shared, socket, Some(peer)).await;
                        });
                    }
                }
            }
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
        }
    }
    drop(listener);
    let _ = stop.send(true);
    while tasks.join_next().await.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests;

/// Exercises catalog SQL parsing, planning and bounded in-memory evaluation.
///
/// This entry point is available only with the `fuzzing` feature, for the
/// standalone cargo-fuzz workspace. It performs no I/O.
#[cfg(feature = "fuzzing")]
pub fn fuzz_catalog_sql(text: &str) {
    catalog::sql::fuzz(text);
}
