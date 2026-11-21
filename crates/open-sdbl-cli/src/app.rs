//! The commands the CLI offers: `lex`, `metadata`, `console` and `serve`.
//!
//! Choosing a command and carrying it out is one responsibility; the
//! binary root only starts the runtime and reports the exit status.

use std::env;
use std::io::Write;

use open_sdbl::tokenize;
use open_sdbl_db::{Credentials, DatabaseSession, Limits, MetadataProgress as _};

use crate::args::parse_connection;
use crate::auth::pgpass::resolve_credentials;
use crate::config::expand_config;
use crate::error::CliError;
use crate::help::Scope;
use crate::output::{
    lex, print_resolution_report, print_snapshot, read_lex_source, resolution_warnings,
};
use crate::progress::{MetadataProgress, log, observe};
use crate::repl;

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;

pub(crate) async fn run(
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Scope::default().write(output);
    };
    let command = match command.as_str() {
        "-h" | "--help" => return Scope::default().write(output),
        "lex" => "lex",
        "metadata" => "metadata",
        "console" => "console",
        "repl" => "repl",
        "serve" => "serve",
        unknown => {
            return Err(CliError::Usage(format!("unknown command {unknown:?}"))
                .with_scope(Scope::default()));
        }
    };
    let arguments: Vec<_> = arguments.collect();
    let scope = Scope::new(command, arguments.first().map(String::as_str));
    if arguments
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        return scope.write(output);
    }

    let arguments = arguments.into_iter();
    let result = match command {
        "lex" => run_lex(arguments, output),
        "metadata" => metadata(arguments, output, credentials).await,
        "console" | "repl" => console(arguments, output, credentials).await,
        "serve" => crate::serve::serve(arguments, credentials).await,
        _ => unreachable!("validated command"),
    };
    result.map_err(|error| error.with_scope(scope))
}

pub(crate) fn run_lex(
    mut arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let path = arguments.next().unwrap_or_else(|| "-".to_owned());
    if matches!(path.as_str(), "-h" | "--help") {
        return Scope::new("lex", None).write(output);
    }
    if let Some(unexpected) = arguments.next() {
        return Err(CliError::Usage(format!(
            "unexpected argument {unexpected:?}"
        )));
    }
    let source = read_lex_source(&path)?;
    let tokens = tokenize(&source).map_err(CliError::Lexical)?;
    lex(output, &tokens).map_err(CliError::standard_output)
}

pub(crate) async fn metadata(
    arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let mut arguments = expand_config(arguments.collect(), false)?.into_iter();
    let backing = parse_connection(&mut arguments, "metadata")?;
    if let Some(warning) = backing.transport_warning() {
        log("WARN", warning);
    }
    let connection = backing.connection;

    let credentials = resolve_credentials(&connection, credentials)?;
    let mut progress = MetadataProgress::start(false, true);
    let result = observe(async {
        let mut session = DatabaseSession::connect_with_transport(
            &connection,
            &credentials,
            Limits::default(),
            backing.mssql_transport,
        )
        .await?;
        let result = session.metadata(&mut progress).await;
        let close_result = session.close().await;
        let (snapshot, report) = result?;
        Ok::<_, CliError>((snapshot, report, close_result))
    })
    .await;
    progress.outcome(&result);
    let (snapshot, report, close_result) = result?;
    print_resolution_report(&report);
    print_snapshot(output, &snapshot).map_err(CliError::standard_output)?;
    if let Err(error) = close_result {
        log(
            "WARN",
            format_args!(
                "metadata was loaded, but the database session did not close cleanly: {error}"
            ),
        );
    }
    Ok(())
}

pub(crate) async fn console(
    arguments: impl Iterator<Item = String>,
    output: &mut impl Write,
    credentials: &Credentials,
) -> Result<(), CliError> {
    let mut arguments = expand_config(arguments.collect(), false)?.into_iter();
    let backing = parse_connection(&mut arguments, "console")?;
    let startup = repl::startup::Startup::new();
    if let Some(warning) = backing.transport_warning() {
        startup.warn(warning);
    }
    let connection = backing.connection;
    let credentials = resolve_credentials(&connection, credentials)?;
    let _utf8 = startup.utf8_guard()?;
    let ((mut session, prepared), input) = startup
        .watch(async {
            let mut progress = startup.reporter();
            let mut session = DatabaseSession::connect_with_transport(
                &connection,
                &credentials,
                Limits::default(),
                backing.mssql_transport,
            )
            .await?;
            // PostgreSQL opens its transaction before entering the shared
            // acquisition pipeline. Coalesce its later transaction callback.
            progress.phase("transaction");
            let (snapshot, report) = session.metadata(&mut progress).await?;
            resolution_warnings(&report, |message| startup.warn(&message));
            progress.phase("line editor");
            let prepared = repl::Prepared::new(snapshot, startup.interactive).await?;
            Ok((session, prepared))
        })
        .await?;
    drop(startup);
    let result = repl::run(&mut session, prepared, input, output).await;
    let close_result = session.close().await;
    result?;
    close_result.map_err(CliError::from)
}
