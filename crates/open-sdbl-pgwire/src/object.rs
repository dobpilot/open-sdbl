//! Read one referenced 1C object and expose its logical attributes as JSON.

use std::collections::HashSet;
use std::io::{self, Write};

use open_sdbl::metadata::{FieldId, MetadataKind, MetadataSnapshot, ObjectId, StandardFieldId};
use open_sdbl::query::{
    ColumnKind, CompiledQuery, LogicalResultColumn, LogicalResultValue, ParameterValue,
    QueryParameter, find_metadata_object, object_query_name,
};
use open_sdbl_db::Cell;
use serde_json::Value;

use crate::codec::{BackendWriter, ErrorFields};

pub(crate) struct Lookup {
    pub(crate) query: String,
    pub(crate) reference: QueryParameter,
}

impl Lookup {
    pub(crate) fn resolve(snapshot: &MetadataSnapshot, link: &str) -> Result<Self, ErrorFields> {
        Self::project(snapshot, link, "*")
    }

    pub(crate) fn attribute(
        snapshot: &MetadataSnapshot,
        link: &str,
        name: &str,
    ) -> Result<Self, ErrorFields> {
        if !identifier(name) {
            return Err(invalid_attribute());
        }
        Self::project(snapshot, link, &format!("Obj.{name}"))
    }

    fn project(
        snapshot: &MetadataSnapshot,
        link: &str,
        projection: &str,
    ) -> Result<Self, ErrorFields> {
        let (name, reference) = link
            .strip_prefix("e1cib/data/")
            .and_then(|tail| tail.split_once("?ref="))
            .ok_or_else(invalid_link)?;
        let name = percent_encoding::percent_decode_str(name)
            .decode_utf8()
            .map_err(|_| invalid_link())?;
        if !object_path(&name)
            || reference.len() != 32
            || !reference.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid_link());
        }
        let id = crate::values::decode_hex(reference)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(invalid_link)?;
        let object = find_metadata_object(snapshot, &name).map_err(|_| {
            ErrorFields::error("42704", "1C object type was not found or is ambiguous")
        })?;
        if object.owner.is_some()
            || !matches!(
                object.kind,
                Some(
                    MetadataKind::Catalog
                        | MetadataKind::Document
                        | MetadataKind::Enumeration
                        | MetadataKind::ChartOfAccounts
                        | MetadataKind::ChartOfCharacteristicTypes
                        | MetadataKind::ChartOfCalculationTypes
                        | MetadataKind::ExchangePlan
                        | MetadataKind::BusinessProcess
                        | MetadataKind::Task
                )
            )
        {
            return Err(ErrorFields::error(
                "0A000",
                "the metadata type does not support lookup by object reference",
            ));
        }
        let target = object_query_name(snapshot, object)
            .filter(|name| object_path(name))
            .ok_or_else(invalid_link)?;
        Ok(Self {
            query: format!(
                "ВЫБРАТЬ ПЕРВЫЕ 2 {projection} ИЗ {target} КАК Obj ГДЕ Obj.Ссылка = &ObjectLink"
            ),
            reference: QueryParameter::new(
                "ObjectLink",
                ParameterValue::Reference {
                    object: ObjectId::from(&object.guid),
                    id,
                },
            ),
        })
    }
}

fn object_path(path: &str) -> bool {
    path.split_once('.')
        .is_some_and(|(kind, name)| identifier(kind) && identifier(name))
}

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
}

fn invalid_attribute() -> ErrorFields {
    ErrorFields::error("22023", "expected a dotted path of 1C attribute names")
}

pub(crate) fn attribute_path(path: &str) -> Result<Vec<&str>, ErrorFields> {
    let names = path.split('.').take(9).collect::<Vec<_>>();
    if names.len() > 8 || !names.iter().all(|name| identifier(name)) {
        return Err(invalid_attribute());
    }
    Ok(names)
}

fn invalid_link() -> ErrorFields {
    ErrorFields::error(
        "22023",
        "expected an internal e1cib/data/Kind.Name?ref=<32 hex digits> object link",
    )
}

pub(crate) fn check_projection(query: &CompiledQuery) -> Result<(), ErrorFields> {
    if query.logical_projection.is_none() || !query.nested.is_empty() {
        return Err(ErrorFields::error(
            "0A000",
            "the object's attributes do not have a complete logical projection",
        ));
    }
    Ok(())
}

pub(crate) fn result_frame(
    snapshot: &MetadataSnapshot,
    query: &CompiledQuery,
    rows: &[Vec<Cell>],
    format: i16,
    budget: usize,
) -> Result<Vec<u8>, ErrorFields> {
    let mut writer = BackendWriter::new();
    let [row] = rows else {
        if rows.is_empty() {
            writer.data_row([None]);
            return Ok(writer.take());
        }
        return Err(ErrorFields::error(
            "21000",
            "the object reference matched more than one row",
        ));
    };
    let mut output = JsonBuffer {
        bytes: Vec::new(),
        limit: budget.min(i32::MAX as usize).saturating_sub(11),
    };
    if format == 1 {
        output.append(b"\x01")?;
    }
    output.append(b"{")?;
    let projection = query.logical_projection.as_deref().ok_or_else(bad_value)?;
    let mut names = HashSet::new();
    for field in projection {
        let name = attribute_name(query, field);
        if !names.insert(name) {
            return Err(ErrorFields::error(
                "42701",
                "the object's attribute names are ambiguous",
            ));
        }
        if names.len() > 1 {
            output.append(b",")?;
        }
        serde_json::to_writer(&mut output, name).map_err(|_| result_limit())?;
        output.append(b":")?;
        let scalar = |column: usize| -> Result<Value, ErrorFields> {
            let cell: &Cell = row.get(column).ok_or_else(bad_value)?;
            let kind = &query.columns.get(column).ok_or_else(bad_value)?.kind;
            let size = match cell {
                Cell::Text(text) | Cell::Number(text) => text.len(),
                Cell::Bytes(bytes) => bytes.len(),
                _ => 0,
            };
            if size > output.limit - output.bytes.len() {
                return Err(result_limit());
            }
            Ok(cell_value(snapshot, kind, cell))
        };
        let value = match &field.value {
            LogicalResultValue::PointInTime { date, reference } => {
                serde_json::json!({"Дата": scalar(*date)?, "Ссылка": scalar(*reference)?})
            }
            value => active_column(value, row)?
                .map(scalar)
                .transpose()?
                .unwrap_or(Value::Null),
        };
        serde_json::to_writer(&mut output, &value).map_err(|_| result_limit())?;
    }
    output.append(b"}")?;
    writer.data_row([Some(output.bytes.as_slice())]);
    Ok(writer.take())
}

fn active_column(value: &LogicalResultValue, row: &[Cell]) -> Result<Option<usize>, ErrorFields> {
    match value {
        LogicalResultValue::Scalar { column } => Ok(Some(*column)),
        LogicalResultValue::Composite {
            discriminator,
            members,
        } => match row.get(*discriminator) {
            Some(Cell::Null) => Ok(None),
            Some(Cell::Bytes(tag)) if tag.len() == 1 => members
                .iter()
                .find(|member| member.type_tag == tag[0])
                .map(|member| member.column)
                .ok_or_else(bad_value),
            _ => Err(bad_value()),
        },
        _ => Err(ErrorFields::error(
            "0A000",
            "the attribute is not a scalar value",
        )),
    }
}

pub(crate) fn attribute_text(
    snapshot: &MetadataSnapshot,
    query: &CompiledQuery,
    rows: &[Vec<Cell>],
    follow: bool,
    budget: usize,
) -> Result<Option<String>, ErrorFields> {
    let row = match rows {
        [] => return Ok(None),
        [row] => row,
        _ => {
            return Err(ErrorFields::error(
                "21000",
                "the object reference matched more than one row",
            ));
        }
    };
    let [attribute] = query.logical_projection.as_deref().ok_or_else(bad_value)? else {
        return Err(bad_value());
    };
    let Some(column) = active_column(&attribute.value, row)? else {
        return Ok(None);
    };
    let cell = row.get(column).ok_or_else(bad_value)?;
    if cell.is_null() {
        return Ok(None);
    }
    let kind = &query.columns.get(column).ok_or_else(bad_value)?.kind;
    if let (ColumnKind::Reference { runtime_typed, .. }, Cell::Bytes(bytes)) = (kind, cell) {
        let expected = if *runtime_typed { 20 } else { 16 };
        if bytes.len() != expected {
            return Err(bad_value());
        }
        if bytes[expected - 16..] == [0; 16] {
            return Ok(None);
        }
        if let Some(link) = navigation_link(snapshot, kind, bytes) {
            return sized_text(link, budget);
        }
    }
    if follow {
        return Err(ErrorFields::error(
            "42804",
            "the attribute path crosses a value without a known reference type",
        ));
    }
    let size = match cell {
        Cell::Text(value) | Cell::Number(value) => value.len(),
        Cell::Bytes(value) => value.len().saturating_mul(2).saturating_add(2),
        _ => 0,
    };
    if size > budget {
        return Err(attribute_result_limit());
    }
    sized_text(cell.render().into_owned(), budget)
}

fn sized_text(value: String, budget: usize) -> Result<Option<String>, ErrorFields> {
    if value.len() > budget {
        return Err(attribute_result_limit());
    }
    Ok(Some(value))
}

fn attribute_result_limit() -> ErrorFields {
    ErrorFields::error("54000", "attribute text exceeds the gateway result limit")
}

fn attribute_name<'a>(query: &CompiledQuery, field: &'a LogicalResultColumn) -> &'a str {
    let LogicalResultValue::Scalar { column } = field.value else {
        return &field.name;
    };
    match query
        .columns
        .get(column)
        .and_then(|column| column.origin.as_ref())
        .map(|origin| origin.field)
    {
        Some(FieldId::Standard(StandardFieldId::Id)) => "Ссылка",
        Some(FieldId::Standard(StandardFieldId::Code)) => "Код",
        Some(FieldId::Standard(StandardFieldId::Description)) => "Наименование",
        Some(FieldId::Standard(StandardFieldId::Marked)) => "ПометкаУдаления",
        Some(FieldId::Standard(StandardFieldId::Version)) => "ВерсияДанных",
        Some(FieldId::Standard(StandardFieldId::Number)) => "Номер",
        Some(FieldId::Standard(StandardFieldId::Date)) => "Дата",
        Some(FieldId::Standard(StandardFieldId::Posted)) => "Проведен",
        _ => &field.name,
    }
}

fn cell_value(snapshot: &MetadataSnapshot, kind: &ColumnKind, cell: &Cell) -> Value {
    match cell {
        Cell::Null => Value::Null,
        Cell::Bool(value) => Value::Bool(*value),
        Cell::Number(number) => number
            .parse()
            .map(Value::Number)
            .unwrap_or_else(|_| Value::String(number.clone())),
        Cell::Bytes(bytes) => Value::String(
            navigation_link(snapshot, kind, bytes).unwrap_or_else(|| cell.render().into_owned()),
        ),
        Cell::Text(_) | Cell::DateTime(_) | Cell::Uuid(_) => {
            Value::String(cell.render().into_owned())
        }
    }
}

fn navigation_link(snapshot: &MetadataSnapshot, kind: &ColumnKind, bytes: &[u8]) -> Option<String> {
    use std::fmt::Write as _;

    let ColumnKind::Reference {
        targets,
        runtime_typed,
    } = kind
    else {
        return None;
    };
    let (target, reference) = if *runtime_typed {
        let (table, reference) = bytes.split_at_checked(4)?;
        let table = u32::from_be_bytes(table.try_into().ok()?);
        (snapshot.object_id_by_database_type(table).ok()?, reference)
    } else {
        let [target] = targets.as_slice() else {
            return None;
        };
        (*target, bytes)
    };
    let reference: &[u8; 16] = reference.try_into().ok()?;
    let name = object_query_name(snapshot, snapshot.object_by_id(target)?)?;
    if !object_path(&name) {
        return None;
    }
    let mut link = format!("e1cib/data/{name}?ref=");
    for byte in reference {
        write!(link, "{byte:02x}").expect("writing to String cannot fail");
    }
    Some(link)
}

fn bad_value() -> ErrorFields {
    ErrorFields::error(
        "XX000",
        "the database row does not match the object's attribute projection",
    )
}

fn result_limit() -> ErrorFields {
    ErrorFields::error("54000", "object JSON exceeds the gateway result limit")
}

struct JsonBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl JsonBuffer {
    fn append(&mut self, bytes: &[u8]) -> Result<(), ErrorFields> {
        self.write_all(bytes).map_err(|_| result_limit())
    }
}

impl Write for JsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(io::Error::other(
                "object JSON exceeds the gateway result limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
