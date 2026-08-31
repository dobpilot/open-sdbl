use std::borrow::Cow;
use std::str::FromStr;

#[cfg(test)]
use super::Value;
use super::{Guid, MetadataError, inflate_raw_deflate};

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
            _ => None,
        }
    }
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
    let Ok(resource_guid) = Guid::from_str(file_name) else {
        return Ok(Vec::new());
    };
    let decoded = inflate_raw_deflate(compressed)?;
    parse_config_descriptors_streaming(&decoded, &resource_guid)
}

fn parse_config_descriptors_streaming(
    input: &[u8],
    resource_guid: &Guid,
) -> Result<Vec<ConfigDescriptor>, MetadataError> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let text = std::str::from_utf8(input)
        .map_err(|error| MetadataError::at(error.valid_up_to(), "metadata is not valid UTF-8"))?;
    ConfigParser::new(text, resource_guid).parse()
}

struct ConfigParser<'input, 'resource> {
    input: &'input str,
    offset: usize,
    resource_guid: &'resource Guid,
}

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
    Other,
}

impl<'input> ConfigCandidate<'input> {
    fn as_scalar(&self) -> Option<&SimpleValue<'input>> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::List(_) | Self::Other => None,
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
            Self::Scalar(_) | Self::Other => None,
        }
    }
}

impl<'input, 'resource> ConfigParser<'input, 'resource> {
    const fn new(input: &'input str, resource_guid: &'resource Guid) -> Self {
        Self {
            input,
            offset: 0,
            resource_guid,
        }
    }

    fn parse(mut self) -> Result<Vec<ConfigDescriptor>, MetadataError> {
        self.skip_whitespace();
        if self.offset == self.input.len() {
            return Err(MetadataError::at(0, "empty metadata serialization"));
        }
        let mut descriptors = Vec::new();
        self.value(&mut descriptors, None, 0)?;
        self.skip_whitespace();
        if self.offset != self.input.len() {
            return Err(MetadataError::at(
                self.offset,
                "unexpected trailing metadata",
            ));
        }
        Ok(descriptors
            .into_iter()
            .map(|projected| projected.descriptor)
            .collect())
    }

    fn value(
        &mut self,
        descriptors: &mut Vec<ProjectedConfigDescriptor>,
        inherited_purpose: Option<(ConfigFieldPurpose, usize)>,
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
            None => Err(MetadataError::at(self.offset, "unexpected end of metadata")),
        }
    }

    fn list(
        &mut self,
        descriptors: &mut Vec<ProjectedConfigDescriptor>,
        inherited_purpose: Option<(ConfigFieldPurpose, usize)>,
        depth: usize,
    ) -> Result<ConfigCandidate<'input>, MetadataError> {
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

        loop {
            self.skip_whitespace();
            match self.current() {
                None => {
                    return Err(MetadataError::at(self.offset, "unterminated metadata list"));
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
                    let candidate =
                        self.value(&mut descendant_descriptors, field_purpose, depth + 1)?;
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
                    return Err(MetadataError::at(
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

#[allow(clippy::too_many_arguments)]
fn record_config_candidate<'input>(
    candidate: ConfigCandidate<'input>,
    value_index: usize,
    simple_values: &mut Option<Vec<SimpleValue<'input>>>,
    window: &mut Vec<ConfigCandidate<'input>>,
    field_purpose: &mut Option<(ConfigFieldPurpose, usize)>,
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
            .and_then(ConfigFieldPurpose::from_collection)
    {
        *field_purpose = Some((purpose, depth));
        *local_purpose_found = true;
        for descriptor in descendant_descriptors {
            if descriptor
                .purpose_depth
                .is_none_or(|purpose_depth| purpose_depth < depth)
            {
                descriptor.descriptor.field_purpose = Some(purpose);
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
    field_purpose: Option<(ConfigFieldPurpose, usize)>,
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
            field_purpose: field_purpose.map(|(purpose, _)| purpose),
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
    inherited_purpose: Option<ConfigFieldPurpose>,
    descriptors: &mut Vec<ConfigDescriptor>,
) {
    let Value::List(values) = value else {
        return;
    };
    let field_purpose = values
        .iter()
        .take(2)
        .filter_map(Value::as_str)
        .find_map(ConfigFieldPurpose::from_collection)
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
            field_purpose,
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
        ConfigFieldPurpose, collect_descriptors, parse_config_descriptor,
        parse_config_descriptors_streaming, parse_synonyms,
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
