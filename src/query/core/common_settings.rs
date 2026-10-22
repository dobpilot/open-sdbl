//! Чтение достигнутых общих и системных настроек с полной областью разделения.

use std::collections::HashSet;

use super::{ColumnKind, ParameterValue, QueryDiagnostic, QueryDiagnosticKind, SqlDialect};
use crate::metadata::settings_storage::SettingsStorage;
use crate::metadata::{Guid, MetadataSnapshot};

/// Разрешённое хостом состояние одного разделителя общих настроек.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonSettingsSeparation {
    /// Разделитель включён с приобретённым значением.
    Active {
        /// Идентификатор общего реквизита.
        guid: Guid,
        /// Уже разрешённое строковое или числовое значение.
        value: ParameterValue,
    },
    /// Разделитель явно выключен: ложный флаг и типизированное пустое значение.
    Inactive {
        /// Идентификатор общего реквизита.
        guid: Guid,
    },
    /// Глобальное условие ложно: истинный флаг и типизированное пустое значение.
    /// Параметры использования и значения для этой области не нужны.
    ConditionDisabled {
        /// Идентификатор общего реквизита.
        guid: Guid,
    },
}

impl CommonSettingsSeparation {
    fn guid(&self) -> &Guid {
        match self {
            Self::Active { guid, .. }
            | Self::Inactive { guid }
            | Self::ConditionDisabled { guid } => guid,
        }
    }
}

fn failure(message: &str) -> QueryDiagnostic {
    QueryDiagnostic::at(QueryDiagnosticKind::UnsupportedFeature, None, message)
}

pub(crate) fn compile(
    metadata: &MetadataSnapshot,
    dialect: SqlDialect,
    storage: SettingsStorage,
    user: &str,
    object: &str,
    setting: &str,
    scope: &[CommonSettingsSeparation],
) -> Result<String, QueryDiagnostic> {
    if dialect != SqlDialect::Postgres {
        return Err(failure(
            "чтение общих настроек этого провайдера не поддержано",
        ));
    }
    let table_name = storage.table_name();
    let table = metadata
        .live_table(table_name)
        .ok_or_else(|| failure("системная таблица общих настроек не приобретена"))?;
    let column = |name: &str| {
        let mut matching = table
            .columns
            .iter()
            .filter(|column| column.name.eq_ignore_ascii_case(name));
        let found = matching
            .next()
            .ok_or_else(|| failure("колонка общих настроек не приобретена"))?;
        if matching.next().is_some() {
            return Err(failure("неоднозначная колонка общих настроек"));
        }
        Ok(found)
    };
    let data = column("_settingsdata")?;
    if data.data_type != "bytea" {
        return Err(failure("тип данных общих настроек не поддержан"));
    }
    let literal = |text: &str| {
        if text.contains('\0') {
            return Err(failure("нулевой символ в ключе или разделителе настройки"));
        }
        Ok(dialect.string_literal(text))
    };
    let mut predicates = Vec::new();
    for (name, value) in [
        ("_userid", user),
        ("_objectkey", object),
        ("_settingskey", setting),
    ] {
        let key_column = column(name)?;
        // Проверка нового хранилища не меняет прежний контракт общих настроек.
        if matches!(storage, SettingsStorage::System)
            && !matches!(
                ColumnKind::from_catalog_type(&key_column.data_type),
                ColumnKind::String { .. }
            )
        {
            return Err(failure("тип ключа системных настроек не поддержан"));
        }
        predicates.push(format!(
            "{} = {}",
            dialect.quote_identifier(&key_column.name),
            literal(value)?
        ));
        if name == "_userid" {
            // `mvarchar` сравнивает без учёта регистра, но имя области настроек
            // на платформе точное. Прежнее равенство сохраняет возможность
            // индексного отбора; проверка в `text` исключает соседнюю область.
            predicates.push(format!(
                "CAST({} AS text) COLLATE \"C\" = {}",
                dialect.quote_identifier(&key_column.name),
                literal(value)?
            ));
        }
    }
    let mut covered_flags = HashSet::new();
    let mut covered_guids = HashSet::new();
    for field in table
        .columns
        .iter()
        .filter(|column| column.name.to_ascii_lowercase().starts_with("_fld"))
    {
        let matching: Vec<_> = metadata
            .db_names()
            .entries()
            .iter()
            .filter(|entry| {
                entry.alias == "Fld"
                    && format!("_Fld{}", entry.number).eq_ignore_ascii_case(&field.name)
            })
            .collect();
        let [entry] = matching.as_slice() else {
            return Err(failure(
                "разделитель настройки не имеет однозначного DBNames",
            ));
        };
        if !covered_guids.insert(entry.guid.clone()) {
            return Err(failure("повторная колонка разделителя настройки"));
        }
        let uses: Vec<_> = metadata
            .db_names()
            .entries()
            .iter()
            .filter(|other| other.alias == "DataSeparationUse" && other.guid == entry.guid)
            .collect();
        let [usage] = uses.as_slice() else {
            return Err(failure(
                "флаг разделителя настройки не имеет однозначного DBNames",
            ));
        };
        let flag = column(&format!("_DataSeparationUse{}", usage.number))?;
        if flag.data_type != "boolean" {
            return Err(failure("тип флага разделителя настройки не поддержан"));
        }
        covered_flags.insert(flag.name.to_ascii_lowercase());
        let states: Vec<_> = scope
            .iter()
            .filter(|state| state.guid() == &entry.guid)
            .collect();
        let [state] = states.as_slice() else {
            return Err(failure(
                "состояние разделителя настройки отсутствует или неоднозначно",
            ));
        };
        let kind = ColumnKind::from_catalog_type(&field.data_type);
        let empty = match kind {
            ColumnKind::String { .. } => dialect.string_literal(""),
            ColumnKind::Number { .. } => "0".to_owned(),
            _ => return Err(failure("тип разделителя общих настроек не поддержан")),
        };
        let (active, value) = match state {
            CommonSettingsSeparation::Inactive { .. } => (false, empty),
            CommonSettingsSeparation::ConditionDisabled { .. } => (true, empty),
            CommonSettingsSeparation::Active { value, .. } => {
                let text = match (&kind, value) {
                    (ColumnKind::String { .. }, ParameterValue::String(value)) => literal(value)?,
                    (ColumnKind::Number { .. }, ParameterValue::Number { unscaled, scale }) => {
                        super::codegen::decimal_text(*unscaled, *scale)
                    }
                    _ => {
                        return Err(failure(
                            "значение не соответствует типу разделителя настройки",
                        ));
                    }
                };
                (true, text)
            }
        };
        predicates.push(format!(
            "{} = {}",
            dialect.quote_identifier(&flag.name),
            dialect.boolean_literal(active)
        ));
        predicates.push(format!(
            "{} = {}",
            dialect.quote_identifier(&field.name),
            value
        ));
    }
    if scope.len() != covered_guids.len()
        || table.columns.iter().any(|column| {
            column
                .name
                .to_ascii_lowercase()
                .starts_with("_dataseparationuse")
                && !covered_flags.contains(&column.name.to_ascii_lowercase())
        })
    {
        return Err(failure(
            "область разделителей общих настроек не подтверждена полностью",
        ));
    }
    Ok(format!(
        "SELECT {} FROM {} WHERE {} LIMIT 2",
        dialect.quote_identifier(&data.name),
        dialect.quote_identifier(&table.name),
        predicates.join(" AND ")
    ))
}
