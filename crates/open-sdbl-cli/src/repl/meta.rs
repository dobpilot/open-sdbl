//! The meta commands of the console — everything typed with a leading
//! backslash.

use std::io::Write;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::TempTablesManager;

use crate::error::CliError;
use crate::params::ParameterStore;
use crate::session::DatabaseSession;

use super::*;

pub(super) enum MetaOutcome {
    Continue,
    Refreshed,
    Quit,
}

pub(super) async fn execute_meta_command(
    session: &mut DatabaseSession,
    snapshot: &mut MetadataSnapshot,
    temporary: &TempTablesManager,
    command: &str,
    output: &mut impl Write,
) -> Result<MetaOutcome, CliError> {
    match command {
        "\\q" => Ok(MetaOutcome::Quit),
        "\\help" | "\\?" => {
            output
                .write_all(CONSOLE_HELP.as_bytes())
                .map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\dt" => {
            print_tables(output, snapshot).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\di" => {
            print_indexes(output, snapshot).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\tables" => {
            print_temporary_tables(output, temporary).map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Continue)
        }
        "\\refresh" => {
            *snapshot = session.metadata().await?;
            writeln!(output, "Metadata refreshed.").map_err(CliError::standard_output)?;
            Ok(MetaOutcome::Refreshed)
        }
        _ if command == "\\d" => Err(CliError::Data(
            "usage: \\d <qualified-or-unique-metadata-name>".to_owned(),
        )),
        _ if command.starts_with("\\d ") || command.starts_with("\\d\t") => {
            let name = command[2..].trim();
            print_description(output, snapshot, name)?;
            Ok(MetaOutcome::Continue)
        }
        _ => Err(CliError::Data(format!(
            "unknown console command {command:?}; type \\help"
        ))),
    }
}

/// Query and session parameter names offered after `&`, without
/// duplicates.
pub(super) fn parameter_names(
    parameters: &ParameterStore,
    session: &ParameterStore,
) -> Vec<String> {
    let mut names = parameters.names();
    for name in session.names() {
        if !names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            names.push(name);
        }
    }
    names
}
