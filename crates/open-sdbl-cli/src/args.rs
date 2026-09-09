use std::env;
use std::io::Write;

use open_sdbl::query::MsSqlDialectLevel;

use crate::error::CliError;
use crate::net::socks5::{Socks5Proxy, parse_socks5_proxy};

pub(crate) const HELP: &str = "open-sdbl — tooling for the 1C query language\n\n\
Usage:\n  open-sdbl lex [FILE|-]\n  open-sdbl metadata postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl metadata mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console mssql --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl --help\n\n\
Commands:\n  lex       Print lexical tokens; reads standard input when FILE is '-' or omitted\n  metadata  Read and resolve 1C information-base metadata\n  console   Run 1C queries and inspect resolved metadata interactively\n\n\
PostgreSQL options:\n  --port PORT                 PostgreSQL port (default: 5432)\n  --sslmode MODE              disable, require, verify-ca, or verify-full (default)\n  --trust-ca-file PATH        Trust only certificates signed by this private CA\n  --insecure-plaintext        Required explicit opt-in for --sslmode disable\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy\n  --socks5-user USER          Authenticate to SOCKS5 using SOCKS5_PASSWORD\n\n\
MSSQL options:\n  --port PORT                 SQL Server port (default: 1433)\n  --socks5-proxy HOST:PORT    Route through a SOCKS5 proxy\n  --socks5-user USER          Authenticate to SOCKS5 using SOCKS5_PASSWORD\n  --trust-server-certificate  Accept any TLS certificate (unsafe; development only)\n  --trust-ca-file PATH        Trust a specific PEM, CRT, or DER certificate\n  --mssql-dialect LEVEL       2008 or 2012; default is detected from the server version\n\n\
Authentication:\n  PostgreSQL: PGPASSWORD, PGPASSFILE, or $HOME/.pgpass\n  MSSQL: MSSQL_PASSWORD\n  SOCKS5: SOCKS5_PASSWORD (when --socks5-user is present)\n  Password environment variables are consumed and removed; password flags are unsupported\n\n\
Read-only requirements:\n  PostgreSQL queries run in verified READ ONLY, READ COMMITTED transactions\n  MSSQL login must belong to db_datareader, but not db_datawriter, db_owner, or sysadmin\n";

pub(crate) const INSECURE_MSSQL_CERTIFICATE_WARNING: &str = "warning: --trust-server-certificate disables MSSQL certificate and hostname verification; prefer --trust-ca-file";

#[derive(Debug)]
pub(crate) enum DatabaseConnection {
    Postgres(PostgresConnection),
    MsSql(MsSqlConnection),
}

#[derive(Clone, Debug)]
pub(crate) struct ConnectionOptions {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: String,
    pub(crate) user: String,
    pub(crate) socks5_proxy: Option<Socks5Proxy>,
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresConnection {
    pub(crate) options: ConnectionOptions,
    pub(crate) sslmode: PostgresSslMode,
    pub(crate) trust_ca_file: Option<String>,
}

impl std::ops::Deref for PostgresConnection {
    type Target = ConnectionOptions;

    fn deref(&self) -> &Self::Target {
        &self.options
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PostgresSslMode {
    Disable,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Clone, Debug)]
pub(crate) struct MsSqlConnection {
    pub(crate) options: ConnectionOptions,
    pub(crate) trust_server_certificate: bool,
    pub(crate) trust_ca_file: Option<String>,
    /// Explicit dialect level; `None` means detect it from the server.
    pub(crate) dialect_level: Option<MsSqlDialectLevel>,
}

pub(crate) fn parse_connection(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
    output: &mut impl Write,
) -> Result<Option<DatabaseConnection>, CliError> {
    let Some(provider) = arguments.next() else {
        return Err(CliError::Usage(format!(
            "missing {command} provider\n\n{HELP}"
        )));
    };
    if matches!(provider.as_str(), "-h" | "--help") {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(None);
    }
    if !matches!(provider.as_str(), "postgres" | "mssql") {
        return Err(CliError::Usage(format!(
            "unsupported {command} provider {provider:?}\n\n{HELP}"
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
    let mut insecure_plaintext = false;
    let mut socks5_user = None;
    while let Some(argument) = arguments.next() {
        let (option, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(option, value)| {
                (option, Some(value.to_owned()))
            });
        if matches!(option, "-h" | "--help") {
            output
                .write_all(HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            return Ok(None);
        }
        if option == "--trust-server-certificate" {
            reject_inline_flag_value(option, inline_value.as_deref())?;
            if provider != "mssql" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
            eprintln!("{INSECURE_MSSQL_CERTIFICATE_WARNING}");
            trust_server_certificate = true;
            continue;
        }
        if option == "--insecure-plaintext" {
            reject_inline_flag_value(option, inline_value.as_deref())?;
            if provider != "postgres" {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
            insecure_plaintext = true;
            continue;
        }
        let value = option_value(arguments, option, inline_value)?;
        match option {
            "--host" => options.host = value,
            "--database" => options.database = value,
            "--user" => options.user = value,
            "--port" => {
                options.port = value.parse().map_err(|_| {
                    CliError::Usage(format!("invalid {provider} port {value:?}\n\n{HELP}"))
                })?;
            }
            "--socks5-proxy" => {
                options.socks5_proxy = Some(parse_socks5_proxy(&value).map_err(|reason| {
                    CliError::Usage(format!(
                        "invalid SOCKS5 proxy {value:?}: {reason}\n\n{HELP}"
                    ))
                })?);
            }
            "--socks5-user" => socks5_user = Some(value),
            "--sslmode" if provider == "postgres" => {
                postgres_sslmode = Some(parse_postgres_sslmode(&value).map_err(|reason| {
                    CliError::Usage(format!(
                        "invalid PostgreSQL sslmode {value:?}: {reason}\n\n{HELP}"
                    ))
                })?);
            }
            "--trust-ca-file" => trust_ca_file = Some(value),
            "--mssql-dialect" if provider == "mssql" => {
                mssql_dialect = Some(MsSqlDialectLevel::parse(&value).ok_or_else(|| {
                    CliError::Usage(format!(
                        "invalid MSSQL dialect level {value:?}: expected 2008 or 2012\n\n{HELP}"
                    ))
                })?);
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
        }
    }
    if options.host.is_empty() || options.database.is_empty() || options.user.is_empty() {
        return Err(CliError::Usage(format!(
            "--host, --database, and --user are required\n\n{HELP}"
        )));
    }
    if socks5_user.is_some() && options.socks5_proxy.is_none() {
        return Err(CliError::Usage(format!(
            "--socks5-user requires --socks5-proxy\n\n{HELP}"
        )));
    }
    if let Some(proxy) = options.socks5_proxy.as_mut() {
        proxy.username = socks5_user;
    }

    Ok(Some(if provider == "postgres" {
        let sslmode = resolve_postgres_sslmode(postgres_sslmode)?;
        if sslmode == PostgresSslMode::Disable && !insecure_plaintext {
            return Err(CliError::PostgresPlaintextOptInRequired);
        }
        if sslmode != PostgresSslMode::Disable && insecure_plaintext {
            return Err(CliError::Usage(format!(
                "--insecure-plaintext is valid only with --sslmode disable\n\n{HELP}"
            )));
        }
        if trust_ca_file.is_some()
            && !matches!(
                sslmode,
                PostgresSslMode::VerifyCa | PostgresSslMode::VerifyFull
            )
        {
            return Err(CliError::Usage(format!(
                "--trust-ca-file requires PostgreSQL --sslmode verify-ca or verify-full\n\n{HELP}"
            )));
        }
        DatabaseConnection::Postgres(PostgresConnection {
            options,
            sslmode,
            trust_ca_file,
        })
    } else {
        if trust_server_certificate && trust_ca_file.is_some() {
            return Err(CliError::Usage(format!(
                "--trust-server-certificate and --trust-ca-file are mutually exclusive\n\n{HELP}"
            )));
        }
        DatabaseConnection::MsSql(MsSqlConnection {
            options,
            trust_server_certificate,
            trust_ca_file,
            dialect_level: mssql_dialect,
        })
    }))
}

fn reject_inline_flag_value(option: &str, inline: Option<&str>) -> Result<(), CliError> {
    if inline.is_some() {
        Err(CliError::Usage(format!(
            "option {option:?} does not take a value\n\n{HELP}"
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
        .ok_or_else(|| CliError::Usage(format!("missing value for {option:?}\n\n{HELP}")))?;
    if value.is_empty() || value.starts_with('-') {
        return Err(CliError::Usage(format!(
            "invalid option-like or empty value {value:?} for {option:?}\n\n{HELP}"
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
                return Err(CliError::Usage(format!(
                    "PGSSLMODE is not valid UTF-8\n\n{HELP}"
                )));
            }
        }
    } else {
        None
    };
    select_postgres_sslmode(explicit, environment.as_deref())
        .map_err(|reason| CliError::Usage(format!("invalid PGSSLMODE: {reason}\n\n{HELP}")))
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
mod tests {
    use super::{DatabaseConnection, parse_connection};

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
        let parsed = parse_connection(&mut inline, "console", &mut Vec::new()).unwrap();
        assert!(matches!(parsed, Some(DatabaseConnection::Postgres(_))));

        let mut invalid = ["postgres", "--host", "--database=test"]
            .into_iter()
            .map(str::to_owned);
        assert!(
            parse_connection(&mut invalid, "console", &mut Vec::new())
                .unwrap_err()
                .to_string()
                .contains("option-like")
        );
    }
}
