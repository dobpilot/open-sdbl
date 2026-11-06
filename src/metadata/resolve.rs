#[path = "resolve_selection.rs"]
mod selection;
pub use selection::{resolve_selected_metadata, resolve_selected_runtime_metadata};

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use crate::names::{folded_name, names_equal};

use super::config::{PredefinedSource, Synonym};
use super::db_names::DbNameFieldConflict;
use super::normalize::{normalize_logical_name, normalize_standard_field_name};
use super::{
    AttributeId, ConfigCriterion, ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue,
    DbNameEntry, DbNames, FieldId, Guid, LookupError, MetadataKind, ObjectId, SchemaStorage,
    SchemaTable, SeparatedDataUse, StandardFieldId, collapse_logical_fields, normalize_index_key,
    recase_postgres_identifier,
};

#[derive(Debug, Clone, Copy)]
enum LookupSlot<T> {
    Unique(T),
    Ambiguous,
}

/// Query-independent lookup data, finalized once and shared by snapshot clones.
#[derive(Debug, Default)]
struct QueryFieldIndex {
    by_table_number: HashMap<(String, u32), usize>,
    own_descriptors: HashMap<ObjectId, usize>,
    attribute_descriptors: HashMap<AttributeId, Vec<usize>>,
    extension_by_number: HashMap<u32, usize>,
    separators: Vec<usize>,
    separator_storage: HashMap<u32, Option<String>>,
}

#[derive(Debug, Clone, Default)]
struct MetadataIndex {
    query_fields: Arc<QueryFieldIndex>,
    objects_by_id: HashMap<ObjectId, usize>,
    objects_by_name: HashMap<(MetadataKind, String), LookupSlot<usize>>,
    objects_by_database_type: HashMap<u32, LookupSlot<ObjectId>>,
    objects_by_physical_table: HashMap<String, LookupSlot<ObjectId>>,
    /// Lower-case live table name to its position.
    live_tables_by_name: HashMap<String, usize>,
    /// Lower-case canonical table name to the positions of its `X<n>`
    /// extension variants, sorted by name.
    extension_variants_by_base: HashMap<String, Vec<usize>>,
    /// Lower-case SchemaStorage table name (without the leading underscore)
    /// to its position.
    schema_tables_by_name: HashMap<String, usize>,
    /// Normalized logical names of fields added by configuration extensions.
    extension_field_names: HashSet<String>,
    /// Reference target of an extension field by its field number.
    extension_reference_targets: HashMap<u32, String>,
    /// Однозначно объявленные скалярные UUID, без догадки по длине SQL.
    uuid_attributes: HashSet<AttributeId>,
    attributes_by_id: HashMap<AttributeId, LookupSlot<usize>>,
    attributes_by_owner_name: HashMap<(ObjectId, String), LookupSlot<usize>>,
    values_by_owner_name: HashMap<(ObjectId, String), LookupSlot<usize>>,
    standard_fields: HashSet<(ObjectId, StandardFieldId)>,
}

/// One live PostgreSQL column observed through the catalogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveColumn {
    /// Lowercase PostgreSQL catalog name.
    pub name: String,
    /// PostgreSQL type name.
    pub data_type: String,
}

/// One live PostgreSQL index observed through the catalogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveIndex {
    /// Lowercase PostgreSQL catalog name.
    pub name: String,
    /// Ordered lowercase PostgreSQL column names.
    pub columns: Vec<String>,
    /// Whether PostgreSQL marks the index as unique.
    pub unique: bool,
}

/// One live PostgreSQL table and its observed columns and indexes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTable {
    /// Lowercase PostgreSQL catalog name.
    pub name: String,
    /// Observed columns.
    pub columns: Vec<LiveColumn>,
    /// Observed indexes.
    pub indexes: Vec<LiveIndex>,
}

/// One resolved logical 1C metadata object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataObject {
    /// Metadata GUID.
    pub guid: Guid,
    /// Tabular kind, or `None` for a descriptor without a main DBNames alias.
    pub kind: Option<MetadataKind>,
    /// Human metadata name from Config, when available.
    pub name: Option<String>,
    /// Localized synonyms from Config in source order; empty when the
    /// object has no descriptor or the descriptor names none.
    pub synonyms: Vec<Synonym>,
    /// Descriptor comment from Config, when the descriptor carries one.
    pub comment: Option<String>,
    /// Descriptor marker from Config, when available.
    pub marker: Option<String>,
    /// Main numeric DBNames code, when tabular.
    pub number: Option<u32>,
    /// Canonical main physical table, when tabular.
    pub physical_table: Option<String>,
    /// Owning metadata object for a service table, when resolved.
    pub owner: Option<ObjectId>,
    /// The chart of accounts an accounting register is bound to, from
    /// Config; `None` for every other object.
    pub chart_of_accounts: Option<ObjectId>,
    /// Whether the table is declared by SchemaStorage.
    pub declared: bool,
    /// Whether the table exists in the live PostgreSQL catalog.
    pub live: bool,
    /// Allowed-length mode inferred from the live Code SQL column.
    pub code_allowed_length: Option<AllowedLength>,
    /// Allowed-length mode inferred from the live Number SQL column.
    pub number_allowed_length: Option<AllowedLength>,
}

/// The synonym of `language` among `synonyms`, trimmed; a blank synonym
/// counts as absent, because the configurator stores one for a field
/// nobody gave a presentation to.
fn synonym_of<'synonyms>(synonyms: &'synonyms [Synonym], language: &str) -> Option<&'synonyms str> {
    synonyms
        .iter()
        .find(|synonym| synonym.language.eq_ignore_ascii_case(language))
        .map(|synonym| synonym.text.trim())
        .filter(|text| !text.is_empty())
}

impl MetadataObject {
    /// The synonym the configuration gives this object in `language`.
    ///
    /// Language codes compare case-insensitively, the text is trimmed, and
    /// a blank synonym reads as absent.
    ///
    /// ```
    /// # fn example(snapshot: &open_sdbl::metadata::MetadataSnapshot) {
    /// let object = &snapshot.objects()[0];
    /// let _ = object.synonym("ru");
    /// # }
    /// ```
    #[must_use]
    pub fn synonym(&self, language: &str) -> Option<&str> {
        synonym_of(&self.synonyms, language)
    }

    /// How to present this object in `language`: its synonym, or the
    /// metadata name when the configuration gives none.
    #[must_use]
    pub fn presentation(&self, language: &str) -> Option<&str> {
        self.synonym(language).or(self.name.as_deref())
    }
}

impl MetadataField {
    /// The synonym the configuration gives this field in `language`,
    /// under the same rule as [`MetadataObject::synonym`].
    #[must_use]
    pub fn synonym(&self, language: &str) -> Option<&str> {
        synonym_of(&self.synonyms, language)
    }

    /// How to present this field in `language`: its synonym, or the
    /// metadata name when the configuration gives none.
    #[must_use]
    pub fn presentation(&self, language: &str) -> Option<&str> {
        self.synonym(language).or(self.name.as_deref())
    }
}

/// Измеренное семейство ссылочных типов конфигурации.
/// Состав семейства раскрывается по полному каталогу конфигурации; достигнутый
/// набор объектов ленивого снимка не доказывает полноту такого раскрытия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceTypeSet {
    /// Все ссылочные типы указанного класса метаданных.
    Kind(MetadataKind),
    /// Все ссылочные типы конфигурации, включая перечисления.
    Any,
}

impl ReferenceTypeSet {
    fn covers(self, kind: Option<MetadataKind>) -> bool {
        kind.is_some_and(|kind| self.contains_kind(kind))
    }

    /// Проверяет принадлежность класса семейству по прежним правилам SQL-резолвера.
    #[must_use]
    pub fn contains_kind(self, kind: MetadataKind) -> bool {
        match self {
            Self::Kind(wanted) => kind == wanted && Self::Any.contains_kind(kind),
            Self::Any => matches!(
                kind,
                MetadataKind::Catalog
                    | MetadataKind::Document
                    | MetadataKind::Enumeration
                    | MetadataKind::ChartOfCharacteristicTypes
                    | MetadataKind::ChartOfCalculationTypes
                    | MetadataKind::ChartOfAccounts
                    | MetadataKind::ExchangePlan
                    | MetadataKind::BusinessProcess
                    | MetadataKind::Task
            ),
        }
    }
}

/// The reference categories by their platform identifier.
const REFERENCE_CATEGORIES: [(&str, ReferenceTypeSet); 10] = [
    (
        "e61ef7b8-f3e1-4f4b-8ac7-676e90524997",
        ReferenceTypeSet::Kind(MetadataKind::Catalog),
    ),
    (
        "38bfd075-3e63-4aaa-a93e-94521380d579",
        ReferenceTypeSet::Kind(MetadataKind::Document),
    ),
    (
        "474c3bf6-08b5-4ddc-a2ad-989cedf11583",
        ReferenceTypeSet::Kind(MetadataKind::Enumeration),
    ),
    (
        "99892482-ed55-4fb5-a7f7-20888820a758",
        ReferenceTypeSet::Kind(MetadataKind::ChartOfCharacteristicTypes),
    ),
    (
        "ac606d60-0209-4159-8e4c-794bc091ce38",
        ReferenceTypeSet::Kind(MetadataKind::ChartOfAccounts),
    ),
    (
        "593cd424-0877-470d-91f9-b90a982059b4",
        ReferenceTypeSet::Kind(MetadataKind::ChartOfCalculationTypes),
    ),
    (
        "0a52f9de-73ea-4507-81e8-66217bead73a",
        ReferenceTypeSet::Kind(MetadataKind::ExchangePlan),
    ),
    (
        "214fa4d8-6ba4-4748-a5e1-6332b5887780",
        ReferenceTypeSet::Kind(MetadataKind::BusinessProcess),
    ),
    (
        "6291e9b3-8df5-44e1-b6b2-d9fe008016c0",
        ReferenceTypeSet::Kind(MetadataKind::Task),
    ),
    (
        "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63",
        ReferenceTypeSet::Any,
    ),
];

pub(super) fn reference_category(reference_type: &Guid) -> Option<ReferenceTypeSet> {
    let text = reference_type.to_string();
    REFERENCE_CATEGORIES
        .iter()
        .find(|(guid, _)| guid.eq_ignore_ascii_case(&text))
        .map(|(_, category)| *category)
}

fn push_unique_table(tables: &mut Vec<String>, table: String) {
    if !tables
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&table))
    {
        tables.push(table);
    }
}

/// One resolved custom field from a Fld DBNames entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataField {
    /// Attribute GUID.
    pub guid: Guid,
    /// Human metadata name from Config, when available.
    pub name: Option<String>,
    /// Localized synonyms from Config in source order; empty when the
    /// field has no descriptor or the descriptor names none.
    pub synonyms: Vec<Synonym>,
    /// Descriptor comment from Config, when the descriptor carries one.
    pub comment: Option<String>,
    /// Semantic purpose from the enclosing Config collection, when recognized.
    pub purpose: Option<ConfigFieldPurpose>,
    /// Whether an accounting-register dimension or resource is a balance
    /// one, from Config; `None` for every other field.
    pub balance: Option<bool>,
    /// Numeric Fld code.
    pub number: u32,
    /// Canonical physical base name such as _Fld2566.
    pub physical_name: String,
    /// Canonical SchemaStorage tables containing the field declaration.
    pub owner_tables: Vec<String>,
    /// Whether this field is a data separator.
    pub data_separator: bool,
    /// Separation settings resolved from Config for a data separator;
    /// `None` for ordinary fields and for a separator whose resource could
    /// not be decoded (reported as [`ResolutionFinding::SeparatorSettingsMissing`]).
    pub separation: Option<DataSeparation>,
    /// Whether SchemaStorage declares the field in at least one table.
    pub declared: bool,
    /// Whether at least one matching physical column exists in PostgreSQL.
    pub live: bool,
    /// Configuration-extension origin for an extension-added field.
    pub extension_origin: Option<String>,
    /// Canonical reference target for a reference-typed extension attribute.
    pub reference_target: Option<String>,
    /// Physical tables the attribute's Config type description names.
    /// SchemaStorage leaves the target of a reference that admits several
    /// tables unnamed, so these are the only declared targets such a field
    /// has; empty for every field whose type description names none.
    pub declared_reference_tables: Vec<String>,
}

/// Separation settings of a data separator with its session parameters
/// resolved to names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSeparation {
    /// Константа глобального условия разделения, если ссылка задана.
    pub conditional_separation: Option<Guid>,
    /// Separated-data-use mode from Config.
    pub mode: SeparatedDataUse,
    /// Name of the session parameter bound as the separator value, when
    /// Config binds one and its descriptor is known.
    pub value_parameter: Option<String>,
    /// Name of the session parameter bound as the use flag, when Config
    /// binds one and its descriptor is known.
    pub use_parameter: Option<String>,
}

/// Caller-provided, already decoded resources belonging to one extension.
///
/// ConfigCAS acquisition stays in the application layer; this dependency-free
/// value is the explicit boundary consumed by metadata resolution.
#[derive(Debug, Clone)]
pub struct ExtensionMetadata {
    /// Stable identifier shown by metadata discovery.
    pub origin: String,
    /// Extension-side DBNames mapping.
    pub db_names: DbNames,
    /// Extension-side Config descriptors.
    pub descriptors: Vec<ConfigDescriptor>,
    /// Extension-side SchemaStorage projection, including `Xn` tables.
    pub schema: SchemaStorage,
    /// Reference targets for reference-typed extension attributes, keyed by
    /// physical `Fld` number.
    pub field_reference_targets: Vec<(u32, String)>,
    /// Malformed restructure records surfaced as resolution findings.
    pub restructure_anomalies: Vec<String>,
}

/// One resolved filter criterion: its name and the fields it searches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataCriterion {
    /// GUID of the criterion.
    pub guid: Guid,
    /// Metadata name accepted after `КритерийОтбора.`.
    pub name: String,
    /// GUIDs of the fields the criterion searches, in declaration order.
    pub content: Vec<Guid>,
}

/// One resolved enumeration value or catalog predefined value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataValue {
    /// Owning enumeration or catalog object.
    pub owner: ObjectId,
    /// Stable metadata GUID of the value.
    pub guid: Guid,
    /// Exact symbolic metadata name.
    pub name: String,
}

/// Fixed-versus-variable storage semantics inferred from a live SQL type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedLength {
    /// Fixed-width character storage.
    Fixed,
    /// Variable-width character storage.
    Variable,
}

impl AllowedLength {
    /// Infers the 1C allowed-length mode from a PostgreSQL catalog type name.
    #[must_use]
    pub fn from_postgres_type(data_type: &str) -> Option<Self> {
        let data_type = data_type.to_ascii_lowercase();
        if data_type.contains("mvarchar")
            || data_type.contains("varchar")
            || data_type.contains("character varying")
        {
            Some(Self::Variable)
        } else if data_type.contains("mchar")
            || data_type.starts_with("nchar")
            || data_type.contains("bpchar")
            || data_type == "character"
            || data_type.starts_with("character(")
        {
            Some(Self::Fixed)
        } else {
            None
        }
    }

    /// Returns a stable CLI spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "Fixed",
            Self::Variable => "Variable",
        }
    }
}

/// Comparison of one SchemaStorage index with the live PostgreSQL catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexComparison {
    /// Canonical owning table.
    pub table: String,
    /// SchemaStorage index name.
    pub declared_name: String,
    /// Logical key after compound-field collapse and separator removal.
    pub logical_key: Vec<String>,
    /// Matching live PostgreSQL index name, when present.
    pub live_name: Option<String>,
    /// Whether declared and live uniqueness flags agree.
    pub unique_matches: bool,
}

/// One typed inconsistency found while reconciling metadata resources.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionFinding {
    /// DBNames declares an object without a corresponding Config descriptor.
    DescriptorMissing {
        /// Object GUID from DBNames.
        guid: Guid,
        /// Expected physical table.
        table: String,
    },
    /// A service DBNames entry has no matching SchemaStorage declaration.
    ServiceTableMappingMissing {
        /// Service owner GUID from DBNames.
        guid: Guid,
        /// Exact service alias from DBNames.
        alias: String,
        /// Numeric DBNames code that failed to match the declaration.
        number: u32,
    },
    /// An extension reused a base DBNames field number for a different GUID.
    ExtensionFieldNumberConflict {
        /// Configuration-extension origin supplied by the caller.
        extension: String,
        /// Colliding `Fld` number.
        number: u32,
        /// GUID retained from the base DBNames mapping.
        base_guid: Guid,
        /// Conflicting GUID rejected from the extension mapping.
        extension_guid: Guid,
    },
    /// A malformed record in an extension restructure resource.
    MalformedExtensionRestructure {
        /// Configuration-extension origin supplied by the caller.
        extension: String,
        /// Human-readable description of the malformed record.
        detail: String,
    },
    /// SchemaStorage declares a physical table absent from the live catalog.
    TableNotLive {
        /// Canonical physical table name.
        table: String,
    },
    /// The live catalog contains a table absent from SchemaStorage.
    TableNotDeclared {
        /// Observed physical table name.
        table: String,
    },
    /// SchemaStorage contains a column type tag unknown to this version.
    UnknownColumnTag {
        /// Canonical physical table name.
        table: String,
        /// Canonical column name.
        column: String,
        /// Unrecognized type tag.
        tag: String,
    },
    /// SchemaStorage contained a malformed child declaration that was skipped.
    InvalidSchemaDeclaration {
        /// Canonical physical table name.
        table: String,
        /// Structural reason reported by the tolerant projector.
        detail: String,
    },
    /// An identity expected to be unique occurs more than once.
    DuplicateGuid {
        /// Duplicated metadata GUID.
        guid: Guid,
    },
    /// A declared index has no equivalent live index or differs in uniqueness.
    IndexMismatch {
        /// Canonical physical table name.
        table: String,
        /// SchemaStorage index name.
        index: String,
    },
    /// A data separator's Config resource carries no decodable separation
    /// settings; the separator is treated as `IndependentAndShared` without
    /// session-parameter bindings.
    SeparatorSettingsMissing {
        /// Common attribute GUID from DBNames.
        guid: Guid,
        /// Physical separator column, such as `_Fld56`.
        column: String,
    },
}

impl fmt::Display for ResolutionFinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorMissing { guid, table } => {
                write!(formatter, "descriptor missing for {guid} ({table})")
            }
            Self::ServiceTableMappingMissing {
                guid,
                alias,
                number,
            } => write!(
                formatter,
                "service DBNames entry {alias}{number} for {guid} has no matching SchemaStorage declaration"
            ),
            Self::ExtensionFieldNumberConflict {
                extension,
                number,
                base_guid,
                extension_guid,
            } => write!(
                formatter,
                "extension {extension:?} maps Fld{number} to {extension_guid}, but the base mapping {base_guid} was retained"
            ),
            Self::MalformedExtensionRestructure { extension, detail } => write!(
                formatter,
                "extension {extension:?} restructure record is malformed: {detail}"
            ),
            Self::TableNotLive { table } => write!(formatter, "table {table} is not live"),
            Self::TableNotDeclared { table } => {
                write!(formatter, "live table {table} is absent from SchemaStorage")
            }
            Self::UnknownColumnTag { table, column, tag } => {
                write!(formatter, "unknown column tag {tag:?} on {table}.{column}")
            }
            Self::InvalidSchemaDeclaration { table, detail } => {
                write!(
                    formatter,
                    "invalid SchemaStorage declaration in {table}: {detail}"
                )
            }
            Self::DuplicateGuid { guid } => write!(formatter, "duplicate metadata GUID {guid}"),
            Self::IndexMismatch { table, index } => {
                write!(
                    formatter,
                    "index {table}.{index} differs from the live catalog"
                )
            }
            Self::SeparatorSettingsMissing { guid, column } => write!(
                formatter,
                "data separator {column} ({guid}) has no decodable separation settings in Config"
            ),
        }
    }
}

/// Structured findings produced by one metadata-resolution pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolutionReport {
    findings: Vec<ResolutionFinding>,
}

impl ResolutionReport {
    /// Returns all findings in deterministic source order.
    #[must_use]
    pub fn findings(&self) -> &[ResolutionFinding] {
        &self.findings
    }

    /// Returns `true` when all metadata sources agree.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Snapshot and reconciliation report returned by metadata resolution.
#[derive(Debug, Clone)]
pub struct ResolvedMetadata {
    /// Query-ready immutable metadata snapshot.
    pub snapshot: MetadataSnapshot,
    /// Typed mismatches observed while building the snapshot.
    pub report: ResolutionReport,
}

impl Deref for ResolvedMetadata {
    type Target = MetadataSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

/// Неизменяемые факты для объектной модели без готовности к запросам.
/// Обычные поля, индексы запросов, отчёт сравнения и fingerprint не строятся
/// при отдельном разрешении этого представления.
#[derive(Debug, Clone)]
pub struct RuntimeMetadataFacts {
    db_names: DbNames,
    descriptors: Arc<Vec<ConfigDescriptor>>,
    schema: SchemaStorage,
    live_tables: Arc<Vec<LiveTable>>,
    objects: Arc<Vec<MetadataObject>>,
    fields: Vec<MetadataField>,
    values: Arc<Vec<MetadataValue>>,
    criteria: Vec<MetadataCriterion>,
    roles: Vec<Guid>,
    objects_by_id: Arc<HashMap<ObjectId, usize>>,
    common_attribute_separation: Arc<HashMap<Guid, Option<bool>>>,
    // Неизменяемые коллекции разделяются при отложенной финализации.
    query_context: Arc<QueryFinalizationContext>,
}

// Данные расширений нужны только отложенной финализации; нормализация уже завершена.
#[derive(Debug, Clone)]
struct QueryFinalizationContext {
    extension_origins: HashMap<u32, String>,
    extension_targets: HashMap<u32, String>,
    extension_field_conflicts: Vec<(String, DbNameFieldConflict)>,
    extension_restructure_anomalies: Vec<(String, String)>,
}

fn common_attribute_separation(db_names: &DbNames) -> HashMap<Guid, Option<bool>> {
    let mut result = HashMap::new();
    for entry in db_names
        .entries()
        .iter()
        .filter(|entry| entry.alias == "Fld")
    {
        result
            .entry(entry.guid.clone())
            .and_modify(|known| *known = None)
            .or_insert_with(|| Some(db_names.is_data_separator(entry.number)));
    }
    result
}

impl RuntimeMetadataFacts {
    /// Строит полный снимок запросов из уже нормализованных фактов.
    /// Не повторяет выбор ресурсов, слияние расширений или нормализацию.
    /// Исходное значение не изменяется; неизменяемые коллекции разделяются через `Arc`.
    /// Строгую проверку недостающих описателей выполняет слой `open-sdbl-db`.
    #[must_use]
    pub fn finalize_query(&self) -> ResolvedMetadata {
        self.clone().into_query_snapshot()
    }

    /// Передаёт факты полному снимку запросов без копирования общего состава.
    /// Дополняет поля и строит индексы, отчёт сравнения и прежний fingerprint.
    /// Выбор ресурсов, слияние расширений и нормализация не повторяются.
    #[must_use]
    pub fn into_query_snapshot(self) -> ResolvedMetadata {
        finalize_runtime_facts(self)
    }

    /// Возвращает нормализованные описатели в порядке полного разрешения.
    #[must_use]
    pub fn descriptors(&self) -> &[ConfigDescriptor] {
        &self.descriptors
    }
    /// Возвращает общую декларацию физической схемы выбранных объектов.
    #[must_use]
    pub const fn schema(&self) -> &SchemaStorage {
        &self.schema
    }
    /// Возвращает разрешённые объекты в порядке полного разрешения.
    #[must_use]
    pub fn objects(&self) -> &[MetadataObject] {
        &self.objects
    }
    /// Находит объект по устойчивому идентификатору.
    #[must_use]
    pub fn object_by_id(&self, id: ObjectId) -> Option<&MetadataObject> {
        self.objects_by_id
            .get(&id)
            .map(|&index| &self.objects[index])
    }
    /// Возвращает классификацию единственного физического поля общего реквизита.
    /// `None` означает отсутствие поля или неоднозначный GUID; `Some(false)`
    /// отличает обычное поле от разделителя без построения поля запроса.
    #[must_use]
    pub fn common_attribute_separation(&self, guid: &Guid) -> Option<bool> {
        self.common_attribute_separation
            .get(guid)
            .copied()
            .flatten()
    }
    /// Перечисляет только поля разделителей с разрешёнными настройками.
    pub fn separators(&self) -> impl Iterator<Item = &MetadataField> {
        self.fields.iter().filter(|field| field.data_separator)
    }
    /// Возвращает разрешённые значения перечислений и предопределённые значения.
    #[must_use]
    pub fn values(&self) -> &[MetadataValue] {
        &self.values
    }
    /// Возвращает приобретённые критерии отбора.
    #[must_use]
    pub fn criteria(&self) -> &[MetadataCriterion] {
        &self.criteria
    }
    /// Присоединяет критерии, разобранные из ресурсов конфигурации.
    pub fn attach_criteria(&mut self, criteria: Vec<ConfigCriterion>) {
        self.criteria = criteria
            .into_iter()
            .map(|criterion| MetadataCriterion {
                guid: criterion.guid,
                name: criterion.name,
                content: criterion.content,
            })
            .collect();
    }
    /// Возвращает роли в порядке корневой коллекции конфигурации.
    #[must_use]
    pub fn roles(&self) -> &[Guid] {
        &self.roles
    }
    /// Присоединяет роли, разобранные из корня конфигурации.
    pub fn attach_roles(&mut self, roles: Vec<Guid>) {
        self.roles = roles;
    }
}

/// Query-ready metadata whose indexed collections cannot be mutated.
///
/// Collections are intentionally exposed only through shared slice accessors,
/// so safe application code cannot invalidate identity indexes by removing or
/// reordering resolved entries.
///
/// ```compile_fail
/// # fn mutate(snapshot: &mut open_sdbl::metadata::MetadataSnapshot) {
/// snapshot.objects.clear();
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MetadataSnapshot {
    facts: RuntimeMetadataFacts,
    indexes: Vec<IndexComparison>,
    index: MetadataIndex,
    fingerprint: SnapshotFingerprint,
}

/// Identifies one loaded snapshot.
///
/// Two snapshots resolved from the same metadata compare equal, which is
/// how a [`crate::query::Prepared`] refuses a snapshot other than the one
/// it was prepared against, and how an application confirms that a reload
/// left its prepared queries valid.
///
/// It says nothing about a base until that base has been read: computing
/// it needs the whole load. To ask whether a configuration changed without
/// loading it, run
/// [`crate::metadata::PostgresMetadataQueries::CONFIG_FINGERPRINT`] or its
/// SQL Server counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotFingerprint([u64; 2]);

impl MetadataSnapshot {
    /// Заимствует общие факты без копирования и без изменения готовности снимка.
    #[must_use]
    pub const fn runtime_facts(&self) -> &RuntimeMetadataFacts {
        &self.facts
    }

    pub(crate) fn attribute_is_uuid(&self, id: AttributeId) -> bool {
        self.index.uuid_attributes.contains(&id)
    }

    /// Returns the authoritative DBNames mapping.
    #[must_use]
    pub const fn db_names(&self) -> &DbNames {
        &self.facts.db_names
    }

    /// Returns Config descriptors in resolution source order.
    #[must_use]
    pub fn descriptors(&self) -> &[ConfigDescriptor] {
        &self.facts.descriptors
    }

    /// Returns the authoritative physical schema declaration.
    #[must_use]
    pub const fn schema(&self) -> &SchemaStorage {
        &self.facts.schema
    }

    /// Returns observed live tables in catalog order.
    #[must_use]
    pub fn live_tables(&self) -> &[LiveTable] {
        &self.facts.live_tables
    }

    /// Returns resolved metadata objects in deterministic source order.
    #[must_use]
    pub fn objects(&self) -> &[MetadataObject] {
        &self.facts.objects
    }

    /// Returns resolved custom fields in deterministic source order.
    #[must_use]
    pub fn fields(&self) -> &[MetadataField] {
        &self.facts.fields
    }

    /// Returns the data-separator fields in deterministic source order.
    pub fn separators(&self) -> impl Iterator<Item = &MetadataField> {
        self.index
            .query_fields
            .separators
            .iter()
            .map(|position| &self.facts.fields[*position])
    }

    /// The first descriptor whose resource and object identities both equal
    /// `object`, in source order. This lookup is indexed at finalization.
    #[must_use]
    pub fn owner_descriptor(&self, object: ObjectId) -> Option<&ConfigDescriptor> {
        self.index
            .query_fields
            .own_descriptors
            .get(&object)
            .map(|position| &self.facts.descriptors[*position])
    }

    /// Every loaded descriptor of `attribute`, in source order, including
    /// duplicates. Callers retain responsibility for checking resource ownership
    /// and whether the declaration is supported; no evidence is discarded.
    pub fn attribute_descriptors(
        &self,
        attribute: AttributeId,
    ) -> impl Iterator<Item = &ConfigDescriptor> {
        self.index
            .query_fields
            .attribute_descriptors
            .get(&attribute)
            .into_iter()
            .flatten()
            .map(|position| &self.facts.descriptors[*position])
    }

    pub(crate) fn query_field(&self, table: &str, number: u32) -> Option<&MetadataField> {
        self.index
            .query_fields
            .by_table_number
            .get(&(folded_name(table), number))
            .map(|position| &self.facts.fields[*position])
    }

    pub(crate) fn query_extension_field(&self, number: u32) -> Option<&MetadataField> {
        self.index
            .query_fields
            .extension_by_number
            .get(&number)
            .map(|position| &self.facts.fields[*position])
    }

    pub(crate) fn separator_storage_type(&self, number: u32) -> Option<&str> {
        self.index
            .query_fields
            .separator_storage
            .get(&number)
            .and_then(Option::as_deref)
    }

    /// Returns resolved predefined values in deterministic source order.
    #[must_use]
    pub fn values(&self) -> &[MetadataValue] {
        &self.facts.values
    }

    /// Returns the filter criteria in deterministic source order.
    #[must_use]
    pub fn criteria(&self) -> &[MetadataCriterion] {
        &self.facts.criteria
    }

    /// Attaches the filter criteria decoded from Config resources.
    pub fn attach_criteria(&mut self, criteria: Vec<ConfigCriterion>) {
        self.facts.criteria = criteria
            .into_iter()
            .map(|criterion| MetadataCriterion {
                guid: criterion.guid,
                name: criterion.name,
                content: criterion.content,
            })
            .collect();
    }

    /// Attaches the role identifiers projected from the configuration
    /// root (`ParsedConfigResource::roles`).
    pub fn attach_roles(&mut self, roles: Vec<Guid>) {
        self.facts.roles = roles;
    }

    /// The roles the configuration declares, in collection order; empty
    /// until attached.
    #[must_use]
    pub fn roles(&self) -> &[Guid] {
        &self.facts.roles
    }

    /// Looks up a filter criterion by its metadata name.
    #[must_use]
    pub fn criterion(&self, name: &str) -> Option<&MetadataCriterion> {
        let wanted = normalize_name(name);
        self.facts
            .criteria
            .iter()
            .find(|criterion| normalize_name(&criterion.name) == wanted)
    }

    /// Returns SchemaStorage/live index comparisons.
    #[must_use]
    pub fn indexes(&self) -> &[IndexComparison] {
        &self.indexes
    }

    /// Identifies this snapshot; see [`SnapshotFingerprint`].
    #[must_use]
    pub const fn fingerprint(&self) -> SnapshotFingerprint {
        self.fingerprint
    }

    /// Looks up a tabular object GUID by kind and Config name in expected O(1)
    /// time after name normalization.
    ///
    /// # Errors
    ///
    /// Returns a typed missing or ambiguity outcome.
    pub fn object_id(&self, kind: MetadataKind, name: &str) -> Result<ObjectId, LookupError> {
        match self
            .index
            .objects_by_name
            .get(&(kind, normalize_name(name)))
        {
            Some(LookupSlot::Unique(index)) => Ok(ObjectId::from(&self.facts.objects[*index].guid)),
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousObject),
            None => Err(LookupError::ObjectNotFound),
        }
    }

    /// Returns the resolved object for a real metadata GUID.
    #[must_use]
    pub fn object_by_id(&self, id: ObjectId) -> Option<&MetadataObject> {
        self.index
            .objects_by_id
            .get(&id)
            .map(|index| &self.facts.objects[*index])
    }

    /// The synonym of the object `id` in `language`, under the rule of
    /// [`MetadataObject::synonym`]; `None` when the snapshot does not
    /// resolve the identifier.
    #[must_use]
    pub fn object_synonym(&self, id: ObjectId, language: &str) -> Option<&str> {
        self.object_by_id(id)
            .and_then(|object| object.synonym(language))
    }

    /// How to present the object `id` in `language`: its synonym, or its
    /// metadata name when the configuration gives none.
    #[must_use]
    pub fn object_presentation(&self, id: ObjectId, language: &str) -> Option<&str> {
        self.object_by_id(id)
            .and_then(|object| object.presentation(language))
    }

    /// Looks up a custom attribute GUID by owner GUID and Config name in
    /// expected O(1) time after name normalization.
    ///
    /// # Errors
    ///
    /// Returns a typed result for a missing owner, missing or ambiguous field,
    /// or a standard field that has no Config metadata GUID.
    pub fn attribute_id(&self, owner: ObjectId, name: &str) -> Result<AttributeId, LookupError> {
        if !self.index.objects_by_id.contains_key(&owner) {
            return Err(LookupError::OwnerNotFound);
        }
        if let Some(standard) = StandardFieldId::from_name(name)
            && self.index.standard_fields.contains(&(owner, standard))
        {
            return Err(LookupError::StandardFieldHasNoMetadataGuid(standard));
        }
        match self
            .index
            .attributes_by_owner_name
            .get(&(owner, normalize_name(name)))
        {
            Some(LookupSlot::Unique(index)) => {
                Ok(AttributeId::from(&self.facts.fields[*index].guid))
            }
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousField),
            None => Err(LookupError::FieldNotFound),
        }
    }

    /// Looks up either a custom GUID-backed field or a numeric standard field.
    ///
    /// # Errors
    ///
    /// Returns a typed result for a missing owner or field and for ambiguity.
    pub fn field_id(&self, owner: ObjectId, name: &str) -> Result<FieldId, LookupError> {
        if !self.index.objects_by_id.contains_key(&owner) {
            return Err(LookupError::OwnerNotFound);
        }
        let custom = self
            .index
            .attributes_by_owner_name
            .get(&(owner, normalize_name(name)));
        let standard = StandardFieldId::from_name(name)
            .filter(|field| self.index.standard_fields.contains(&(owner, *field)));
        match (custom, standard) {
            (Some(LookupSlot::Unique(index)), None) => Ok(FieldId::Metadata(AttributeId::from(
                &self.facts.fields[*index].guid,
            ))),
            (None, Some(field)) => Ok(FieldId::Standard(field)),
            (Some(LookupSlot::Ambiguous), _) | (Some(LookupSlot::Unique(_)), Some(_)) => {
                Err(LookupError::AmbiguousField)
            }
            (None, None) => Err(LookupError::FieldNotFound),
        }
    }

    /// Returns the resolved custom field for its real metadata GUID.
    ///
    /// # Errors
    ///
    /// Returns a typed missing or ambiguity outcome.
    pub fn attribute_by_id(&self, id: AttributeId) -> Result<&MetadataField, LookupError> {
        match self.index.attributes_by_id.get(&id) {
            Some(LookupSlot::Unique(index)) => Ok(&self.facts.fields[*index]),
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousField),
            None => Err(LookupError::FieldNotFound),
        }
    }

    /// Resolves a DBNames/RTRef database type number to an object GUID.
    ///
    /// # Errors
    ///
    /// Returns a typed missing or ambiguity outcome.
    pub fn object_id_by_database_type(&self, number: u32) -> Result<ObjectId, LookupError> {
        match self.index.objects_by_database_type.get(&number) {
            Some(LookupSlot::Unique(id)) => Ok(*id),
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousObject),
            None => Err(LookupError::ObjectNotFound),
        }
    }

    /// Looks up a live table by its physical name, case-insensitively, in
    /// expected O(1) time.
    #[must_use]
    pub fn live_table(&self, physical_name: &str) -> Option<&LiveTable> {
        self.index
            .live_tables_by_name
            .get(&physical_name.to_ascii_lowercase())
            .map(|position| &self.facts.live_tables[*position])
    }

    /// Returns the `X<n>` configuration-extension variants of a canonical
    /// physical table, sorted by name.
    pub fn extension_live_tables(&self, physical_name: &str) -> impl Iterator<Item = &LiveTable> {
        self.index
            .extension_variants_by_base
            .get(&physical_name.to_ascii_lowercase())
            .into_iter()
            .flatten()
            .map(|position| &self.facts.live_tables[*position])
    }

    /// Looks up a SchemaStorage table by physical or schema name in expected
    /// O(1) time.
    #[must_use]
    pub fn schema_table(&self, physical_name: &str) -> Option<&SchemaTable> {
        let name = physical_name.strip_prefix('_').unwrap_or(physical_name);
        self.index
            .schema_tables_by_name
            .get(&name.to_ascii_lowercase())
            .map(|position| &self.facts.schema.tables[*position])
    }

    pub(crate) fn is_extension_field_name(&self, logical_name: &str) -> bool {
        self.index
            .extension_field_names
            .contains(&normalize_name(logical_name))
    }

    pub(crate) fn extension_reference_target(&self, number: u32) -> Option<&str> {
        self.index
            .extension_reference_targets
            .get(&number)
            .map(String::as_str)
    }

    /// Looks up the tabular object owning a physical table by its canonical
    /// name, accepting both the live spelling (`_Reference57`) and the
    /// SchemaStorage spelling (`Reference57`), in expected O(1) time.
    ///
    /// # Errors
    ///
    /// Returns a typed missing or ambiguity outcome.
    pub fn object_id_by_physical_table(&self, table: &str) -> Result<ObjectId, LookupError> {
        match self
            .index
            .objects_by_physical_table
            .get(&normalize_physical_table_name(table))
        {
            Some(LookupSlot::Unique(id)) => Ok(*id),
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousObject),
            None => Err(LookupError::ObjectNotFound),
        }
    }

    /// Looks up a predefined value by owner and exact normalized metadata name.
    ///
    /// # Errors
    ///
    /// Returns a typed missing-owner, missing-value, or ambiguity outcome.
    pub fn predefined_value(
        &self,
        owner: ObjectId,
        name: &str,
    ) -> Result<&MetadataValue, LookupError> {
        if !self.index.objects_by_id.contains_key(&owner) {
            return Err(LookupError::OwnerNotFound);
        }
        match self
            .index
            .values_by_owner_name
            .get(&(owner, normalize_name(name)))
        {
            Some(LookupSlot::Unique(index)) => Ok(&self.facts.values[*index]),
            Some(LookupSlot::Ambiguous) => Err(LookupError::AmbiguousValue),
            None => Err(LookupError::ValueNotFound),
        }
    }
}

/// Resolves authoritative 1C resources against observational PostgreSQL rows.
#[must_use]
pub fn resolve_metadata(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
    resolve_metadata_with_predefined_values(db_names, descriptors, Vec::new(), schema, live_tables)
}

/// Resolves base metadata together with caller-provided extension resources.
#[must_use]
pub fn resolve_metadata_with_extensions(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    extensions: Vec<ExtensionMetadata>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
    resolve_metadata_with_predefined_values_and_extensions(
        db_names,
        descriptors,
        Vec::new(),
        extensions,
        schema,
        live_tables,
    )
}

/// Resolves authoritative 1C resources including catalog predefined values.
#[must_use]
pub fn resolve_metadata_with_predefined_values(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
    resolve_metadata_with_predefined_values_and_extensions(
        db_names,
        descriptors,
        predefined_values,
        Vec::new(),
        schema,
        live_tables,
    )
}

/// Resolves authoritative 1C resources including the filter criteria the
/// Config resources declare.
#[must_use]
pub fn resolve_metadata_with_criteria(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    criteria: Vec<ConfigCriterion>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
    let mut resolved = resolve_metadata_with_predefined_values_and_extensions(
        db_names,
        descriptors,
        predefined_values,
        Vec::new(),
        schema,
        live_tables,
    );
    resolved.snapshot.facts.criteria = criteria
        .into_iter()
        .map(|criterion| MetadataCriterion {
            guid: criterion.guid,
            name: criterion.name,
            content: criterion.content,
        })
        .collect();
    resolved
}

/// Resolves predefined values and caller-provided extension resources.
#[must_use]
pub fn resolve_metadata_with_predefined_values_and_extensions(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
    match resolve_metadata_parts(
        db_names,
        descriptors,
        predefined_values,
        extensions,
        schema,
        live_tables,
        false,
    ) {
        MetadataResolution::Snapshot(resolved) => resolved,
        MetadataResolution::Runtime(_) => unreachable!(),
    }
}

pub(super) fn resolve_runtime_metadata_with_extensions(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> RuntimeMetadataFacts {
    match resolve_metadata_parts(
        db_names,
        descriptors,
        predefined_values,
        extensions,
        schema,
        live_tables,
        true,
    ) {
        MetadataResolution::Runtime(facts) => facts,
        MetadataResolution::Snapshot(_) => unreachable!(),
    }
}

// Результат немедленно разбирает одна из двух обёрток. Дополнительная
// аллокация ради выравнивания размеров вариантов здесь не нужна.
#[allow(clippy::large_enum_variant)]
enum MetadataResolution {
    Snapshot(ResolvedMetadata),
    Runtime(RuntimeMetadataFacts),
}

#[allow(clippy::too_many_arguments)]
fn resolve_metadata_parts(
    mut db_names: DbNames,
    mut descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    mut schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
    runtime_only: bool,
) -> MetadataResolution {
    let mut extension_origins = HashMap::<u32, String>::new();
    let mut extension_targets = HashMap::<u32, String>::new();
    let mut extension_field_conflicts = Vec::<(String, DbNameFieldConflict)>::new();
    let mut extension_restructure_anomalies = Vec::<(String, String)>::new();
    let mut descriptor_guids = descriptors
        .iter()
        .map(|descriptor| descriptor.object_guid.clone())
        .collect::<HashSet<_>>();
    for extension in extensions {
        let added_field_numbers = extension
            .db_names
            .entries()
            .iter()
            .filter(|entry| entry.alias == "Fld")
            .filter(|entry| db_names.field_guid(entry.number).is_none())
            .map(|entry| entry.number)
            .collect::<Vec<_>>();
        let conflicts = db_names.extend_from(extension.db_names);
        for number in added_field_numbers {
            extension_origins
                .entry(number)
                .or_insert_with(|| extension.origin.clone());
        }
        extension_field_conflicts.extend(
            conflicts
                .into_iter()
                .map(|conflict| (extension.origin.clone(), conflict)),
        );
        for descriptor in extension.descriptors {
            if descriptor_guids.insert(descriptor.object_guid.clone()) {
                descriptors.push(descriptor);
            }
        }
        for (number, target) in extension.field_reference_targets {
            extension_targets.entry(number).or_insert(target);
        }
        schema.tables.extend(extension.schema.tables);
        schema.anomalies.extend(extension.schema.anomalies);
        extension_restructure_anomalies.extend(
            extension
                .restructure_anomalies
                .into_iter()
                .map(|detail| (extension.origin.clone(), detail)),
        );
    }
    super::runtime_metadata::resolve_named_types(&mut descriptors);
    super::runtime_metadata::resolve_functional_options(&mut descriptors, &db_names);
    super::runtime_metadata::resolve_common_attributes(&mut descriptors, &db_names);
    let live_table_by_name = index_live_tables(&live_tables);
    let physical_tables = PhysicalTableIndex::new(&schema);
    let schema_tables: HashSet<String> = schema
        .tables
        .iter()
        .map(|table| folded_name(&table.name))
        .collect();
    let descriptor_by_guid: HashMap<&Guid, &ConfigDescriptor> = descriptors
        .iter()
        .map(|descriptor| (&descriptor.object_guid, descriptor))
        .collect();
    let mut seen_primary = HashSet::new();
    let mut seen_entries = HashSet::new();
    let mut objects = Vec::new();

    for (entry, kind) in db_names.objects() {
        if entry.guid.is_nil()
            || !seen_entries.insert((kind, entry.guid.clone(), entry.number, entry.alias.clone()))
            || (!kind.is_service() && !seen_primary.insert(entry.guid.clone()))
        {
            continue;
        }
        let Some(physical_table) = physical_tables.table_for_entry(entry, kind) else {
            continue;
        };
        let descriptor = descriptor_by_guid.get(&entry.guid).copied();
        let live_table = live_table_by_name
            .get(&physical_table.to_ascii_lowercase())
            .map(|position| &live_tables[*position]);
        objects.push(MetadataObject {
            guid: entry.guid.clone(),
            kind: Some(kind),
            name: descriptor
                .map(|value| value.name.clone())
                .or_else(|| kind.is_service().then(|| entry.alias.clone())),
            synonyms: descriptor
                .map(|value| value.synonyms.clone())
                .unwrap_or_default(),
            comment: descriptor.and_then(|value| value.comment.clone()),
            marker: descriptor.map(|value| value.marker.clone()),
            number: Some(entry.number),
            declared: schema_tables.contains(&folded_name(
                physical_table.strip_prefix('_').unwrap_or(&physical_table),
            )),
            live: live_table.is_some(),
            code_allowed_length: infer_allowed_length(live_table, "_code"),
            number_allowed_length: infer_allowed_length(live_table, "_number"),
            physical_table: Some(physical_table),
            owner: None,
            chart_of_accounts: descriptor
                .and_then(|value| value.chart_of_accounts.as_ref())
                .map(ObjectId::from),
        });
    }

    let primary_owners = objects
        .iter()
        .filter(|object| object.kind.is_some_and(|kind| !kind.is_service()))
        .filter_map(|object| {
            object
                .physical_table
                .as_deref()
                .map(|table| (folded_name(table), ObjectId::from(&object.guid)))
        })
        .collect::<HashMap<_, _>>();
    let primary_by_guid = objects
        .iter()
        .filter(|object| object.kind.is_some_and(|kind| !kind.is_service()))
        .map(|object| (object.guid.clone(), ObjectId::from(&object.guid)))
        .collect::<HashMap<_, _>>();
    for object in &mut objects {
        if object.kind.is_some_and(MetadataKind::is_service) {
            object.owner = primary_by_guid.get(&object.guid).copied().or_else(|| {
                object.physical_table.as_deref().and_then(|table| {
                    physical_tables
                        .schema_row(table)
                        .map(|position| &schema.tables[position])
                        .and_then(|declaration| {
                            declaration.owner.as_deref().and_then(|owner| {
                                primary_owners
                                    .get(&folded_name(&format!("_{owner}")))
                                    .copied()
                            })
                        })
                })
            });
        }
    }
    let (dependency_tables, dependency_owners) = dependency_indexes(&objects);
    let dependency_objects = schema
        .tables
        .iter()
        .filter_map(|table| {
            let physical = table.physical_name();
            let name = table
                .inline_name
                .as_deref()
                .filter(|name| matches!(*name, "BaseCK" | "LeadingCK" | "DisplacedCK"))?;
            let owner_table = format!("_{}", table.owner.as_deref()?);
            if dependency_tables.contains(&folded_name(&physical)) {
                return None;
            }
            let owner = &objects[*dependency_owners.get(&folded_name(&owner_table))?];
            let live_table = live_table_by_name
                .get(&physical.to_ascii_lowercase())
                .map(|position| &live_tables[*position]);
            Some(MetadataObject {
                guid: owner.guid.clone(),
                kind: Some(MetadataKind::CalculationKindDependency),
                name: Some(name.to_owned()),
                synonyms: Vec::new(),
                comment: None,
                marker: None,
                number: Some(table.number),
                physical_table: Some(physical),
                owner: Some(ObjectId::from(&owner.guid)),
                declared: true,
                live: live_table.is_some(),
                code_allowed_length: None,
                number_allowed_length: None,
                chart_of_accounts: None,
            })
        })
        .collect::<Vec<_>>();
    objects.extend(dependency_objects);

    for descriptor in &descriptors {
        if descriptor.resource_guid == descriptor.object_guid
            && seen_primary.insert(descriptor.object_guid.clone())
        {
            objects.push(MetadataObject {
                guid: descriptor.object_guid.clone(),
                kind: None,
                name: Some(descriptor.name.clone()),
                synonyms: descriptor.synonyms.clone(),
                comment: descriptor.comment.clone(),
                marker: Some(descriptor.marker.clone()),
                number: None,
                physical_table: None,
                declared: false,
                live: false,
                code_allowed_length: None,
                number_allowed_length: None,
                owner: None,
                chart_of_accounts: None,
            });
        }
    }

    // Только опубликованным runtime-фактам нужны отдельные поля разделителей.
    // Полный путь сразу построит все поля при финализации, поэтому не создаёт
    // временные поля и их вспомогательные индексы второй раз.
    let fields = if runtime_only {
        resolve_fields(
            &db_names,
            &descriptors,
            &schema,
            &live_tables,
            &objects,
            &extension_origins,
            &extension_targets,
            true,
        )
        .0
    } else {
        Vec::new()
    };

    let object_ids = objects
        .iter()
        .filter(|object| object.kind.is_none_or(|kind| !kind.is_service()))
        .map(|object| {
            (
                object.guid.clone(),
                (ObjectId::from(&object.guid), object.kind),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut values = Vec::new();
    for descriptor in &descriptors {
        if descriptor.resource_guid == descriptor.object_guid || !descriptor.enumeration_value {
            continue;
        }
        let Some((owner, Some(MetadataKind::Enumeration))) =
            object_ids.get(&descriptor.resource_guid).copied()
        else {
            continue;
        };
        values.push(MetadataValue {
            owner,
            guid: descriptor.object_guid.clone(),
            name: descriptor.name.clone(),
        });
    }
    for predefined in predefined_values {
        // A value belongs to an object of the kind its resource serves:
        // `.1c` to a catalog, `.9` to a chart of accounts.
        let Some((owner, Some(kind))) = object_ids.get(&predefined.owner_guid).copied() else {
            continue;
        };
        let matching = match predefined.source {
            PredefinedSource::Catalog => kind == MetadataKind::Catalog,
            PredefinedSource::ChartOfAccounts => kind == MetadataKind::ChartOfAccounts,
            PredefinedSource::ChartOfCharacteristicTypes => {
                kind == MetadataKind::ChartOfCharacteristicTypes
            }
        };
        if !matching {
            continue;
        }
        values.push(MetadataValue {
            owner,
            guid: predefined.value_guid,
            name: predefined.name,
        });
    }

    let objects_by_id = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.kind.is_none_or(|kind| !kind.is_service()))
        .map(|(position, object)| (ObjectId::from(&object.guid), position))
        .collect();
    let facts = RuntimeMetadataFacts {
        common_attribute_separation: Arc::new(common_attribute_separation(&db_names)),
        db_names,
        descriptors: Arc::new(descriptors),
        schema,
        live_tables: Arc::new(live_tables),
        objects: Arc::new(objects),
        fields,
        values: Arc::new(values),
        criteria: Vec::new(),
        roles: Vec::new(),
        objects_by_id: Arc::new(objects_by_id),
        query_context: Arc::new(QueryFinalizationContext {
            extension_origins,
            extension_targets,
            extension_field_conflicts,
            extension_restructure_anomalies,
        }),
    };
    if runtime_only {
        MetadataResolution::Runtime(facts)
    } else {
        MetadataResolution::Snapshot(finalize_runtime_facts(facts))
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_fields(
    db_names: &DbNames,
    descriptors: &[ConfigDescriptor],
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    objects: &[MetadataObject],
    extension_origins: &HashMap<u32, String>,
    extension_targets: &HashMap<u32, String>,
    runtime_only: bool,
) -> (Vec<MetadataField>, Vec<ResolutionFinding>) {
    let schema_field_owners = index_schema_field_owners(schema);
    let schema_field_targets = index_schema_field_targets(schema);
    let live_fields = index_live_fields(live_tables);
    let descriptor_by_guid: HashMap<&Guid, &ConfigDescriptor> = descriptors
        .iter()
        .map(|descriptor| (&descriptor.object_guid, descriptor))
        .collect();
    let object_guid_by_reference_type: HashMap<&Guid, &Guid> = descriptors
        .iter()
        .filter_map(|descriptor| {
            descriptor
                .object_reference_type
                .as_ref()
                .map(|reference_type| (reference_type, &descriptor.object_guid))
        })
        .collect();
    let mut separator_findings = Vec::new();
    // Every object's reference type resolved to the table it stores, so an
    // attribute's declared targets become physical tables.
    let table_by_reference_type: HashMap<&Guid, String> = object_guid_by_reference_type
        .iter()
        .filter_map(|(reference_type, object_guid)| {
            objects
                .iter()
                .find(|object| &&object.guid == object_guid)
                .and_then(|object| object.physical_table.clone())
                .map(|table| (*reference_type, table))
        })
        .collect();
    let entries = db_names
        .entries()
        .iter()
        .filter(|entry| {
            entry.alias == "Fld" && (!runtime_only || db_names.is_data_separator(entry.number))
        })
        .collect::<Vec<_>>();
    let resolved = parallel_ordered(&entries, |entry| {
        let logical_name = format!("Fld{}", entry.number);
        let physical_name = format!("_{logical_name}");
        let owner_tables = schema_field_owners
            .get(&logical_name)
            .cloned()
            .unwrap_or_default();
        let live = live_fields.contains(&logical_name);
        let data_separator = db_names.is_data_separator(entry.number);
        let separation = data_separator
            .then(|| {
                descriptor_by_guid
                    .get(&entry.guid)
                    .and_then(|descriptor| descriptor.separation.as_ref())
                    .map(|settings| {
                        let parameter_name = |guid: &Option<Guid>| {
                            guid.as_ref()
                                .and_then(|guid| descriptor_by_guid.get(guid))
                                .map(|descriptor| descriptor.name.clone())
                        };
                        DataSeparation {
                            conditional_separation: settings.conditional_separation.clone(),
                            mode: settings.mode,
                            value_parameter: parameter_name(&settings.value_parameter),
                            use_parameter: parameter_name(&settings.use_parameter),
                        }
                    })
            })
            .flatten();
        let finding = (data_separator && separation.is_none()).then(|| {
            ResolutionFinding::SeparatorSettingsMissing {
                guid: entry.guid.clone(),
                column: physical_name.clone(),
            }
        });
        let field = MetadataField {
            guid: entry.guid.clone(),
            name: descriptor_by_guid
                .get(&entry.guid)
                .map(|descriptor| descriptor.name.clone()),
            synonyms: descriptor_by_guid
                .get(&entry.guid)
                .map(|descriptor| descriptor.synonyms.clone())
                .unwrap_or_default(),
            comment: descriptor_by_guid
                .get(&entry.guid)
                .and_then(|descriptor| descriptor.comment.clone()),
            purpose: descriptor_by_guid
                .get(&entry.guid)
                .and_then(|descriptor| descriptor.field_purpose),
            balance: descriptor_by_guid
                .get(&entry.guid)
                .and_then(|descriptor| descriptor.balance),
            number: entry.number,
            physical_name,
            declared: !owner_tables.is_empty(),
            live,
            owner_tables,
            data_separator,
            separation,
            extension_origin: extension_origins.get(&entry.number).cloned(),
            reference_target: schema_field_targets
                .get(&logical_name)
                .or_else(|| extension_targets.get(&entry.number))
                .cloned(),
            // A type description that names something other than a stored
            // object or a reference category — a defined type, a
            // characteristic — cannot be turned into a complete list of
            // tables, and a partial list would silently narrow the field,
            // so it is left empty.
            declared_reference_tables: descriptor_by_guid
                .get(&entry.guid)
                .map(|descriptor| {
                    let mut tables = Vec::new();
                    let complete = descriptor.reference_types.iter().all(|reference_type| {
                        if let Some(table) = table_by_reference_type.get(reference_type) {
                            push_unique_table(&mut tables, table.clone());
                            return true;
                        }
                        let Some(category) = reference_category(reference_type) else {
                            return false;
                        };
                        for object in objects {
                            if category.covers(object.kind)
                                && let Some(table) = &object.physical_table
                            {
                                push_unique_table(&mut tables, table.clone());
                            }
                        }
                        true
                    });
                    if complete { tables } else { Vec::new() }
                })
                .unwrap_or_default(),
        };
        (field, finding)
    });
    let mut fields = Vec::with_capacity(resolved.len());
    for (field, finding) in resolved {
        fields.push(field);
        separator_findings.extend(finding);
    }

    (fields, separator_findings)
}

fn parallel_ordered<T, O>(items: &[T], work: impl Fn(&T) -> O + Sync) -> Vec<O>
where
    T: Sync,
    O: Send,
{
    const CHUNK_SIZE: usize = 256;
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(items.len().div_ceil(CHUNK_SIZE));
    parallel_ordered_with_workers(items, workers, work)
}

fn parallel_ordered_with_workers<T, O>(
    items: &[T],
    workers: usize,
    work: impl Fn(&T) -> O + Sync,
) -> Vec<O>
where
    T: Sync,
    O: Send,
{
    if workers <= 1 {
        return items.iter().map(work).collect();
    }
    let chunk_size = items.len().div_ceil(workers);
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for chunk in items.chunks(chunk_size) {
            let work = &work;
            handles.push(scope.spawn(move || chunk.iter().map(work).collect::<Vec<_>>()));
        }
        let mut output = Vec::with_capacity(items.len());
        for handle in handles {
            output.extend(handle.join().expect("metadata field worker panicked"));
        }
        output
    })
}

fn finalize_runtime_facts(mut facts: RuntimeMetadataFacts) -> ResolvedMetadata {
    let live_table_by_name = index_live_tables(&facts.live_tables);
    let physical_tables = PhysicalTableIndex::new(&facts.schema);
    let indexes = compare_indexes(
        &facts.schema,
        &facts.live_tables,
        &live_table_by_name,
        &facts.db_names,
    );
    let (fields, separator_findings) = resolve_fields(
        &facts.db_names,
        &facts.descriptors,
        &facts.schema,
        &facts.live_tables,
        &facts.objects,
        &facts.query_context.extension_origins,
        &facts.query_context.extension_targets,
        false,
    );
    facts.fields = fields;
    let RuntimeMetadataFacts {
        db_names,
        descriptors,
        schema,
        live_tables,
        objects,
        fields,
        values,
        ..
    } = &facts;
    let report = || {
        build_resolution_report(
            db_names,
            descriptors,
            schema,
            live_tables,
            &indexes,
            &facts.query_context.extension_field_conflicts,
            &facts.query_context.extension_restructure_anomalies,
            &physical_tables,
        )
    };
    let index = || {
        let mut index = build_metadata_index(
            objects,
            fields,
            values,
            descriptors,
            schema,
            live_tables,
            &live_table_by_name,
        );
        let mut uuid_attributes = HashMap::new();
        for descriptor in descriptors.iter() {
            let known_uuid = descriptor.declared_types.as_deref().is_some_and(|types| {
                matches!(types, [ty] if ty.tag == "#" && ty.qualifiers.is_empty()
                    && ty.reference.as_ref().is_some_and(|id| id.as_str() == "fc01b5df-97fe-449b-83d4-218a090e681e"))
            });
            uuid_attributes
                .entry(AttributeId::from(&descriptor.object_guid))
                .and_modify(|known| *known = false)
                .or_insert(known_uuid);
        }
        index.uuid_attributes = uuid_attributes
            .into_iter()
            .filter_map(|(id, known)| known.then_some(id))
            .collect();
        index
    };
    let fingerprint = || {
        snapshot_fingerprint(
            db_names,
            descriptors,
            schema,
            live_tables,
            objects,
            fields,
            values,
            &indexes,
        )
    };
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let (mut report, index, fingerprint) = if fields.len() >= 256 && workers >= 3 {
        std::thread::scope(|scope| {
            let report_worker = scope.spawn(report);
            let index_worker = scope.spawn(index);
            let fingerprint = fingerprint();
            (
                report_worker
                    .join()
                    .expect("resolution report worker panicked"),
                index_worker.join().expect("metadata index worker panicked"),
                fingerprint,
            )
        })
    } else if fields.len() >= 256 && workers == 2 {
        std::thread::scope(|scope| {
            let report_worker = scope.spawn(report);
            let index = index();
            let fingerprint = fingerprint();
            (
                report_worker
                    .join()
                    .expect("resolution report worker panicked"),
                index,
                fingerprint,
            )
        })
    } else {
        (report(), index(), fingerprint())
    };
    report.findings.extend(separator_findings);
    ResolvedMetadata {
        snapshot: MetadataSnapshot {
            facts,
            indexes,
            index,
            fingerprint,
        },
        report,
    }
}

#[allow(clippy::too_many_arguments)]
fn snapshot_fingerprint(
    db_names: &DbNames,
    descriptors: &[ConfigDescriptor],
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    objects: &[MetadataObject],
    fields: &[MetadataField],
    values: &[MetadataValue],
    indexes: &[IndexComparison],
) -> SnapshotFingerprint {
    struct Fingerprint([u64; 2]);

    impl Fingerprint {
        fn write_fragment(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0[0] = (self.0[0] ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
                self.0[1] = (self.0[1] ^ u64::from(byte))
                    .rotate_left(7)
                    .wrapping_mul(0x9e37_79b1_85eb_ca87);
            }
        }

        fn finish_value(&mut self) {
            self.0[0] ^= 0xff;
            self.0[1] ^= 0x9d;
        }

        fn write(&mut self, bytes: &[u8]) {
            self.write_fragment(bytes);
            self.finish_value();
        }

        fn write_debug(&mut self, value: &impl std::fmt::Debug) {
            use std::fmt::Write as _;

            write!(self, "{value:?}").expect("formatting into a fingerprint cannot fail");
            self.finish_value();
        }
    }

    impl std::fmt::Write for Fingerprint {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.write_fragment(value.as_bytes());
            Ok(())
        }
    }

    let mut state = Fingerprint([0xcbf2_9ce4_8422_2325, 0x6a09_e667_f3bc_c909]);
    for entry in db_names.entries() {
        state.write(entry.guid.as_str().as_bytes());
        state.write(entry.alias.as_bytes());
        state.write(&entry.number.to_le_bytes());
    }
    for descriptor in descriptors {
        state.write_debug(descriptor);
    }
    state.write_debug(schema);
    state.write_debug(&live_tables);
    state.write_debug(&objects);
    state.write_debug(&fields);
    state.write_debug(&values);
    state.write_debug(&indexes);
    SnapshotFingerprint(state.0)
}

#[allow(clippy::too_many_arguments)]
fn build_resolution_report(
    db_names: &DbNames,
    descriptors: &[ConfigDescriptor],
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    indexes: &[IndexComparison],
    extension_field_conflicts: &[(String, DbNameFieldConflict)],
    extension_restructure_anomalies: &[(String, String)],
    physical_tables: &PhysicalTableIndex,
) -> ResolutionReport {
    const KNOWN_COLUMN_TAGS: [&str; 7] = ["S", "N", "T", "B", "L", "R", "V"];
    let mut findings = Vec::new();
    findings.extend(
        extension_restructure_anomalies
            .iter()
            .map(
                |(extension, detail)| ResolutionFinding::MalformedExtensionRestructure {
                    extension: extension.clone(),
                    detail: detail.clone(),
                },
            ),
    );
    findings.extend(
        extension_field_conflicts
            .iter()
            .map(
                |(extension, conflict)| ResolutionFinding::ExtensionFieldNumberConflict {
                    extension: extension.clone(),
                    number: conflict.number,
                    base_guid: conflict.base_guid.clone(),
                    extension_guid: conflict.extension_guid.clone(),
                },
            ),
    );
    let descriptor_guids = descriptors
        .iter()
        .map(|descriptor| &descriptor.object_guid)
        .collect::<HashSet<_>>();
    let mut reported_missing = HashSet::new();
    for (entry, kind) in db_names.objects() {
        if !entry.guid.is_nil()
            && !kind.is_service()
            && !descriptor_guids.contains(&entry.guid)
            && reported_missing.insert(entry.guid.clone())
        {
            findings.push(ResolutionFinding::DescriptorMissing {
                guid: entry.guid.clone(),
                table: physical_tables
                    .table_for_entry(entry, kind)
                    .unwrap_or_else(|| format!("{}{}", kind.physical_prefix(), entry.number)),
            });
        }
        if !entry.guid.is_nil()
            && kind.is_service()
            && physical_tables.table_for_entry(entry, kind).is_none()
        {
            findings.push(ResolutionFinding::ServiceTableMappingMissing {
                guid: entry.guid.clone(),
                alias: entry.alias.clone(),
                number: entry.number,
            });
        }
    }

    let mut duplicate_guids = Vec::new();
    let mut seen_db_names = HashSet::new();
    for (entry, kind) in db_names.objects() {
        if !entry.guid.is_nil() && !kind.is_service() && !seen_db_names.insert(&entry.guid) {
            duplicate_guids.push(entry.guid.clone());
        }
    }
    let mut seen_descriptors = HashSet::new();
    for descriptor in descriptors {
        if !seen_descriptors.insert(&descriptor.object_guid) {
            duplicate_guids.push(descriptor.object_guid.clone());
        }
    }
    duplicate_guids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    duplicate_guids.dedup();
    findings.extend(
        duplicate_guids
            .into_iter()
            .map(|guid| ResolutionFinding::DuplicateGuid { guid }),
    );

    let live_names = live_tables
        .iter()
        .map(|table| table.name.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let declared_names = schema
        .tables
        .iter()
        .map(|table| table.physical_name().to_ascii_lowercase())
        .collect::<HashSet<_>>();
    findings.extend(schema.anomalies.iter().map(|anomaly| {
        ResolutionFinding::InvalidSchemaDeclaration {
            table: format!("_{}", anomaly.table),
            detail: anomaly.detail.clone(),
        }
    }));
    for table in &schema.tables {
        let physical = table.physical_name();
        if !live_names.contains(&physical.to_ascii_lowercase()) {
            findings.push(ResolutionFinding::TableNotLive {
                table: physical.clone(),
            });
        }
        for column in &table.columns {
            for column_type in &column.types {
                if !KNOWN_COLUMN_TAGS.contains(&column_type.tag.as_str()) {
                    findings.push(ResolutionFinding::UnknownColumnTag {
                        table: physical.clone(),
                        column: column.physical_name(),
                        tag: column_type.tag.clone(),
                    });
                } else if column_type.tag == "R" && column_type.reference_target.is_none() {
                    findings.push(ResolutionFinding::InvalidSchemaDeclaration {
                        table: physical.clone(),
                        detail: format!(
                            "reference column {} has no target table",
                            column.physical_name()
                        ),
                    });
                }
            }
        }
    }
    for table in live_tables {
        if !declared_names.contains(&table.name.to_ascii_lowercase()) {
            findings.push(ResolutionFinding::TableNotDeclared {
                table: table.name.clone(),
            });
        }
    }

    findings.extend(
        indexes
            .iter()
            .filter(|index| index.live_name.is_none() || !index.unique_matches)
            .map(|index| ResolutionFinding::IndexMismatch {
                table: index.table.clone(),
                index: index.declared_name.clone(),
            }),
    );
    ResolutionReport { findings }
}

fn index_live_tables(live_tables: &[LiveTable]) -> HashMap<String, usize> {
    let mut by_name = HashMap::with_capacity(live_tables.len());
    for (position, table) in live_tables.iter().enumerate() {
        by_name
            .entry(table.name.to_ascii_lowercase())
            .or_insert(position);
    }
    by_name
}

fn index_schema_field_targets(schema: &SchemaStorage) -> HashMap<String, String> {
    let mut targets = HashMap::<String, String>::new();
    for table in &schema.tables {
        for column in &table.columns {
            let Some(field) = canonical_field_base(&column.name) else {
                continue;
            };
            if let Some(target) = column
                .types
                .iter()
                .find_map(|column_type| column_type.reference_target.clone())
            {
                targets.entry(field.to_owned()).or_insert(target);
            }
        }
    }
    targets
}

fn index_schema_field_owners(schema: &SchemaStorage) -> HashMap<String, Vec<String>> {
    let mut owners = HashMap::<String, Vec<String>>::new();
    for table in &schema.tables {
        let mut fields_in_table = HashSet::new();
        for column in &table.columns {
            let Some(field) = canonical_field_base(&column.name) else {
                continue;
            };
            if fields_in_table.insert(field) {
                let physical = table.physical_name();
                let field_owners = owners.entry(field.to_owned()).or_default();
                field_owners.push(physical.clone());
                if let Some(base) = extension_table_base(&physical)
                    && !field_owners.iter().any(|owner| names_equal(owner, base))
                {
                    field_owners.push(base.to_owned());
                }
            }
        }
    }
    owners
}

fn extension_table_base(candidate: &str) -> Option<&str> {
    let position = candidate.rfind(['X', 'x'])?;
    let (prefix, suffix) = candidate.split_at(position);
    let extension_number = &suffix[1..];
    prefix
        .as_bytes()
        .last()
        .is_some_and(u8::is_ascii_digit)
        .then_some(())
        .filter(|()| {
            !extension_number.is_empty()
                && extension_number.bytes().all(|digit| digit.is_ascii_digit())
        })
        .map(|()| prefix)
}

fn index_live_fields(live_tables: &[LiveTable]) -> HashSet<String> {
    live_tables
        .iter()
        .flat_map(|table| &table.columns)
        .filter_map(|column| {
            let canonical = recase_postgres_identifier(&column.name);
            canonical
                .strip_prefix('_')
                .and_then(canonical_field_base)
                .map(str::to_owned)
        })
        .collect()
}

fn canonical_field_base(identifier: &str) -> Option<&str> {
    let base = identifier
        .split_once('_')
        .map_or(identifier, |(base, _)| base);
    // A non-balance field of an accounting register is stored per side:
    // `Fld414Dt` and `Fld414Ct` both belong to field 414.
    let base = base
        .strip_suffix("Dt")
        .or_else(|| base.strip_suffix("Ct"))
        .filter(|stripped| stripped.starts_with("Fld"))
        .unwrap_or(base);
    let number = base.strip_prefix("Fld")?;
    (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())).then_some(base)
}

fn build_metadata_index(
    objects: &[MetadataObject],
    fields: &[MetadataField],
    values: &[MetadataValue],
    descriptors: &[ConfigDescriptor],
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    live_table_by_name: &HashMap<String, usize>,
) -> MetadataIndex {
    let mut index = MetadataIndex {
        live_tables_by_name: live_table_by_name.clone(),
        ..MetadataIndex::default()
    };
    for (position, table) in live_tables.iter().enumerate() {
        if let Some(base) = extension_table_base(&table.name) {
            index
                .extension_variants_by_base
                .entry(base.to_ascii_lowercase())
                .or_default()
                .push(position);
        }
    }
    for variants in index.extension_variants_by_base.values_mut() {
        variants.sort_by_key(|position| live_tables[*position].name.to_ascii_lowercase());
    }
    for (position, table) in schema.tables.iter().enumerate() {
        index
            .schema_tables_by_name
            .entry(table.name.to_ascii_lowercase())
            .or_insert(position);
    }
    let mut query_fields = QueryFieldIndex::default();
    for (position, descriptor) in descriptors.iter().enumerate() {
        if descriptor.object_guid == descriptor.resource_guid {
            query_fields
                .own_descriptors
                .entry(ObjectId::from(&descriptor.object_guid))
                .or_insert(position);
        }
        query_fields
            .attribute_descriptors
            .entry(AttributeId::from(&descriptor.object_guid))
            .or_default()
            .push(position);
    }
    for (position, field) in fields.iter().enumerate() {
        for owner in &field.owner_tables {
            query_fields
                .by_table_number
                .entry((folded_name(owner), field.number))
                .or_insert(position);
        }
        if field.data_separator {
            query_fields.separators.push(position);
        }
        if field.extension_origin.is_some() {
            query_fields
                .extension_by_number
                .entry(field.number)
                .or_insert(position);
        }
        if field.extension_origin.is_none() {
            continue;
        }
        let logical = collapse_logical_fields([field.physical_name.as_str()])
            .into_iter()
            .next()
            .map_or_else(
                || normalize_logical_name(&field.physical_name),
                |field| field.name,
            );
        index.extension_field_names.insert(normalize_name(&logical));
        if let Some(target) = field.reference_target.as_deref() {
            index
                .extension_reference_targets
                .entry(field.number)
                .or_insert_with(|| target.to_owned());
        }
    }
    let separator_numbers = query_fields
        .separators
        .iter()
        .map(|position| fields[*position].number)
        .collect::<HashSet<_>>();
    for column in schema.tables.iter().flat_map(|table| &table.columns) {
        let name = column.name.to_ascii_lowercase();
        if let Some(number) = name.strip_prefix("fld").and_then(|n| n.parse::<u32>().ok())
            && separator_numbers.contains(&number)
            && name == format!("fld{number}")
        {
            query_fields
                .separator_storage
                .entry(number)
                .or_insert_with(|| column.types.first().map(|kind| kind.tag.clone()));
        }
    }
    index.query_fields = Arc::new(query_fields);
    let mut owners_by_table = HashMap::<String, ObjectId>::new();
    for (position, object) in objects.iter().enumerate() {
        let id = ObjectId::from(&object.guid);
        if object.kind.is_none_or(|kind| !kind.is_service()) {
            index.objects_by_id.insert(id, position);
        }
        if let (Some(kind), Some(name)) = (object.kind, object.name.as_deref()) {
            insert_slot(
                &mut index.objects_by_name,
                (kind, normalize_name(name)),
                position,
            );
        }
        if let Some(number) = object
            .number
            .filter(|_| object.kind.is_none_or(|kind| !kind.is_service()))
        {
            insert_slot(&mut index.objects_by_database_type, number, id);
        }
        if let Some(table) = object.physical_table.as_deref() {
            owners_by_table.insert(normalize_name(table), id);
            insert_slot(
                &mut index.objects_by_physical_table,
                normalize_physical_table_name(table),
                id,
            );
            if let Some(live) = live_table_by_name
                .get(&table.to_ascii_lowercase())
                .map(|position| &live_tables[*position])
            {
                for column in &live.columns {
                    let logical = collapse_logical_fields([column.name.as_str()])
                        .into_iter()
                        .next()
                        .map_or_else(|| normalize_logical_name(&column.name), |field| field.name);
                    if let Some(standard) =
                        StandardFieldId::from_name(normalize_standard_field_name(&logical))
                    {
                        index.standard_fields.insert((id, standard));
                    }
                }
            }
        }
    }
    for (position, field) in fields.iter().enumerate() {
        let id = AttributeId::from(&field.guid);
        insert_slot(&mut index.attributes_by_id, id, position);
        for owner_table in &field.owner_tables {
            if let Some(owner) = owners_by_table.get(&normalize_name(owner_table)) {
                if let Some(name) = field.name.as_deref() {
                    insert_slot(
                        &mut index.attributes_by_owner_name,
                        (*owner, normalize_name(name)),
                        position,
                    );
                }
            }
        }
    }
    for (position, value) in values.iter().enumerate() {
        insert_slot(
            &mut index.values_by_owner_name,
            (value.owner, normalize_name(&value.name)),
            position,
        );
    }
    index
}

fn insert_slot<K, V>(map: &mut HashMap<K, LookupSlot<V>>, key: K, value: V)
where
    K: std::hash::Hash + Eq,
    V: Copy + PartialEq,
{
    match map.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(LookupSlot::Unique(value));
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if !matches!(entry.get(), LookupSlot::Unique(existing) if *existing == value) {
                entry.insert(LookupSlot::Ambiguous);
            }
        }
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Canonical key for a physical table name: SchemaStorage spells tables
/// without the leading underscore that the live catalog uses.
fn normalize_physical_table_name(name: &str) -> String {
    let trimmed = name.trim();
    normalize_name(trimmed.strip_prefix('_').unwrap_or(trimmed))
}

fn infer_allowed_length(table: Option<&LiveTable>, column_name: &str) -> Option<AllowedLength> {
    table?
        .columns
        .iter()
        .find(|column| column.name.eq_ignore_ascii_case(column_name))
        .and_then(|column| AllowedLength::from_postgres_type(&column.data_type))
}

// Имена индексируются тем же ключом, что соответствует `names_equal`.
// Позиция строки сохраняет приоритет первого совпадения общего OR-предиката.
struct PhysicalTableIndex {
    physical_names: Vec<String>,
    by_name: HashMap<String, usize>,
    extra_dimensions: HashMap<u32, usize>,
}
impl PhysicalTableIndex {
    fn new(schema: &SchemaStorage) -> Self {
        let mut index = Self {
            physical_names: Vec::with_capacity(schema.tables.len()),
            by_name: HashMap::new(),
            extra_dimensions: HashMap::new(),
        };
        for (position, table) in schema.tables.iter().enumerate() {
            index.physical_names.push(table.physical_name());
            index
                .by_name
                .entry(folded_name(&table.name))
                .or_insert(position);
            if table
                .inline_name
                .as_deref()
                .is_some_and(|name| names_equal(name, &format!("ExtDim{}", table.number)))
            {
                index
                    .extra_dimensions
                    .entry(table.number)
                    .or_insert(position);
            }
        }
        index
    }
    fn schema_row(&self, physical_name: &str) -> Option<usize> {
        self.by_name
            .get(&folded_name(
                physical_name.strip_prefix('_').unwrap_or(physical_name),
            ))
            .copied()
    }
    fn table_for_entry(&self, entry: &DbNameEntry, kind: MetadataKind) -> Option<String> {
        if !kind.is_service() {
            return Some(format!("{}{}", kind.physical_prefix(), entry.number));
        }
        let alias = self.by_name.get(&folded_name(&entry.alias));
        let numbered = self
            .by_name
            .get(&folded_name(&format!("{}{}", entry.alias, entry.number)));
        let extra = (kind == MetadataKind::ExtraDimension)
            .then(|| self.extra_dimensions.get(&entry.number))
            .flatten();
        [alias, numbered, extra]
            .into_iter()
            .flatten()
            .min()
            .map(|position| self.physical_names[*position].clone())
    }
}

fn dependency_indexes(objects: &[MetadataObject]) -> (HashSet<String>, HashMap<String, usize>) {
    let mut physical = HashSet::new();
    let mut owners = HashMap::new();
    for (position, object) in objects.iter().enumerate() {
        if let Some(table) = object.physical_table.as_deref() {
            let key = folded_name(table);
            physical.insert(key.clone());
            if object.kind.is_some_and(|kind| !kind.is_service()) {
                owners.entry(key).or_insert(position);
            }
        }
    }
    (physical, owners)
}

// Исходный линейный поиск остаётся только эталоном эквивалентности в тестах.
#[cfg(test)]
fn plain_physical_table_for_entry(
    entry: &DbNameEntry,
    kind: MetadataKind,
    schema: &SchemaStorage,
) -> Option<String> {
    if !kind.is_service() {
        return Some(format!("{}{}", kind.physical_prefix(), entry.number));
    }
    let numbered = format!("{}{}", entry.alias, entry.number);
    let ext_dim_name = format!("ExtDim{}", entry.number);
    schema
        .tables
        .iter()
        .find(|table| {
            names_equal(&table.name, &entry.alias)
                || names_equal(&table.name, &numbered)
                || (kind == MetadataKind::ExtraDimension
                    && table.number == entry.number
                    && table
                        .inline_name
                        .as_deref()
                        .is_some_and(|name| names_equal(name, &ext_dim_name)))
        })
        .map(|table| table.physical_name())
}

fn compare_indexes(
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    live_table_by_name: &HashMap<String, usize>,
    db_names: &DbNames,
) -> Vec<IndexComparison> {
    let mut comparisons = Vec::new();
    for table in &schema.tables {
        let physical_table = table.physical_name();
        let live_table = live_table_by_name
            .get(&physical_table.to_ascii_lowercase())
            .map(|position| &live_tables[*position]);
        for index in &table.indexes {
            let logical_key = normalize_index_key(&index.columns, db_names);
            let matching = live_table.and_then(|table| {
                table.indexes.iter().find(|live| {
                    logical_keys_equal(&normalize_index_key(&live.columns, db_names), &logical_key)
                })
            });
            comparisons.push(IndexComparison {
                table: physical_table.clone(),
                declared_name: index.name.clone(),
                logical_key,
                live_name: matching.map(|index| index.name.clone()),
                unique_matches: matching.is_some_and(|live| live.unique == index.unique),
            });
        }
    }
    comparisons
}

fn logical_keys_equal(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

#[cfg(test)]
mod tests {
    use super::{
        AllowedLength, LiveColumn, LiveTable, index_live_fields, index_schema_field_owners,
        logical_keys_equal, parallel_ordered, parallel_ordered_with_workers,
    };
    use crate::metadata::{SchemaColumn, SchemaStorage, SchemaTable, recase_postgres_identifier};

    #[test]
    fn live_names_are_recased_only_at_the_catalog_boundary() {
        let table = LiveTable {
            name: "_reference2565".to_owned(),
            columns: Vec::new(),
            indexes: Vec::new(),
        };
        assert_eq!(recase_postgres_identifier(&table.name), "_Reference2565");
    }

    #[test]
    fn infers_fixed_and_variable_allowed_length_from_postgres_types() {
        assert_eq!(
            AllowedLength::from_postgres_type("mchar(9)"),
            Some(AllowedLength::Fixed)
        );
        assert_eq!(
            AllowedLength::from_postgres_type("character(12)"),
            Some(AllowedLength::Fixed)
        );
        assert_eq!(
            AllowedLength::from_postgres_type("mvarchar(25)"),
            Some(AllowedLength::Variable)
        );
        assert_eq!(AllowedLength::from_postgres_type("bytea"), None);
    }

    #[test]
    fn compares_arbitrary_postgres_names_against_schema_case_insensitively() {
        assert!(logical_keys_equal(
            &["UserIdHash".to_owned(), "ObjectKey".to_owned()],
            &["useridhash".to_owned(), "objectkey".to_owned()]
        ));
    }

    #[test]
    fn indexes_schema_field_owners_once_in_table_order() {
        let schema = SchemaStorage {
            tables: vec![
                schema_table("Reference1", &["Fld12", "Fld12_TYPE", "Fld123"]),
                schema_table("Document2", &["Fld12_RRRef"]),
            ],
            anomalies: Vec::new(),
        };

        let owners = index_schema_field_owners(&schema);
        assert_eq!(
            owners.get("Fld12").unwrap(),
            &["_Reference1".to_owned(), "_Document2".to_owned()]
        );
        assert_eq!(owners.get("Fld123").unwrap(), &["_Reference1".to_owned()]);
    }

    #[test]
    fn indexes_exact_and_compound_live_fields_without_prefix_collisions() {
        let live = vec![LiveTable {
            name: "_reference1".to_owned(),
            columns: ["_fld5", "_fld12_type", "_fld123", "_fld7oops"]
                .into_iter()
                .map(|name| LiveColumn {
                    name: name.to_owned(),
                    data_type: "bytea".to_owned(),
                })
                .collect(),
            indexes: Vec::new(),
        }];

        let fields = index_live_fields(&live);
        assert!(fields.contains("Fld5"));
        assert!(fields.contains("Fld12"));
        assert!(fields.contains("Fld123"));
        assert!(!fields.contains("Fld7"));
    }

    #[test]
    fn parallel_chunks_retain_input_order_despite_completion_order() {
        let input = (0..2048_usize).collect::<Vec<_>>();
        let (sender, receiver) = std::sync::mpsc::channel();
        let receiver = std::sync::Mutex::new(receiver);
        let output = parallel_ordered_with_workers(&input, 2, |value| {
            if *value == 0 {
                receiver.lock().unwrap().recv().unwrap();
            } else if *value == 2047 {
                sender.send(()).unwrap();
            }
            *value * 2
        });
        assert_eq!(
            output,
            input.iter().map(|value| value * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn small_field_inputs_use_the_same_ordered_projection() {
        let input = [1, 2, 3];
        assert_eq!(parallel_ordered(&input, |value| value + 10), [11, 12, 13]);
    }

    fn schema_table(name: &str, columns: &[&str]) -> SchemaTable {
        SchemaTable {
            name: name.to_owned(),
            number: 0,
            owner: None,
            inline_name: None,
            columns: columns
                .iter()
                .map(|name| SchemaColumn {
                    name: (*name).to_owned(),
                    types: Vec::new(),
                })
                .collect(),
            indexes: Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "resolution_indexes_tests.rs"]
mod resolution_indexes_tests;
