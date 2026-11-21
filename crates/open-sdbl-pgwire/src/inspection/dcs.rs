//! Query data sets in 1C data composition schema XML.

use std::borrow::Cow;
use std::fmt::Display;

use roxmltree::Node;
use xmlparser::{ElementEnd, Token, Tokenizer};

use super::{DCS_SCHEMA_NS, parse_xml};
use crate::codec::{BackendWriter, ErrorFields};

const MAX_XML_DEPTH: usize = 128;
const MAX_ELEMENT_ATTRIBUTES: usize = 128;
const MAX_NAMESPACE_DECLARATIONS: usize = 4096;
const MAX_ADJACENT_TEXT_SEGMENTS: usize = 32;
// The tree parser reserves space for each '<' and '=', including within text.
const XML_BYTES_PER_RESERVED_SLOT: usize = 128;

pub(crate) fn query_rows(
    xml: &str,
    budget: usize,
    limit: usize,
) -> Result<Vec<Vec<u8>>, ErrorFields> {
    let mut remaining = budget
        .checked_sub(xml.len())
        .ok_or_else(|| ErrorFields::error("54000", "DCS XML exceeds the gateway limit"))?;
    let reserved_slots = xml
        .bytes()
        .filter(|byte| matches!(byte, b'<' | b'='))
        .count();
    if reserved_slots > remaining / XML_BYTES_PER_RESERVED_SLOT {
        return Err(ErrorFields::error(
            "54000",
            "DCS XML exceeds the gateway tree allocation limit",
        ));
    }
    check_xml_structure(xml)?;
    let document = parse_xml(xml).map_err(|error| match error {
        roxmltree::Error::NodesLimitReached | roxmltree::Error::NamespacesLimitReached => {
            ErrorFields::error("54000", "DCS XML exceeds the gateway structure limits")
        }
        _ => invalid_xml(error),
    })?;
    let root = document.root_element();
    let is_schema = |node: &Node<'_, '_>| {
        node.has_tag_name((DCS_SCHEMA_NS, "dataCompositionSchema"))
            || node.has_tag_name((DCS_SCHEMA_NS, "DataCompositionSchema"))
    };
    let schema = if is_schema(&root) {
        Some(root)
    } else if root.has_tag_name("SchemaFile")
        && root.tag_name().namespace().unwrap_or_default().is_empty()
    {
        root.children().find(is_schema)
    } else {
        None
    }
    .ok_or_else(|| ErrorFields::error("22023", "expected a 1C data composition schema"))?;

    let mut datasets = schema
        .children()
        .rev()
        .filter(|node| node.has_tag_name((DCS_SCHEMA_NS, "dataSet")))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut out = BackendWriter::new();
    while let Some(dataset) = datasets.pop() {
        datasets.extend(
            dataset
                .children()
                .rev()
                .filter(|node| node.has_tag_name((DCS_SCHEMA_NS, "item"))),
        );
        let Some(query) = dataset
            .children()
            .find(|node| node.has_tag_name((DCS_SCHEMA_NS, "query")))
        else {
            continue;
        };
        let name = dataset
            .children()
            .find(|node| node.has_tag_name((DCS_SCHEMA_NS, "name")))
            .map(element_text);
        let query = element_text(query);
        remaining = remaining
            .checked_sub(
                name.as_deref().map_or(0, str::len) + query.len() + 15 + size_of::<Vec<u8>>(),
            )
            .filter(|_| rows.len() < limit)
            .ok_or_else(|| {
                ErrorFields::error("54000", "DCS query result exceeds the gateway limit")
            })?;
        // The text and binary wire formats of PostgreSQL text are identical.
        out.data_row([name.as_deref().map(str::as_bytes), Some(query.as_bytes())]);
        rows.push(out.take());
    }
    Ok(rows)
}

// Check recursion depth and bound the work of roxmltree's attribute,
// namespace and adjacent-text processing before building the tree.
fn check_xml_structure(xml: &str) -> Result<(), ErrorFields> {
    let mut depth = 0_usize;
    let mut attributes = 0;
    let mut namespaces = 0;
    let mut text_segments = 0;
    for token in Tokenizer::from(xml) {
        let exceeds_limit = match token.map_err(invalid_xml)? {
            Token::ElementStart { .. } => {
                attributes = 0;
                text_segments = 0;
                depth >= MAX_XML_DEPTH
            }
            Token::ElementEnd { end, .. } => {
                match end {
                    ElementEnd::Open => depth += 1,
                    ElementEnd::Close(..) => depth = depth.saturating_sub(1),
                    ElementEnd::Empty => {}
                }
                text_segments = 0;
                false
            }
            Token::Attribute { prefix, local, .. } => {
                attributes += 1;
                if prefix.as_str() == "xmlns" || prefix.is_empty() && local.as_str() == "xmlns" {
                    namespaces += 1;
                }
                attributes > MAX_ELEMENT_ATTRIBUTES || namespaces > MAX_NAMESPACE_DECLARATIONS
            }
            Token::Text { .. } | Token::Cdata { .. } => {
                text_segments += 1;
                text_segments > MAX_ADJACENT_TEXT_SEGMENTS
            }
            Token::DtdStart { .. } | Token::EmptyDtd { .. } => {
                return Err(invalid_xml("DTDs are unsupported"));
            }
            _ => {
                text_segments = 0;
                false
            }
        };
        if exceeds_limit {
            return Err(ErrorFields::error(
                "54000",
                "DCS XML exceeds the gateway structure limits",
            ));
        }
    }
    Ok(())
}

fn invalid_xml(error: impl Display) -> ErrorFields {
    ErrorFields::error("2200M", format!("invalid DCS XML: {error}"))
}

fn element_text<'a>(element: Node<'a, '_>) -> Cow<'a, str> {
    let mut parts = element
        .children()
        .filter(Node::is_text)
        .filter_map(|node| node.text());
    let first = parts.next().unwrap_or_default();
    parts.fold(Cow::Borrowed(first), |mut text, part| {
        text.to_mut().push_str(part);
        text
    })
}
