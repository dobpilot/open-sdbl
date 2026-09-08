//! Metadata lookup and queryable-field projection.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::Token;
use crate::metadata::{
    FieldId, LiveColumn, LiveTable, MetadataKind, MetadataObject, MetadataSnapshot, ObjectId,
    SchemaColumn, SchemaTable, normalize_standard_field_name, recase_postgres_identifier,
};
use crate::query::core::ast::SourceAst;
use crate::query::core::names::{folded_name, names_equal};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// Structured value type of one physical or compiled output column.
///
/// Kinds are derived from the resolved live catalog and SchemaStorage without
/// database round trips. The enum is `#[non_exhaustive]`: callers must keep a
/// fallback match arm.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColumnKind {
    /// A 1C reference. Without `runtime_typed` the value is the 16-byte
    /// `RRRef`; with `runtime_typed` it is the 20-byte concatenation of the
    /// big-endian 4-byte `RTRef` table number and the 16-byte `RRRef`.
    Reference {
        /// Possible target objects; empty for a universal reference.
        targets: Vec<ObjectId>,
        /// Whether the value carries its `RTRef` discriminator.
        runtime_typed: bool,
    },
    /// Raw bytes such as `RTRef` discriminators or row versions.
    Binary {
        /// Declared length when the catalog fixes it.
        length: Option<u32>,
    },
    /// Character data.
    String {
        /// Declared length when the catalog fixes it.
        length: Option<u32>,
    },
    /// Numeric data, including integers with scale zero.
    Number {
        /// Declared decimal precision when known.
        precision: Option<u8>,
        /// Declared decimal scale when known.
        scale: Option<u8>,
    },
    /// Boolean data (`boolean` on PostgreSQL, `bit` on MSSQL).
    Boolean,
    /// Date and time without time zone in the logical 1C domain.
    DateTime,
    /// A native UUID value.
    Uuid,
    /// The `NULL` literal, compatible with every other kind.
    Null,
    /// A catalog type the compiler does not classify.
    Unknown {
        /// Raw catalog type name.
        data_type: String,
    },
}

impl ColumnKind {
    /// Reports whether two kinds may share one UNION column position.
    ///
    /// Only the variant participates: lengths, precision, scale, and reference
    /// targets are ignored. [`ColumnKind::Null`] and [`ColumnKind::Unknown`]
    /// are compatible with every kind.
    #[must_use]
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.is_wildcard()
            || other.is_wildcard()
            || std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub(crate) const fn is_wildcard(&self) -> bool {
        matches!(self, Self::Null | Self::Unknown { .. })
    }

    /// Classifies one live catalog type name such as `numeric(15,2)`,
    /// `character varying(150)`, `mvarchar(9)`, `timestamp without time zone`,
    /// `binary(16)`, or `datetime2`.
    #[must_use]
    pub fn from_catalog_type(data_type: &str) -> Self {
        let (base, parameters) = split_catalog_type(data_type);
        let first = parameters.first().copied();
        let second = parameters.get(1).copied();
        match base.as_str() {
            "bytea" | "image" | "varbinary" => Self::Binary { length: first },
            "binary" => Self::Binary { length: first },
            "timestamp" | "rowversion" => Self::Binary { length: Some(8) },
            "numeric" | "decimal" => Self::Number {
                precision: first.and_then(|value| u8::try_from(value).ok()),
                scale: second.and_then(|value| u8::try_from(value).ok()),
            },
            "tinyint" => Self::integer(3),
            "smallint" | "int2" => Self::integer(5),
            "integer" | "int" | "int4" => Self::integer(10),
            "bigint" | "int8" => Self::integer(19),
            "real" | "float4" | "double precision" | "float8" | "float" | "money"
            | "smallmoney" => Self::Number {
                precision: None,
                scale: None,
            },
            "boolean" | "bool" | "bit" => Self::Boolean,
            "date" | "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" => {
                Self::DateTime
            }
            _ if base.starts_with("timestamp ") || base == "timestamptz" => Self::DateTime,
            "text" | "character varying" | "varchar" | "character" | "char" | "bpchar"
            | "mchar" | "mvarchar" | "nvarchar" | "nchar" | "ntext" | "name" => {
                Self::String { length: first }
            }
            "uuid" | "uniqueidentifier" => Self::Uuid,
            _ => Self::Unknown {
                data_type: data_type.to_owned(),
            },
        }
    }

    const fn integer(precision: u8) -> Self {
        Self::Number {
            precision: Some(precision),
            scale: Some(0),
        }
    }
}

/// Splits `base(p,s)` into a lower-case base name and numeric parameters;
/// `max` and other non-numeric parameters are dropped.
fn split_catalog_type(data_type: &str) -> (String, Vec<u32>) {
    let trimmed = data_type.trim();
    let Some((base, rest)) = trimmed.split_once('(') else {
        return (trimmed.to_ascii_lowercase(), Vec::new());
    };
    let parameters = rest
        .trim_end_matches(')')
        .split(',')
        .filter_map(|parameter| parameter.trim().parse::<u32>().ok())
        .collect();
    (base.trim().to_ascii_lowercase(), parameters)
}

/// One physical SQL member of a queryable logical field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryableColumn {
    /// Exact catalog identifier used for SQL generation.
    pub physical_name: String,
    /// Database catalog type name.
    pub data_type: String,
    /// Stable output label used when projecting this member.
    pub output_label: String,
    /// Structured value type of this member.
    pub kind: ColumnKind,
}

impl QueryableColumn {
    /// Whether this member is the `RTRef` discriminator of a reference field.
    #[must_use]
    pub fn is_reference_type_member(&self) -> bool {
        self.physical_name.to_ascii_lowercase().ends_with("rtref")
    }

    /// Whether this member is the `RRRef`/`IDRRef` value of a reference field.
    #[must_use]
    pub fn is_reference_value_member(&self) -> bool {
        let lower = self.physical_name.to_ascii_lowercase();
        lower.ends_with("rref") && !lower.ends_with("rtref")
    }
}

/// One logical 1C field and all physical members implementing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryableField {
    /// Human-facing field name, preferring Config metadata names.
    pub name: String,
    /// Canonical schema field name such as `Code`, `ID`, or `Fld54`.
    pub schema_name: String,
    /// Names accepted by the bounded compiler.
    pub aliases: Vec<String>,
    /// Exact live physical members.
    pub columns: Vec<QueryableColumn>,
    /// Unique canonical SchemaStorage target table for an `R` field.
    pub reference_target: Option<String>,
    /// Every canonical SchemaStorage target table for a pure reference field.
    pub reference_targets: Vec<String>,
}

/// Queryable logical fields indexed by stable metadata object identity.
pub type QueryableFieldCatalog = HashMap<ObjectId, Vec<QueryableField>>;

pub(super) fn logical_column_name(physical_name: &str) -> String {
    crate::metadata::collapse_logical_fields([physical_name])
        .into_iter()
        .next()
        .map_or_else(
            || {
                physical_name
                    .strip_prefix('_')
                    .unwrap_or(physical_name)
                    .to_owned()
            },
            |field| field.name,
        )
}

pub(super) fn reference_targets(column: &SchemaColumn) -> Vec<String> {
    column
        .types
        .iter()
        .filter_map(|column_type| column_type.reference_target.as_deref())
        .fold(Vec::<String>::new(), |mut targets, target| {
            if !targets
                .iter()
                .any(|candidate| names_equal(candidate, target))
            {
                targets.push(target.to_owned());
            }
            targets
        })
}

pub(super) fn push_unique_name(names: &mut Vec<String>, name: String) {
    if !names.iter().any(|candidate| names_equal(candidate, &name)) {
        names.push(name);
    }
}

pub(super) fn compound_label(display_name: &str, schema_name: &str, physical_name: &str) -> String {
    let canonical = recase_postgres_identifier(physical_name);
    let base = format!("_{schema_name}");
    canonical.strip_prefix(&base).map_or_else(
        || {
            format!(
                "{display_name}_{}",
                canonical.strip_prefix('_').unwrap_or(&canonical)
            )
        },
        |suffix| format!("{display_name}{suffix}"),
    )
}

pub(super) fn standard_field_aliases(schema_name: &str) -> &'static [&'static str] {
    match schema_name {
        "ID" => &["ID", "Ссылка"],
        "Code" => &["Code", "Код"],
        "Description" => &["Description", "Наименование"],
        "Marked" => &["Marked", "ПометкаУдаления"],
        "Version" => &["Version", "ВерсияДанных"],
        "Number" => &["Number", "Номер"],
        "Date" => &["Date", "Дата"],
        "Posted" => &["Posted", "Проведен"],
        "Recorder" => &["Recorder", "Регистратор"],
        "LineNo" => &["LineNo", "НомерСтроки"],
        "Period" => &["Period", "Период"],
        "Active" => &["Active", "Активность"],
        _ => &[],
    }
}

pub(super) fn kind_from_query_name(name: &str) -> Option<MetadataKind> {
    let names = [
        (
            MetadataKind::Catalog,
            ["Catalog", "Reference", "Справочник"],
        ),
        (MetadataKind::Document, ["Document", "Document", "Документ"]),
        (
            MetadataKind::Enumeration,
            ["Enumeration", "Enum", "Перечисление"],
        ),
        (
            MetadataKind::InformationRegister,
            ["InformationRegister", "InfoRg", "РегистрСведений"],
        ),
        (
            MetadataKind::AccumulationRegister,
            ["AccumulationRegister", "AccumRg", "РегистрНакопления"],
        ),
        (
            MetadataKind::AccountingRegister,
            ["AccountingRegister", "AccRg", "РегистрБухгалтерии"],
        ),
        (
            MetadataKind::CalculationRegister,
            ["CalculationRegister", "CRg", "РегистрРасчета"],
        ),
        (
            MetadataKind::ChartOfCharacteristicTypes,
            [
                "ChartOfCharacteristicTypes",
                "Chrc",
                "ПланВидовХарактеристик",
            ],
        ),
        (
            MetadataKind::ChartOfCalculationTypes,
            ["ChartOfCalculationTypes", "CKinds", "ПланВидовРасчета"],
        ),
        (
            MetadataKind::ChartOfAccounts,
            ["ChartOfAccounts", "Acc", "ПланСчетов"],
        ),
        (MetadataKind::Constant, ["Constant", "Const", "Константа"]),
        (
            MetadataKind::ExchangePlan,
            ["ExchangePlan", "Node", "ПланОбмена"],
        ),
        (
            MetadataKind::BusinessProcess,
            ["BusinessProcess", "BPr", "БизнесПроцесс"],
        ),
        (MetadataKind::Task, ["Task", "Task", "Задача"]),
        (
            MetadataKind::Sequence,
            ["Sequence", "Seq", "Последовательность"],
        ),
        (
            MetadataKind::ChangeRegistration,
            ["ChangeRegistration", "ChngR", "РегистрацияИзменений"],
        ),
        (
            MetadataKind::Recalculation,
            ["Recalculation", "CRgRecalc", "Перерасчет"],
        ),
        (
            MetadataKind::CalculationKindDependency,
            [
                "CalculationKindDependency",
                "CKDependency",
                "ЗависимостьВидовРасчета",
            ],
        ),
        (
            MetadataKind::ExtraDimension,
            ["ExtraDimension", "ExtDim", "ВидСубконто"],
        ),
        (
            MetadataKind::ResolveOnlyService,
            ["ResolveOnlyService", "Service", "СлужебнаяТаблица"],
        ),
    ];
    names.into_iter().find_map(|(kind, aliases)| {
        aliases
            .iter()
            .any(|alias| names_equal(alias, name))
            .then_some(kind)
    })
}

pub(super) fn custom_field_name(
    snapshot: &MetadataSnapshot,
    physical_table: &str,
    schema_name: &str,
) -> Option<String> {
    let number = schema_name.strip_prefix("Fld")?.parse::<u32>().ok()?;
    snapshot
        .fields()
        .iter()
        .find(|field| {
            field.number == number
                && field
                    .owner_tables
                    .iter()
                    .any(|owner| names_equal(owner, physical_table))
        })
        .or_else(|| {
            // Extension attributes are addressed by their globally unique
            // field number rather than a base-declared owner table.
            snapshot
                .fields()
                .iter()
                .find(|field| field.number == number && field.extension_origin.is_some())
        })?
        .name
        .clone()
}

pub(super) fn indexed_custom_field_name(
    names: &CustomFieldNameIndex,
    physical_table: &str,
    schema_name: &str,
) -> Option<String> {
    let number = schema_name.strip_prefix("Fld")?.parse::<u32>().ok()?;
    names
        .get(&(folded_name(physical_table), number))
        .or_else(|| names.get(&(String::new(), number)))
        .cloned()
        .flatten()
}

pub(super) fn is_extension_table_name(canonical: &str, candidate: &str) -> bool {
    let Some(prefix) = candidate.get(..canonical.len()) else {
        return false;
    };
    if !names_equal(prefix, canonical) {
        return false;
    }
    let Some(suffix) = candidate.get(canonical.len()..) else {
        return false;
    };
    let Some(number) = suffix
        .strip_prefix('X')
        .or_else(|| suffix.strip_prefix('x'))
    else {
        return false;
    };
    !number.is_empty() && number.chars().all(|digit| digit.is_ascii_digit())
}

#[cfg(test)]
mod extension_table_name_tests {
    use super::is_extension_table_name;

    #[test]
    fn requires_a_non_empty_numeric_extension_suffix() {
        assert!(is_extension_table_name("_Reference53", "_Reference53X1"));
        assert!(is_extension_table_name("_Reference53", "_reference53x12"));
        assert!(!is_extension_table_name("_Reference53", "_Reference53X"));
        assert!(!is_extension_table_name("_Reference53", "_Reference53Xone"));
    }
}

/// Per-compilation, demand-populated field catalog.
///
/// Physical table names are used as keys because malformed metadata can
/// contain duplicate object GUIDs. This preserves the exact table selected by
/// the resolver without scanning every object to detect such duplicates.
pub(super) struct CompilationCatalog<'snapshot> {
    snapshot: &'snapshot MetadataSnapshot,
    custom_names: OnceCell<CustomFieldNameIndex>,
    fields: RefCell<HashMap<String, Arc<[QueryableField]>>>,
    work: Cell<usize>,
}

impl<'snapshot> CompilationCatalog<'snapshot> {
    // Allows wide generated 1C documents to participate in substantial UNION
    // queries while still bounding adversarial cross-branch work.
    const WORK_LIMIT: usize = 32_768;

    pub(super) fn new(snapshot: &'snapshot MetadataSnapshot) -> Self {
        Self {
            snapshot,
            custom_names: OnceCell::new(),
            fields: RefCell::new(HashMap::new()),
            work: Cell::new(0),
        }
    }

    pub(super) fn charge(
        &self,
        units: usize,
        token: Option<&Token<'_>>,
    ) -> Result<(), QueryDiagnostic> {
        let next = self.work.get().checked_add(units).ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::WorkBudgetExceeded,
                token,
                "query compilation work budget exceeded",
            )
        })?;
        if next > Self::WORK_LIMIT {
            return Err(QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::WorkBudgetExceeded,
                token,
                format!(
                    "query compilation work budget exceeds limit of {} units",
                    Self::WORK_LIMIT
                ),
            ));
        }
        self.work.set(next);
        Ok(())
    }

    pub(super) fn fields(
        &self,
        object: &MetadataObject,
        token: Option<&Token<'_>>,
    ) -> Result<Arc<[QueryableField]>, QueryDiagnostic> {
        let physical_table = object.physical_table.as_deref().ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::Metadata,
                token,
                "metadata object has no physical table",
            )
        })?;
        let key = folded_name(physical_table);
        if let Some(fields) = self.fields.borrow().get(&key) {
            return Ok(Arc::clone(fields));
        }
        let table = self.snapshot.live_table(physical_table).ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::NotLive,
                token,
                format!("physical table {physical_table:?} is not live"),
            )
        })?;
        // Lookups are indexed, so the work is the merged projection itself:
        // one unit per live variant plus one per merged column.
        let extension_live_count = self.snapshot.extension_live_tables(physical_table).count();
        self.charge(extension_live_count.saturating_add(1), token)?;
        let (merged_live, merged_schema) = merged_extension_projection(
            self.snapshot,
            physical_table,
            table,
            self.snapshot.schema_table(physical_table),
        );
        self.charge(merged_live.columns.len().max(1), token)?;
        let custom_names = self
            .custom_names
            .get_or_init(|| index_custom_field_names(self.snapshot));
        let fields: Arc<[QueryableField]> = project_queryable_fields(
            self.snapshot,
            physical_table,
            &merged_live,
            merged_schema.as_ref(),
            &CustomFieldNames::Indexed(custom_names),
        )
        .into();
        self.fields.borrow_mut().insert(key, Arc::clone(&fields));
        Ok(fields)
    }

    pub(super) fn fields_for_table(
        &self,
        physical_table: &str,
        table: &LiveTable,
        schema_table: Option<&crate::metadata::SchemaTable>,
        token: Option<&Token<'_>>,
    ) -> Result<Arc<[QueryableField]>, QueryDiagnostic> {
        let key = folded_name(physical_table);
        if let Some(fields) = self.fields.borrow().get(&key) {
            return Ok(Arc::clone(fields));
        }
        self.charge(table.columns.len().max(1), token)?;
        let custom_names = self
            .custom_names
            .get_or_init(|| index_custom_field_names(self.snapshot));
        let fields: Arc<[QueryableField]> = project_queryable_fields(
            self.snapshot,
            physical_table,
            table,
            schema_table,
            &CustomFieldNames::Indexed(custom_names),
        )
        .into();
        self.fields.borrow_mut().insert(key, Arc::clone(&fields));
        Ok(fields)
    }
}

/// One safe node in an application-defined reference-presentation template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationExpression {
    /// A field owned by the target metadata object.
    Field(FieldId),
    /// Literal text. The core performs database-specific quoting.
    Literal(String),
    /// Concatenation evaluated by the selected database.
    Concat(Vec<Self>),
}

/// Application policy for presenting one reference target type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationPlan {
    /// Target object whose rows the plan can present.
    pub object: ObjectId,
    /// Fields explicitly authorized for this plan.
    pub fields: Vec<FieldId>,
    /// Safe structured template; raw SQL is intentionally impossible.
    pub expression: PresentationExpression,
}

/// One target requested by the core before SQL generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PresentationTarget {
    /// Real metadata GUID of the possible reference target.
    pub object: ObjectId,
}

/// Deduplicated batch callback request for application presentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRequest {
    /// Possible reference targets in stable GUID order.
    pub targets: Vec<PresentationTarget>,
}

/// One output column of a compiled query.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledColumn {
    /// Emitted result label.
    pub label: String,
    /// Structured value type of the column.
    pub kind: ColumnKind,
}

impl CompiledColumn {
    pub(crate) const fn new(label: String, kind: ColumnKind) -> Self {
        Self { label, kind }
    }
}

/// Native SQL text generated from one bounded 1C query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledQuery {
    /// SELECT-only statement in the requested database dialect.
    pub sql: String,
    /// Output columns in statement order, each with its emitted label and
    /// structured kind.
    pub columns: Vec<CompiledColumn>,
    /// Zero-based output columns whose cells contain deferred reference
    /// presentation payloads for application-side batch resolution.
    pub deferred_presentations: Vec<usize>,
}

/// Finds a tabular metadata object by qualified name, unique bare name, or
/// canonical physical table name.
///
/// # Errors
///
/// Returns a diagnostic when the name is missing, ambiguous, non-tabular, or
/// has an unknown kind qualifier.
pub fn find_metadata_object<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    name: &str,
) -> Result<&'snapshot MetadataObject, QueryDiagnostic> {
    find_metadata_object_at(snapshot, name, None)
}

pub(super) fn find_metadata_object_at<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    name: &str,
    token: Option<&Token<'_>>,
) -> Result<&'snapshot MetadataObject, QueryDiagnostic> {
    let name = name.trim();
    if name.is_empty() {
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnknownObject,
            token,
            "metadata name is empty",
        ));
    }

    let (kind, object_name) = if let Some((kind, object_name)) = name.split_once('.') {
        let kind = kind_from_query_name(kind).ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::UnknownObject,
                token,
                format!("unknown metadata kind {kind:?}"),
            )
        })?;
        (Some(kind), object_name)
    } else {
        (None, name)
    };

    let matches: Vec<&MetadataObject> = snapshot
        .objects()
        .iter()
        .filter(|object| object.kind.is_some() && object.physical_table.is_some())
        .filter(|object| kind.is_none_or(|kind| object.kind == Some(kind)))
        .filter(|object| {
            object
                .name
                .as_deref()
                .is_some_and(|candidate| names_equal(candidate, object_name))
                || (kind.is_none()
                    && object
                        .physical_table
                        .as_deref()
                        .is_some_and(|candidate| names_equal(candidate, object_name)))
        })
        .collect();

    match matches.as_slice() {
        [object] => Ok(*object),
        [] => Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::UnknownObject,
            token,
            format!("metadata object {name:?} was not found"),
        )),
        _ => Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::AmbiguousObject,
            token,
            format!("metadata object name {name:?} is ambiguous; use <kind>.<name>"),
        )),
    }
}

/// Builds queryable logical fields for one resolved live object.
///
/// # Errors
///
/// Returns a diagnostic when the object has no live physical table.
pub fn queryable_fields(
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
) -> Result<Vec<QueryableField>, QueryDiagnostic> {
    let physical_table = object.physical_table.as_deref().ok_or_else(|| {
        QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            "metadata object has no physical table",
        )
    })?;
    let table = snapshot.live_table(physical_table).ok_or_else(|| {
        QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            format!("physical table {physical_table} is not live"),
        )
    })?;
    let schema_table = snapshot.schema_table(physical_table);
    let (merged_live, merged_schema) =
        merged_extension_projection(snapshot, physical_table, table, schema_table);

    Ok(project_queryable_fields(
        snapshot,
        physical_table,
        &merged_live,
        merged_schema.as_ref(),
        &CustomFieldNames::Scan(snapshot),
    ))
}

fn merged_extension_projection(
    snapshot: &MetadataSnapshot,
    physical_table: &str,
    canonical_live: &LiveTable,
    canonical_schema: Option<&SchemaTable>,
) -> (LiveTable, Option<SchemaTable>) {
    let mut live = canonical_live.clone();
    for variant in snapshot.extension_live_tables(physical_table) {
        let variant_schema = snapshot.schema_table(&variant.name);
        for column in &variant.columns {
            let logical = logical_column_name(&column.name);
            let declared_in_variant = variant_schema.is_some_and(|variant_schema| {
                variant_schema.columns.iter().any(|declared| {
                    names_equal(&logical_column_name(&declared.physical_name()), &logical)
                })
            });
            let is_extension_field = snapshot.is_extension_field_name(&logical);
            if (declared_in_variant || is_extension_field)
                && !live
                    .columns
                    .iter()
                    .any(|existing| names_equal(&existing.name, &column.name))
            {
                live.columns.push(column.clone());
            }
        }
    }
    let mut schema = canonical_schema.cloned();
    let schema_variants = snapshot
        .extension_live_tables(physical_table)
        .filter_map(|variant| snapshot.schema_table(&variant.name));
    for variant in schema_variants {
        let merged = schema.get_or_insert_with(|| SchemaTable {
            name: physical_table.trim_start_matches('_').to_owned(),
            number: variant.number,
            owner: None,
            inline_name: None,
            columns: Vec::new(),
            indexes: Vec::new(),
        });
        for column in &variant.columns {
            if !merged
                .columns
                .iter()
                .any(|existing| names_equal(&existing.name, &column.name))
            {
                merged.columns.push(column.clone());
            }
        }
    }
    // Reference targets of extension attributes live only in the extension's
    // carrier declaration, keyed by field number rather than by this table.
    // Surface them so a reference-typed extension attribute dereferences.
    for column in &live.columns {
        let logical = logical_column_name(&column.name);
        let Some(number) = logical
            .strip_prefix("Fld")
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(target) = snapshot
            .extension_reference_target(number)
            .map(str::to_owned)
        else {
            continue;
        };
        let merged = schema.get_or_insert_with(|| SchemaTable {
            name: physical_table.trim_start_matches('_').to_owned(),
            number: 0,
            owner: None,
            inline_name: None,
            columns: Vec::new(),
            indexes: Vec::new(),
        });
        if !merged
            .columns
            .iter()
            .any(|existing| names_equal(&logical_column_name(&existing.name), &logical))
        {
            merged.columns.push(SchemaColumn {
                name: logical.clone(),
                types: vec![crate::metadata::ColumnType {
                    tag: "R".to_owned(),
                    reference_target: Some(target),
                }],
            });
        }
    }
    (live, schema)
}

pub(super) struct ResolvedSourceMetadata<'snapshot> {
    pub(super) object: &'snapshot MetadataObject,
    pub(super) live_table: &'snapshot LiveTable,
    pub(super) fields: Arc<[QueryableField]>,
    pub(super) qualifier_name: String,
    pub(super) identity_is_base: bool,
}

pub(super) fn resolve_source_metadata<'snapshot>(
    source: &SourceAst<'_, '_>,
    snapshot: &'snapshot MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
) -> Result<ResolvedSourceMetadata<'snapshot>, QueryDiagnostic> {
    let qualified_name = format!("{}.{}", source.kind.lexeme, source.object.lexeme);
    let object = find_metadata_object_at(snapshot, &qualified_name, Some(source.object))?;
    let Some(table_part) = source.table_part else {
        if object.kind.is_some_and(MetadataKind::is_service) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(source.object),
                format!(
                    "service source {:?} is available for metadata discovery but has no direct FROM spelling",
                    object
                        .physical_table
                        .as_deref()
                        .unwrap_or(source.object.lexeme)
                ),
            ));
        }
        let live_table = object
            .physical_table
            .as_deref()
            .and_then(|physical| snapshot.live_table(physical))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::NotLive,
                    Some(source.object),
                    "metadata table is not live",
                )
            })?;
        return Ok(ResolvedSourceMetadata {
            object,
            live_table,
            fields: catalog.fields(object, Some(source.object))?,
            qualifier_name: source.object.lexeme.to_owned(),
            identity_is_base: true,
        });
    };

    if let Some(service) = service_table_part(table_part.lexeme) {
        return resolve_service_table_part(service, table_part, object, snapshot, catalog);
    }

    if !matches!(
        object.kind,
        Some(MetadataKind::Catalog | MetadataKind::Document)
    ) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(table_part),
            "tabular-section sources are supported only for catalogs and documents",
        ));
    }
    let descriptors = snapshot
        .descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.resource_guid == object.guid
                && names_equal(&descriptor.name, table_part.lexeme)
        })
        .collect::<Vec<_>>();
    if descriptors.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(table_part),
            format!(
                "tabular section {:?} was not found under metadata object {:?}",
                table_part.lexeme, source.object.lexeme
            ),
        ));
    }
    if descriptors.len() > 1 {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::AmbiguousObject,
            Some(table_part),
            format!(
                "tabular section {:?} is ambiguous under metadata object {:?}",
                table_part.lexeme, source.object.lexeme
            ),
        ));
    }
    let mut mappings = snapshot
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.alias == "VT" && entry.guid == descriptors[0].object_guid)
        .collect::<Vec<_>>();
    mappings.sort_by_key(|entry| entry.number);
    mappings.dedup_by(|left, right| left.guid == right.guid && left.number == right.number);
    let mapping = match mappings.as_slice() {
        [mapping] => *mapping,
        [] => {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(table_part),
                format!(
                    "tabular section {:?} has no exact DBNames VT entry",
                    table_part.lexeme
                ),
            ));
        }
        _ => {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::AmbiguousObject,
                Some(table_part),
                format!(
                    "tabular section {:?} has ambiguous DBNames VT entries",
                    table_part.lexeme
                ),
            ));
        }
    };
    let parent_physical = object.physical_table.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(source.object),
            "metadata object has no physical table",
        )
    })?;
    let physical_table = format!("{parent_physical}_VT{}", mapping.number);
    let mut live_variants = snapshot
        .live_table(&physical_table)
        .into_iter()
        .chain(snapshot.extension_live_tables(&physical_table))
        .collect::<Vec<_>>();
    live_variants.sort_by_key(|table| table.name.to_ascii_lowercase());
    if live_variants.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::NotLive,
            Some(table_part),
            format!(
                "tabular-section table {physical_table:?} and its exact extension variants are not live"
            ),
        ));
    }
    let mut schema_variants = snapshot
        .schema()
        .tables
        .iter()
        .filter(|table| {
            let candidate = table.physical_name();
            names_equal(&candidate, &physical_table)
                || is_extension_table_name(&physical_table, &candidate)
        })
        .collect::<Vec<_>>();
    schema_variants.sort_by_key(|table| table.name.to_ascii_lowercase());
    if schema_variants.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(table_part),
            format!(
                "tabular-section table {physical_table:?} and its exact extension variants are absent from SchemaStorage"
            ),
        ));
    }
    let mut matching_variants = live_variants
        .iter()
        .filter_map(|live| {
            schema_variants
                .iter()
                .find(|schema| names_equal(&schema.physical_name(), &live.name))
                .map(|schema| (*live, *schema))
        })
        .collect::<Vec<_>>();
    matching_variants.sort_by_key(|(live, _)| live.name.to_ascii_lowercase());
    let (live_table, schema_table) = matching_variants
        .iter()
        .copied()
        .find(|(live, _)| names_equal(&live.name, &physical_table))
        .or_else(|| matching_variants.first().copied())
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(table_part),
                format!(
                    "tabular-section table {physical_table:?} has no same-named variant in SchemaStorage and the live catalog"
                ),
            )
        })?;
    let mut fields = catalog
        .fields_for_table(
            &live_table.name,
            live_table,
            Some(schema_table),
            Some(table_part),
        )?
        .to_vec();
    normalize_table_part_standard_fields(&mut fields, parent_physical);
    Ok(ResolvedSourceMetadata {
        object,
        live_table,
        fields: fields.into(),
        qualifier_name: table_part.lexeme.to_owned(),
        identity_is_base: false,
    })
}

#[derive(Clone, Copy)]
enum ServiceTablePart {
    Changes,
    BaseCalculationKinds,
    LeadingCalculationKinds,
    DisplacedCalculationKinds,
    ExtraDimensions,
}

fn service_table_part(name: &str) -> Option<ServiceTablePart> {
    let choices: &[(ServiceTablePart, &[&str])] = &[
        (ServiceTablePart::Changes, &["Changes", "Изменения"]),
        (
            ServiceTablePart::BaseCalculationKinds,
            &[
                "BaseCalculationKinds",
                "BaseCalculationTypes",
                "БазовыеВидыРасчета",
            ],
        ),
        (
            ServiceTablePart::LeadingCalculationKinds,
            &[
                "LeadingCalculationKinds",
                "LeadingCalculationTypes",
                "ВедущиеВидыРасчета",
            ],
        ),
        (
            ServiceTablePart::DisplacedCalculationKinds,
            &[
                "DisplacedCalculationKinds",
                "DisplacingCalculationTypes",
                "ВытесняющиеВидыРасчета",
            ],
        ),
        (
            ServiceTablePart::ExtraDimensions,
            &["ExtraDimensions", "ВидыСубконто"],
        ),
    ];
    choices.iter().find_map(|(kind, aliases)| {
        aliases
            .iter()
            .any(|alias| names_equal(alias, name))
            .then_some(*kind)
    })
}

fn resolve_service_table_part<'snapshot>(
    service: ServiceTablePart,
    token: &Token<'_>,
    object: &'snapshot MetadataObject,
    snapshot: &'snapshot MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
) -> Result<ResolvedSourceMetadata<'snapshot>, QueryDiagnostic> {
    let parent = object.physical_table.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            "service-table owner has no physical table",
        )
    })?;
    let physical = match service {
        ServiceTablePart::Changes => {
            let owner_id = ObjectId::from(&object.guid);
            let mut candidates = snapshot.objects().iter().filter(|candidate| {
                candidate.kind == Some(MetadataKind::ChangeRegistration)
                    && candidate.owner == Some(owner_id)
            });
            let first = candidates.next().ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::UnknownObject,
                    Some(token),
                    "no change-registration table is linked to this source",
                )
            })?;
            if candidates.next().is_some() {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::AmbiguousObject,
                    Some(token),
                    "more than one change-registration table matches this source",
                ));
            }
            first.physical_table.clone().ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(token),
                    "change-registration metadata has no physical table",
                )
            })?
        }
        ServiceTablePart::BaseCalculationKinds => {
            inline_service_physical(snapshot, parent, |name| names_equal(name, "BaseCK"), token)?
        }
        ServiceTablePart::LeadingCalculationKinds => inline_service_physical(
            snapshot,
            parent,
            |name| names_equal(name, "LeadingCK"),
            token,
        )?,
        ServiceTablePart::DisplacedCalculationKinds => inline_service_physical(
            snapshot,
            parent,
            |name| names_equal(name, "DisplacedCK"),
            token,
        )?,
        ServiceTablePart::ExtraDimensions => {
            inline_service_physical(snapshot, parent, |name| name.starts_with("ExtDim"), token)?
        }
    };
    let live_table = snapshot.live_table(&physical).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::NotLive,
            Some(token),
            format!("service table {physical:?} is not live"),
        )
    })?;
    let schema_table = snapshot.schema().table(&physical).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!("service table {physical:?} is absent from SchemaStorage"),
        )
    })?;
    let mut fields = catalog
        .fields_for_table(&physical, live_table, Some(schema_table), Some(token))?
        .to_vec();
    normalize_table_part_standard_fields(&mut fields, parent);
    Ok(ResolvedSourceMetadata {
        object,
        live_table,
        fields: fields.into(),
        qualifier_name: token.lexeme.to_owned(),
        identity_is_base: false,
    })
}

fn inline_service_physical(
    snapshot: &MetadataSnapshot,
    parent: &str,
    accepts: impl Fn(&str) -> bool,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    let owner = parent.strip_prefix('_').unwrap_or(parent);
    let mut matches = snapshot.schema().tables.iter().filter(|table| {
        table
            .owner
            .as_deref()
            .is_some_and(|candidate| names_equal(candidate, owner))
            && table.inline_name.as_deref().is_some_and(&accepts)
    });
    let first = matches.next().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(token),
            "the metadata object has no matching service table",
        )
    })?;
    if matches.next().is_some() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::AmbiguousObject,
            Some(token),
            "the metadata object has multiple matching service tables",
        ));
    }
    Ok(first.physical_name())
}

fn normalize_table_part_standard_fields(fields: &mut [QueryableField], parent_physical: &str) {
    let parent = parent_physical.strip_prefix('_').unwrap_or(parent_physical);
    let owner_reference = format!("_{parent}_IDRRef");
    for field in fields {
        let standard = if field
            .columns
            .iter()
            .any(|column| names_equal(&column.physical_name, &owner_reference))
        {
            Some("ID")
        } else if field.schema_name == "LineNo"
            || field.schema_name.ends_with("LineNo")
            || field
                .schema_name
                .strip_prefix("LineNo")
                .is_some_and(|number| {
                    !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit())
                })
        {
            Some("LineNo")
        } else {
            None
        };
        let Some(standard) = standard else {
            continue;
        };
        let old_name = std::mem::replace(&mut field.name, standard.to_owned());
        let old_schema = std::mem::replace(&mut field.schema_name, standard.to_owned());
        let mut aliases = standard_field_aliases(standard)
            .iter()
            .map(|alias| (*alias).to_owned())
            .collect::<Vec<_>>();
        push_unique_name(&mut aliases, old_name);
        push_unique_name(&mut aliases, old_schema);
        field.aliases = aliases;
        let compound = field.columns.len() > 1;
        for column in &mut field.columns {
            column.output_label = if compound {
                compound_label(standard, standard, &column.physical_name)
            } else {
                standard.to_owned()
            };
        }
    }
}

/// Builds queryable fields for every currently live object in one indexed
/// pass over the immutable snapshot.
///
/// Objects without a live physical table are omitted. If malformed source
/// metadata resolves the same GUID more than once, the first live object in
/// deterministic snapshot order owns the catalog entry; later duplicates are
/// deliberately collapsed by [`HashMap::entry`].
#[must_use]
pub fn queryable_field_catalog(snapshot: &MetadataSnapshot) -> QueryableFieldCatalog {
    let custom_names = index_custom_field_names(snapshot);
    let mut live_tables = HashMap::with_capacity(snapshot.live_tables().len());
    for table in snapshot.live_tables() {
        live_tables.entry(folded_name(&table.name)).or_insert(table);
    }
    let mut schema_tables = HashMap::with_capacity(snapshot.schema().tables.len());
    for table in &snapshot.schema().tables {
        schema_tables
            .entry(folded_name(&table.name))
            .or_insert(table);
    }

    let mut catalog = HashMap::new();
    for object in snapshot.objects() {
        let Some(physical_table) = object.physical_table.as_deref() else {
            continue;
        };
        let Some(table) = live_tables.get(&folded_name(physical_table)) else {
            continue;
        };
        let schema_table = schema_tables
            .get(&folded_name(
                physical_table.strip_prefix('_').unwrap_or(physical_table),
            ))
            .copied();
        catalog
            .entry(ObjectId::from(&object.guid))
            .or_insert_with(|| {
                project_queryable_fields(
                    snapshot,
                    physical_table,
                    table,
                    schema_table,
                    &CustomFieldNames::Indexed(&custom_names),
                )
            });
    }
    catalog
}

pub(super) type CustomFieldNameIndex = HashMap<(String, u32), Option<String>>;

enum CustomFieldNames<'snapshot> {
    Scan(&'snapshot MetadataSnapshot),
    Indexed(&'snapshot CustomFieldNameIndex),
}

fn index_custom_field_names(snapshot: &MetadataSnapshot) -> CustomFieldNameIndex {
    let mut names = HashMap::new();
    for field in snapshot.fields() {
        for owner in &field.owner_tables {
            names
                .entry((folded_name(owner), field.number))
                .or_insert_with(|| field.name.clone());
        }
        // Extension attributes are addressed by their globally unique field
        // number: the physical `…x1` column that carries them belongs to the
        // extended object, not to a base-declared owner table.
        if field.extension_origin.is_some() {
            names
                .entry((String::new(), field.number))
                .or_insert_with(|| field.name.clone());
        }
    }
    names
}

fn project_queryable_fields(
    snapshot: &MetadataSnapshot,
    physical_table: &str,
    table: &LiveTable,
    schema_table: Option<&crate::metadata::SchemaTable>,
    custom_names: &CustomFieldNames<'_>,
) -> Vec<QueryableField> {
    let mut schema_columns = BTreeMap::new();
    if let Some(table) = schema_table {
        for column in &table.columns {
            let canonical = logical_column_name(&column.physical_name());
            schema_columns
                .entry(canonical.to_lowercase())
                .or_insert((canonical, column));
        }
    }

    let mut order = Vec::<String>::new();
    let mut groups = BTreeMap::<String, Vec<&LiveColumn>>::new();
    for column in &table.columns {
        let observed_name = logical_column_name(&column.name);
        let schema_name = schema_columns
            .get(&observed_name.to_lowercase())
            .map_or(observed_name, |(canonical, _)| canonical.clone());
        if !groups.contains_key(&schema_name) {
            order.push(schema_name.clone());
        }
        groups.entry(schema_name).or_default().push(column);
    }

    order
        .into_iter()
        .map(|schema_name| {
            let columns = groups.remove(&schema_name).unwrap_or_default();
            let reference_targets = schema_columns
                .get(&schema_name.to_lowercase())
                .map(|(_, column)| *column)
                .map(reference_targets)
                .unwrap_or_default();
            let reference_target = match reference_targets.as_slice() {
                [target] if !target.is_empty() => Some(target.clone()),
                _ => None,
            };
            let query_schema_name = normalize_standard_field_name(&schema_name).to_owned();
            let custom_name = match custom_names {
                CustomFieldNames::Scan(snapshot) => {
                    custom_field_name(snapshot, physical_table, &schema_name)
                }
                CustomFieldNames::Indexed(names) => {
                    indexed_custom_field_name(names, physical_table, &schema_name)
                }
            };
            let name = custom_name.unwrap_or_else(|| query_schema_name.clone());
            let mut aliases = standard_field_aliases(&query_schema_name)
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect::<Vec<_>>();
            push_unique_name(&mut aliases, query_schema_name.clone());
            push_unique_name(&mut aliases, schema_name.clone());
            push_unique_name(&mut aliases, name.clone());
            let compound = columns.len() > 1;
            let runtime_typed = columns
                .iter()
                .any(|column| column.name.to_ascii_lowercase().ends_with("rtref"));
            let reference_kind = reference_column_kind(
                snapshot,
                physical_table,
                &schema_name,
                &reference_targets,
                runtime_typed,
            );
            let columns = columns
                .into_iter()
                .map(|column| {
                    let lower = column.name.to_ascii_lowercase();
                    let is_reference_value = lower.ends_with("rref") && !lower.ends_with("rtref");
                    QueryableColumn {
                        output_label: if compound {
                            compound_label(&name, &schema_name, &column.name)
                        } else {
                            name.clone()
                        },
                        physical_name: column.name.clone(),
                        data_type: column.data_type.clone(),
                        kind: match &reference_kind {
                            Some(kind) if is_reference_value => kind.clone(),
                            _ => ColumnKind::from_catalog_type(&column.data_type),
                        },
                    }
                })
                .collect();
            QueryableField {
                name,
                schema_name: query_schema_name,
                aliases,
                columns,
                reference_target,
                reference_targets,
            }
        })
        .collect()
}

/// Derives the reference kind of a field's `RRRef`/`IDRRef` member, or `None`
/// when the field is not a reference.
///
/// SchemaStorage targets are resolved to object IDs through the snapshot
/// index; targets whose table is absent are skipped because projecting the
/// field remains valid. A bare `_IDRRef` without SchemaStorage targets points
/// at the owner of the physical table.
fn reference_column_kind(
    snapshot: &MetadataSnapshot,
    physical_table: &str,
    schema_name: &str,
    reference_targets: &[String],
    runtime_typed: bool,
) -> Option<ColumnKind> {
    if reference_targets.is_empty() {
        // An `RTRef` member marks a reference even when SchemaStorage does
        // not enumerate its targets.
        if runtime_typed {
            return Some(ColumnKind::Reference {
                targets: Vec::new(),
                runtime_typed: true,
            });
        }
        if names_equal(schema_name, "ID") {
            let targets = snapshot
                .object_id_by_physical_table(physical_table)
                .ok()
                .into_iter()
                .collect();
            return Some(ColumnKind::Reference {
                targets,
                runtime_typed: false,
            });
        }
        return None;
    }
    let mut targets = Vec::new();
    for target in reference_targets {
        if target.is_empty() {
            continue;
        }
        if let Ok(id) = snapshot.object_id_by_physical_table(target)
            && !targets.contains(&id)
        {
            targets.push(id);
        }
    }
    Some(ColumnKind::Reference {
        targets,
        runtime_typed,
    })
}
