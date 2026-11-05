//! Shared TOML configuration for commands that connect to a database.

use std::fs;

use crate::args::HELP;
use crate::error::CliError;

fn usage(message: impl std::fmt::Display) -> CliError {
    CliError::Usage(format!("{message}\n\n{HELP}"))
}

/// Convert the configuration to the existing command-line representation so
/// both entry points share all connection and security validation.
pub(crate) fn config_arguments(text: &str, gateway: bool) -> Result<Vec<String>, CliError> {
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
        let mut fields = Vec::new();
        let option = format!("--{}", key.replace('_', "-"));
        match key.as_str() {
            "provider" => {}
            "host" | "database" | "user" | "sslmode" | "trust_ca_file" | "socks5_proxy"
            | "socks5_user" | "mssql_dialect" | "listen" | "tls_cert" | "tls_key" => {
                let value = value
                    .as_str()
                    .ok_or_else(|| usage(format!("{key} must be a string")))?;
                fields.extend([option, value.to_owned()]);
            }
            "port" | "catalog_memory_limit_mib" => {
                let value = value
                    .as_integer()
                    .ok_or_else(|| usage(format!("{key} must be an integer")))?;
                fields.extend([option, value.to_string()]);
            }
            "insecure_plaintext"
            | "trust_server_certificate"
            | "allow_insecure_loopback"
            | "allow_insecure_network"
            | "verbose_errors"
            | "debug_authentication"
            | "debug_authentication_secrets" => {
                if value
                    .as_bool()
                    .ok_or_else(|| usage(format!("{key} must be a boolean")))?
                {
                    fields.push(option);
                }
            }
            "session_params" => {
                let values = value
                    .as_array()
                    .ok_or_else(|| usage("session_params must be an array of strings"))?;
                for value in values {
                    fields.extend([
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
        if gateway
            || !matches!(
                key.as_str(),
                "catalog_memory_limit_mib"
                    | "listen"
                    | "tls_cert"
                    | "tls_key"
                    | "session_params"
                    | "allow_insecure_loopback"
                    | "allow_insecure_network"
                    | "verbose_errors"
                    | "debug_authentication"
                    | "debug_authentication_secrets"
            )
        {
            args.extend(fields);
        }
    }
    Ok(args)
}

pub(crate) fn expand_config(
    arguments: Vec<String>,
    gateway: bool,
) -> Result<Vec<String>, CliError> {
    if arguments.first().is_some_and(|arg| arg == "--config") {
        if arguments.len() != 2 {
            return Err(usage(
                "--config requires a path and cannot be combined with other options",
            ));
        }
        let path = &arguments[1];
        let text = fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read configuration {path:?}"), error))?;
        config_arguments(&text, gateway)
    } else {
        Ok(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse_connection;

    #[test]
    fn gateway_config_reuses_connection_settings_without_gateway_validation() {
        let text = r#"
provider = "mssql"
host = "db"
database = "erp"
user = "reader"
trust_server_certificate = true
listen = "0.0.0.0:5433"
allow_insecure_network = true
session_params = ["X=1"]
"#;
        for command in ["console", "metadata"] {
            let args = config_arguments(text, false).unwrap();
            assert!(!args.iter().any(|arg| arg == "--listen"));
            assert!(
                parse_connection(&mut args.into_iter(), command, &mut Vec::new())
                    .unwrap()
                    .is_some()
            );
        }
        assert!(config_arguments("provider = 'mssql'\nunknown = true", false).is_err());
        assert!(
            config_arguments("provider = 'mssql'\nallow_insecure_network = 'yes'", false).is_err()
        );
    }
}
