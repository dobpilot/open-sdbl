//! Data-separator predicates for the physical tables a statement reads.

use super::params::render_scalar_parameter;
use crate::Token;
use crate::metadata::LiveTable;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{CompilationCatalog, SeparatorState};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// Формирует `<alias>.<column> = <value>` для разделителей таблицы в порядке
/// снимка. Выключенный совместный разделитель не создаёт предиката;
/// выключенный независимый запрещает таблицу. Отсутствующие связанные
/// параметры и неизвестное пустое значение диагностируются у `token`.
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
            SeparatorState::Forbidden => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(token),
                    format!(
                        "cannot use table: data separator {:?} with independent usage is disabled",
                        separator.name
                    ),
                ));
            }
            SeparatorState::Value(value) => {
                let literal = render_scalar_parameter(value, token, dialect, true)?;
                predicates.push(format!(
                    "{} = {literal}",
                    dialect.qualified_column(Some(alias), &column.name)
                ));
            }
            SeparatorState::Missing(parameter) => {
                return Err(QueryDiagnostic::missing_parameter(
                    parameter,
                    token,
                    format!(
                        "data separator {:?} requires session parameter {parameter:?}",
                        separator.name
                    ),
                    true,
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
