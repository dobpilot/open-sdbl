//! Проекция физических индексов перед обычным разрешением выбранных ресурсов.

use super::*;

/// Разрешает явно переданные описатели, не создавая объекты из остального индекса.
/// Состав выбирается по GUID ресурсов, их вложенных описателей и владельцев
/// предопределённых значений. Связанные описатели передаёт вызывающая сторона.
/// Физические индексы заимствуются; копируются только выбранные таблицы и поля.
/// Как и основной резолвер, возвращает находки о неполных настройках, а не ошибку.
/// До компиляции запросов вызывающая сторона обязана проверить отчёт и приобрести
/// отсутствующие настройки разделения. Строгий помощник `open-sdbl-db` выполняет
/// эту проверку и возвращает GUID недостающих описателей.
#[must_use]
pub fn resolve_selected_metadata(
    db_names: &DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
) -> ResolvedMetadata {
    resolve_selected_with(
        db_names,
        descriptors,
        predefined_values,
        extensions,
        schema,
        live_tables,
        resolve_metadata_with_predefined_values_and_extensions,
    )
}

/// Разрешает факты объектной модели без финализации снимка запросов.
/// Использует тот же выбор физических данных, слияние расширений и нормализацию.
/// Строгая проверка отсутствующих разделителей выполняется в `open-sdbl-db`.
#[must_use]
pub fn resolve_selected_runtime_metadata(
    db_names: &DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
) -> RuntimeMetadataFacts {
    resolve_selected_with(
        db_names,
        descriptors,
        predefined_values,
        extensions,
        schema,
        live_tables,
        super::resolve_runtime_metadata_with_extensions,
    )
}

fn resolve_selected_with<T>(
    db_names: &DbNames,
    descriptors: Vec<ConfigDescriptor>,
    predefined_values: Vec<ConfigPredefinedValue>,
    extensions: Vec<ExtensionMetadata>,
    schema: &SchemaStorage,
    live_tables: &[LiveTable],
    finish: impl FnOnce(
        DbNames,
        Vec<ConfigDescriptor>,
        Vec<ConfigPredefinedValue>,
        Vec<ExtensionMetadata>,
        SchemaStorage,
        Vec<LiveTable>,
    ) -> T,
) -> T {
    let mut guids = HashSet::new();
    for descriptor in descriptors
        .iter()
        .chain(extensions.iter().flat_map(|ext| &ext.descriptors))
    {
        guids.insert(descriptor.resource_guid.clone());
        guids.insert(descriptor.object_guid.clone());
    }
    guids.extend(
        predefined_values
            .iter()
            .map(|value| value.owner_guid.clone()),
    );
    let sources = std::iter::once((db_names, schema))
        .chain(
            extensions
                .iter()
                .map(|extension| (&extension.db_names, &extension.schema)),
        )
        .collect::<Vec<_>>();
    let physical_indexes = sources
        .iter()
        .map(|(_, declarations)| PhysicalTableIndex::new(declarations))
        .collect::<Vec<_>>();
    let key = |name: &str| name.strip_prefix('_').unwrap_or(name).to_ascii_lowercase();
    let mut tables = HashSet::new();
    // У хранилищ настроек нет владельца Config. Сохраняем только поддержанные
    // физические таблицы до выбора полей, чтобы не потерять разделители.
    for storage in crate::metadata::settings_storage::SettingsStorage::ALL {
        let name = storage.table_name();
        if live_tables
            .iter()
            .any(|table| names_equal(&table.name, name))
            || sources
                .iter()
                .any(|(_, schema)| schema.table(name).is_some())
        {
            tables.insert(key(name));
        }
    }

    for ((names, _), physical) in sources.iter().zip(&physical_indexes) {
        for (entry, kind) in names
            .objects()
            .filter(|(entry, _)| guids.contains(&entry.guid))
        {
            if let Some(table) = physical
                .table_for_entry(entry, kind)
                .or_else(|| physical_indexes[0].table_for_entry(entry, kind))
            {
                tables.insert(key(&table));
            }
        }
    }
    loop {
        let before = tables.len();
        for (_, declarations) in &sources {
            for table in &declarations.tables {
                if table
                    .owner
                    .as_deref()
                    .is_some_and(|owner| tables.contains(&key(owner)))
                    || extension_table_base(&table.name)
                        .is_some_and(|base| tables.contains(&key(base)))
                {
                    tables.insert(key(&table.name));
                }
            }
        }
        if tables.len() == before {
            break;
        }
    }
    let selected_table = |name: &str| {
        tables.contains(&key(name))
            || extension_table_base(name).is_some_and(|base| tables.contains(&key(base)))
    };
    let mut fields = HashSet::new();
    for (_, declarations) in &sources {
        for table in declarations
            .tables
            .iter()
            .filter(|table| selected_table(&table.name))
        {
            for column in &table.columns {
                if let Some(number) = canonical_field_base(&column.name)
                    .and_then(|name| name.strip_prefix("Fld"))
                    .and_then(|number| number.parse::<u32>().ok())
                {
                    fields.insert(number);
                }
            }
        }
    }
    for table in live_tables
        .iter()
        .filter(|table| selected_table(&table.name))
    {
        for column in &table.columns {
            let canonical = recase_postgres_identifier(&column.name);
            if let Some(number) = canonical
                .strip_prefix('_')
                .and_then(canonical_field_base)
                .and_then(|name| name.strip_prefix("Fld"))
                .and_then(|number| number.parse::<u32>().ok())
            {
                fields.insert(number);
            }
        }
    }
    let project_names = |names: &DbNames, physical: &PhysicalTableIndex| {
        let field_guids = names
            .entries()
            .iter()
            .filter(|entry| entry.alias == "Fld" && fields.contains(&entry.number))
            .map(|entry| &entry.guid)
            .collect::<HashSet<_>>();
        names.select_entries(|entry| {
            guids.contains(&entry.guid)
                || (entry.alias == "Fld" && fields.contains(&entry.number))
                || (entry.alias == "DataSeparationUse" && field_guids.contains(&entry.guid))
                || MetadataKind::from_alias(&entry.alias)
                    .and_then(|kind| physical.table_for_entry(entry, kind))
                    .is_some_and(|table| selected_table(&table))
        })
    };
    let project_schema = |schema: &SchemaStorage| SchemaStorage {
        tables: schema
            .tables
            .iter()
            .filter(|table| selected_table(&table.name))
            .cloned()
            .collect(),
        anomalies: schema
            .anomalies
            .iter()
            .filter(|anomaly| selected_table(&anomaly.table))
            .cloned()
            .collect(),
    };
    let selected_names = project_names(db_names, &physical_indexes[0]);
    let selected_schema = project_schema(schema);
    let selected_live = live_tables
        .iter()
        .filter(|table| selected_table(&table.name))
        .cloned()
        .collect();
    // Наборы выбора уже принадлежат этому вызову и не заимствуют расширения.
    let extensions = extensions
        .into_iter()
        .zip(physical_indexes.into_iter().skip(1))
        .map(|(mut extension, physical)| {
            extension.db_names = project_names(&extension.db_names, &physical);
            extension.schema = project_schema(&extension.schema);
            extension
                .field_reference_targets
                .retain(|(number, _)| fields.contains(number));
            extension
        })
        .collect();
    finish(
        selected_names,
        descriptors,
        predefined_values,
        extensions,
        selected_schema,
        selected_live,
    )
}
