use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

#[cfg(test)]
use super::Value;
use super::{Guid, MetadataError, inflate_raw_deflate};

/// A physical-name entry from `Params.DBNames`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbNameEntry {
    /// Metadata or attribute GUID.
    pub guid: Guid,
    /// Platform alias such as `Reference` or `Fld`.
    pub alias: String,
    /// Numeric suffix assigned by the platform.
    pub number: u32,
}

/// A supported tabular 1C metadata kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataKind {
    /// Catalog (`Справочник`).
    Catalog,
    /// Document (`Документ`).
    Document,
    /// Enumeration (`Перечисление`).
    Enumeration,
    /// Information register (`РегистрСведений`).
    InformationRegister,
    /// Accumulation register (`РегистрНакопления`).
    AccumulationRegister,
    /// Accounting register (`РегистрБухгалтерии`).
    AccountingRegister,
    /// Calculation register (`РегистрРасчета`).
    CalculationRegister,
    /// Chart of characteristic types (`ПланВидовХарактеристик`).
    ChartOfCharacteristicTypes,
    /// Chart of calculation types (`ПланВидовРасчета`).
    ChartOfCalculationTypes,
    /// Chart of accounts (`ПланСчетов`).
    ChartOfAccounts,
    /// Constant (`Константа`).
    Constant,
    /// Exchange plan (`ПланОбмена`).
    ExchangePlan,
    /// Business process (`БизнесПроцесс`).
    BusinessProcess,
    /// Task (`Задача`).
    Task,
    /// Sequence (`Последовательность`).
    Sequence,
}

impl MetadataKind {
    /// Resolves a DBNames alias to a tabular metadata kind.
    #[must_use]
    pub fn from_alias(alias: &str) -> Option<Self> {
        match alias {
            "Reference" => Some(Self::Catalog),
            "Document" => Some(Self::Document),
            "Enum" => Some(Self::Enumeration),
            "InfoRg" => Some(Self::InformationRegister),
            "AccumRg" => Some(Self::AccumulationRegister),
            "AccRg" => Some(Self::AccountingRegister),
            "CRg" => Some(Self::CalculationRegister),
            "Chrc" => Some(Self::ChartOfCharacteristicTypes),
            "CKinds" => Some(Self::ChartOfCalculationTypes),
            "Acc" => Some(Self::ChartOfAccounts),
            "Const" => Some(Self::Constant),
            "Node" => Some(Self::ExchangePlan),
            "BPr" => Some(Self::BusinessProcess),
            "Task" => Some(Self::Task),
            "Seq" => Some(Self::Sequence),
            _ => None,
        }
    }

    /// Returns the exact DBNames alias for the main object table.
    #[must_use]
    pub const fn alias(self) -> &'static str {
        match self {
            Self::Catalog => "Reference",
            Self::Document => "Document",
            Self::Enumeration => "Enum",
            Self::InformationRegister => "InfoRg",
            Self::AccumulationRegister => "AccumRg",
            Self::AccountingRegister => "AccRg",
            Self::CalculationRegister => "CRg",
            Self::ChartOfCharacteristicTypes => "Chrc",
            Self::ChartOfCalculationTypes => "CKinds",
            Self::ChartOfAccounts => "Acc",
            Self::Constant => "Const",
            Self::ExchangePlan => "Node",
            Self::BusinessProcess => "BPr",
            Self::Task => "Task",
            Self::Sequence => "Seq",
        }
    }

    /// Returns the canonical physical table prefix, including the underscore.
    #[must_use]
    pub const fn physical_prefix(self) -> &'static str {
        match self {
            Self::Catalog => "_Reference",
            Self::Document => "_Document",
            Self::Enumeration => "_Enum",
            Self::InformationRegister => "_InfoRg",
            Self::AccumulationRegister => "_AccumRg",
            Self::AccountingRegister => "_AccRg",
            Self::CalculationRegister => "_CRg",
            Self::ChartOfCharacteristicTypes => "_Chrc",
            Self::ChartOfCalculationTypes => "_CKinds",
            Self::ChartOfAccounts => "_Acc",
            Self::Constant => "_Const",
            Self::ExchangePlan => "_Node",
            Self::BusinessProcess => "_BPr",
            Self::Task => "_Task",
            Self::Sequence => "_Seq",
        }
    }

    /// Returns a stable English kind name used by the CLI.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "Catalog",
            Self::Document => "Document",
            Self::Enumeration => "Enumeration",
            Self::InformationRegister => "InformationRegister",
            Self::AccumulationRegister => "AccumulationRegister",
            Self::AccountingRegister => "AccountingRegister",
            Self::CalculationRegister => "CalculationRegister",
            Self::ChartOfCharacteristicTypes => "ChartOfCharacteristicTypes",
            Self::ChartOfCalculationTypes => "ChartOfCalculationTypes",
            Self::ChartOfAccounts => "ChartOfAccounts",
            Self::Constant => "Constant",
            Self::ExchangePlan => "ExchangePlan",
            Self::BusinessProcess => "BusinessProcess",
            Self::Task => "Task",
            Self::Sequence => "Sequence",
        }
    }
}

impl fmt::Display for MetadataKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Parsed DBNames entries and their derived field/separator indexes.
#[derive(Debug, Clone)]
pub struct DbNames {
    entries: Vec<DbNameEntry>,
    fields: HashMap<u32, Guid>,
    separators: HashSet<u32>,
}

impl DbNames {
    /// Returns every valid entry in source order, including platform-owned ones.
    #[must_use]
    pub fn entries(&self) -> &[DbNameEntry] {
        &self.entries
    }

    /// Returns the metadata GUID assigned to a numbered `Fld` entry.
    #[must_use]
    pub fn field_guid(&self, number: u32) -> Option<&Guid> {
        self.fields.get(&number)
    }

    /// Returns whether a numbered `Fld` belongs to a data-separation GUID.
    #[must_use]
    pub fn is_data_separator(&self, number: u32) -> bool {
        self.separators.contains(&number)
    }

    /// Iterates recognized main-table entries.
    pub fn objects(&self) -> impl Iterator<Item = (&DbNameEntry, MetadataKind)> {
        self.entries
            .iter()
            .filter_map(|entry| MetadataKind::from_alias(&entry.alias).map(|kind| (entry, kind)))
    }
}

/// Decodes and parses the authoritative `Params.DBNames` raw-DEFLATE blob.
///
/// # Errors
///
/// Returns [`MetadataError`] when decompression or serialization fails, or no
/// valid `{GUID,"Alias",Number}` entries are present.
pub fn parse_db_names(compressed: &[u8]) -> Result<DbNames, MetadataError> {
    let decoded = inflate_raw_deflate(compressed)?;
    let entries = parse_db_name_entries(&decoded)?;
    if entries.is_empty() {
        return Err(MetadataError::new(
            "DBNames contains no valid GUID/alias/number entries",
        ));
    }

    let separator_guids: HashSet<Guid> = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.alias.as_str(),
                "DataSeparationUse" | "DataSeparationHolder"
            )
        })
        .map(|entry| entry.guid.clone())
        .collect();
    let mut fields = HashMap::new();
    let mut separators = HashSet::new();
    for entry in &entries {
        if entry.alias == "Fld" {
            fields.insert(entry.number, entry.guid.clone());
            if separator_guids.contains(&entry.guid) {
                separators.insert(entry.number);
            }
        }
    }
    Ok(DbNames {
        entries,
        fields,
        separators,
    })
}

fn parse_db_name_entries(input: &[u8]) -> Result<Vec<DbNameEntry>, MetadataError> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let text = std::str::from_utf8(input)
        .map_err(|error| MetadataError::at(error.valid_up_to(), "metadata is not valid UTF-8"))?;
    DbNamesParser::new(text).parse()
}

struct DbNamesParser<'input> {
    input: &'input str,
    offset: usize,
}

enum Candidate<'input> {
    Atom(&'input str),
    String(Cow<'input, str>),
    Other,
}

impl<'input> Candidate<'input> {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Atom(value) => Some(value),
            Self::String(value) => Some(value.as_ref()),
            Self::Other => None,
        }
    }

    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_ref()),
            Self::Atom(_) | Self::Other => None,
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Atom(value) => value.parse().ok(),
            Self::String(_) | Self::Other => None,
        }
    }
}

impl<'input> DbNamesParser<'input> {
    const fn new(input: &'input str) -> Self {
        Self { input, offset: 0 }
    }

    fn parse(mut self) -> Result<Vec<DbNameEntry>, MetadataError> {
        self.skip_whitespace();
        if self.offset == self.input.len() {
            return Err(MetadataError::at(0, "empty metadata serialization"));
        }
        let mut entries = Vec::new();
        self.value(&mut entries)?;
        self.skip_whitespace();
        if self.offset != self.input.len() {
            return Err(MetadataError::at(
                self.offset,
                "unexpected trailing metadata",
            ));
        }
        Ok(entries)
    }

    fn value(
        &mut self,
        entries: &mut Vec<DbNameEntry>,
    ) -> Result<Candidate<'input>, MetadataError> {
        match self.current() {
            Some(b'{') => self.list(entries),
            Some(b'"') => self.string().map(Candidate::String),
            Some(b',' | b'}') => Ok(Candidate::Other),
            Some(_) => self.atom().map(Candidate::Atom),
            None => Err(MetadataError::at(self.offset, "unexpected end of metadata")),
        }
    }

    fn list(&mut self, entries: &mut Vec<DbNameEntry>) -> Result<Candidate<'input>, MetadataError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut candidates = [None, None, None];
        let mut value_count = 0usize;
        let mut expecting_value = true;

        loop {
            self.skip_whitespace();
            match self.current() {
                None => {
                    return Err(MetadataError::at(self.offset, "unterminated metadata list"));
                }
                Some(b'}') => {
                    if expecting_value && value_count != 0 {
                        value_count += 1;
                    }
                    self.offset += 1;
                    break;
                }
                Some(b',') if expecting_value => {
                    value_count += 1;
                    self.offset += 1;
                }
                Some(b',') => {
                    self.offset += 1;
                    expecting_value = true;
                }
                Some(_) if expecting_value => {
                    let candidate = self.value(entries)?;
                    if value_count < candidates.len() {
                        candidates[value_count] = Some(candidate);
                    }
                    value_count += 1;
                    expecting_value = false;
                }
                Some(_) => {
                    return Err(MetadataError::at(
                        self.offset,
                        "expected ',' or '}' in metadata list",
                    ));
                }
            }
        }

        if value_count == 3
            && let [Some(guid), Some(alias), Some(number)] = &candidates
            && let (Some(guid), Some(alias), Some(number)) =
                (guid.as_str(), alias.as_string(), number.as_u32())
            && number > 0
            && let Ok(guid) = Guid::from_str(guid)
        {
            entries.push(DbNameEntry {
                guid,
                alias: alias.to_owned(),
                number,
            });
        }
        Ok(Candidate::Other)
    }

    fn string(&mut self) -> Result<Cow<'input, str>, MetadataError> {
        self.offset += 1;
        let mut segment_start = self.offset;
        let mut owned = None::<String>;
        loop {
            let Some(relative_quote) = self.input.as_bytes()[self.offset..]
                .iter()
                .position(|byte| *byte == b'"')
            else {
                return Err(MetadataError::at(
                    self.offset,
                    "unterminated metadata string",
                ));
            };
            let quote = self.offset + relative_quote;
            if self.input.as_bytes().get(quote + 1) == Some(&b'"') {
                let value = owned.get_or_insert_with(|| String::with_capacity(relative_quote + 16));
                value.push_str(&self.input[segment_start..quote]);
                value.push('"');
                self.offset = quote + 2;
                segment_start = self.offset;
                continue;
            }
            self.offset = quote + 1;
            return if let Some(mut value) = owned {
                value.push_str(&self.input[segment_start..quote]);
                Ok(Cow::Owned(value))
            } else {
                Ok(Cow::Borrowed(&self.input[segment_start..quote]))
            };
        }
    }

    fn atom(&mut self) -> Result<&'input str, MetadataError> {
        let start = self.offset;
        while self
            .current()
            .is_some_and(|byte| !matches!(byte, b',' | b'}' | b'{'))
        {
            self.offset += 1;
        }
        let atom = self.input[start..self.offset].trim();
        if atom.is_empty() {
            return Err(MetadataError::at(start, "empty metadata atom"));
        }
        Ok(atom)
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.current() {
                Some(byte) if byte.is_ascii_whitespace() => self.offset += 1,
                Some(byte) if !byte.is_ascii() => {
                    let character = self.input[self.offset..]
                        .chars()
                        .next()
                        .expect("current byte belongs to a valid UTF-8 character");
                    if !character.is_whitespace() {
                        break;
                    }
                    self.offset += character.len_utf8();
                }
                Some(_) | None => break,
            }
        }
    }

    fn current(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }
}

#[cfg(test)]
fn collect_entries(value: &Value, output: &mut Vec<DbNameEntry>) {
    let Value::List(values) = value else {
        return;
    };
    if let [guid, alias, number] = values.as_slice()
        && let (Some(guid), Some(alias), Some(number)) =
            (guid.as_str(), alias.as_string(), number.as_u32())
        && number > 0
        && let Ok(guid) = Guid::from_str(guid)
    {
        output.push(DbNameEntry {
            guid,
            alias: alias.to_owned(),
            number,
        });
    }
    for value in values {
        collect_entries(value, output);
    }
}

#[cfg(test)]
mod tests {
    use super::{MetadataKind, collect_entries, parse_db_name_entries, parse_db_names};
    use crate::metadata::{Guid, normalize_index_key, parse_serialized};
    use std::str::FromStr;

    #[test]
    fn collects_object_field_and_separator_entries() {
        let value = parse_serialized(
            br#"{3,{b56f25d2-72a9-4d80-8998-77ac3097c873,"Reference",2565},{03bd775a-e0a1-4205-82ce-6068e73ad134,"Fld",2566},{03bd775a-e0a1-4205-82ce-6068e73ad134,"DataSeparationUse",9}}"#,
        )
        .unwrap();
        let mut entries = Vec::new();
        collect_entries(&value, &mut entries);
        assert_eq!(entries.len(), 3);
        assert_eq!(
            MetadataKind::from_alias(&entries[0].alias),
            Some(MetadataKind::Catalog)
        );
        assert_eq!(entries[0].number, 2565);
        assert_eq!(
            entries[1].guid,
            Guid::from_str("03bd775a-e0a1-4205-82ce-6068e73ad134").unwrap()
        );
    }

    #[test]
    fn keeps_similar_metadata_kinds_distinct() {
        assert_eq!(
            MetadataKind::from_alias("AccRg").unwrap().physical_prefix(),
            "_AccRg"
        );
        assert_eq!(
            MetadataKind::from_alias("Acc").unwrap().physical_prefix(),
            "_Acc"
        );
        assert_eq!(
            MetadataKind::from_alias("CRg").unwrap().physical_prefix(),
            "_CRg"
        );
        assert_eq!(
            MetadataKind::from_alias("Chrc").unwrap().physical_prefix(),
            "_Chrc"
        );
        assert_eq!(
            MetadataKind::from_alias("CKinds")
                .unwrap()
                .physical_prefix(),
            "_CKinds"
        );
    }

    #[test]
    fn indexes_fields_and_removes_data_separators() {
        let compressed = hex(
            "8dcb410a03210c00c0bf7836a0899a782f7d404b1f90d52c14cab66cf7b6f8f7fa84c25ce7247f4e8196ce9c152c6884842183603328a18831698f94bcbbbebaf35884c6bfe3a287deeda3bb1ecff7f6f89af371e6259715734760d40aa94b00a95580591b85ca4d98bcbbd96abb6d6d26cc258f317e",
        );
        let db_names = parse_db_names(&compressed).unwrap();
        assert!(db_names.is_data_separator(2683));
        assert_eq!(
            db_names.field_guid(2683).unwrap().as_str(),
            "03bd775a-e0a1-4205-82ce-6068e73ad134"
        );
        assert_eq!(
            normalize_index_key(["_Fld2683", "_IDRRef"], &db_names),
            ["ID"]
        );
    }

    #[test]
    fn rejects_a_decoded_map_without_entries() {
        let compressed = hex("ab36d0a936a8ad0500");
        assert!(parse_db_names(&compressed).is_err());
    }

    #[test]
    fn streaming_projection_matches_the_generic_tree_and_validates_all_input() {
        let source = br#"{4,"ignored ""text""",{b56f25d2-72a9-4d80-8998-77ac3097c873,"Reference",2565},{{03bd775a-e0a1-4205-82ce-6068e73ad134,"Fld",2566}},}"#;
        let value = parse_serialized(source).unwrap();
        let mut generic = Vec::new();
        collect_entries(&value, &mut generic);
        assert_eq!(parse_db_name_entries(source).unwrap(), generic);

        let malformed = br#"{1,{"unterminated}"#;
        let error = parse_db_name_entries(malformed).unwrap_err();
        assert_eq!(error.message(), "unterminated metadata string");
        assert!(error.offset().is_some());
    }

    fn hex(input: &str) -> Vec<u8> {
        input
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
}
