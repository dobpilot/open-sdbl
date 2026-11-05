//! PostgreSQL types of result columns and bound parameters.
//!
//! Result columns are described from their [`ColumnKind`] and each cell is
//! encoded in the text or binary format the client asked for. Bound values
//! are decoded by their declared type OID into [`ParameterValue`]s.

use std::fmt::Write as _;

use open_sdbl::metadata::Guid;
use open_sdbl::query::{ColumnKind, ParameterDate, ParameterValue};
use open_sdbl_db::{Cell, DateTimeParts};

use crate::codec::FieldDescription;

/// Type OIDs the gateway reads or writes.
pub(crate) mod oid {
    pub(crate) const UNSPECIFIED: u32 = 0;
    pub(crate) const BOOL: u32 = 16;
    pub(crate) const BYTEA: u32 = 17;
    pub(crate) const NAME: u32 = 19;
    pub(crate) const INT8: u32 = 20;
    pub(crate) const INT2: u32 = 21;
    pub(crate) const INT4: u32 = 23;
    pub(crate) const TEXT: u32 = 25;
    pub(crate) const FLOAT4: u32 = 700;
    pub(crate) const FLOAT8: u32 = 701;
    pub(crate) const UNKNOWN: u32 = 705;
    pub(crate) const BPCHAR: u32 = 1042;
    pub(crate) const VARCHAR: u32 = 1043;
    pub(crate) const DATE: u32 = 1082;
    pub(crate) const TIMESTAMP: u32 = 1114;
    pub(crate) const TIMESTAMPTZ: u32 = 1184;
    pub(crate) const NUMERIC: u32 = 1700;
    pub(crate) const UUID: u32 = 2950;

    /// The element type of a one-dimensional array type the gateway binds.
    pub(crate) const fn array_element(array: u32) -> Option<u32> {
        Some(match array {
            1000 => BOOL,
            1001 => BYTEA,
            1003 => NAME,
            1005 => INT2,
            1007 => INT4,
            1016 => INT8,
            1009 => TEXT,
            1014 => BPCHAR,
            1015 => VARCHAR,
            1021 => FLOAT4,
            1022 => FLOAT8,
            1182 => DATE,
            1115 => TIMESTAMP,
            1185 => TIMESTAMPTZ,
            1231 => NUMERIC,
            2951 => UUID,
            _ => return None,
        })
    }
}

/// How one result column is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireType {
    Bytea,
    Text,
    Numeric,
    Bool,
    Timestamp,
    Uuid,
}

/// The description of a result column of `kind` labelled `name`.
pub(crate) fn describe_column(
    name: &str,
    kind: &ColumnKind,
    format: i16,
) -> (FieldDescription, WireType) {
    let (wire, type_oid, type_length, type_modifier) = match kind {
        ColumnKind::Reference { .. } | ColumnKind::Binary { .. } => {
            (WireType::Bytea, oid::BYTEA, -1, -1)
        }
        ColumnKind::String {
            length: Some(length),
        } if *length > 0 => (
            WireType::Text,
            oid::VARCHAR,
            -1,
            i32::try_from(*length).map_or(-1, |length| length.saturating_add(4)),
        ),
        ColumnKind::String { .. } => (WireType::Text, oid::TEXT, -1, -1),
        ColumnKind::Number { precision, scale } => {
            let modifier = match (precision, scale) {
                (Some(precision), Some(scale)) if *precision > 0 => {
                    ((i32::from(*precision) << 16) | i32::from(*scale)) + 4
                }
                _ => -1,
            };
            (WireType::Numeric, oid::NUMERIC, -1, modifier)
        }
        ColumnKind::Boolean => (WireType::Bool, oid::BOOL, 1, -1),
        ColumnKind::DateTime => (WireType::Timestamp, oid::TIMESTAMP, 8, -1),
        ColumnKind::Uuid => (WireType::Uuid, oid::UUID, 16, -1),
        // Type values, НЕОПРЕДЕЛЕНО, NULL literals and unclassified catalog
        // types are sent as the console renders them.
        _ => (WireType::Text, oid::TEXT, -1, -1),
    };
    (
        FieldDescription {
            name: name.to_owned(),
            type_oid,
            type_length,
            type_modifier,
            format,
        },
        wire,
    )
}

/// The format code of result column `index` from Bind's result formats.
pub(crate) fn format_of(formats: &[i16], index: usize) -> i16 {
    match formats {
        [] => 0,
        [single] => *single,
        many => many.get(index).copied().unwrap_or(0),
    }
}

/// Days from 2000-01-01 to a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468 - 10_957
}

fn civil_from_days(days_since_2000: i64) -> (i64, u32, u32) {
    let z = days_since_2000 + 10_957 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    (
        if month <= 2 { year + 1 } else { year },
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

fn timestamp_micros(parts: &DateTimeParts) -> i64 {
    let days = days_from_civil(parts.year, parts.month, parts.day);
    let seconds =
        i64::from(parts.hour) * 3600 + i64::from(parts.minute) * 60 + i64::from(parts.second);
    (days * 86_400 + seconds) * 1_000_000
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(2 + bytes.len() * 2);
    text.push_str("\\x");
    for byte in bytes {
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

fn uuid_text(bytes: &[u8; 16]) -> String {
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

/// Encodes one cell; `None` is SQL `NULL`.
pub(crate) fn encode_cell(cell: &Cell, wire: WireType, binary: bool) -> Option<Vec<u8>> {
    if cell.is_null() {
        return None;
    }
    Some(match (wire, cell) {
        (WireType::Bytea, Cell::Bytes(bytes)) => {
            if binary {
                bytes.clone()
            } else {
                lower_hex(bytes).into_bytes()
            }
        }
        (WireType::Numeric, Cell::Number(number)) if binary => {
            encode_numeric(number).unwrap_or_else(|| number.clone().into_bytes())
        }
        (WireType::Bool, Cell::Bool(value)) => {
            if binary {
                vec![u8::from(*value)]
            } else {
                (if *value { "t" } else { "f" }).as_bytes().to_vec()
            }
        }
        // SQL Server reports `bit` through a number on some paths.
        (WireType::Bool, Cell::Number(number)) => {
            let value = number.trim() != "0";
            if binary {
                vec![u8::from(value)]
            } else {
                (if value { "t" } else { "f" }).as_bytes().to_vec()
            }
        }
        (WireType::Timestamp, Cell::DateTime(parts)) => {
            if binary {
                timestamp_micros(parts).to_be_bytes().to_vec()
            } else {
                parts.to_string().into_bytes()
            }
        }
        (WireType::Uuid, Cell::Uuid(bytes)) => {
            if binary {
                bytes.to_vec()
            } else {
                uuid_text(bytes).into_bytes()
            }
        }
        // Everything else travels as the text the console prints, and a
        // binary text value is its UTF-8.
        (_, other) => other.render().into_owned().into_bytes(),
    })
}

/// Encodes decimal text as PostgreSQL binary `numeric`.
pub(crate) fn encode_numeric(text: &str) -> Option<Vec<u8>> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if integer.is_empty() && fraction.is_empty()
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let scale = u16::try_from(fraction.len()).ok()?;
    // Pad the integer part on the left and the fraction on the right to
    // whole base-10000 groups.
    let integer = integer.trim_start_matches('0');
    let integer_pad = (4 - integer.len() % 4) % 4;
    let fraction_pad = (4 - fraction.len() % 4) % 4;
    let digits_text = format!(
        "{}{integer}{fraction}{}",
        "0".repeat(integer_pad),
        "0".repeat(fraction_pad)
    );
    let mut groups = digits_text
        .as_bytes()
        .chunks(4)
        .map(|chunk| {
            chunk
                .iter()
                .fold(0_u16, |value, digit| value * 10 + u16::from(digit - b'0'))
        })
        .collect::<Vec<_>>();
    let integer_groups = i32::try_from((integer.len() + integer_pad) / 4).ok()?;
    let mut weight = integer_groups - 1;
    // Leading and trailing zero groups are not stored.
    let leading = groups.iter().take_while(|group| **group == 0).count();
    groups.drain(..leading);
    weight -= i32::try_from(leading).ok()?;
    while groups.last() == Some(&0) {
        groups.pop();
    }
    if groups.is_empty() {
        weight = 0;
    }
    let mut output = Vec::with_capacity(8 + groups.len() * 2);
    output.extend_from_slice(&u16::try_from(groups.len()).ok()?.to_be_bytes());
    output.extend_from_slice(&i16::try_from(weight).ok()?.to_be_bytes());
    let sign: u16 = if negative && !groups.is_empty() {
        0x4000
    } else {
        0
    };
    output.extend_from_slice(&sign.to_be_bytes());
    output.extend_from_slice(&scale.to_be_bytes());
    for group in groups {
        output.extend_from_slice(&group.to_be_bytes());
    }
    Some(output)
}

/// Why a bound value could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BindError {
    /// The declared type is not one the gateway binds (`42804`).
    UnsupportedType(u32),
    /// The value does not parse as its type (`22P02`).
    Invalid(String),
}

fn invalid(message: impl Into<String>) -> BindError {
    BindError::Invalid(message.into())
}

/// Decodes one bound value of declared type `type_oid` and `format`.
pub(crate) fn decode_parameter(
    type_oid: u32,
    binary: bool,
    value: Option<&[u8]>,
) -> Result<ParameterValue, BindError> {
    let Some(bytes) = value else {
        if is_supported(type_oid) {
            return Ok(ParameterValue::Null);
        }
        return Err(BindError::UnsupportedType(type_oid));
    };
    if let Some(element) = oid::array_element(type_oid) {
        return decode_array(element, binary, bytes);
    }
    // An unspecified type was described to the client as `text`.
    let type_oid = if type_oid == oid::UNSPECIFIED {
        oid::TEXT
    } else {
        type_oid
    };
    decode_scalar(type_oid, binary, bytes)
}

fn is_supported(type_oid: u32) -> bool {
    oid::array_element(type_oid).is_some()
        || matches!(
            type_oid,
            oid::UNSPECIFIED
                | oid::BOOL
                | oid::BYTEA
                | oid::NAME
                | oid::INT2
                | oid::INT4
                | oid::INT8
                | oid::TEXT
                | oid::FLOAT4
                | oid::FLOAT8
                | oid::UNKNOWN
                | oid::BPCHAR
                | oid::VARCHAR
                | oid::DATE
                | oid::TIMESTAMP
                | oid::TIMESTAMPTZ
                | oid::NUMERIC
                | oid::UUID
        )
}

fn text_of(bytes: &[u8]) -> Result<&str, BindError> {
    std::str::from_utf8(bytes).map_err(|_| invalid("parameter value is not valid UTF-8"))
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], BindError> {
    <[u8; N]>::try_from(bytes).map_err(|_| invalid(format!("binary value is not {N} bytes")))
}

fn decode_scalar(type_oid: u32, binary: bool, bytes: &[u8]) -> Result<ParameterValue, BindError> {
    match type_oid {
        oid::BOOL => {
            if binary {
                return match bytes {
                    [0] => Ok(ParameterValue::Boolean(false)),
                    [1] => Ok(ParameterValue::Boolean(true)),
                    _ => Err(invalid("binary boolean is not one byte 0 or 1")),
                };
            }
            match text_of(bytes)?.trim().to_ascii_lowercase().as_str() {
                "t" | "true" | "y" | "yes" | "on" | "1" => Ok(ParameterValue::Boolean(true)),
                "f" | "false" | "n" | "no" | "off" | "0" => Ok(ParameterValue::Boolean(false)),
                other => Err(invalid(format!("invalid boolean {other:?}"))),
            }
        }
        oid::INT2 | oid::INT4 | oid::INT8 => {
            let value = if binary {
                match bytes.len() {
                    2 => i128::from(i16::from_be_bytes(fixed(bytes)?)),
                    4 => i128::from(i32::from_be_bytes(fixed(bytes)?)),
                    8 => i128::from(i64::from_be_bytes(fixed(bytes)?)),
                    _ => return Err(invalid("binary integer has a wrong length")),
                }
            } else {
                text_of(bytes)?
                    .trim()
                    .parse::<i128>()
                    .map_err(|_| invalid("invalid integer"))?
            };
            Ok(ParameterValue::Number {
                unscaled: value,
                scale: 0,
            })
        }
        oid::NUMERIC => {
            let text = if binary {
                decode_numeric(bytes)?
            } else {
                text_of(bytes)?.trim().to_owned()
            };
            parse_decimal(&text)
        }
        oid::FLOAT4 | oid::FLOAT8 => {
            let value = if binary {
                match bytes.len() {
                    4 => f64::from(f32::from_be_bytes(fixed(bytes)?)),
                    8 => f64::from_be_bytes(fixed(bytes)?),
                    _ => return Err(invalid("binary float has a wrong length")),
                }
            } else {
                text_of(bytes)?
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| invalid("invalid floating-point number"))?
            };
            float_number(value)
        }
        oid::TEXT | oid::VARCHAR | oid::BPCHAR | oid::UNKNOWN | oid::NAME => {
            Ok(ParameterValue::String(text_of(bytes)?.to_owned()))
        }
        oid::DATE | oid::TIMESTAMP | oid::TIMESTAMPTZ => {
            let parts = if binary {
                if type_oid == oid::DATE {
                    let days = i64::from(i32::from_be_bytes(fixed(bytes)?));
                    let (year, month, day) = civil_from_days(days);
                    (year, month, day, 0, 0, 0)
                } else {
                    let micros = i64::from_be_bytes(fixed(bytes)?);
                    let seconds = micros.div_euclid(1_000_000);
                    let days = seconds.div_euclid(86_400);
                    let second_of_day = seconds.rem_euclid(86_400);
                    let (year, month, day) = civil_from_days(days);
                    (
                        year,
                        month,
                        day,
                        second_of_day / 3600,
                        second_of_day % 3600 / 60,
                        second_of_day % 60,
                    )
                }
            } else {
                parse_timestamp(text_of(bytes)?.trim(), type_oid == oid::TIMESTAMPTZ)?
            };
            date_value(parts)
        }
        // Raw bytes stay raw: the compiler reads a 16-byte constant as a
        // bare `RRRef` and a 20-byte one as `RTRef ‖ RRRef`, so a 16-byte
        // `bytea` compares with a reference field as a reference does.
        oid::BYTEA => Ok(ParameterValue::Binary(if binary {
            bytes.to_vec()
        } else {
            decode_bytea_text(text_of(bytes)?)?
        })),
        oid::UUID => {
            let raw: [u8; 16] = if binary {
                fixed(bytes)?
            } else {
                let text = text_of(bytes)?.trim();
                let hex = text.replace('-', "");
                let hex = hex.trim_start_matches('{').trim_end_matches('}');
                let decoded = decode_hex(hex).ok_or_else(|| invalid("invalid uuid"))?;
                fixed(&decoded)?
            };
            let text = uuid_text(&raw);
            text.parse::<Guid>()
                .map(ParameterValue::Uuid)
                .map_err(|_| invalid("invalid uuid"))
        }
        other => Err(BindError::UnsupportedType(other)),
    }
}

fn float_number(value: f64) -> Result<ParameterValue, BindError> {
    if !value.is_finite() {
        return Err(invalid("a floating-point infinity or NaN has no 1C number"));
    }
    // Rust prints the shortest text that reads back as the same value; the
    // value binds only if that text is an exact decimal of it.
    let text = format!("{value}");
    if text.parse::<f64>().ok() != Some(value) {
        return Err(invalid("floating-point value does not round-trip exactly"));
    }
    parse_decimal(&text)
}

/// Parses decimal text into a scaled number.
pub(crate) fn parse_decimal(text: &str) -> Result<ParameterValue, BindError> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if (integer.is_empty() && fraction.is_empty())
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(format!("invalid number {text:?}")));
    }
    let fraction = fraction.trim_end_matches('0');
    let scale =
        u8::try_from(fraction.len()).map_err(|_| invalid("number has too many decimals"))?;
    let digits = format!("{integer}{fraction}");
    let magnitude = if digits.is_empty() {
        0
    } else {
        digits
            .parse::<i128>()
            .map_err(|_| invalid("number is out of range"))?
    };
    Ok(ParameterValue::Number {
        unscaled: if negative { -magnitude } else { magnitude },
        scale,
    })
}

fn decode_numeric(bytes: &[u8]) -> Result<String, BindError> {
    let header = |offset: usize| -> Result<u16, BindError> {
        bytes
            .get(offset..offset + 2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .ok_or_else(|| invalid("binary numeric is truncated"))
    };
    let count = usize::from(header(0)?);
    let weight = i32::from(header(2)? as i16);
    let sign = header(4)?;
    let scale = usize::from(header(6)?);
    if bytes.len() != 8 + count * 2 {
        return Err(invalid("binary numeric has a wrong length"));
    }
    if !matches!(sign, 0 | 0x4000) {
        return Err(invalid("binary numeric is NaN or infinite"));
    }
    let groups = (0..count)
        .map(|index| header(8 + index * 2))
        .collect::<Result<Vec<_>, _>>()?;
    if groups.iter().any(|group| *group > 9999) {
        return Err(invalid("binary numeric digit is out of range"));
    }
    let group_at = |index: i32| -> u16 {
        usize::try_from(index)
            .ok()
            .and_then(|index| groups.get(index).copied())
            .unwrap_or(0)
    };
    let mut text = String::new();
    if sign == 0x4000 {
        text.push('-');
    }
    if weight < 0 {
        text.push('0');
    } else {
        for index in 0..=weight {
            if index == 0 {
                write!(text, "{}", group_at(index)).expect("writing to String cannot fail");
            } else {
                write!(text, "{:04}", group_at(index)).expect("writing to String cannot fail");
            }
        }
    }
    if scale > 0 {
        text.push('.');
        let mut fraction = String::new();
        let mut index = weight + 1;
        while fraction.len() < scale {
            write!(fraction, "{:04}", group_at(index)).expect("writing to String cannot fail");
            index += 1;
        }
        fraction.truncate(scale);
        text.push_str(&fraction);
    }
    Ok(text)
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

fn decode_bytea_text(text: &str) -> Result<Vec<u8>, BindError> {
    if let Some(hex) = text.strip_prefix("\\x") {
        return decode_hex(hex).ok_or_else(|| invalid("invalid hexadecimal bytea"));
    }
    // The escape format: `\\` is a backslash, `\nnn` an octal byte.
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            output.push(bytes[index]);
            index += 1;
        } else if bytes.get(index + 1) == Some(&b'\\') {
            output.push(b'\\');
            index += 2;
        } else {
            let octal = bytes
                .get(index + 1..index + 4)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 8).ok())
                .ok_or_else(|| invalid("invalid escaped bytea"))?;
            output.push(octal);
            index += 4;
        }
    }
    Ok(output)
}

type Parts = (i64, u32, u32, i64, i64, i64);

fn parse_timestamp(text: &str, with_zone: bool) -> Result<Parts, BindError> {
    let error = || invalid(format!("invalid date or timestamp {text:?}"));
    let (date, rest) = text.split_once([' ', 'T']).unwrap_or((text, ""));
    let mut date_parts = date.splitn(3, '-');
    let (Some(year), Some(month), Some(day)) =
        (date_parts.next(), date_parts.next(), date_parts.next())
    else {
        return Err(error());
    };
    let year = year.parse::<i64>().map_err(|_| error())?;
    let month = month.parse::<u32>().map_err(|_| error())?;
    let day = day.parse::<u32>().map_err(|_| error())?;
    let rest = rest.trim();
    // The zone, when present, follows the time: `+03`, `-05:30`, `Z`.
    let (time, zone) = match rest.find(['+', '-', 'Z']) {
        Some(at) => (&rest[..at], Some(&rest[at..])),
        None => (rest, None),
    };
    let (mut hour, mut minute, mut second) = (0_i64, 0_i64, 0_i64);
    if !time.is_empty() {
        let mut parts = time.splitn(3, ':');
        hour = parts
            .next()
            .ok_or_else(error)?
            .parse()
            .map_err(|_| error())?;
        minute = parts
            .next()
            .ok_or_else(error)?
            .parse()
            .map_err(|_| error())?;
        // Fractions of a second are below the 1C date precision.
        let seconds = parts.next().unwrap_or("0");
        second = seconds
            .split('.')
            .next()
            .unwrap_or("0")
            .parse()
            .map_err(|_| error())?;
    }
    let mut offset_seconds = 0_i64;
    if let Some(zone) = zone.filter(|_| with_zone) {
        if zone != "Z" {
            let (sign, digits) = zone.split_at(1);
            let (hours, minutes) = digits.split_once(':').unwrap_or((digits, "0"));
            let hours = hours.parse::<i64>().map_err(|_| error())?;
            let minutes = minutes.parse::<i64>().map_err(|_| error())?;
            offset_seconds = (hours * 3600 + minutes * 60) * if sign == "-" { -1 } else { 1 };
        }
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(error());
    }
    // A zoned value is converted to UTC.
    let total = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second
        - offset_seconds;
    let days = total.div_euclid(86_400);
    let second_of_day = total.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Ok((
        year,
        month,
        day,
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60,
    ))
}

fn date_value(
    (year, month, day, hour, minute, second): Parts,
) -> Result<ParameterValue, BindError> {
    let narrow = |value: i64| u8::try_from(value).map_err(|_| invalid("date is out of range"));
    let year = u16::try_from(year).map_err(|_| invalid("date is out of range"))?;
    ParameterDate::new(
        year,
        u8::try_from(month).map_err(|_| invalid("date is out of range"))?,
        u8::try_from(day).map_err(|_| invalid("date is out of range"))?,
        narrow(hour)?,
        narrow(minute)?,
        narrow(second)?,
    )
    .map(ParameterValue::Date)
    .map_err(|error| invalid(error.to_string()))
}

fn decode_array(element: u32, binary: bool, bytes: &[u8]) -> Result<ParameterValue, BindError> {
    let mut items = Vec::new();
    if binary {
        let word = |offset: usize| -> Result<i32, BindError> {
            bytes
                .get(offset..offset + 4)
                .map(|word| i32::from_be_bytes([word[0], word[1], word[2], word[3]]))
                .ok_or_else(|| invalid("binary array is truncated"))
        };
        let dimensions = word(0)?;
        let declared =
            u32::try_from(word(8)?).map_err(|_| invalid("invalid array element type"))?;
        if declared != element {
            return Err(invalid("array element type differs from the declared type"));
        }
        match dimensions {
            0 => return Ok(ParameterValue::List(Vec::new())),
            1 => {}
            _ => return Err(BindError::UnsupportedType(element)),
        }
        let count = usize::try_from(word(12)?).map_err(|_| invalid("negative array length"))?;
        let mut offset = 20;
        for _ in 0..count {
            let length = word(offset)?;
            offset += 4;
            if length < 0 {
                items.push(ParameterValue::Null);
                continue;
            }
            let length = usize::try_from(length).map_err(|_| invalid("negative length"))?;
            let value = bytes
                .get(offset..offset + length)
                .ok_or_else(|| invalid("binary array is truncated"))?;
            offset += length;
            items.push(decode_scalar(element, true, value)?);
        }
        if offset != bytes.len() {
            return Err(invalid("binary array has trailing bytes"));
        }
    } else {
        for item in parse_text_array(text_of(bytes)?)? {
            items.push(match item {
                None => ParameterValue::Null,
                Some(text) => decode_scalar(element, false, text.as_bytes())?,
            });
        }
    }
    Ok(ParameterValue::List(items))
}

/// Splits a one-dimensional text array `{a,"b c",NULL}`.
pub(crate) fn parse_text_array(text: &str) -> Result<Vec<Option<String>>, BindError> {
    let inner = text
        .trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| invalid("array text is not enclosed in braces"))?;
    let mut items = Vec::new();
    if inner.trim().is_empty() {
        return Ok(items);
    }
    let mut characters = inner.chars().peekable();
    loop {
        while characters
            .peek()
            .is_some_and(|character| character.is_whitespace())
        {
            characters.next();
        }
        let mut item = String::new();
        let quoted = characters.peek() == Some(&'"');
        if quoted {
            characters.next();
            loop {
                match characters.next() {
                    Some('\\') => {
                        item.push(characters.next().ok_or_else(|| invalid("bad escape"))?)
                    }
                    Some('"') => break,
                    Some(character) => item.push(character),
                    None => return Err(invalid("unterminated quoted array element")),
                }
            }
            while characters
                .peek()
                .is_some_and(|character| character.is_whitespace())
            {
                characters.next();
            }
        } else {
            while let Some(&character) = characters.peek() {
                if character == ',' {
                    break;
                }
                if matches!(character, '{' | '}' | '"') {
                    return Err(BindError::Invalid(
                        "nested or malformed array text".to_owned(),
                    ));
                }
                item.push(character);
                characters.next();
            }
            item = item.trim().to_owned();
        }
        items.push(if !quoted && item.eq_ignore_ascii_case("NULL") {
            None
        } else {
            Some(item)
        });
        match characters.next() {
            None => break,
            Some(',') => {}
            Some(_) => return Err(invalid("malformed array text")),
        }
    }
    Ok(items)
}

#[cfg(test)]
#[path = "tests/values.rs"]
mod tests;
