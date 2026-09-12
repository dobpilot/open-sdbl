//! The `Константы` source: one derived row holding every constant the
//! statement reads.

use std::cell::RefCell;
use std::collections::BTreeSet;

use super::context::SourceScope;
use super::separators::separator_predicates;
use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::SourceAst;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{CompilationCatalog, SeparatorState};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// Alias of one constant table inside its `UNION ALL` branch.
const BRANCH_ALIAS: &str = "__constant";
/// Alias of the `UNION ALL` the aggregates read.
const UNION_ALIAS: &str = "__constants";

/// The constants a `Константы` scope can read, one entry per field of the
/// scope in the same order.
pub(super) struct ConstantsSource {
    entries: Vec<ConstantEntry>,
}

struct ConstantEntry {
    /// Constant name, for diagnostics.
    name: String,
    /// Quoted-ready physical table such as `_Const61`.
    table: String,
    /// Physical columns of the constant's value with their catalog types,
    /// so the `NULL` placeholders of other branches carry the same type.
    columns: Vec<(String, String)>,
    /// Separator predicates qualified with [`BRANCH_ALIAS`].
    separators: Vec<String>,
    /// Whether a disabled separator applies to the table, which leaves no
    /// single row to aggregate.
    disabled: bool,
}

/// Builds the scope of `ИЗ Константы`: every live constant becomes one
/// field named after it; the relation is rendered once the statement is
/// compiled and the used constants are known.
pub(super) fn constants_source_scope(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    default_alias: &str,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
    let mut fields = Vec::new();
    let mut entries = Vec::new();
    for (object, field) in catalog.constants_fields(Some(source.object))? {
        let table = object
            .physical_table
            .as_deref()
            .and_then(|physical| snapshot.live_table(physical))
            .expect("constants_fields lists live constants only");
        let separators =
            separator_predicates(catalog, table, BRANCH_ALIAS, source.object, dialect)?;
        // Preparation runs unbound with every separator disabled; the check
        // matters only once the session values are known.
        let disabled = catalog.is_bound()
            && catalog.separators().iter().any(|separator| {
                separator.state == SeparatorState::Disabled
                    && table
                        .columns
                        .iter()
                        .any(|column| names_equal(&column.name, &separator.column))
            });
        entries.push(ConstantEntry {
            name: field.name.clone(),
            table: table.name.clone(),
            columns: field
                .columns
                .iter()
                .map(|column| (column.physical_name.clone(), column.data_type.clone()))
                .collect(),
            separators,
            disabled,
        });
        fields.push(field);
    }
    let alias = source
        .alias
        .map_or_else(|| default_alias.to_owned(), |token| token.lexeme.to_owned());
    Ok(SourceScope {
        object: ObjectId::from_bytes([0; 16]),
        fields: fields.into(),
        relation: String::new(),
        sql_alias: alias,
        object_name: source.object.lexeme.to_owned(),
        source_alias: source.alias.map(|token| token.lexeme.to_owned()),
        identity_is_base: false,
        reference_joins: Vec::new(),
        separator_predicates: Vec::new(),
        constants: Some(ConstantsSource { entries }),
        used_fields: RefCell::new(BTreeSet::new()),
    })
}

/// Renders the relation of a constants scope from the fields the statement
/// resolved against it: a `UNION ALL` of one branch per used constant,
/// aggregated with `MAX` into exactly one row.
pub(super) fn finalize_constants_relation(
    scope: &mut SourceScope,
    token: &Token<'_>,
    dialect: SqlDialect,
) -> Result<(), QueryDiagnostic> {
    let Some(constants) = &scope.constants else {
        return Ok(());
    };
    let used = scope.used_fields.borrow();
    let entries = constants
        .entries
        .iter()
        .enumerate()
        .filter(|(index, _)| used.contains(index))
        .map(|(_, entry)| entry)
        .collect::<Vec<_>>();
    if let Some(entry) = entries.iter().find(|entry| entry.disabled) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "constant {:?} is separated; the constants table needs a separator value",
                entry.name
            ),
        ));
    }
    let relation = if entries.is_empty() {
        format!(
            "(SELECT 1 AS {})",
            dialect.quote_identifier("__constants_row")
        )
    } else {
        let columns = entries
            .iter()
            .flat_map(|entry| entry.columns.iter())
            .collect::<Vec<_>>();
        let branches = entries
            .iter()
            .map(|entry| {
                let projection = columns
                    .iter()
                    .map(|(column, data_type)| {
                        // An untyped NULL in the first branches would make
                        // PostgreSQL resolve the UNION column as text.
                        let value = if entry.columns.iter().any(|(own, _)| own == column) {
                            dialect.qualified_column(Some(BRANCH_ALIAS), column)
                        } else {
                            format!("CAST(NULL AS {data_type})")
                        };
                        format!("{value} AS {}", dialect.quote_identifier(column))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut branch = format!(
                    "SELECT {projection} FROM {} AS {}",
                    dialect.quote_identifier(&entry.table),
                    dialect.quote_identifier(BRANCH_ALIAS)
                );
                if !entry.separators.is_empty() {
                    branch.push_str(" WHERE ");
                    branch.push_str(&entry.separators.join(" AND "));
                }
                branch
            })
            .collect::<Vec<_>>();
        let aggregates = columns
            .iter()
            .map(|(column, _)| {
                format!(
                    "MAX({}) AS {}",
                    dialect.qualified_column(Some(UNION_ALIAS), column),
                    dialect.quote_identifier(column)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "(SELECT {aggregates} FROM ({}) AS {})",
            branches.join(" UNION ALL "),
            dialect.quote_identifier(UNION_ALIAS)
        )
    };
    drop(used);
    scope.relation = relation;
    Ok(())
}
