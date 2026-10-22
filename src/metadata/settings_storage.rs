//! Общие фиксированные имена уже поддержанных хранилищ настроек.

/// Фиксированный выбор приобретённого хранилища: произвольные таблицы недоступны.
#[derive(Clone, Copy)]
pub(crate) enum SettingsStorage {
    Common,
    System,
}

impl SettingsStorage {
    pub(crate) const ALL: [Self; 2] = [Self::Common, Self::System];

    pub(crate) const fn table_name(self) -> &'static str {
        match self {
            Self::Common => "_commonsettings",
            Self::System => "_systemsettings",
        }
    }
}
