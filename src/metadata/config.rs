use std::str::FromStr;

use super::{Guid, MetadataError, Value, inflate_raw_deflate, parse_serialized};

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
    let value = parse_serialized(&decoded)?;
    let mut descriptors = Vec::new();
    collect_descriptors(&value, &resource_guid, None, &mut descriptors);
    Ok(descriptors)
}

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
    use super::{ConfigFieldPurpose, collect_descriptors, parse_config_descriptor, parse_synonyms};
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
}
