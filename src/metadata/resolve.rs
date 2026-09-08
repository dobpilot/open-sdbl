use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;

use crate::names::{folded_name, names_equal};

use super::db_names::DbNameFieldConflict;
use super::normalize::{normalize_logical_name, normalize_standard_field_name};
use super::{
    AttributeId, ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue, DbNameEntry, DbNames,
    FieldId, Guid, LookupError, MetadataKind, ObjectId, SchemaStorage, StandardFieldId,
    collapse_logical_fields, normalize_index_key, recase_postgres_identifier,
};

#[derive(Debug, Clone, Copy)]
enum LookupSlot<T> {
    Unique(T),
    Ambiguous,
}

#[derive(Debug, Clone, Default)]
struct MetadataIndex {
    objects_by_id: HashMap<ObjectId, usize>,
    objects_by_name: HashMap<(MetadataKind, String), LookupSlot<usize>>,
    objects_by_database_type: HashMap<u32, LookupSlot<ObjectId>>,
    objects_by_physical_table: HashMap<String, LookupSlot<ObjectId>>,
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
    /// Descriptor marker from Config, when available.
    pub marker: Option<String>,
    /// Main numeric DBNames code, when tabular.
    pub number: Option<u32>,
    /// Canonical main physical table, when tabular.
    pub physical_table: Option<String>,
    /// Owning metadata object for a service table, when resolved.
    pub owner: Option<ObjectId>,
    /// Whether the table is declared by SchemaStorage.
    pub declared: bool,
    /// Whether the table exists in the live PostgreSQL catalog.
    pub live: bool,
    /// Allowed-length mode inferred from the live Code SQL column.
    pub code_allowed_length: Option<AllowedLength>,
    /// Allowed-length mode inferred from the live Number SQL column.
    pub number_allowed_length: Option<AllowedLength>,
}

/// One resolved custom field from a Fld DBNames entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataField {
    /// Attribute GUID.
    pub guid: Guid,
    /// Human metadata name from Config, when available.
    pub name: Option<String>,
    /// Semantic purpose from the enclosing Config collection, when recognized.
    pub purpose: Option<ConfigFieldPurpose>,
    /// Numeric Fld code.
    pub number: u32,
    /// Canonical physical base name such as _Fld2566.
    pub physical_name: String,
    /// Canonical SchemaStorage tables containing the field declaration.
    pub owner_tables: Vec<String>,
    /// Whether this field is a data separator.
    pub data_separator: bool,
    /// Whether SchemaStorage declares the field in at least one table.
    pub declared: bool,
    /// Whether at least one matching physical column exists in PostgreSQL.
    pub live: bool,
    /// Configuration-extension origin for an extension-added field.
    pub extension_origin: Option<String>,
    /// Canonical reference target for a reference-typed extension attribute.
    pub reference_target: Option<String>,
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
    /// Authoritative DBNames mapping.
    db_names: DbNames,
    /// Bare-GUID Config descriptors.
    descriptors: Vec<ConfigDescriptor>,
    /// Authoritative current physical schema.
    schema: SchemaStorage,
    /// Observed PostgreSQL catalog tables.
    live_tables: Vec<LiveTable>,
    /// Resolved metadata objects.
    objects: Vec<MetadataObject>,
    /// Resolved custom fields.
    fields: Vec<MetadataField>,
    /// Resolved enumeration and catalog predefined values.
    values: Vec<MetadataValue>,
    /// SchemaStorage indexes compared with the live catalog.
    indexes: Vec<IndexComparison>,
    index: MetadataIndex,
    fingerprint: SnapshotFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotFingerprint([u64; 2]);

impl MetadataSnapshot {
    /// Returns the authoritative DBNames mapping.
    #[must_use]
    pub const fn db_names(&self) -> &DbNames {
        &self.db_names
    }

    /// Returns Config descriptors in resolution source order.
    #[must_use]
    pub fn descriptors(&self) -> &[ConfigDescriptor] {
        &self.descriptors
    }

    /// Returns the authoritative physical schema declaration.
    #[must_use]
    pub const fn schema(&self) -> &SchemaStorage {
        &self.schema
    }

    /// Returns observed live tables in catalog order.
    #[must_use]
    pub fn live_tables(&self) -> &[LiveTable] {
        &self.live_tables
    }

    /// Returns resolved metadata objects in deterministic source order.
    #[must_use]
    pub fn objects(&self) -> &[MetadataObject] {
        &self.objects
    }

    /// Returns resolved custom fields in deterministic source order.
    #[must_use]
    pub fn fields(&self) -> &[MetadataField] {
        &self.fields
    }

    /// Returns resolved predefined values in deterministic source order.
    #[must_use]
    pub fn values(&self) -> &[MetadataValue] {
        &self.values
    }

    /// Returns SchemaStorage/live index comparisons.
    #[must_use]
    pub fn indexes(&self) -> &[IndexComparison] {
        &self.indexes
    }

    pub(crate) const fn fingerprint(&self) -> SnapshotFingerprint {
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
            Some(LookupSlot::Unique(index)) => Ok(ObjectId::from(&self.objects[*index].guid)),
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
            .map(|index| &self.objects[*index])
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
            Some(LookupSlot::Unique(index)) => Ok(AttributeId::from(&self.fields[*index].guid)),
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
                &self.fields[*index].guid,
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
            Some(LookupSlot::Unique(index)) => Ok(&self.fields[*index]),
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
            Some(LookupSlot::Unique(index)) => Ok(&self.values[*index]),
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

/// Resolves predefined values and caller-provided extension resources.
#[must_use]
pub fn resolve_metadata_with_predefined_values_and_extensions(
    mut db_names: DbNames,
    mut descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    mut schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> ResolvedMetadata {
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
    let live_table_by_name = index_live_tables(&live_tables);
    let indexes = compare_indexes(&schema, &live_tables, &live_table_by_name, &db_names);
    let report = build_resolution_report(
        &db_names,
        &descriptors,
        &schema,
        &live_tables,
        &indexes,
        &extension_field_conflicts,
        &extension_restructure_anomalies,
    );
    let schema_tables: HashSet<String> = schema
        .tables
        .iter()
        .map(|table| folded_name(&table.name))
        .collect();
    let schema_field_owners = index_schema_field_owners(&schema);
    let schema_field_targets = index_schema_field_targets(&schema);
    let live_fields = index_live_fields(&live_tables);
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
        let Some(physical_table) = physical_table_for_entry(entry, kind, &schema) else {
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
                    schema.table(table).and_then(|declaration| {
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
            if objects.iter().any(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|candidate| names_equal(candidate, &physical))
            }) {
                return None;
            }
            let owner = objects.iter().find(|object| {
                object.kind.is_some_and(|kind| !kind.is_service())
                    && object
                        .physical_table
                        .as_deref()
                        .is_some_and(|candidate| names_equal(candidate, &owner_table))
            })?;
            let live_table = live_table_by_name
                .get(&physical.to_ascii_lowercase())
                .map(|position| &live_tables[*position]);
            Some(MetadataObject {
                guid: owner.guid.clone(),
                kind: Some(MetadataKind::CalculationKindDependency),
                name: Some(name.to_owned()),
                marker: None,
                number: Some(table.number),
                physical_table: Some(physical),
                owner: Some(ObjectId::from(&owner.guid)),
                declared: true,
                live: live_table.is_some(),
                code_allowed_length: None,
                number_allowed_length: None,
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
                marker: Some(descriptor.marker.clone()),
                number: None,
                physical_table: None,
                declared: false,
                live: false,
                code_allowed_length: None,
                number_allowed_length: None,
                owner: None,
            });
        }
    }

    let mut fields = Vec::new();
    for entry in db_names
        .entries()
        .iter()
        .filter(|entry| entry.alias == "Fld")
    {
        let logical_name = format!("Fld{}", entry.number);
        let physical_name = format!("_{logical_name}");
        let owner_tables = schema_field_owners
            .get(&logical_name)
            .cloned()
            .unwrap_or_default();
        let live = live_fields.contains(&logical_name);
        fields.push(MetadataField {
            guid: entry.guid.clone(),
            name: descriptor_by_guid
                .get(&entry.guid)
                .map(|descriptor| descriptor.name.clone()),
            purpose: descriptor_by_guid
                .get(&entry.guid)
                .and_then(|descriptor| descriptor.field_purpose),
            number: entry.number,
            physical_name,
            declared: !owner_tables.is_empty(),
            live,
            owner_tables,
            data_separator: db_names.is_data_separator(entry.number),
            extension_origin: extension_origins.get(&entry.number).cloned(),
            reference_target: schema_field_targets
                .get(&logical_name)
                .or_else(|| extension_targets.get(&entry.number))
                .cloned(),
        });
    }

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
        let Some((owner, Some(MetadataKind::Catalog))) =
            object_ids.get(&predefined.owner_guid).copied()
        else {
            continue;
        };
        values.push(MetadataValue {
            owner,
            guid: predefined.value_guid,
            name: predefined.name,
        });
    }

    let index = build_metadata_index(
        &objects,
        &fields,
        &values,
        &live_tables,
        &live_table_by_name,
    );

    let fingerprint = snapshot_fingerprint(
        &db_names,
        &descriptors,
        &schema,
        &live_tables,
        &objects,
        &fields,
        &values,
        &indexes,
    );
    ResolvedMetadata {
        snapshot: MetadataSnapshot {
            db_names,
            descriptors,
            schema,
            live_tables,
            objects,
            fields,
            values,
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

fn build_resolution_report(
    db_names: &DbNames,
    descriptors: &[ConfigDescriptor],
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    indexes: &[IndexComparison],
    extension_field_conflicts: &[(String, DbNameFieldConflict)],
    extension_restructure_anomalies: &[(String, String)],
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
                table: physical_table_for_entry(entry, kind, schema)
                    .unwrap_or_else(|| format!("{}{}", kind.physical_prefix(), entry.number)),
            });
        }
        if !entry.guid.is_nil()
            && kind.is_service()
            && physical_table_for_entry(entry, kind, schema).is_none()
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
    let number = base.strip_prefix("Fld")?;
    (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())).then_some(base)
}

fn build_metadata_index(
    objects: &[MetadataObject],
    fields: &[MetadataField],
    values: &[MetadataValue],
    live_tables: &[LiveTable],
    live_table_by_name: &HashMap<String, usize>,
) -> MetadataIndex {
    let mut index = MetadataIndex::default();
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

fn physical_table_for_entry(
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
        logical_keys_equal,
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
