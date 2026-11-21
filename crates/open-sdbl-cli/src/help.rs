use std::io::Write;

use crate::error::CliError;
use crate::terminal::Surface;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Scope {
    command: &'static str,
    provider: Option<&'static str>,
}

impl Scope {
    pub(crate) fn new(command: &'static str, provider: Option<&str>) -> Self {
        Self {
            command,
            provider: match provider {
                Some("postgres") => Some("postgres"),
                Some("mssql") => Some("mssql"),
                _ => None,
            },
        }
    }

    pub(crate) fn usage(self) -> String {
        match self.command {
            "" => "open-sdbl <command> [options]".into(),
            "lex" => "open-sdbl lex [FILE|-]".into(),
            command => format!(
                "open-sdbl {command} {} --host HOST --database DB --user USER [options]",
                self.provider.unwrap_or("<postgres|mssql>")
            ),
        }
    }

    pub(crate) fn invocation(self) -> String {
        match (self.command, self.provider) {
            ("", _) => "open-sdbl".into(),
            (command, None) => format!("open-sdbl {command}"),
            (command, Some(provider)) => format!("open-sdbl {command} {provider}"),
        }
    }

    pub(crate) fn write(self, output: &mut impl Write) -> Result<(), CliError> {
        self.write_page(output).map_err(CliError::standard_output)
    }

    fn write_page(self, output: &mut impl Write) -> std::io::Result<()> {
        let description = match self.command {
            "lex" => "Print lexical tokens",
            "metadata" => "Read and resolve 1C metadata",
            "console" | "repl" => "Run 1C queries interactively",
            "serve" => "Serve SDBL over the PostgreSQL protocol",
            _ => "Tools for the 1C query language",
        };
        let palette = Surface::stdout().palette;
        writeln!(
            output,
            "{}  {description}\n\nUsage\n  {}",
            palette.title("open-sdbl", Some(3)),
            palette.bold(&self.usage()),
        )?;
        if matches!(self.command, "metadata" | "console" | "repl" | "serve") {
            writeln!(output, "  open-sdbl {} --config PATH", self.command)?;
        }
        match self.command {
            "" => write!(
                output,
                "\nCommands\n  lex        Print lexical tokens\n  metadata   Read 1C metadata\n  console    Run 1C queries interactively\n  serve      Serve SDBL over the PostgreSQL protocol\n\nExample\n  open-sdbl console --config base.toml\n\nRun open-sdbl <command> --help for command options.\n"
            ),
            "lex" => write!(
                output,
                "\nInput\n  Omit FILE or use '-' to read standard input.\n\nExample\n  open-sdbl lex query.sdbl\n"
            ),
            command => {
                if self.provider.is_none() {
                    writeln!(
                        output,
                        "  open-sdbl {command} postgres --help\n  open-sdbl {command} mssql --help"
                    )?;
                }
                write!(
                    output,
                    "\nConnection\n  --host HOST, --database DB, --user USER\n  --port PORT   PostgreSQL defaults to 5432, MSSQL to 1433\n  --config PATH   Read options from TOML\n\nBacking TLS and proxy\n  --tls-mode MODE   disable, require, verify-full (default)\n                    PostgreSQL also accepts verify-ca\n  --trust-ca-file PATH   Trust a private certificate in a verifying mode\n  --socks5-proxy HOST:PORT   Route through a SOCKS5 proxy\n  --socks5-user USER   Authenticate using SOCKS5_PASSWORD\n"
                )?;
                if command == "serve" {
                    write!(
                        output,
                        "\nListener\n  --listen ADDR:PORT   Default is 127.0.0.1:5433\n  --tls-cert PATH, --tls-key PATH   PEM chain and private key\n  --allow-insecure-loopback   Allow passwords without TLS on loopback\n  --allow-insecure-network    Allow passwords without TLS from any peer\n\nAdvanced\n  --catalog-memory-limit-mib N   Allocation budget per query; default 64\n  --session-param NAME=LIT   Bind a session parameter (repeatable)\n  --verbose-errors   Send safe database details to clients\n  --disable-rls-check   Bypass role restrictions after authentication\n  --debug-authentication   Log authentication failure reasons\n  --debug-authentication-secrets   Log supplied passwords and stored values\n"
                    )?;
                }
                if self.provider != Some("postgres") {
                    writeln!(
                        output,
                        "\nMSSQL\n  --mssql-dialect LEVEL   Use 2008 or 2012; detected by default\n  MSSQL_PASSWORD supplies the database password."
                    )?;
                }
                if self.provider != Some("mssql") {
                    writeln!(
                        output,
                        "\nPostgreSQL\n  PGPASSWORD, PGPASSFILE, or $HOME/.pgpass supply the database password."
                    )?;
                }
                write!(output, "\nCompatibility\n")?;
                if self.provider != Some("mssql") {
                    writeln!(
                        output,
                        "  PostgreSQL accepts --sslmode MODE and PGSSLMODE.\n  Legacy plaintext requires --sslmode disable and --insecure-plaintext."
                    )?;
                }
                if self.provider != Some("postgres") {
                    writeln!(
                        output,
                        "  MSSQL accepts --trust-server-certificate as an alias for --tls-mode require."
                    )?;
                }
                if matches!(command, "console" | "repl") {
                    writeln!(output, "  repl remains an alias for console.")?;
                }
                writeln!(
                    output,
                    "\nExample\n  open-sdbl {command} --config base.toml"
                )
            }
        }
    }
}
