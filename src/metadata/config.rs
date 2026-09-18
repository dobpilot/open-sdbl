use std::borrow::Cow;
use std::str::FromStr;

use super::{
    DEFAULT_OUTPUT_LIMIT, Guid, MetadataError, Value, ensure_serialized_depth,
    inflate_raw_deflate_bounded, parse_serialized,
};

/// Semantic role of a custom field declared by a recognized Config collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFieldPurpose {
    /// Information-register dimension.
    InformationRegisterDimension,
    /// Information-register resource.
    InformationRegisterResource,
    /// Information-register attribute.
    InformationRegisterAttribute,
    /// Accumulation-register dimension.
    AccumulationRegisterDimension,
    /// Accumulation-register resource.
    AccumulationRegisterResource,
    /// Accumulation-register attribute.
    AccumulationRegisterAttribute,
    /// Accounting-register dimension; [`ConfigDescriptor::balance`] tells a
    /// balance dimension from a non-balance one.
    AccountingRegisterDimension,
    /// Accounting-register resource; [`ConfigDescriptor::balance`] tells a
    /// balance resource from a non-balance one.
    AccountingRegisterResource,
    /// Accounting-register attribute.
    AccountingRegisterAttribute,
}

impl ConfigFieldPurpose {
    fn from_collection(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "13134203-f60b-11d5-a3c7-0050bae0a776" => Some(Self::InformationRegisterDimension),
            "13134202-f60b-11d5-a3c7-0050bae0a776" => Some(Self::InformationRegisterResource),
            "a2207540-1400-11d6-a3c7-0050bae0a776" => Some(Self::InformationRegisterAttribute),
            "b64d9a43-1642-11d6-a3c7-0050bae0a776" => Some(Self::AccumulationRegisterDimension),
            "b64d9a41-1642-11d6-a3c7-0050bae0a776" => Some(Self::AccumulationRegisterResource),
            "b64d9a42-1642-11d6-a3c7-0050bae0a776" => Some(Self::AccumulationRegisterAttribute),
            // Measured on 8.3.27 against the UNF register `Управленческий`.
            "35b63b9d-0adf-4625-a047-10ae874c19a3" => Some(Self::AccountingRegisterDimension),
            "63405499-7491-4ce3-ac72-43433cbe4112" => Some(Self::AccountingRegisterResource),
            "9d28ee33-9c7e-4a1b-8f13-50aa9b36607b" => Some(Self::AccountingRegisterAttribute),
            _ => None,
        }
    }
}

const ENUM_VALUES_COLLECTION_GUID: &str = "bee0a08c-07eb-40c0-8544-5c364c171465";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigCollectionPurpose {
    Field(ConfigFieldPurpose),
    EnumerationValue,
}

impl ConfigCollectionPurpose {
    fn from_collection(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case(ENUM_VALUES_COLLECTION_GUID) {
            Some(Self::EnumerationValue)
        } else {
            ConfigFieldPurpose::from_collection(value).map(Self::Field)
        }
    }
}

/// Class id of a common-attribute Config resource (`{1,{5,{27,…}}}`).
const COMMON_ATTRIBUTE_CLASS_ID: &str = "5";

/// How a data separator treats data written without a separator value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatedDataUse {
    /// `Независимо`: every read needs a separator value.
    Independent,
    /// `Независимо и совместно`: shared data lives under the empty value.
    IndependentAndShared,
}

/// Separation settings of a common attribute in `Разделять` mode, as
/// stored after the attribute's content list in its Config resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSeparationSettings {
    /// Separated-data-use mode.
    pub mode: SeparatedDataUse,
    /// Session parameter carrying the separator value, when bound.
    pub value_parameter: Option<Guid>,
    /// Session parameter carrying the use flag, when bound.
    pub use_parameter: Option<Guid>,
}

/// Tracks the `{1,<guid>}` references and the scalars that follow them in
/// the body list of a common-attribute resource.
#[derive(Default)]
struct SeparationTracker {
    references: Vec<Guid>,
    trailing: Vec<Option<u32>>,
    complete: bool,
}

impl SeparationTracker {
    const REFERENCES: usize = 3;
    const TRAILING: usize = 3;

    fn record(&mut self, candidate: &ConfigCandidate<'_>) {
        if self.complete {
            return;
        }
        if let Some(guid) = candidate.as_list().and_then(single_reference) {
            if self.references.len() < Self::REFERENCES {
                self.references.push(guid);
            } else {
                self.complete = true;
            }
            return;
        }
        if self.references.len() < Self::REFERENCES {
            self.references.clear();
            return;
        }
        match candidate.as_scalar() {
            Some(value) => {
                self.trailing
                    .push(value.as_str().and_then(|atom| atom.parse().ok()));
                if self.trailing.len() == Self::TRAILING {
                    self.complete = true;
                }
            }
            None => self.complete = true,
        }
    }

    fn settings(&self) -> Option<DataSeparationSettings> {
        if self.references.len() != Self::REFERENCES || self.trailing.len() != Self::TRAILING {
            return None;
        }
        let mode = match self.trailing[2]? {
            0 => SeparatedDataUse::Independent,
            1 => SeparatedDataUse::IndependentAndShared,
            _ => return None,
        };
        let bound = |guid: &Guid| (!guid.is_nil()).then(|| guid.clone());
        Some(DataSeparationSettings {
            mode,
            value_parameter: bound(&self.references[0]),
            use_parameter: bound(&self.references[1]),
        })
    }
}

fn single_reference(values: &[SimpleValue<'_>]) -> Option<Guid> {
    let [marker, guid] = values else {
        return None;
    };
    if marker.as_str() != Some("1") {
        return None;
    }
    let SimpleValue::Atom(guid) = guid else {
        return None;
    };
    Guid::from_str(guid).ok()
}

/// One localized synonym from a Config descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Synonym {
    /// Language code, for example `ru` or `en`.
    pub language: String,
    /// Localized presentation.
    pub text: String,
}

/// Human-readable data extracted from a bare-GUID Config resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDescriptor {
    /// GUID used as the bare Config `FileName`.
    pub resource_guid: Guid,
    /// GUID found in the descriptor self-reference.
    pub object_guid: Guid,
    /// First descriptor self-reference marker, normally `1`.
    pub marker: String,
    /// Configuration metadata name.
    pub name: String,
    /// Localized synonyms in source order.
    pub synonyms: Vec<Synonym>,
    /// Descriptor comment when present immediately after synonyms.
    pub comment: Option<String>,
    /// Field role established by a recognized enclosing Config collection.
    pub field_purpose: Option<ConfigFieldPurpose>,
    /// Whether the descriptor belongs to the authoritative enum-values collection.
    pub enumeration_value: bool,
    /// Separation settings of a common attribute in `Разделять` mode; `None`
    /// for every other descriptor and for a resource whose tail does not
    /// match the known layout.
    pub separation: Option<DataSeparationSettings>,
    /// Reference type identifiers the type description of this field names,
    /// in source order. SchemaStorage leaves the target of a reference that
    /// admits several tables unnamed, so this is where those targets come
    /// from. Empty for a descriptor that declares no reference type.
    pub reference_types: Vec<Guid>,
    /// The reference type identifier of the object the resource describes,
    /// carried by that object's own descriptor. It is the identifier an
    /// attribute of another object names to point at this one.
    pub object_reference_type: Option<Guid>,
    /// Whether an accounting-register dimension or resource is a balance
    /// one: the flag that follows the field's own block in its collection
    /// entry (`{6, {27, …}, 1, …}`), measured on 8.3.27 against the UNF
    /// register. `None` for every other descriptor.
    pub balance: Option<bool>,
    /// The chart of accounts an accounting register is bound to: the first
    /// identifier after the register's own header in its class list,
    /// measured on 8.3.27. `None` for every other descriptor.
    pub chart_of_accounts: Option<Guid>,
}

/// One catalog predefined value decoded from an authoritative `.1c` resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPredefinedValue {
    /// GUID of the owning object, taken from the resource file name.
    pub owner_guid: Guid,
    /// Stable predefined-value GUID stored in `_PredefinedID`.
    pub value_guid: Guid,
    /// Exact symbolic metadata name accepted by `ЗНАЧЕНИЕ`/`VALUE`.
    pub name: String,
    /// The resource the value was decoded from, which says the kind of
    /// object it may belong to.
    pub source: PredefinedSource,
}

/// The Config resource a predefined value comes from. A catalog keeps its
/// predefined items in `<guid>.1c`; a chart of accounts keeps its accounts
/// in `<guid>.9` (measured on 8.3.27 against the UNF chart
/// `Управленческий`). The suffixes of other classes carry other content,
/// so resolution keeps a value only for an object of the matching kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredefinedSource {
    /// A `<guid>.1c` resource of a catalog.
    Catalog,
    /// A `<guid>.9` resource of a chart of accounts.
    ChartOfAccounts,
    /// A `<guid>.7` resource of a chart of characteristic types (measured
    /// on the demo Бухгалтерия предприятия base on 8.3.27).
    ChartOfCharacteristicTypes,
}

/// One filter criterion decoded from its Config resource: the objects it
/// searches are the fields listed in its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigCriterion {
    /// GUID of the criterion itself.
    pub guid: Guid,
    /// Metadata name accepted after `КритерийОтбора.`.
    pub name: String,
    /// GUIDs of the fields the criterion searches, in declaration order.
    pub content: Vec<Guid>,
}

/// Parsed contents of one relevant Config resource plus its decoded size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedConfigResource {
    /// Object and field descriptors found in a bare-GUID resource.
    pub descriptors: Vec<ConfigDescriptor>,
    /// Predefined values found in a `<catalog-guid>.1c` resource.
    pub predefined_values: Vec<ConfigPredefinedValue>,
    /// The filter criterion this resource declares, if it declares one.
    pub criterion: Option<ConfigCriterion>,
    /// The role identifiers the roles collection of the configuration
    /// root lists, when this is the root resource; empty otherwise.
    pub roles: Vec<Guid>,
    /// Number of inflated bytes charged to the caller's total budget.
    pub decoded_bytes: usize,
}

/// Parses a bare-GUID, part-zero Config resource.
///
/// Returns `Ok(None)` for suffixed slots or for a valid resource without a
/// descriptor self-reference matching its file GUID.
///
/// # Errors
///
/// Returns [`MetadataError`] when `file_name` looks like a bare GUID resource
/// but its compressed or serialized content is malformed.
pub fn parse_config_descriptor(
    file_name: &str,
    compressed: &[u8],
) -> Result<Option<ConfigDescriptor>, MetadataError> {
    let resource_guid = Guid::from_str(file_name).ok();
    Ok(parse_config_descriptors(file_name, compressed)?
        .into_iter()
        .find(|descriptor| Some(&descriptor.object_guid) == resource_guid.as_ref()))
}

/// Parses every owner and nested descriptor from one bare-GUID Config resource.
///
/// Returns an empty vector for suffixed resource slots without decoding them.
///
/// # Errors
///
/// Returns [`MetadataError`] when a bare-GUID resource is malformed.
pub fn parse_config_descriptors(
    file_name: &str,
    compressed: &[u8],
) -> Result<Vec<ConfigDescriptor>, MetadataError> {
    if Guid::from_str(file_name).is_err() {
        return Ok(Vec::new());
    }
    Ok(parse_config_resource_bounded(file_name, compressed, DEFAULT_OUTPUT_LIMIT)?.descriptors)
}

/// Parses catalog predefined values from a part-zero `<catalog-guid>.1c`
/// Config resource.
///
/// Returns an empty vector for every other file-name shape without decoding
/// the payload. Only rows with the verified seven-column predefined-value
/// signature are projected.
///
/// # Errors
///
/// Returns [`MetadataError`] when a `.1c` resource is compressed or serialized
/// incorrectly.
pub fn parse_config_predefined_values(
    file_name: &str,
    compressed: &[u8],
) -> Result<Vec<ConfigPredefinedValue>, MetadataError> {
    if [".1c", ".9", ".7"]
        .iter()
        .all(|suffix| file_name.strip_suffix(suffix).is_none())
    {
        return Ok(Vec::new());
    }
    Ok(
        parse_config_resource_bounded(file_name, compressed, DEFAULT_OUTPUT_LIMIT)?
            .predefined_values,
    )
}

/// Parses the filter criterion a bare-GUID Config resource declares.
///
/// Returns `None` for every other resource without decoding it.
///
/// # Errors
///
/// Returns [`MetadataError`] when a bare-GUID resource is malformed.
pub fn parse_config_criterion(
    file_name: &str,
    compressed: &[u8],
) -> Result<Option<ConfigCriterion>, MetadataError> {
    if Guid::from_str(file_name).is_err() {
        return Ok(None);
    }
    Ok(parse_config_resource_bounded(file_name, compressed, DEFAULT_OUTPUT_LIMIT)?.criterion)
}

/// Inflates and parses one Config resource with an explicit decoded-byte cap.
///
/// Irrelevant suffixed resources return an empty result without decompression.
/// A relevant bare-GUID or `.1c` resource is inflated exactly once.
///
/// # Errors
///
/// Returns [`MetadataError`] when a relevant resource exceeds `decoded_limit`
/// or contains malformed compressed/serialized metadata.
pub fn parse_config_resource_bounded(
    file_name: &str,
    compressed: &[u8],
    decoded_limit: usize,
) -> Result<ParsedConfigResource, MetadataError> {
    enum ResourceKind {
        Descriptors(Guid),
        Predefined(Guid, PredefinedSource),
    }

    let kind = if let Ok(resource) = Guid::from_str(file_name) {
        ResourceKind::Descriptors(resource)
    } else if let Some(owner) = file_name.strip_suffix(".1c") {
        ResourceKind::Predefined(Guid::from_str(owner)?, PredefinedSource::Catalog)
    } else if let Some(owner) = file_name.strip_suffix(".9") {
        ResourceKind::Predefined(Guid::from_str(owner)?, PredefinedSource::ChartOfAccounts)
    } else if let Some(owner) = file_name.strip_suffix(".7") {
        ResourceKind::Predefined(
            Guid::from_str(owner)?,
            PredefinedSource::ChartOfCharacteristicTypes,
        )
    } else {
        return Ok(ParsedConfigResource {
            descriptors: Vec::new(),
            predefined_values: Vec::new(),
            criterion: None,
            roles: Vec::new(),
            decoded_bytes: 0,
        });
    };
    let decoded = inflate_raw_deflate_bounded(compressed, decoded_limit)?;
    let decoded_bytes = decoded.len();
    match kind {
        ResourceKind::Descriptors(resource) => Ok(ParsedConfigResource {
            descriptors: parse_config_descriptors_streaming(&decoded, &resource)?,
            predefined_values: Vec::new(),
            criterion: parse_criterion(&decoded, &resource),
            roles: super::roles::roles_collection(&decoded),
            decoded_bytes,
        }),
        ResourceKind::Predefined(owner, source) => {
            let value = parse_serialized(&decoded)?;
            let mut predefined_values = Vec::new();
            collect_predefined_values(&value, &owner, source, &mut predefined_values);
            Ok(ParsedConfigResource {
                descriptors: Vec::new(),
                predefined_values,
                criterion: None,
                roles: Vec::new(),
                decoded_bytes,
            })
        }
    }
}

/// The marker of a filter criterion in the serialized class list of a
/// Config resource, followed by its generated types, its own descriptor
/// and the content list.
const CRITERION_CLASS: u32 = 14;

/// The type identifier every content item of a filter criterion carries.
const METADATA_REFERENCE_TYPE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

/// Decodes the filter criterion a resource declares, if it declares one.
/// The content is the list of fields the criterion searches; each item is
/// a metadata reference to one attribute.
fn parse_criterion(decoded: &[u8], resource: &Guid) -> Option<ConfigCriterion> {
    let body = parse_serialized(decoded).ok()?;
    let outer = body.as_list()?;
    let class = outer.get(1)?.as_list()?;
    if class.first()?.as_u32()? != CRITERION_CLASS {
        return None;
    }
    let mut name = None;
    let mut content = Vec::new();
    for value in class {
        let Some(values) = value.as_list() else {
            continue;
        };
        if name.is_none()
            && let Some(found) = criterion_name(values, resource)
        {
            name = Some(found);
        }
        if content.is_empty() {
            content = criterion_content(values);
        }
    }
    Some(ConfigCriterion {
        guid: resource.clone(),
        name: name?,
        content,
    })
}

/// The criterion's own name, read from the descriptor that refers to the
/// resource GUID.
fn criterion_name(values: &[Value], resource: &Guid) -> Option<String> {
    let descriptor = values.get(1)?.as_list()?;
    let identity = descriptor.get(1)?.as_list()?;
    let Value::Atom(guid) = identity.get(2)? else {
        return None;
    };
    if !guid.eq_ignore_ascii_case(resource.as_str()) {
        return None;
    }
    Some(descriptor.get(2)?.as_string()?.to_owned())
}

/// The GUIDs of the fields a criterion searches.
fn criterion_content(values: &[Value]) -> Vec<Guid> {
    let mut content = Vec::new();
    for value in values {
        let Some(item) = value.as_list() else {
            continue;
        };
        if item.len() != 3 || item.first().and_then(Value::as_string) != Some("#") {
            continue;
        }
        let Value::Atom(kind) = &item[1] else {
            continue;
        };
        if !kind.eq_ignore_ascii_case(METADATA_REFERENCE_TYPE) {
            continue;
        }
        let Some(reference) = item[2].as_list() else {
            continue;
        };
        let Some(Value::Atom(guid)) = reference.get(1) else {
            continue;
        };
        if let Ok(guid) = Guid::from_str(guid)
            && !guid.is_nil()
        {
            content.push(guid);
        }
    }
    content
}

fn collect_predefined_values(
    value: &Value,
    owner_guid: &Guid,
    source: PredefinedSource,
    predefined: &mut Vec<ConfigPredefinedValue>,
) {
    let Value::List(values) = value else {
        return;
    };
    if let Some(projected) = project_predefined_value(values, owner_guid, source) {
        predefined.push(projected);
    }
    for value in values {
        collect_predefined_values(value, owner_guid, source, predefined);
    }
}

/// One row of a predefined-item table: `{2, <index>, <column count>,
/// {"#", <type>, {1, <guid>}}, <columns…>, <trailer>}`. A catalog's `.1c`
/// table has seven columns and names the item in its fourth; a chart of
/// accounts' `.9` table has ten and names the account in its second — in
/// both the name is the first string column after the reference.
fn project_predefined_value(
    values: &[Value],
    owner_guid: &Guid,
    source: PredefinedSource,
) -> Option<ConfigPredefinedValue> {
    if values.first()?.as_u32()? != 2 {
        return None;
    }
    // A row of a chart of accounts whose account has subaccounts carries
    // their rows in one more trailing element (measured on the demo
    // Бухгалтерия chart: `…, 1, {1, <count>, {2, <index>, 13, …}, …}}`).
    let column_count = values.get(2)?.as_u32()? as usize;
    if column_count < 2 || values.len() < column_count + 4 {
        return None;
    }
    let identifier = values.get(3)?.as_list()?;
    if identifier.first()?.as_string()? != "#" || identifier.len() != 3 {
        return None;
    }
    let reference = identifier.get(2)?.as_list()?;
    if reference.len() != 2 || reference.first()?.as_u32()? != 1 {
        return None;
    }
    let Value::Atom(guid) = reference.get(1)? else {
        return None;
    };
    let value_guid = Guid::from_str(guid).ok()?;
    if value_guid.is_nil() {
        return None;
    }
    let name = values[4..].iter().find_map(typed_string)?;
    if name.is_empty() {
        return None;
    }
    Some(ConfigPredefinedValue {
        owner_guid: owner_guid.clone(),
        value_guid,
        name: name.to_owned(),
        source,
    })
}

fn typed_string(value: &Value) -> Option<&str> {
    let values = value.as_list()?;
    if values.len() != 2 || values.first()?.as_string()? != "S" {
        return None;
    }
    values.get(1)?.as_string()
}

fn parse_config_descriptors_streaming(
    input: &[u8],
    resource_guid: &Guid,
) -> Result<Vec<ConfigDescriptor>, MetadataError> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let text = std::str::from_utf8(input)
        .map_err(|error| MetadataError::utf8(error.valid_up_to(), "metadata is not valid UTF-8"))?;
    ConfigParser::new(text, resource_guid).parse()
}

struct ConfigParser<'input, 'resource> {
    input: &'input str,
    offset: usize,
    resource_guid: &'resource Guid,
    /// Class id read from the first scalar of the depth-one list.
    class_id: Option<&'input str>,
    /// Separation settings found in a common-attribute body list.
    separation: Option<DataSeparationSettings>,
    /// Reference type of the object the resource describes: the third
    /// identifier of the class list, measured on 8.3.27 against the
    /// identifiers an attribute type description names.
    object_reference_type: Option<Guid>,
    /// Whether the header of the object has been read at depth one, after
    /// which the class list of an accounting register names its chart of
    /// accounts.
    header_seen: bool,
    /// The chart of accounts of an accounting-register resource.
    chart_of_accounts: Option<Guid>,
}

/// The class id an accounting-register resource opens with, measured on
/// 8.3.27 against the UNF register `Управленческий`.
const ACCOUNTING_REGISTER_CLASS_ID: &str = "21";
const ZERO_GUID: &str = "00000000-0000-0000-0000-000000000000";

struct ProjectedConfigDescriptor {
    descriptor: ConfigDescriptor,
    purpose_depth: Option<usize>,
}

#[derive(Clone)]
enum SimpleValue<'input> {
    Atom(&'input str),
    String(Cow<'input, str>),
    Null,
}

impl<'input> SimpleValue<'input> {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Atom(value) => Some(value),
            Self::String(value) => Some(value.as_ref()),
            Self::Null => None,
        }
    }

    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_ref()),
            Self::Atom(_) | Self::Null => None,
        }
    }
}

enum ConfigCandidate<'input> {
    Scalar(SimpleValue<'input>),
    List(Vec<SimpleValue<'input>>),
    /// A `{"Pattern", {"#", <id>}, …}` type description, reduced to the
    /// reference types it names.
    Pattern(Vec<Guid>),
    Other,
}

impl<'input> ConfigCandidate<'input> {
    fn as_scalar(&self) -> Option<&SimpleValue<'input>> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::List(_) | Self::Pattern(_) | Self::Other => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        self.as_scalar().and_then(SimpleValue::as_str)
    }

    fn as_string(&self) -> Option<&str> {
        self.as_scalar().and_then(SimpleValue::as_string)
    }

    fn as_list(&self) -> Option<&[SimpleValue<'input>]> {
        match self {
            Self::List(values) => Some(values),
            Self::Scalar(_) | Self::Pattern(_) | Self::Other => None,
        }
    }
}

impl<'input, 'resource> ConfigParser<'input, 'resource> {
    const fn new(input: &'input str, resource_guid: &'resource Guid) -> Self {
        Self {
            input,
            offset: 0,
            resource_guid,
            class_id: None,
            separation: None,
            object_reference_type: None,
            header_seen: false,
            chart_of_accounts: None,
        }
    }

    fn parse(mut self) -> Result<Vec<ConfigDescriptor>, MetadataError> {
        self.skip_whitespace();
        if self.offset == self.input.len() {
            return Err(MetadataError::serialization(
                self.offset,
                "empty metadata serialization",
            ));
        }
        let mut descriptors = Vec::new();
        self.value(&mut descriptors, None, 0)?;
        self.skip_whitespace();
        if self.offset != self.input.len() {
            return Err(MetadataError::serialization(
                self.offset,
                "unexpected trailing metadata",
            ));
        }
        let mut descriptors = descriptors
            .into_iter()
            .map(|projected| projected.descriptor)
            .collect::<Vec<_>>();
        if let Some(separation) = self.separation.take()
            && let Some(owner) = descriptors
                .iter_mut()
                .find(|descriptor| &descriptor.object_guid == self.resource_guid)
        {
            owner.separation = Some(separation);
        }
        if let Some(reference_type) = self.object_reference_type.take()
            && let Some(owner) = descriptors
                .iter_mut()
                .find(|descriptor| &descriptor.object_guid == self.resource_guid)
        {
            owner.object_reference_type = Some(reference_type);
        }
        if let Some(chart) = self.chart_of_accounts.take()
            && let Some(owner) = descriptors
                .iter_mut()
                .find(|descriptor| &descriptor.object_guid == self.resource_guid)
        {
            owner.chart_of_accounts = Some(chart);
        }
        Ok(descriptors)
    }

    fn value(
        &mut self,
        descriptors: &mut Vec<ProjectedConfigDescriptor>,
        inherited_purpose: Option<(ConfigCollectionPurpose, usize)>,
        depth: usize,
    ) -> Result<ConfigCandidate<'input>, MetadataError> {
        match self.current() {
            Some(b'{') => self.list(descriptors, inherited_purpose, depth),
            Some(b'"') => self
                .string()
                .map(|value| ConfigCandidate::Scalar(SimpleValue::String(value))),
            Some(b',' | b'}') => Ok(ConfigCandidate::Scalar(SimpleValue::Null)),
            Some(_) => self
                .atom()
                .map(|value| ConfigCandidate::Scalar(SimpleValue::Atom(value))),
            None => Err(MetadataError::serialization(
                self.offset,
                "unexpected end of metadata",
            )),
        }
    }

    fn list(
        &mut self,
        descriptors: &mut Vec<ProjectedConfigDescriptor>,
        inherited_purpose: Option<(ConfigCollectionPurpose, usize)>,
        depth: usize,
    ) -> Result<ConfigCandidate<'input>, MetadataError> {
        ensure_serialized_depth(depth, self.offset)?;
        self.offset += 1;
        self.skip_whitespace();
        let mut local_descriptors = Vec::new();
        let mut descendant_descriptors = Vec::new();
        let mut field_purpose = inherited_purpose;
        let mut local_purpose_found = false;
        let mut simple_values = Some(Vec::new());
        let mut window = Vec::with_capacity(4);
        let mut value_count = 0usize;
        let mut expecting_value = true;
        // The class list `{5,{27,…},{3,…},…}` of a common-attribute resource
        // carries the separation settings after its content list.
        let mut separation: Option<SeparationTracker> = None;
        // A `{"Pattern", …}` list describes the type of the field its
        // sibling descriptor names.
        let mut pattern: Option<Vec<Guid>> = None;
        // The entry of an accounting-register dimension or resource is
        // `{6, {27, …}, <balance>, …}`: two levels below the collection
        // list, the scalar after the field's own block is the balance flag.
        let accounting_entry = matches!(
            inherited_purpose,
            Some((
                ConfigCollectionPurpose::Field(
                    ConfigFieldPurpose::AccountingRegisterDimension
                        | ConfigFieldPurpose::AccountingRegisterResource
                ),
                collection_depth
            )) if depth == collection_depth + 2
        );
        let mut pending_balance: Option<usize> = None;

        loop {
            self.skip_whitespace();
            match self.current() {
                None => {
                    return Err(MetadataError::serialization(
                        self.offset,
                        "unterminated metadata list",
                    ));
                }
                Some(b'}') => {
                    if expecting_value && value_count != 0 {
                        record_config_candidate(
                            ConfigCandidate::Scalar(SimpleValue::Null),
                            value_count,
                            &mut simple_values,
                            &mut window,
                            &mut field_purpose,
                            &mut local_purpose_found,
                            depth,
                            self.resource_guid,
                            &mut local_descriptors,
                            &mut descendant_descriptors,
                        );
                    }
                    self.offset += 1;
                    break;
                }
                Some(b',') if expecting_value => {
                    record_config_candidate(
                        ConfigCandidate::Scalar(SimpleValue::Null),
                        value_count,
                        &mut simple_values,
                        &mut window,
                        &mut field_purpose,
                        &mut local_purpose_found,
                        depth,
                        self.resource_guid,
                        &mut local_descriptors,
                        &mut descendant_descriptors,
                    );
                    value_count += 1;
                    self.offset += 1;
                }
                Some(b',') => {
                    self.offset += 1;
                    expecting_value = true;
                }
                Some(_) if expecting_value => {
                    let descendants_before = descendant_descriptors.len();
                    let candidate =
                        self.value(&mut descendant_descriptors, field_purpose, depth + 1)?;
                    if let Some(index) = pending_balance.take() {
                        if let Some(atom) = candidate.as_str()
                            && let Some(owner) = descendant_descriptors.get_mut(index)
                        {
                            owner.descriptor.balance = Some(atom == "1");
                        }
                    } else if accounting_entry && descendant_descriptors.len() > descendants_before
                    {
                        pending_balance = Some(descendants_before);
                    }
                    // The class list of an accounting register names its
                    // chart of accounts right after the register's header.
                    if depth == 1 && self.class_id == Some(ACCOUNTING_REGISTER_CLASS_ID) {
                        if descendant_descriptors.len() > descendants_before {
                            self.header_seen = true;
                        } else if self.header_seen
                            && self.chart_of_accounts.is_none()
                            && let Some(guid) = candidate
                                .as_str()
                                .filter(|atom| *atom != ZERO_GUID)
                                .and_then(|atom| Guid::from_str(atom).ok())
                        {
                            self.chart_of_accounts = Some(guid);
                        }
                    }
                    // The class list opens with the class id and then the
                    // identifiers of the object; the third of them is the
                    // reference type an attribute of another object names.
                    if depth == 1 && value_count == 3 && self.object_reference_type.is_none() {
                        self.object_reference_type = candidate
                            .as_str()
                            .and_then(|atom| Guid::from_str(atom).ok());
                    }
                    if depth == 1 && value_count == 0 {
                        self.class_id = candidate.as_scalar().and_then(|value| match value {
                            SimpleValue::Atom(atom) => Some(*atom),
                            SimpleValue::String(_) | SimpleValue::Null => None,
                        });
                        if self.class_id == Some(COMMON_ATTRIBUTE_CLASS_ID)
                            && self.separation.is_none()
                        {
                            separation = Some(SeparationTracker::default());
                        }
                    } else if let Some(tracker) = &mut separation {
                        tracker.record(&candidate);
                    }
                    if value_count == 0 && candidate.as_string() == Some("Pattern") {
                        pattern = Some(Vec::new());
                    } else if let Some(types) = &mut pattern {
                        if let Some([marker, identifier]) = candidate.as_list()
                            && marker.as_string() == Some("#")
                            && let Some(guid) = identifier
                                .as_str()
                                .and_then(|atom| Guid::from_str(atom).ok())
                        {
                            types.push(guid);
                        }
                    } else if let ConfigCandidate::Pattern(types) = &candidate {
                        // The type description follows the descriptor of the
                        // field it belongs to.
                        if let Some(owner) = descendant_descriptors
                            .last_mut()
                            .or_else(|| local_descriptors.last_mut())
                        {
                            owner.descriptor.reference_types.clone_from(types);
                        }
                    }
                    record_config_candidate(
                        candidate,
                        value_count,
                        &mut simple_values,
                        &mut window,
                        &mut field_purpose,
                        &mut local_purpose_found,
                        depth,
                        self.resource_guid,
                        &mut local_descriptors,
                        &mut descendant_descriptors,
                    );
                    value_count += 1;
                    expecting_value = false;
                }
                Some(_) => {
                    return Err(MetadataError::serialization(
                        self.offset,
                        "expected ',' or '}' in metadata list",
                    ));
                }
            }
        }

        if window.len() == 3 {
            project_config_descriptor(
                &window[0],
                &window[1],
                &window[2],
                None,
                field_purpose,
                self.resource_guid,
                &mut local_descriptors,
            );
        }
        descriptors.append(&mut local_descriptors);
        descriptors.append(&mut descendant_descriptors);
        if let Some(settings) = separation.as_ref().and_then(SeparationTracker::settings) {
            self.separation = Some(settings);
        }
        if let Some(types) = pattern {
            return Ok(ConfigCandidate::Pattern(types));
        }
        Ok(simple_values.map_or(ConfigCandidate::Other, ConfigCandidate::List))
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
                return Err(MetadataError::serialization(
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
            return Err(MetadataError::serialization(start, "empty metadata atom"));
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

#[allow(clippy::too_many_arguments)]
fn record_config_candidate<'input>(
    candidate: ConfigCandidate<'input>,
    value_index: usize,
    simple_values: &mut Option<Vec<SimpleValue<'input>>>,
    window: &mut Vec<ConfigCandidate<'input>>,
    field_purpose: &mut Option<(ConfigCollectionPurpose, usize)>,
    local_purpose_found: &mut bool,
    depth: usize,
    resource_guid: &Guid,
    local_descriptors: &mut Vec<ProjectedConfigDescriptor>,
    descendant_descriptors: &mut [ProjectedConfigDescriptor],
) {
    if let Some(value) = candidate.as_scalar() {
        if let Some(values) = simple_values {
            values.push(value.clone());
        }
    } else {
        *simple_values = None;
    }

    if !*local_purpose_found
        && value_index < 2
        && let Some(purpose) = candidate
            .as_str()
            .and_then(ConfigCollectionPurpose::from_collection)
    {
        *field_purpose = Some((purpose, depth));
        *local_purpose_found = true;
        for descriptor in descendant_descriptors {
            if descriptor
                .purpose_depth
                .is_none_or(|purpose_depth| purpose_depth < depth)
            {
                descriptor.descriptor.field_purpose = match purpose {
                    ConfigCollectionPurpose::Field(purpose) => Some(purpose),
                    ConfigCollectionPurpose::EnumerationValue => None,
                };
                descriptor.descriptor.enumeration_value =
                    purpose == ConfigCollectionPurpose::EnumerationValue;
                descriptor.purpose_depth = Some(depth);
            }
        }
    }

    window.push(candidate);
    if window.len() == 4 {
        project_config_descriptor(
            &window[0],
            &window[1],
            &window[2],
            Some(&window[3]),
            *field_purpose,
            resource_guid,
            local_descriptors,
        );
        window.remove(0);
    }
}

fn project_config_descriptor(
    self_reference: &ConfigCandidate<'_>,
    name: &ConfigCandidate<'_>,
    synonyms: &ConfigCandidate<'_>,
    comment: Option<&ConfigCandidate<'_>>,
    field_purpose: Option<(ConfigCollectionPurpose, usize)>,
    resource_guid: &Guid,
    descriptors: &mut Vec<ProjectedConfigDescriptor>,
) {
    let Some([marker, zero, guid]) = self_reference.as_list() else {
        return;
    };
    if zero.as_str() != Some("0") {
        return;
    }
    let Some(guid) = guid.as_str() else {
        return;
    };
    let Ok(object_guid) = Guid::from_str(guid) else {
        return;
    };
    let Some(name) = name.as_string() else {
        return;
    };
    let Some(marker) = marker.as_str() else {
        return;
    };
    let synonyms = synonyms
        .as_list()
        .map(streaming_synonyms)
        .unwrap_or_default();
    let comment = comment
        .and_then(ConfigCandidate::as_string)
        .filter(|comment| !comment.is_empty())
        .map(str::to_owned);
    descriptors.push(ProjectedConfigDescriptor {
        descriptor: ConfigDescriptor {
            resource_guid: resource_guid.clone(),
            object_guid,
            marker: marker.to_owned(),
            name: name.to_owned(),
            synonyms,
            comment,
            field_purpose: field_purpose.and_then(|(purpose, _)| match purpose {
                ConfigCollectionPurpose::Field(purpose) => Some(purpose),
                ConfigCollectionPurpose::EnumerationValue => None,
            }),
            enumeration_value: field_purpose
                .is_some_and(|(purpose, _)| purpose == ConfigCollectionPurpose::EnumerationValue),
            separation: None,
            reference_types: Vec::new(),
            object_reference_type: None,
            balance: None,
            chart_of_accounts: None,
        },
        purpose_depth: field_purpose.map(|(_, depth)| depth),
    });
}

fn streaming_synonyms(values: &[SimpleValue<'_>]) -> Vec<Synonym> {
    values
        .get(1..)
        .unwrap_or_default()
        .chunks_exact(2)
        .filter_map(|pair| {
            Some(Synonym {
                language: pair[0].as_string()?.to_owned(),
                text: pair[1].as_string()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
fn collect_descriptors(
    value: &Value,
    resource_guid: &Guid,
    inherited_purpose: Option<ConfigCollectionPurpose>,
    descriptors: &mut Vec<ConfigDescriptor>,
) {
    let Value::List(values) = value else {
        return;
    };
    let field_purpose = values
        .iter()
        .take(2)
        .filter_map(Value::as_str)
        .find_map(ConfigCollectionPurpose::from_collection)
        .or(inherited_purpose);
    for (index, window) in values.windows(3).enumerate() {
        let Some(self_reference) = window.first().and_then(Value::as_list) else {
            continue;
        };
        let [marker, zero, guid] = self_reference else {
            continue;
        };
        if zero.as_str() != Some("0") {
            continue;
        }
        let Some(guid) = guid.as_str() else {
            continue;
        };
        let Ok(object_guid) = Guid::from_str(guid) else {
            continue;
        };
        let Some(name) = window.get(1).and_then(Value::as_string) else {
            continue;
        };
        let Some(marker) = marker.as_str() else {
            continue;
        };
        let comment = values
            .get(index + 3)
            .and_then(Value::as_string)
            .filter(|comment| !comment.is_empty())
            .map(str::to_owned);
        descriptors.push(ConfigDescriptor {
            resource_guid: resource_guid.clone(),
            object_guid,
            marker: marker.to_owned(),
            name: name.to_owned(),
            synonyms: parse_synonyms(&window[2]),
            comment,
            field_purpose: field_purpose.and_then(|purpose| match purpose {
                ConfigCollectionPurpose::Field(purpose) => Some(purpose),
                ConfigCollectionPurpose::EnumerationValue => None,
            }),
            enumeration_value: field_purpose == Some(ConfigCollectionPurpose::EnumerationValue),
            separation: None,
            reference_types: Vec::new(),
            object_reference_type: None,
            balance: None,
            chart_of_accounts: None,
        });
    }
    for value in values {
        collect_descriptors(value, resource_guid, field_purpose, descriptors);
    }
}

#[cfg(test)]
fn parse_synonyms(value: &Value) -> Vec<Synonym> {
    let Some(values) = value.as_list() else {
        return Vec::new();
    };
    values[1..]
        .chunks_exact(2)
        .filter_map(|pair| {
            Some(Synonym {
                language: pair[0].as_string()?.to_owned(),
                text: pair[1].as_string()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigFieldPurpose, PredefinedSource, collect_descriptors, collect_predefined_values,
        parse_config_descriptor, parse_config_descriptors_streaming,
        parse_config_predefined_values, parse_synonyms,
    };
    use crate::metadata::{Guid, parse_serialized};
    use std::str::FromStr;

    #[test]
    fn extracts_matching_name_synonyms_and_comment() {
        let guid = Guid::from_str("03bd775a-e0a1-4205-82ce-6068e73ad134").unwrap();
        let value = parse_serialized(
            r#"{1,{3,{1,0,03bd775a-e0a1-4205-82ce-6068e73ad134},"КоррСчет",{2,"ru","Корр. счет","en","Corr. account"},"Комментарий"}}"#
                .as_bytes(),
        )
        .unwrap();
        let mut descriptors = Vec::new();
        collect_descriptors(&value, &guid, None, &mut descriptors);
        let descriptor = &descriptors[0];
        assert_eq!(descriptor.name, "КоррСчет");
        assert_eq!(descriptor.synonyms.len(), 2);
        assert_eq!(descriptor.synonyms[0].text, "Корр. счет");
        assert_eq!(descriptor.comment.as_deref(), Some("Комментарий"));
    }

    #[test]
    fn rejects_pathologically_deep_config() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let input = format!("{}0{}", "{".repeat(5_000), "}".repeat(5_000));
        let error = parse_config_descriptors_streaming(input.as_bytes(), &owner).unwrap_err();
        assert_eq!(error.offset(), Some(512));
        assert!(
            error
                .message()
                .contains("nesting depth exceeds limit of 512")
        );
    }

    #[test]
    fn extracts_a_nested_attribute_from_its_owner_resource() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let value = parse_serialized(
            br#"{2,{{1,0,b8bac76b-c91b-4d78-8a70-ffa39f8de694},"Owner",{0}},{{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"ProbeAttribute",{1,"ru","Probe attribute"}}}"#,
        )
        .unwrap();
        let mut descriptors = Vec::new();
        collect_descriptors(&value, &owner, None, &mut descriptors);
        assert_eq!(descriptors.len(), 2);
        assert_eq!(descriptors[1].name, "ProbeAttribute");
        assert_eq!(descriptors[1].resource_guid, owner);
    }

    #[test]
    fn classifies_fields_from_information_register_collections() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let value = parse_serialized(
            br#"{13134203-f60b-11d5-a3c7-0050bae0a776,1,{{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"Dimension",{0}}}"#,
        )
        .unwrap();
        let mut descriptors = Vec::new();
        collect_descriptors(&value, &owner, None, &mut descriptors);
        assert_eq!(descriptors.len(), 1);
        assert_eq!(
            descriptors[0].field_purpose,
            Some(ConfigFieldPurpose::InformationRegisterDimension)
        );
    }

    #[test]
    fn classifies_accumulation_register_field_collections() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let cases = [
            (
                "b64d9a43-1642-11d6-a3c7-0050bae0a776",
                ConfigFieldPurpose::AccumulationRegisterDimension,
            ),
            (
                "b64d9a41-1642-11d6-a3c7-0050bae0a776",
                ConfigFieldPurpose::AccumulationRegisterResource,
            ),
            (
                "b64d9a42-1642-11d6-a3c7-0050bae0a776",
                ConfigFieldPurpose::AccumulationRegisterAttribute,
            ),
        ];
        for (collection, expected) in cases {
            let value = parse_serialized(
                format!(
                    "{{{collection},1,{{{{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b}},\"Field\",{{0}}}}}}"
                )
                .as_bytes(),
            )
            .unwrap();
            let mut descriptors = Vec::new();
            collect_descriptors(&value, &owner, None, &mut descriptors);
            assert_eq!(descriptors[0].field_purpose, Some(expected));
        }
    }

    #[test]
    fn marks_only_descriptors_from_the_enum_values_collection() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let source = br#"{
            {bee0a08c-07eb-40c0-8544-5c364c171465,1,
                {{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"RealValue",{0}}},
            {forms,1,
                {{1,0,03bd775a-e0a1-4205-82ce-6068e73ad134},"ListForm",{0}}}}
        "#;
        let value = parse_serialized(source).unwrap();
        let mut expected = Vec::new();
        collect_descriptors(&value, &owner, None, &mut expected);
        let actual = parse_config_descriptors_streaming(source, &owner).unwrap();

        assert_eq!(actual, expected);
        let real = actual
            .iter()
            .find(|descriptor| descriptor.name == "RealValue")
            .unwrap();
        let form = actual
            .iter()
            .find(|descriptor| descriptor.name == "ListForm")
            .unwrap();
        assert!(real.enumeration_value);
        assert!(!form.enumeration_value);
    }

    /// The shape of an accounting-register resource, trimmed from the UNF
    /// register `Управленческий` on 8.3.27: the class list opens with
    /// `21`, names the chart of accounts after the register's header, and
    /// each dimension or resource entry carries its balance flag after
    /// the field's own block.
    #[test]
    fn reads_accounting_register_balance_flags_and_chart() {
        let owner = Guid::from_str("7cbb9946-2e19-4797-8ed4-d7a0ece334bb").unwrap();
        let source = "{1,\n{21,4076244c-8b8e-4570-ba9d-a827d6fb70b5,e5b2e77d-518b-48e3-bdc9-061eb16665f8,68167ac0-494f-4264-8207-a9747c95ff63,9384bcb5-228b-40ae-a0ee-b4ff8c29977b,\n{0,\n{3,\n{1,0,7cbb9946-2e19-4797-8ed4-d7a0ece334bb},\"Управленческий\",\n{1,\"ru\",\"Журнал проводок\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\n},1,1,3b0c4744-6437-4ccc-bc96-9dd9bae7a7ae,00000000-0000-0000-0000-000000000000,1,1,0,0,\n{35b63b9d-0adf-4625-a047-10ae874c19a3,2,\n{\n{6,\n{27,\n{2,\n{3,\n{1,0,82bc429e-f433-4291-8ee0-32be4e838c9a},\"Организация\",\n{1,\"ru\",\"Организация\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\n{\"Pattern\",\n{\"#\",a1af1af2-f26f-40c9-a516-a66ff64531ed} } },0},1,00000000-0000-0000-0000-000000000000,0,1,1},0},\n{\n{6,\n{27,\n{2,\n{3,\n{1,0,99f211b1-3ccc-4a3b-b494-76926ac4bbf2},\"Валюта\",\n{1,\"ru\",\"Валюта\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\n{\"Pattern\",\n{\"#\",0f4ff832-736a-4115-a535-59166a4c1904} } },0},0,792ab2d9-7d4a-476c-bf3f-7ece60992129,0,0,1},0} },\n{63405499-7491-4ce3-ac72-43433cbe4112,1,\n{\n{2,\n{27,\n{2,\n{3,\n{1,0,d0b139bb-1ad7-4825-b238-e91efa2ba32b},\"Сумма\",\n{1,\"ru\",\"Сумма\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\n{\"Pattern\",\n{\"N\",15,2,0} } },0},1,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1},0} },\n{9d28ee33-9c7e-4a1b-8f13-50aa9b36607b,1,\n{\n{3,\n{27,\n{2,\n{3,\n{1,0,38f6382e-b801-4185-b096-eaf62c794186},\"Содержание\",\n{1,\"ru\",\"Содержание\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\n{\"Pattern\",\n{\"S\",150,1} } },0},0},0} } } }";
        let descriptors = parse_config_descriptors_streaming(source.as_bytes(), &owner).unwrap();
        let by_name = |name: &str| {
            descriptors
                .iter()
                .find(|descriptor| descriptor.name == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        assert_eq!(
            by_name("Управленческий").chart_of_accounts,
            Some(Guid::from_str("3b0c4744-6437-4ccc-bc96-9dd9bae7a7ae").unwrap())
        );
        let organization = by_name("Организация");
        assert_eq!(
            organization.field_purpose,
            Some(ConfigFieldPurpose::AccountingRegisterDimension)
        );
        assert_eq!(organization.balance, Some(true));
        let currency = by_name("Валюта");
        assert_eq!(
            currency.field_purpose,
            Some(ConfigFieldPurpose::AccountingRegisterDimension)
        );
        assert_eq!(currency.balance, Some(false));
        let amount = by_name("Сумма");
        assert_eq!(
            amount.field_purpose,
            Some(ConfigFieldPurpose::AccountingRegisterResource)
        );
        assert_eq!(amount.balance, Some(true));
        let content = by_name("Содержание");
        assert_eq!(
            content.field_purpose,
            Some(ConfigFieldPurpose::AccountingRegisterAttribute)
        );
        assert_eq!(content.balance, None);
        assert_eq!(content.chart_of_accounts, None);
    }

    /// Two rows of the `.9` table of the UNF chart `Управленческий` on
    /// 8.3.27: ten columns, the account's reference first and its name
    /// in the first string column.
    #[test]
    fn reads_predefined_accounts_from_a_dot9_resource() {
        let source = "{2, {1, {10, {0,\"\", {\"Pattern\", {\"#\",ae135932-4f94-44df-92c1-c91f15a92848} },\"\",0}, {1,\"\", {\"Pattern\", {\"S\"} },\"\",0} }, {2,10,0,0,1,1, {1,1, {2,0,7, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,00000000-0000-0000-0000-000000000000} }, {\"S\",\"Счета\"}, {\"S\",\"\"}, {\"S\",\"\"}, {\"N\",0}, {\"B\",0}, {\"U\"},1, {1,2, {2,1,10, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,ceacb23a-fb1f-4e1c-99a5-22726a63629b} }, {\"S\",\"Служебный\"}, {\"S\",\"00      \"}, {\"S\",\"Служебный\"}, {\"N\",2}, {\"B\",0}, {\"U\"}, {\"S\",\" 00\"}, {\"N\",0}, {\"B\",0},0}, {2,55,10, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,814aeeef-6b5f-495c-b297-f2ce3f8a7901} }, {\"S\",\"ПрочиеРасходы\"}, {\"S\",\"91.02   \"}, {\"S\",\"Прочие расходы\"}, {\"N\",0}, {\"B\",0}, {\"U\"}, {\"S\",\" 91.02\"}, {\"N\",0}, {\"B\",0},0} } } } } } }";
        let owner = Guid::from_str("3b0c4744-6437-4ccc-bc96-9dd9bae7a7ae").unwrap();
        let value = parse_serialized(source.as_bytes()).unwrap();
        let mut values = Vec::new();
        collect_predefined_values(
            &value,
            &owner,
            PredefinedSource::ChartOfAccounts,
            &mut values,
        );
        let names = values
            .iter()
            .map(|value| value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Служебный", "ПрочиеРасходы"]);
        assert!(
            values
                .iter()
                .all(|value| value.source == PredefinedSource::ChartOfAccounts
                    && value.owner_guid == owner)
        );
        assert_eq!(
            values[1].value_guid,
            Guid::from_str("814aeeef-6b5f-495c-b297-f2ce3f8a7901").unwrap()
        );
        // The root row names the folder `Счета` with a nil reference and
        // is not a value.
        assert_eq!(values.len(), 2);
    }

    /// A row of the demo Бухгалтерия chart whose account has subaccounts:
    /// the flag `1` after the columns is followed by the children's list,
    /// one element more than a leaf row carries.
    #[test]
    fn reads_an_account_with_subaccounts_and_its_children() {
        let source = "{2,124,10, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,d35af89e-5806-4c55-9c2e-6c19862fa8db} }, {\"S\",\"Касса\"}, {\"S\",\"50\"}, {\"S\",\"Касса\"}, {\"N\",0}, {\"B\",0}, {\"U\"}, {\"S\",\" 50\"}, {\"N\",0}, {\"B\",0},1, {1,1, {2,125,10, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,ceacb23a-fb1f-4e1c-99a5-22726a63629b} }, {\"S\",\"КассаОрганизации\"}, {\"S\",\"50.01\"}, {\"S\",\"Касса организации\"}, {\"N\",0}, {\"B\",0}, {\"U\"}, {\"S\",\" 50.01\"}, {\"N\",0}, {\"B\",0},0} } }";
        let owner = Guid::from_str("3796bdf5-5d0b-4232-b22e-6d2cd8beb488").unwrap();
        let value = parse_serialized(source.as_bytes()).unwrap();
        let mut values = Vec::new();
        collect_predefined_values(
            &value,
            &owner,
            PredefinedSource::ChartOfAccounts,
            &mut values,
        );
        let names = values
            .iter()
            .map(|value| value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Касса", "КассаОрганизации"]);
    }

    /// Two rows of the `.7` table of the chart of characteristic types
    /// `ВидыСубконтоХозрасчетные` of the demo Бухгалтерия предприятия base
    /// on 8.3.27: seven columns, the item's reference first, a boolean,
    /// then the name.
    #[test]
    fn reads_predefined_kinds_from_a_dot7_resource() {
        let source = "{1, {1, {7, {1,\"\", {\"Pattern\", {\"#\",ae135932-4f94-44df-92c1-c91f15a92848} },\"\",0}, {2,\"\", {\"Pattern\", {\"B\"} },\"\",0} }, {2,7,0,1,1,2,2, {1,1, {2,0,6, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,00000000-0000-0000-0000-000000000000} }, {\"B\",1}, {\"S\",\"Характеристики\"}, {\"S\",\"     \"}, {\"S\",\"\"}, {\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d, {\"Pattern\"} },1, {1,2, {2,23,7, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,ac901067-a86f-48d4-93e0-bc525fc3dbe0} }, {\"B\",0}, {\"S\",\"Контрагенты\"}, {\"S\",\"00005\"}, {\"S\",\"Контрагенты\"}, {\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d, {\"Pattern\", {\"#\",9f6206b2-1ed6-423c-9b08-fd4978930c49} } }, {\"N\",0},0}, {2,24,7, {\"#\",ae135932-4f94-44df-92c1-c91f15a92848, {1,1c19c5a6-6cc6-4f6d-a5f3-4d8f7a3d9e10} }, {\"B\",0}, {\"S\",\"Договоры\"}, {\"S\",\"00006\"}, {\"S\",\"Договоры\"}, {\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d, {\"Pattern\"} }, {\"N\",0},0} } } } } } }";
        let owner = Guid::from_str("46d9709b-4192-401b-bf2f-12b93eb1842b").unwrap();
        let value = parse_serialized(source.as_bytes()).unwrap();
        let mut values = Vec::new();
        collect_predefined_values(
            &value,
            &owner,
            PredefinedSource::ChartOfCharacteristicTypes,
            &mut values,
        );
        let names = values
            .iter()
            .map(|value| value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Контрагенты", "Договоры"]);
        assert_eq!(
            values[0].value_guid,
            Guid::from_str("ac901067-a86f-48d4-93e0-bc525fc3dbe0").unwrap()
        );
        assert!(
            values
                .iter()
                .all(|value| value.source == PredefinedSource::ChartOfCharacteristicTypes)
        );
    }

    #[test]
    fn parses_empty_synonyms() {
        let value = parse_serialized(br#"{0}"#).unwrap();
        assert!(parse_synonyms(&value).is_empty());
    }

    #[test]
    fn ignores_suffixed_config_slots_without_decoding_them() {
        let result =
            parse_config_descriptor("03bd775a-e0a1-4205-82ce-6068e73ad134.0", b"not deflate")
                .unwrap();
        assert!(result.is_none());
        assert!(
            parse_config_predefined_values(
                "03bd775a-e0a1-4205-82ce-6068e73ad134.3",
                b"not deflate"
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn projects_verified_catalog_predefined_rows() {
        let owner = Guid::from_str("bee248ca-acc6-46a7-899e-148a9d9e4729").unwrap();
        let value = parse_serialized(
            r##"{0,{2,249,7,{"#",ae135932-4f94-44df-92c1-c91f15a92848,{1,2e22ad88-32b5-4456-a3da-e56fa2f94623}},{"B",0},{"#",ae135932-4f94-44df-92c1-c91f15a92848,{1,00000000-0000-0000-0000-000000000000}},{"S","Утвержден"},{"S","000000175"},{"S","Утвержден"},{"N",0},0},{2,1,6,{"S","not a predefined row"}}}"##.as_bytes(),
        )
        .unwrap();
        let mut predefined = Vec::new();
        collect_predefined_values(&value, &owner, PredefinedSource::Catalog, &mut predefined);

        assert_eq!(predefined.len(), 1);
        assert_eq!(predefined[0].owner_guid, owner);
        assert_eq!(
            predefined[0].value_guid.as_str(),
            "2e22ad88-32b5-4456-a3da-e56fa2f94623"
        );
        assert_eq!(predefined[0].name, "Утвержден");
    }

    #[test]
    fn streaming_projection_matches_the_generic_tree_projection() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let source = r#"{b64d9a43-1642-11d6-a3c7-0050bae0a776,1,
            {{1,0,b8bac76b-c91b-4d78-8a70-ffa39f8de694},"Owner",{2,"ru","Владелец"},"Owner comment"},
            {wrapper,{{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"Field",{2,"ru","Поле ""quoted"""},"Field comment"}}}"#
            .as_bytes();

        let value = parse_serialized(source).unwrap();
        let mut expected = Vec::new();
        collect_descriptors(&value, &owner, None, &mut expected);

        let actual = parse_config_descriptors_streaming(source, &owner).unwrap();
        assert_eq!(actual, expected);

        let child_before_parent = br#"{
            {{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"Child",{0}},
            {1,0,b8bac76b-c91b-4d78-8a70-ffa39f8de694},"Parent",{0},"Parent comment"}"#;
        let value = parse_serialized(child_before_parent).unwrap();
        let mut expected = Vec::new();
        collect_descriptors(&value, &owner, None, &mut expected);
        let actual = parse_config_descriptors_streaming(child_before_parent, &owner).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual[0].name, "Parent");
        assert_eq!(actual[1].name, "Child");

        let nested_override = br#"{13134203-f60b-11d5-a3c7-0050bae0a776,
            {{{1,0,25c96bd3-fac4-42ef-b695-74c9af43589b},"LatePurpose",{0}},b64d9a41-1642-11d6-a3c7-0050bae0a776},
            {13134202-f60b-11d5-a3c7-0050bae0a776,b64d9a42-1642-11d6-a3c7-0050bae0a776,
                {{1,0,03bd775a-e0a1-4205-82ce-6068e73ad134},"FirstPurposeWins",{0}}}}"#;
        let value = parse_serialized(nested_override).unwrap();
        let mut expected = Vec::new();
        collect_descriptors(&value, &owner, None, &mut expected);
        let actual = parse_config_descriptors_streaming(nested_override, &owner).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual[0].field_purpose,
            Some(ConfigFieldPurpose::AccumulationRegisterResource)
        );
        assert_eq!(
            actual[1].field_purpose,
            Some(ConfigFieldPurpose::InformationRegisterResource)
        );
    }

    #[test]
    fn streaming_projection_rejects_truncated_input() {
        let owner = Guid::from_str("b8bac76b-c91b-4d78-8a70-ffa39f8de694").unwrap();
        let error =
            parse_config_descriptors_streaming(br#"{1,{"unterminated}"#, &owner).unwrap_err();
        assert!(error.to_string().contains("unterminated metadata string"));
    }
}
