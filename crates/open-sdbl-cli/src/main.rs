//! Entry point of the `open-sdbl` command-line application.
//!
//! It starts the runtime, hands control to [`app::run`], and turns the
//! result into an exit status. Everything else lives in a module of its
//! own.

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

mod access;
mod access_cache;
mod app;
mod args;
mod auth;
mod cells;
mod db;
mod error;
mod extensions;
mod limits;
mod net;
mod output;
mod params;
mod pipeline;
mod progress;
mod repl;
mod restrict;
mod session;

use auth::pgpass::Credentials;
use error::CliError;
use output::write_top_level_error;

/// The hex decoder the library tests use, shared so that a fixture written
/// as hex reads the same way on both sides.
#[cfg(test)]
#[path = "../../../tests/support/hex.rs"]
mod hex_test_support;

fn main() -> ExitCode {
    let credentials = Credentials::take_from_environment();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("cannot start Tokio runtime: {error}");
            return ExitCode::from(2);
        }
    };
    runtime.block_on(async_main(credentials))
}

async fn async_main(credentials: Credentials) -> ExitCode {
    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let result = app::run(&mut output, &credentials).await.and_then(|()| {
        output
            .flush()
            .map_err(|error| CliError::Io("cannot flush standard output".to_owned(), error))
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.is_broken_pipe() => ExitCode::SUCCESS,
        Err(error) => {
            let stderr = io::stderr();
            let _ = write_top_level_error(&mut stderr.lock(), &error);
            ExitCode::from(error.exit_code())
        }
    }
}
