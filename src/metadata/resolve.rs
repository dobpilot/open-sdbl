use std::collections::{HashMap, HashSet};

use super::{
    AttributeId, ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue, DbNames, FieldId,
    Guid, LookupError, MetadataKind, ObjectId, SchemaStorage, StandardFieldId,
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

/// Source records and the resulting metadata resolution.
#[derive(Debug, Clone)]
pub struct MetadataSnapshot {
    /// Authoritative DBNames mapping.
    pub db_names: DbNames,
    /// Bare-GUID Config descriptors.
    pub descriptors: Vec<ConfigDescriptor>,
    /// Authoritative current physical schema.
    pub schema: SchemaStorage,
    /// Observed PostgreSQL catalog tables.
    pub live_tables: Vec<LiveTable>,
    /// Resolved metadata objects.
    pub objects: Vec<MetadataObject>,
    /// Resolved custom fields.
    pub fields: Vec<MetadataField>,
    /// Resolved enumeration and catalog predefined values.
    pub values: Vec<MetadataValue>,
    /// SchemaStorage indexes compared with the live catalog.
    pub indexes: Vec<IndexComparison>,
    index: MetadataIndex,
}

impl MetadataSnapshot {
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
) -> MetadataSnapshot {
    resolve_metadata_with_predefined_values(db_names, descriptors, Vec::new(), schema, live_tables)
}

/// Resolves authoritative 1C resources including catalog predefined values.
#[must_use]
pub fn resolve_metadata_with_predefined_values(
    db_names: DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    schema: SchemaStorage,
    live_tables: Vec<LiveTable>,
) -> MetadataSnapshot {
    let live_table_by_name = index_live_tables(&live_tables);
    let schema_tables: HashSet<&str> = schema
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect();
    let schema_field_owners = index_schema_field_owners(&schema);
    let live_fields = index_live_fields(&live_tables);
    let descriptor_by_guid: HashMap<&Guid, &ConfigDescriptor> = descriptors
        .iter()
        .map(|descriptor| (&descriptor.object_guid, descriptor))
        .collect();
    let mut seen = HashSet::new();
    let mut objects = Vec::new();

    for (entry, kind) in db_names.objects() {
        if !seen.insert(entry.guid.clone()) {
            continue;
        }
        let physical_table = format!("{}{}", kind.physical_prefix(), entry.number);
        let descriptor = descriptor_by_guid.get(&entry.guid).copied();
        let live_table = live_table_by_name
            .get(&physical_table.to_ascii_lowercase())
            .map(|position| &live_tables[*position]);
        objects.push(MetadataObject {
            guid: entry.guid.clone(),
            kind: Some(kind),
            name: descriptor.map(|value| value.name.clone()),
            marker: descriptor.map(|value| value.marker.clone()),
            number: Some(entry.number),
            declared: schema_tables.contains(physical_table.trim_start_matches('_')),
            live: live_table.is_some(),
            code_allowed_length: infer_allowed_length(live_table, "_code"),
            number_allowed_length: infer_allowed_length(live_table, "_number"),
            physical_table: Some(physical_table),
        });
    }

    for descriptor in &descriptors {
        if descriptor.resource_guid == descriptor.object_guid
            && seen.insert(descriptor.object_guid.clone())
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
        });
    }

    let object_ids = objects
        .iter()
        .map(|object| {
            (
                object.guid.clone(),
                (ObjectId::from(&object.guid), object.kind),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut values = Vec::new();
    for descriptor in &descriptors {
        if descriptor.resource_guid == descriptor.object_guid {
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

    let indexes = compare_indexes(&schema, &live_tables, &live_table_by_name, &db_names);

    let index = build_metadata_index(
        &objects,
        &fields,
        &values,
        &live_tables,
        &live_table_by_name,
    );

    MetadataSnapshot {
        db_names,
        descriptors,
        schema,
        live_tables,
        objects,
        fields,
        values,
        indexes,
        index,
    }
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

fn index_schema_field_owners(schema: &SchemaStorage) -> HashMap<String, Vec<String>> {
    let mut owners = HashMap::<String, Vec<String>>::new();
    for table in &schema.tables {
        let mut fields_in_table = HashSet::new();
        for column in &table.columns {
            let Some(field) = canonical_field_base(&column.name) else {
                continue;
            };
            if fields_in_table.insert(field) {
                owners
                    .entry(field.to_owned())
                    .or_default()
                    .push(table.physical_name());
            }
        }
    }
    owners
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
        index.objects_by_id.insert(id, position);
        if let (Some(kind), Some(name)) = (object.kind, object.name.as_deref()) {
            insert_slot(
                &mut index.objects_by_name,
                (kind, normalize_name(name)),
                position,
            );
        }
        if let Some(number) = object.number {
            insert_slot(&mut index.objects_by_database_type, number, id);
        }
        if let Some(table) = object.physical_table.as_deref() {
            owners_by_table.insert(normalize_name(table), id);
            if let Some(live) = live_table_by_name
                .get(&table.to_ascii_lowercase())
                .map(|position| &live_tables[*position])
            {
                for column in &live.columns {
                    let logical = collapse_logical_fields([column.name.as_str()])
                        .into_iter()
                        .next()
                        .map_or_else(
                            || column.name.trim_start_matches('_').to_owned(),
                            |field| field.name,
                        );
                    let logical = if logical == "Date_Time" {
                        "Date"
                    } else {
                        &logical
                    };
                    if let Some(standard) = StandardFieldId::from_name(logical) {
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

fn infer_allowed_length(table: Option<&LiveTable>, column_name: &str) -> Option<AllowedLength> {
    table?
        .columns
        .iter()
        .find(|column| column.name.eq_ignore_ascii_case(column_name))
        .and_then(|column| AllowedLength::from_postgres_type(&column.data_type))
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
