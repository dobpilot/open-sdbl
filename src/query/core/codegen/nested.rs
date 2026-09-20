//! Tabular sections projected as columns.
//!
//! The platform answers such a column with a table of its own for every row
//! of the main result, which one SQL statement cannot return. Measured on
//! 8.3.27, it runs the main statement, materializes the owner keys, and
//! then reads the section joined to those keys. The compiler does the same
//! with two statements linked by the owner key.

use std::sync::Arc;

use super::context::{CompilationContext, ScopeId};
use super::expression::resolve_named_field;
use crate::Token;
use crate::metadata::{MetadataKind, ObjectId};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, ColumnOrigin, CompiledColumn, NestedResult, QueryableField,
    normalize_table_part_standard_fields,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// A tabular section the statement projects, resolved against metadata but
/// not yet rendered: the main statement must be complete first, because the
/// nested statement filters by the owners that statement names.
pub(super) struct PendingSection {
    /// Label the section takes in the logical result.
    pub(super) label: String,
    /// Its position among the logical result columns.
    pub(super) position: usize,
    /// Scope of the owning source.
    pub(super) owner_scope: ScopeId,
    /// Live table of the section.
    pub(super) table: String,
    /// Physical column of the section holding the owner reference.
    pub(super) owner_column: String,
    /// The object owning the section, and the section name as the
    /// metadata spells it, which the columns of the nested result report
    /// as their origin.
    owner_object: ObjectId,
    section_name: String,
    /// Fields the nested statement selects, in order.
    fields: Vec<QueryableField>,
    /// Every field of the section, including the storage columns a
    /// projection leaves out; a predicate may name any of them.
    all_fields: Arc<[QueryableField]>,
}

/// One column of a tabular section, addressed from a predicate of the
/// owning statement.
pub(super) struct SectionColumn {
    /// Live table of the section.
    pub(super) table: String,
    /// Physical column holding the owner reference.
    pub(super) owner_column: String,
    /// Physical column the predicate names, or the pair of type and
    /// identifier members when the column holds any reference.
    pub(super) column: SectionValue,
}

/// How the compared column of a section is stored.
pub(super) enum SectionValue {
    /// One physical column.
    Single(String),
    /// A reference pair: the type member and the identifier member, which
    /// compare as one `RTRef ‖ RRRef` payload.
    ReferencePair { type_member: String, value: String },
}

/// Resolves `<источник>.<Состав>.<Поле>` for a predicate. Returns `None`
/// when the middle segment does not name a tabular section of the source,
/// which leaves the ordinary dereference path to report the error.
pub(super) fn section_column(
    context: &CompilationContext<'_, '_>,
    scope: ScopeId,
    section: &Token<'_>,
    field: &Token<'_>,
) -> Option<SectionColumn> {
    let resolved = resolve_section(context, scope, section, &[], String::new(), 0).ok()?;
    let (index, _) = resolve_named_field(&resolved.all_fields, field).ok()?;
    let columns = resolved.all_fields[index].columns.as_slice();
    // A reference pair compares as one payload; any other composite column
    // would have to be compared member by member inside the EXISTS, which
    // this path does not reach, so it keeps its diagnostic.
    let value = match columns {
        [column] => SectionValue::Single(column.physical_name.clone()),
        _ => {
            let type_member = columns
                .iter()
                .find(|column| column.is_reference_type_member())?;
            let value_member = columns
                .iter()
                .find(|column| column.is_reference_value_member())?;
            SectionValue::ReferencePair {
                type_member: type_member.physical_name.clone(),
                value: value_member.physical_name.clone(),
            }
        }
    };
    Some(SectionColumn {
        table: resolved.table.clone(),
        owner_column: resolved.owner_column.clone(),
        column: value,
    })
}

/// Resolves `<источник>.<Состав>` against the metadata of the owning
/// source. `requested` names the columns of the parenthesized form; empty
/// means every column, which is what the platform answers for `Состав` and
/// `Состав.*`.
pub(super) fn resolve_section(
    context: &CompilationContext<'_, '_>,
    scope: ScopeId,
    section: &Token<'_>,
    requested: &[&Token<'_>],
    label: String,
    position: usize,
) -> Result<PendingSection, QueryDiagnostic> {
    // The section rows are fetched by a second query the caller runs on
    // its own, so the decision of this compilation does not reach them.
    // A restricted compilation refuses the projection rather than
    // returning rows nothing filtered.
    context
        .catalog
        .refuse_unfiltered_read(Some(section), "a nested tabular-section projection")?;
    let source = context.source(scope);
    let object = context
        .snapshot
        .object_by_id(source.object)
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(section),
                "a tabular section can be projected only from a metadata source",
            )
        })?;
    if !matches!(
        object.kind,
        Some(
            MetadataKind::Catalog
                | MetadataKind::Document
                | MetadataKind::ChartOfCharacteristicTypes
                | MetadataKind::ChartOfAccounts
                | MetadataKind::ChartOfCalculationTypes
                | MetadataKind::BusinessProcess
                | MetadataKind::Task
                | MetadataKind::ExchangePlan
        )
    ) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(section),
            "this object kind has no tabular sections",
        ));
    }
    let descriptors = context
        .snapshot
        .descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.resource_guid == object.guid && names_equal(&descriptor.name, section.lexeme)
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(section),
            format!(
                "tabular section {:?} was not found under {:?}",
                section.lexeme, source.object_name
            ),
        ));
    };
    let mut mappings = context
        .snapshot
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.alias == "VT" && entry.guid == descriptor.object_guid)
        .collect::<Vec<_>>();
    mappings.sort_by_key(|entry| entry.number);
    mappings.dedup_by(|left, right| left.number == right.number);
    let [mapping] = mappings.as_slice() else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(section),
            format!(
                "tabular section {:?} has no exact DBNames VT entry",
                section.lexeme
            ),
        ));
    };
    let parent_physical = object.physical_table.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(section),
            "metadata object has no physical table",
        )
    })?;
    let physical_table = format!("{parent_physical}_VT{}", mapping.number);
    // An extension stores the section in its own suffixed table, so the
    // exact name may be absent while a variant of it is live.
    let mut variants = context
        .snapshot
        .live_table(&physical_table)
        .into_iter()
        .chain(context.snapshot.extension_live_tables(&physical_table))
        .collect::<Vec<_>>();
    variants.sort_by_key(|table| table.name.to_ascii_lowercase());
    let Some(live) = variants.first().copied() else {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::NotLive,
            Some(section),
            format!(
                "tabular-section table {physical_table:?} and its extension variants are not live"
            ),
        ));
    };
    let schema = context
        .snapshot
        .schema_table(&live.name)
        .or_else(|| context.snapshot.schema_table(&physical_table));
    let mut fields = context
        .catalog
        .fields_for_table(&live.name, live, schema, Some(section))?
        .to_vec();
    // The owner reference and the line number answer to their standard
    // names, exactly as they do when the section is read as a source.
    normalize_table_part_standard_fields(&mut fields, parent_physical);
    let fields: Arc<[QueryableField]> = fields.into();
    let owner_column = live
        .columns
        .iter()
        .find(|column| {
            let name = column.name.to_ascii_lowercase();
            name.ends_with("_idrref") && !names_equal(&column.name, "_IDRRef")
        })
        .map(|column| column.name.clone())
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(section),
                format!("tabular-section table {physical_table:?} has no owner column"),
            )
        })?;
    let selected = select_fields(&fields, requested, section)?;
    // The platform answers a section with its reference, its line number
    // and its attributes — measured on 8.3.27, where `Д.Товары` yields
    // exactly those. The storage columns behind them stay out.
    let selected = if requested.is_empty() {
        selected
            .into_iter()
            .filter(|field| !is_storage_field(context, field))
            .collect::<Vec<_>>()
    } else {
        selected
    };
    // The platform lists the reference first and the line number second,
    // then the attributes in metadata order.
    let selected = if requested.is_empty() {
        let rank = |field: &QueryableField| match field.schema_name.as_str() {
            "ID" => 0,
            "LineNo" => 1,
            _ => 2,
        };
        let mut ordered = selected;
        ordered.sort_by_key(rank);
        ordered
    } else {
        selected
    };
    Ok(PendingSection {
        label,
        position,
        owner_scope: scope,
        table: live.name.clone(),
        owner_column,
        owner_object: source.object,
        section_name: descriptor.name.clone(),
        fields: selected,
        all_fields: fields,
    })
}

/// Whether a field is storage rather than data: a data separator or the
/// key the platform keeps beside a section's rows.
fn is_storage_field(context: &CompilationContext<'_, '_>, field: &QueryableField) -> bool {
    if names_equal(&field.schema_name, "KeyField") {
        return true;
    }
    field.columns.iter().any(|column| {
        context
            .catalog
            .separators()
            .iter()
            .any(|separator| names_equal(&separator.column, &column.physical_name))
    })
}

/// The fields the nested statement selects: the requested ones in written
/// order, or every field of the section.
fn select_fields(
    fields: &Arc<[QueryableField]>,
    requested: &[&Token<'_>],
    section: &Token<'_>,
) -> Result<Vec<QueryableField>, QueryDiagnostic> {
    if requested.is_empty() {
        return Ok(fields.to_vec());
    }
    let mut selected = Vec::with_capacity(requested.len());
    for name in requested {
        let (index, _) = resolve_named_field(fields, name).map_err(|error| {
            QueryDiagnostic::at(
                error.kind(),
                Some(section),
                format!(
                    "column {:?} was not found in tabular section {:?}",
                    name.lexeme, section.lexeme
                ),
            )
        })?;
        selected.push(fields[index].clone());
    }
    Ok(selected)
}

impl PendingSection {
    /// Renders the nested statement against the finished main statement.
    /// The rows are filtered by the owners the main statement names and
    /// ordered by owner and line number, so a consumer can walk them
    /// group by group.
    pub(super) fn render(
        &self,
        dialect: SqlDialect,
        owner_key_sql: &str,
        main_sql: &str,
        owner_column: usize,
    ) -> NestedResult {
        let alias = dialect.quote_identifier("__section");
        let mut projections = Vec::with_capacity(self.fields.len() + 1);
        let mut columns = Vec::with_capacity(self.fields.len() + 1);
        for field in &self.fields {
            for column in &field.columns {
                projections.push(format!(
                    "{alias}.{} AS {}",
                    dialect.quote_identifier(&column.physical_name),
                    dialect.quote_identifier(&column.output_label)
                ));
                columns.push(
                    CompiledColumn::new(column.output_label.clone(), column.kind.clone())
                        .with_origin(field.field.map(|identity| ColumnOrigin {
                            object: self.owner_object,
                            table_part: Some(self.section_name.clone()),
                            field: identity,
                            composite_member: field.columns.len() > 1,
                        })),
                );
            }
        }
        let key_label = "__owner";
        projections.push(format!(
            "{alias}.{} AS {}",
            dialect.quote_identifier(&self.owner_column),
            dialect.quote_identifier(key_label)
        ));
        columns.push(CompiledColumn::new(
            key_label.to_owned(),
            ColumnKind::Binary { length: None },
        ));
        let key_column = columns.len() - 1;
        let sql = format!(
            "SELECT {} FROM {} AS {alias} WHERE {alias}.{} IN (SELECT {} FROM ({main_sql}) AS {}) ORDER BY {}",
            projections.join(", "),
            dialect.quote_identifier(&self.table),
            dialect.quote_identifier(&self.owner_column),
            owner_key_sql,
            dialect.quote_identifier("__owners"),
            format_args!(
                "{}, {}",
                dialect.quote_identifier(key_label),
                self.line_number_order(dialect, &alias)
            ),
        );
        NestedResult {
            label: self.label.clone(),
            position: self.position,
            sql,
            columns,
            owner_column,
            key_column,
        }
    }

    /// Orders the rows of one owner the way the section stores them.
    fn line_number_order(&self, dialect: SqlDialect, alias: &str) -> String {
        self.fields
            .iter()
            .flat_map(|field| field.columns.iter())
            .find(|column| {
                column
                    .physical_name
                    .to_ascii_lowercase()
                    .starts_with("_lineno")
            })
            .map_or_else(
                || format!("{alias}.{}", dialect.quote_identifier(&self.owner_column)),
                |column| {
                    format!(
                        "{alias}.{}",
                        dialect.quote_identifier(&column.physical_name)
                    )
                },
            )
    }
}
