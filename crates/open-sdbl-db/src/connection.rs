//! How to reach a base: host, port, credentials policy and transport.
//!
//! These are plain descriptions. Where the values come from — a command
//! line, a configuration file, a request — is the application's business.

use open_sdbl::query::MsSqlDialectLevel;

use crate::net::socks5::Socks5Proxy;

/// The base to open a session against.
#[derive(Debug)]
pub enum DatabaseConnection {
    /// A PostgreSQL base.
    Postgres(PostgresConnection),
    /// A Microsoft SQL Server base.
    MsSql(MsSqlConnection),
}

/// What both providers need: where the server is and who connects.
#[derive(Clone, Debug)]
pub struct ConnectionOptions {
    /// Host name or address of the server.
    pub host: String,
    /// TCP port of the server.
    pub port: u16,
    /// Name of the information base.
    pub database: String,
    /// The login to authenticate as.
    pub user: String,
    /// The SOCKS5 proxy to route through, when one is used.
    pub socks5_proxy: Option<Socks5Proxy>,
}

/// A PostgreSQL base and its transport security.
#[derive(Clone, Debug)]
pub struct PostgresConnection {
    /// Where the server is and who connects.
    pub options: ConnectionOptions,
    /// How the transport is secured.
    pub sslmode: PostgresSslMode,
    /// A private CA to trust instead of the system roots.
    pub trust_ca_file: Option<String>,
}

impl std::ops::Deref for PostgresConnection {
    type Target = ConnectionOptions;

    fn deref(&self) -> &Self::Target {
        &self.options
    }
}

/// How a PostgreSQL connection is secured, in `libpq` terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PostgresSslMode {
    /// No encryption and no verification.
    Disable,
    /// Encryption without certificate verification.
    Require,
    /// Encryption with the certificate chain verified.
    VerifyCa,
    /// Encryption with the chain and the host name verified.
    VerifyFull,
}

/// A Microsoft SQL Server base and its transport security.
#[derive(Clone, Debug)]
pub struct MsSqlConnection {
    /// Where the server is and who connects.
    pub options: ConnectionOptions,
    /// Accept any certificate the server presents; unsafe.
    pub trust_server_certificate: bool,
    /// A specific PEM, CRT, or DER certificate to trust.
    pub trust_ca_file: Option<String>,
    /// Explicit dialect level; `None` means detect it from the server.
    pub dialect_level: Option<MsSqlDialectLevel>,
}
