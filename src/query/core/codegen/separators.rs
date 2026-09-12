//! Data-separator predicates for the physical tables a statement reads.

use super::params::render_scalar_parameter;
use crate::Token;
use crate::metadata::LiveTable;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{CompilationCatalog, SeparatorState};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// Renders `<alias>.<column> = <value>` for every separator column the
/// live table declares, in snapshot order. A disabled separator yields
/// nothing; an `Independent` separator without a value, or a separator
/// without an empty value, is a diagnostic at `token`.
pub(super) fn separator_predicates(
    catalog: &CompilationCatalog<'_>,
    table: &LiveTable,
    alias: &str,
    token: &Token<'_>,
    dialect: SqlDialect,
) -> Result<Vec<String>, QueryDiagnostic> {
    let mut predicates = Vec::new();
    for separator in catalog.separators() {
        let Some(column) = table
            .columns
            .iter()
            .find(|column| names_equal(&column.name, &separator.column))
        else {
            continue;
        };
        match &separator.state {
            SeparatorState::Disabled => {}
            SeparatorState::Value(value) => {
                let literal = render_scalar_parameter(value, token, dialect, true)?;
                predicates.push(format!(
                    "{} = {literal}",
                    dialect.qualified_column(Some(alias), &column.name)
                ));
            }
            SeparatorState::Missing(parameter) => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Parameter,
                    Some(token),
                    format!(
                        "data separator {:?} requires session parameter {parameter:?}",
                        separator.name
                    ),
                ));
            }
            SeparatorState::Unsupported => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(token),
                    format!(
                        "data separator {:?} has no empty value; supply session parameter {:?}",
                        separator.name, separator.name
                    ),
                ));
            }
        }
    }
    Ok(predicates)
}
