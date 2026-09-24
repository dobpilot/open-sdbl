//! The structure a configuration extension declares: the classes and
//! objects of its configuration, and the header of each object it
//! describes, with the link an adopted object carries to the main
//! configuration.
//!
//! Both are read from resources of the extension itself — its root index,
//! its configuration root and its descriptions — never from the names the
//! content-addressed store gives its rows.

use std::str::FromStr;

use super::value::{Value, parse_serialized, parse_serialized_sequence};
use super::{Guid, MetadataError, MetadataErrorKind};

/// The classes and objects an extension's configuration declares.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCatalog {
    /// The identifier of the extension's configuration object.
    pub configuration: Guid,
    /// Every class the configuration lists, in stored order, each with the
    /// objects the extension declares in it — adopted and its own alike.
    /// A class listing no object is kept.
    pub classes: Vec<ExtensionClass>,
}

/// One class of metadata objects, as an extension's configuration lists it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionClass {
    /// The identifier of the class, such as
    /// `cf4abea6-37b2-11d4-940f-008048da11f9` for catalogs. The library
    /// does not name classes.
    pub class: Guid,
    /// The objects of the class, in stored order.
    pub objects: Vec<Guid>,
}

/// Whether an object belongs to the extension or is adopted from the main
/// configuration.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectBelonging {
    /// The extension creates the object.
    Own,
    /// The extension adopts an object of the main configuration.
    Adopted,
}

/// The header of one object an extension describes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionObject {
    /// The identifier the extension gives the object.
    pub id: Guid,
    /// The name of the object.
    pub name: String,
    /// Whether the object is the extension's own or adopted, or `None`
    /// when the header is not in the shape that was measured.
    pub belonging: Option<ObjectBelonging>,
    /// The identifier of the main-configuration object an adopted object
    /// extends, as its own header records it.
    ///
    /// `None` for an object of the extension's own, for a header not in the
    /// measured shape, and for an adopted object that records the nil
    /// identifier — an extension that maps its objects by name rather than
    /// by identifier writes that, and a name is not an explicit link.
    pub extends: Option<Guid>,
}

/// The identifier of the resource holding an extension's configuration
/// object, as the extension's root index names it.
///
/// The index carries, beside the name/key pairs, a record
/// `{2,<guid>,…}` whose identifier is that resource. Answers `None` when
/// no record has that shape.
///
/// # Errors
///
/// Returns [`MetadataError`] when the root is not the serialization.
pub fn extension_configuration_root(root: &[u8]) -> Result<Option<Guid>, MetadataError> {
    let records = parse_serialized_sequence(root)?;
    Ok(records.iter().find_map(|record| {
        let [version, configuration, _] = record.as_list()? else {
            return None;
        };
        if version.as_scalar()? != "2" {
            return None;
        }
        Guid::from_str(configuration.as_scalar()?).ok()
    }))
}

/// Decodes the configuration root of an extension into the classes and
/// objects it declares.
///
/// The resource is `{2,{<self>},<n>,<group>×n}`; a group is a class and an
/// inclusion list, wrapped as `{6,<list>}` in some groups; an inclusion
/// list is `{<k>,<header>,<m>,<entry>×m}`; an entry is
/// `{<class>,<c>,<object>×c}`.
///
/// # Errors
///
/// Returns [`MetadataError`] when the resource is not the serialization or
/// any part of it has another shape. No part is skipped: a catalog missing
/// an entry would be a wrong catalog, not a smaller one.
pub fn parse_extension_catalog(resource: &[u8]) -> Result<ExtensionCatalog, MetadataError> {
    let value = parse_serialized(resource)?;
    let root = value
        .as_list()
        .ok_or_else(|| malformed("the configuration root is not a list"))?;
    let configuration = root
        .get(1)
        .and_then(Value::as_list)
        .and_then(|own| own.first())
        .and_then(guid)
        .ok_or_else(|| malformed("the configuration root names no configuration"))?;
    let groups = counted(root, 2, "the configuration root")?;
    let mut classes = Vec::new();
    for group in groups {
        let group = group
            .as_list()
            .ok_or_else(|| malformed("a group of the configuration root is not a list"))?;
        let versioned = group
            .get(1)
            .and_then(Value::as_list)
            .ok_or_else(|| malformed("a group of the configuration root carries no list"))?;
        let inclusion = if versioned.first().and_then(Value::as_scalar) == Some("6") {
            versioned
                .get(1)
                .and_then(Value::as_list)
                .ok_or_else(|| malformed("a versioned group carries no list"))?
        } else {
            versioned
        };
        for entry in counted(inclusion, 2, "a group of the configuration root")? {
            classes.push(class_entry(entry)?);
        }
    }
    Ok(ExtensionCatalog {
        configuration,
        classes,
    })
}

/// Reads every object header of one description resource.
///
/// A header is `{3,{1,0,<id>},<name>,<synonym>,<comment>,<belonging>,<n>,
/// (<guid>,<tag>)×n,<extends>,<x>}`, found wherever it is nested: the
/// object itself and each object inside it, such as an attribute. The
/// pairs are not interpreted.
///
/// # Errors
///
/// Returns [`MetadataError`] when the resource is not the serialization.
pub fn parse_extension_objects(resource: &[u8]) -> Result<Vec<ExtensionObject>, MetadataError> {
    let mut objects = Vec::new();
    for record in parse_serialized_sequence(resource)? {
        collect_objects(&record, &mut objects);
    }
    Ok(objects)
}

fn collect_objects(value: &Value, objects: &mut Vec<ExtensionObject>) {
    let Some(items) = value.as_list() else {
        return;
    };
    if let Some(object) = object_header(items) {
        objects.push(object);
    }
    for item in items {
        collect_objects(item, objects);
    }
}

fn object_header(items: &[Value]) -> Option<ExtensionObject> {
    if items.first()?.as_scalar()? != "3" {
        return None;
    }
    let [one, zero, id] = items.get(1)?.as_list()? else {
        return None;
    };
    if one.as_scalar()? != "1" || zero.as_scalar()? != "0" {
        return None;
    }
    let id = guid(id)?;
    let name = items.get(2)?.as_string()?.to_owned();
    items.get(3)?.as_list()?;
    items.get(4)?.as_string()?;
    let (belonging, extends) = belonging(&items[5..]).unwrap_or((None, None));
    Some(ExtensionObject {
        id,
        name,
        belonging,
        extends,
    })
}

/// Reads the tail of a header, or `None` when it is not in the measured
/// shape: `<belonging>,<n>,(<guid>,<tag>)×n,<extends>,<x>`.
fn belonging(tail: &[Value]) -> Option<(Option<ObjectBelonging>, Option<Guid>)> {
    let pairs = tail.get(1)?.as_scalar()?.parse::<usize>().ok()?;
    if tail.len() != pairs.checked_mul(2)?.checked_add(4)? {
        return None;
    }
    let belonging = match tail[0].as_scalar()? {
        "0" => ObjectBelonging::Own,
        "1" => ObjectBelonging::Adopted,
        _ => return None,
    };
    let extends = guid(&tail[2 + pairs * 2])?;
    let extends = (belonging == ObjectBelonging::Adopted && !extends.is_nil()).then_some(extends);
    Some((Some(belonging), extends))
}

/// The `count` items after the count at `at`, refusing a count the list
/// does not hold.
fn counted<'value>(
    list: &'value [Value],
    at: usize,
    what: &str,
) -> Result<&'value [Value], MetadataError> {
    let count = list
        .get(at)
        .and_then(Value::as_u32)
        .ok_or_else(|| malformed(&format!("{what} carries no count")))? as usize;
    list.get(at + 1..at + 1 + count)
        .ok_or_else(|| malformed(&format!("{what} counts {count} items it does not hold")))
}

fn class_entry(entry: &Value) -> Result<ExtensionClass, MetadataError> {
    let items = entry
        .as_list()
        .ok_or_else(|| malformed("a class entry is not a list"))?;
    let class = items
        .first()
        .and_then(guid)
        .ok_or_else(|| malformed("a class entry names no class"))?;
    let listed = counted(items, 1, &format!("the entry of class {class}"))?;
    if listed.len() + 2 != items.len() {
        return Err(malformed(&format!(
            "the entry of class {class} holds more than it counts"
        )));
    }
    let objects = listed
        .iter()
        .map(|object| {
            guid(object).ok_or_else(|| {
                malformed(&format!(
                    "the entry of class {class} lists a non-identifier"
                ))
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(ExtensionClass { class, objects })
}

fn guid(value: &Value) -> Option<Guid> {
    Guid::from_str(value.as_scalar()?).ok()
}

fn malformed(message: &str) -> MetadataError {
    MetadataError::new(
        MetadataErrorKind::Serialization,
        format!("extension catalog: {message}"),
    )
}
