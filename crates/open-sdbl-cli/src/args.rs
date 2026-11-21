use std::env;

use open_sdbl::query::MsSqlDialectLevel;
use open_sdbl_db::{
    ConnectionOptions, DatabaseConnection, MsSqlConnection, MsSqlTransportSecurity,
    PostgresConnection, PostgresSslMode, parse_socks5_proxy,
};

use crate::error::CliError;

pub(crate) const INSECURE_MSSQL_CERTIFICATE_WARNING: &str = "MSSQL TLS encrypts traffic; the certificate and hostname are unverified. Prefer --trust-ca-file.";

#[derive(Debug)]
pub(crate) struct BackingDatabase {
    pub(crate) connection: DatabaseConnection,
    pub(crate) mssql_transport: MsSqlTransportSecurity,
}

impl BackingDatabase {
    pub(crate) fn transport_warning(&self) -> Option<&'static str> {
        match &self.connection {
            DatabaseConnection::Postgres(connection) => match connection.sslmode {
                PostgresSslMode::Disable => Some("no TLS for the backing PostgreSQL connection"),
                PostgresSslMode::Require => Some(
                    "PostgreSQL TLS encrypts traffic; the certificate and hostname are unverified",
                ),
                _ => None,
            },
            DatabaseConnection::MsSql(_)
                if self.mssql_transport == MsSqlTransportSecurity::Plaintext =>
            {
                Some("no TLS for the backing MSSQL connection")
            }
            DatabaseConnection::MsSql(connection) if connection.trust_server_certificate => {
                Some(INSECURE_MSSQL_CERTIFICATE_WARNING)
            }
            DatabaseConnection::MsSql(_) => None,
        }
    }
}

pub(crate) fn parse_connection(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
) -> Result<BackingDatabase, CliError> {
    let Some(provider) = arguments.next() else {
        return Err(CliError::Usage(format!("missing {command} provider")));
    };
    if !matches!(provider.as_str(), "postgres" | "mssql") {
        return Err(CliError::Usage(format!(
            "unsupported {command} provider {provider:?}"
        )));
    }

    let mut options = ConnectionOptions {
        host: String::new(),
        port: if provider == "postgres" { 5432 } else { 1433 },
        database: String::new(),
        user: String::new(),
        socks5_proxy: None,
    };
    let mut trust_server_certificate = false;
    let mut trust_ca_file = None;
    let mut mssql_dialect = None;
    let mut postgres_sslmode = None;
    let mut tls_mode = None;
    let mut insecure_plaintext = false;
    let mut socks5_user = None;
    while let Some(argument) = arguments.next() {
        let (option, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(option, value)| {
                (option, Some(value.to_owned()))
            });
        if option == "--trust-server-certificate" {
            reject_inline_flag_value(option, inline_value.as_deref())?;
            if provider != "mssql" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}"
                )));
            }
            trust_server_certificate = true;
            continue;
        }
        if option == "--insecure-plaintext" {
            reject_inline_flag_value(option, inline_value.as_deref())?;
            if provider != "postgres" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}"
                )));
            }
            insecure_plaintext = true;
            continue;
        }
        if !matches!(
            option,
            "--host"
                | "--database"
                | "--user"
                | "--port"
                | "--socks5-proxy"
                | "--socks5-user"
                | "--trust-ca-file"
                | "--tls-mode"
        ) && !(option == "--sslmode" && provider == "postgres")
            && !(option == "--mssql-dialect" && provider == "mssql")
        {
            return Err(CliError::Usage(format!(
                "unknown {command} option {option:?}"
            )));
        }
        let value = option_value(arguments, option, inline_value)?;
        match option {
            "--host" => options.host = value,
            "--database" => options.database = value,
            "--user" => options.user = value,
            "--port" => {
                options.port = value
                    .parse()
                    .map_err(|_| CliError::Usage(format!("invalid {provider} port {value:?}")))?;
            }
            "--socks5-proxy" => {
                options.socks5_proxy = Some(parse_socks5_proxy(&value).map_err(|reason| {
                    CliError::Usage(format!("invalid SOCKS5 proxy: {reason}"))
                })?);
            }
            "--socks5-user" => socks5_user = Some(value),
            "--tls-mode" => {
                let mode = parse_postgres_sslmode(&value).map_err(|reason| {
                    CliError::Usage(format!("invalid --tls-mode {value:?}: {reason}"))
                })?;
                if tls_mode.is_some_and(|previous| previous != mode) {
                    return Err(CliError::Usage("conflicting --tls-mode values".into()));
                }
                tls_mode = Some(mode);
            }
            "--sslmode" if provider == "postgres" => {
                postgres_sslmode = Some(parse_postgres_sslmode(&value).map_err(|reason| {
                    CliError::Usage(format!("invalid PostgreSQL sslmode {value:?}: {reason}"))
                })?);
            }
            "--trust-ca-file" => trust_ca_file = Some(value),
            "--mssql-dialect" if provider == "mssql" => {
                mssql_dialect = Some(MsSqlDialectLevel::parse(&value).ok_or_else(|| {
                    CliError::Usage(format!(
                        "invalid MSSQL dialect level {value:?}: expected 2008 or 2012"
                    ))
                })?);
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}"
                )));
            }
        }
    }
    let missing: Vec<_> = [
        ("--host", &options.host),
        ("--database", &options.database),
        ("--user", &options.user),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.is_empty().then_some(name))
    .collect();
    if !missing.is_empty() {
        return Err(CliError::Usage(format!(
            "missing required options {}",
            missing.join(", ")
        )));
    }
    if socks5_user.is_some() && options.socks5_proxy.is_none() {
        return Err(CliError::Usage(
            "--socks5-user requires --socks5-proxy".to_owned(),
        ));
    }
    if let Some(proxy) = options.socks5_proxy.as_mut() {
        proxy.username = socks5_user;
    }

    if tls_mode.is_some()
        && (postgres_sslmode.is_some() || insecure_plaintext || trust_server_certificate)
    {
        return Err(CliError::Usage(
            "--tls-mode cannot be combined with legacy TLS flags".into(),
        ));
    }
    if trust_ca_file.is_some()
        && matches!(
            tls_mode,
            Some(PostgresSslMode::Disable | PostgresSslMode::Require)
        )
    {
        return Err(CliError::Usage(
            "--trust-ca-file requires a verifying --tls-mode".into(),
        ));
    }
    let mut mssql_transport = MsSqlTransportSecurity::Tls;
    let connection = if provider == "postgres" {
        let sslmode = resolve_postgres_sslmode(tls_mode.or(postgres_sslmode))?;
        if sslmode == PostgresSslMode::Disable && !insecure_plaintext && tls_mode.is_none() {
            return Err(CliError::PostgresPlaintextOptInRequired);
        }
        if sslmode != PostgresSslMode::Disable && insecure_plaintext {
            return Err(CliError::Usage(
                "--insecure-plaintext is valid only with --sslmode disable".to_owned(),
            ));
        }
        if trust_ca_file.is_some()
            && !matches!(
                sslmode,
                PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull
            )
        {
            return Err(CliError::Usage(
                "--trust-ca-file requires PostgreSQL --sslmode verify-ca or verify-full".to_owned(),
            ));
        }
        DatabaseConnection::Postgres(PostgresConnection {
            options,
            sslmode,
            trust_ca_file,
        })
    } else {
        match tls_mode {
            Some(PostgresSslMode::Disable) => mssql_transport = MsSqlTransportSecurity::Plaintext,
            Some(PostgresSslMode::Require) => trust_server_certificate = true,
            Some(PostgresSslMode::VerifyCa) => {
                return Err(CliError::Usage(
                    "MSSQL does not support --tls-mode verify-ca".into(),
                ));
            }
            _ => {}
        }
        if trust_server_certificate && trust_ca_file.is_some() {
            return Err(CliError::Usage(
                "--trust-server-certificate and --trust-ca-file are mutually exclusive".to_owned(),
            ));
        }
        DatabaseConnection::MsSql(MsSqlConnection {
            options,
            trust_server_certificate,
            trust_ca_file,
            dialect_level: mssql_dialect,
        })
    };
    Ok(BackingDatabase {
        connection,
        mssql_transport,
    })
}

fn reject_inline_flag_value(option: &str, inline: Option<&str>) -> Result<(), CliError> {
    if inline.is_some() {
        Err(CliError::Usage(format!(
            "option {option:?} does not take a value"
        )))
    } else {
        Ok(())
    }
}

fn option_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
    inline: Option<String>,
) -> Result<String, CliError> {
    let value = inline
        .or_else(|| arguments.next())
        .ok_or_else(|| CliError::Usage(format!("missing value for {option:?}")))?;
    if value.is_empty() || value.starts_with('-') {
        return Err(CliError::Usage(format!(
            "invalid option-like or empty value {value:?} for {option:?}"
        )));
    }
    Ok(value)
}

fn parse_postgres_sslmode(value: &str) -> Result<PostgresSslMode, &'static str> {
    match value {
        "disable" => Ok(PostgresSslMode::Disable),
        "require" => Ok(PostgresSslMode::Require),
        "verify-ca" => Ok(PostgresSslMode::VerifyCa),
        "verify-full" => Ok(PostgresSslMode::VerifyFull),
        _ => Err("expected disable, require, verify-ca, or verify-full"),
    }
}

fn resolve_postgres_sslmode(
    explicit: Option<PostgresSslMode>,
) -> Result<PostgresSslMode, CliError> {
    let environment = if explicit.is_none() {
        match env::var("PGSSLMODE") {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(CliError::Usage("PGSSLMODE is not valid UTF-8".to_owned()));
            }
        }
    } else {
        None
    };
    select_postgres_sslmode(explicit, environment.as_deref())
        .map_err(|reason| CliError::Usage(format!("invalid PGSSLMODE: {reason}")))
}

pub(crate) fn select_postgres_sslmode(
    explicit: Option<PostgresSslMode>,
    environment: Option<&str>,
) -> Result<PostgresSslMode, String> {
    if let Some(mode) = explicit {
        return Ok(mode);
    }
    environment.map_or(Ok(PostgresSslMode::VerifyFull), |value| {
        parse_postgres_sslmode(value).map_err(|reason| format!("{value:?}: {reason}"))
    })
}

#[cfg(test)]
#[path = "tests/args.rs"]
mod tests;
