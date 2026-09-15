//! The commands the CLI offers: `lex`, `metadata` and `console`.
//!
//! Choosing a command and carrying it out is one responsibility; the
//! binary root only starts the runtime and reports the exit status.

use std::env;
use std::io::Write;

use open_sdbl::tokenize;

use crate::args::{HELP, parse_connection};
use crate::auth::pgpass::Credentials;
use crate::error::CliError;
use crate::output::{escape_field, lex, print_snapshot, read_lex_source};
use crate::repl;
use crate::session::DatabaseSession;

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;

pub(crate) async fn run(
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" => {
            output
                .write_all(HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            Ok(())
        }
        "lex" => run_lex(arguments, output),
        "metadata" => metadata(arguments, output, credentials).await,
        "console" | "repl" => console(arguments, output, credentials).await,
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

pub(crate) fn run_lex(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let path = arguments.next().unwrap_or_else(|| "-".to_owned());
    if matches!(path.as_str(), "-h" | "--help") {
        output
            .write_all(HELP.as_bytes())
            .map_err(CliError::standard_output)?;
        return Ok(());
    }
    if let Some(unexpected) = arguments.next() {
        return Err(CliError::Usage(format!(
            "unexpected argument {unexpected:?}\n\n{HELP}"
        )));
    }
    let source = read_lex_source(&path)?;
    let tokens = tokenize(&source).map_err(CliError::Lexical)?;
    lex(output, &tokens).map_err(CliError::standard_output)
}

pub(crate) async fn metadata(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "metadata", output)? else {
        return Ok(());
    };

    let mut session = DatabaseSession::connect(&connection, credentials).await?;
    let result = session.metadata().await;
    let close_result = session.close().await;
    let snapshot = result?;
    print_snapshot(output, &snapshot).map_err(CliError::standard_output)?;
    if let Err(error) = close_result {
        eprintln!(
            "warning: metadata was loaded, but the database session did not close cleanly: {}",
            escape_field(&error.to_string())
        );
    }
    Ok(())
}

pub(crate) async fn console(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let Some(connection) = parse_connection(&mut arguments, "console", output)? else {
        return Ok(());
    };
    let mut session = DatabaseSession::connect(&connection, credentials).await?;
    let result = async {
        let snapshot = session.metadata().await?;
        repl::run(&mut session, snapshot, output).await
    }
    .await;
    let close_result = session.close().await;
    result?;
    close_result
}
