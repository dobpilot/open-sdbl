//! Повторное разрешение уже разобранных неизменяемых ресурсов.

use crate::pipeline::ConfigDecodeLimits;
use crate::{ConfigResource, ConfigurationMetadataInputs, DbError, Limits, check_retention_totals};
use open_sdbl::metadata::{
    ExtensionMetadata, Guid, ParsedConfigResource, ResolvedMetadata, parse_config_resource_bounded,
};
use std::collections::{HashMap, HashSet};

/// Накопленные факты выбранных ресурсов без повторного разбора их байтов.
/// Возвращаемые снимки владеют своими фактами, поэтому разрешение копирует
/// разобранные данные, но не распаковывает старые ресурсы. Сжатые буферы
/// остаются у вызывающей стороны; кэш не удерживает ни их копии, ни `Arc`.
#[derive(Debug)]
pub struct ConfigurationMetadataCache {
    limits: Limits,
    names: HashMap<String, usize>,
    resource_guids: HashSet<Guid>,
    facts: Vec<ParsedConfigResource>,
    compressed_bytes: usize,
    decoded_bytes: usize,
}
impl ConfigurationMetadataCache {
    /// Создаёт пустой кэш с обычными пределами числа и сжатых байтов ресурсов.
    /// Пределы распаковки совпадают с обычным декодером выбранного Config.
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            names: HashMap::new(),
            resource_guids: HashSet::new(),
            facts: Vec::new(),
            compressed_bytes: 0,
            decoded_bytes: 0,
        }
    }

    /// Принимает один новый неизменяемый ресурс и разбирает его ровно один раз.
    /// При отказе уже принятые факты и оставшиеся бюджеты сохраняются.
    ///
    /// # Errors
    /// Возвращает ошибку повторного имени, превышения обычных пределов числа,
    /// сжатых/распакованных байтов либо повреждённого ресурса. Замена принятого
    /// ресурса не поддерживается, даже если вызывающая сторона передала те же байты.
    pub fn admit(&mut self, resource: &ConfigResource) -> Result<(), DbError> {
        if self.names.contains_key(&resource.file_name) {
            return Err(DbError::Data(format!(
                "immutable Config resource {} is already admitted",
                resource.file_name
            )));
        }
        let compressed = self
            .compressed_bytes
            .checked_add(resource.compressed.len())
            .ok_or_else(|| DbError::Data("cached Config compressed byte count overflow".into()))?;
        check_retention_totals(self.limits, self.names.len() as u64 + 1, compressed as u64)?;
        let limits = ConfigDecodeLimits::default();
        let parsed = parse_config_resource_bounded(
            &resource.file_name,
            &resource.compressed,
            limits
                .resource_bytes
                .min(limits.total_bytes.saturating_sub(self.decoded_bytes)),
        )?;
        let decoded = self
            .decoded_bytes
            .checked_add(parsed.decoded_bytes)
            .filter(|size| *size <= limits.total_bytes)
            .ok_or_else(|| {
                DbError::Data("cached Config resources exceed decoded byte limit".into())
            })?;
        if let Ok(guid) = resource.file_name.parse() {
            self.resource_guids.insert(guid);
        }
        self.names
            .insert(resource.file_name.clone(), self.facts.len());
        self.facts.push(parsed);
        self.compressed_bytes = compressed;
        self.decoded_bytes = decoded;
        Ok(())
    }

    /// Заимствует исходные разобранные факты по точному имени принятого ресурса.
    /// Возвращает `None`, если имя не было успешно принято через `admit`.
    /// Не разбирает, не распаковывает и не копирует данные; разрешение снимка
    /// не изменяет эти факты. SQL и операции ввода-вывода не выполняются.
    #[must_use]
    pub fn parsed_resource(&self, file_name: &str) -> Option<&ParsedConfigResource> {
        self.names.get(file_name).map(|&index| &self.facts[index])
    }

    /// Разрешает все принятые факты тем же строгим путём, что и обычная функция.
    /// Зависимости приобретаются вызывающей стороной и допускаются через `admit`.
    /// Снимок и его fingerprint отражают весь накопленный состав; подготовленный
    /// запрос по-прежнему принадлежит своему неизменяемому снимку.
    ///
    /// # Errors
    /// Возвращает те же ошибки недостающих описателей и неподдержанных настроек,
    /// что и [`crate::resolve_configuration_resources`]. SQL не выполняется.
    pub fn resolve(
        &self,
        inputs: &ConfigurationMetadataInputs,
        extensions: Vec<ExtensionMetadata>,
    ) -> Result<ResolvedMetadata, DbError> {
        crate::metadata_inputs::resolve_parsed_configuration(
            inputs,
            self.facts.iter().cloned(),
            &self.resource_guids,
            extensions,
        )
    }
    /// Финализирует явно выбранные нормализованные факты без повторного разрешения.
    /// Состав определяется переданными фактами: позднее принятые ресурсы
    /// не добавляются к ним неявно. Исходные факты не изменяются.
    /// Распаковка, SQL и операции ввода-вывода не выполняются.
    ///
    /// # Errors
    /// Возвращает обычные ошибки недостающих или неподдержанных описателей
    /// разделения, как [`Self::resolve`].
    pub fn finalize_runtime(
        &self,
        facts: &open_sdbl::metadata::RuntimeMetadataFacts,
    ) -> Result<ResolvedMetadata, DbError> {
        crate::metadata_inputs::validate_separator_descriptors(facts, &self.resource_guids)?;
        Ok(facts.finalize_query())
    }

    /// Разрешает факты объектной модели без финализации снимка запросов.
    /// Использует принятые разобранные ресурсы и ту же нормализацию и строгую
    /// проверку разделителей; не строит обычные поля, индексы запросов,
    /// отчёт сравнения или fingerprint. SQL и ввод-вывод не выполняются.
    ///
    /// # Errors
    /// Возвращает те же ошибки отсутствующих описателей разделения и
    /// неподдержанных настроек, что и [`Self::resolve`].
    pub fn resolve_runtime(
        &self,
        inputs: &ConfigurationMetadataInputs,
        extensions: Vec<ExtensionMetadata>,
    ) -> Result<open_sdbl::metadata::RuntimeMetadataFacts, DbError> {
        crate::metadata_inputs::resolve_parsed_runtime_configuration(
            inputs,
            self.facts.iter().cloned(),
            &self.resource_guids,
            extensions,
        )
    }
}

#[cfg(test)]
#[path = "tests/metadata_cache.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/runtime_metadata_facts.rs"]
mod runtime_tests;
