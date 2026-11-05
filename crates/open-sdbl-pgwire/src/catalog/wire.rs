//! Catalog protocol encoding, including integer OIDs and arrays.
use super::sql::{Column, Table, Value, array_oid};
use crate::codec::{BackendWriter, ErrorFields, FieldDescription};
use crate::values::{WireType, encode_cell, format_of};
use open_sdbl_db::Cell;

pub(crate) fn encode(
    table: &Table,
    formats: &[i16],
) -> Result<(Vec<FieldDescription>, Vec<Vec<u8>>), ErrorFields> {
    if formats.len() > 1 && formats.len() != table.columns.len() {
        return Err(ErrorFields::error(
            "08P01",
            "bind message has a wrong number of result formats",
        ));
    }
    let fields = table
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| c.field(format_of(formats, i)))
        .collect();
    let mut rows = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        let values = row
            .iter()
            .zip(&table.columns)
            .enumerate()
            .map(|(i, (v, c))| cell(v, c, format_of(formats, i)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut writer = BackendWriter::new();
        writer.data_row(values.iter().map(Option::as_deref));
        rows.push(writer.take());
    }
    Ok((fields, rows))
}
fn cell(value: &Value, column: &Column, format: i16) -> Result<Option<Vec<u8>>, ErrorFields> {
    if *value == Value::Null {
        return Ok(None);
    }
    if format == 0 {
        return Ok(Some(value.text().into_bytes()));
    }
    let invalid = || {
        ErrorFields::error(
            "0A000",
            format!(
                "binary format of catalog column {} (OID {}) is unsupported",
                column.name, column.oid
            ),
        )
    };
    let bytes = match column.oid {
        16 => vec![u8::from(*value == Value::Bool(true))],
        18 => {
            let s = value.text();
            if s.len() != 1 {
                return Err(invalid());
            }
            s.into_bytes()
        }
        19 | 25 | 1043 => value.text().into_bytes(),
        20 => value.int()?.to_be_bytes().to_vec(),
        21 => i16::try_from(value.int()?)
            .map_err(|_| invalid())?
            .to_be_bytes()
            .to_vec(),
        23 => i32::try_from(value.int()?)
            .map_err(|_| invalid())?
            .to_be_bytes()
            .to_vec(),
        24 | 26 | 28 | 2203 => u32::try_from(value.int()?)
            .map_err(|_| invalid())?
            .to_be_bytes()
            .to_vec(),
        701 => value
            .text()
            .parse::<f64>()
            .map_err(|_| invalid())?
            .to_be_bytes()
            .to_vec(),
        1700 => {
            encode_cell(&Cell::Number(value.text()), WireType::Numeric, true).ok_or_else(invalid)?
        }
        oid if element_oid(oid).is_some() => {
            let element = element_oid(oid).unwrap();
            let Value::Array(values) = value else {
                return Err(invalid());
            };
            let mut out = Vec::new();
            out.extend(1_i32.to_be_bytes());
            out.extend(i32::from(values.contains(&Value::Null)).to_be_bytes());
            out.extend(element.to_be_bytes());
            out.extend(
                i32::try_from(values.len())
                    .map_err(|_| invalid())?
                    .to_be_bytes(),
            );
            out.extend(1_i32.to_be_bytes());
            let col = Column::new(&column.name, element);
            for v in values {
                match cell(v, &col, 1)? {
                    None => out.extend((-1_i32).to_be_bytes()),
                    Some(bytes) => {
                        out.extend(
                            i32::try_from(bytes.len())
                                .map_err(|_| invalid())?
                                .to_be_bytes(),
                        );
                        out.extend(bytes);
                    }
                }
            }
            out
        }
        _ => return Err(invalid()),
    };
    Ok(Some(bytes))
}
fn element_oid(oid: u32) -> Option<u32> {
    [16, 17, 18, 19, 20, 21, 23, 25, 26, 28, 1043, 1700]
        .into_iter()
        .find(|e| array_oid(*e) == Some(oid))
}
pub(crate) fn decode(oid: u32, binary: bool, bytes: Option<&[u8]>) -> Result<Value, ErrorFields> {
    let Some(bytes) = bytes else {
        return Ok(Value::Null);
    };
    let invalid = || {
        ErrorFields::error(
            "22P02",
            format!("invalid catalog parameter of type OID {oid}"),
        )
    };
    if let Some(element) = element_oid(oid) {
        if !binary {
            let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
            let values = crate::values::parse_text_array(text)
                .map_err(|_| invalid())?
                .into_iter()
                .map(|s| decode(element, false, s.as_deref().map(str::as_bytes)))
                .collect::<Result<_, _>>()?;
            return Ok(Value::Array(values));
        }
        let word = |at: usize| -> Result<i32, ErrorFields> {
            Ok(i32::from_be_bytes(
                bytes
                    .get(at..at + 4)
                    .ok_or_else(invalid)?
                    .try_into()
                    .map_err(|_| invalid())?,
            ))
        };
        let dimensions = word(0)?;
        let declared = u32::try_from(word(8)?).map_err(|_| invalid())?;
        if declared != element {
            return Err(invalid());
        }
        if dimensions == 0 && bytes.len() == 12 {
            return Ok(Value::Array(vec![]));
        }
        if dimensions != 1 || word(16)? != 1 {
            return Err(ErrorFields::error(
                "0A000",
                "only one-dimensional arrays with lower bound 1 are supported",
            ));
        }
        let count = usize::try_from(word(12)?).map_err(|_| invalid())?;
        if count > bytes.len().saturating_sub(20) / 4 {
            return Err(invalid());
        }
        let mut at = 20;
        let mut values = vec![];
        for _ in 0..count {
            let length = word(at)?;
            at += 4;
            if length == -1 {
                values.push(Value::Null);
                continue;
            }
            let length = usize::try_from(length).map_err(|_| invalid())?;
            let end = at.checked_add(length).ok_or_else(invalid)?;
            let value = bytes.get(at..end).ok_or_else(invalid)?;
            values.push(decode(element, true, Some(value))?);
            at = end;
        }
        if at != bytes.len() {
            return Err(invalid());
        }
        return Ok(Value::Array(values));
    }
    if !binary {
        let s = std::str::from_utf8(bytes).map_err(|_| invalid())?;
        return Ok(match oid {
            16 => match s {
                "t" | "true" | "1" => Value::Bool(true),
                "f" | "false" | "0" => Value::Bool(false),
                _ => return Err(invalid()),
            },
            20 | 21 | 23 | 24 | 26 | 2203 | 2205 | 2206 | 4089 => {
                Value::Int(s.parse().map_err(|_| invalid())?)
            }
            28 => Value::Int(i64::from(s.parse::<u32>().map_err(|_| invalid())?)),
            0 | 18 | 19 | 25 | 1043 | 1700 => Value::Text(s.into()),
            _ => {
                return Err(ErrorFields::error(
                    "0A000",
                    "unsupported catalog parameter type",
                ));
            }
        });
    }
    Ok(match oid {
        16 if bytes.len() == 1 && bytes[0] <= 1 => Value::Bool(bytes[0] == 1),
        20 => Value::Int(i64::from_be_bytes(bytes.try_into().map_err(|_| invalid())?)),
        21 => Value::Int(i64::from(i16::from_be_bytes(
            bytes.try_into().map_err(|_| invalid())?,
        ))),
        23 => Value::Int(i64::from(i32::from_be_bytes(
            bytes.try_into().map_err(|_| invalid())?,
        ))),
        24 | 26 | 28 | 2203 => Value::Int(i64::from(u32::from_be_bytes(
            bytes.try_into().map_err(|_| invalid())?,
        ))),
        18 | 19 | 25 | 1043 => {
            Value::Text(std::str::from_utf8(bytes).map_err(|_| invalid())?.into())
        }
        _ => {
            return Err(ErrorFields::error(
                "0A000",
                format!("unsupported binary catalog parameter OID {oid}"),
            ));
        }
    })
}
