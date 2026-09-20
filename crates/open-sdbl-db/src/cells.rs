//! Typed result cells shared by both database drivers and their callers.
//!
//! Drivers decode native values into [`Cell`]; rendering to text happens once
//! at print time with one provider-independent policy: bytes as `0x` plus
//! upper-case hexadecimal, booleans as `true`/`false`, date-times as
//! `YYYY-MM-DD HH:MM:SS` without fractional seconds, numbers with their
//! declared scale, and UUIDs in canonical lower-case form.

/// The rows a query returns, as the providers hand them over.
pub type QueryRows = Vec<Vec<Cell>>;

use std::borrow::Cow;
use std::fmt::Write as _;

/// One decoded result value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// SQL `NULL`.
    Null,
    /// Character data.
    Text(String),
    /// Raw bytes: references, `RTRef` discriminators, row versions.
    Bytes(Vec<u8>),
    /// A number already formatted as decimal text with its declared scale.
    Number(String),
    /// A boolean.
    Bool(bool),
    /// A calendar date and time of day.
    DateTime(DateTimeParts),
    /// A UUID in canonical byte order.
    Uuid([u8; 16]),
}

impl Cell {
    /// Renders the cell for terminal output; `NULL` for absent values.
    pub fn render(&self) -> Cow<'_, str> {
        match self {
            Self::Null => Cow::Borrowed("NULL"),
            Self::Text(text) => Cow::Borrowed(text),
            Self::Number(number) => Cow::Borrowed(number),
            Self::Bytes(bytes) => Cow::Owned(format_binary(bytes)),
            Self::Bool(value) => Cow::Borrowed(if *value { "true" } else { "false" }),
            Self::DateTime(parts) => Cow::Owned(parts.to_string()),
            Self::Uuid(bytes) => Cow::Owned(format_uuid(bytes)),
        }
    }

    /// Returns the text of a textual cell.
    /// The text of a character cell.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the bytes of a binary cell.
    /// The bytes of a binary cell.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Whether the cell is SQL `NULL`.
    /// Whether the value is SQL `NULL`.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Broken-down date and time in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeParts {
    /// The year, as the provider reported it.
    pub year: i64,
    /// The month, from 1 to 12.
    pub month: u32,
    /// The day of the month, from 1.
    pub day: u32,
    /// The hour, from 0 to 23.
    pub hour: u32,
    /// The minute, from 0 to 59.
    pub minute: u32,
    /// The second, from 0 to 59.
    pub second: u32,
}

impl DateTimeParts {
    /// Builds parts from days since 1970-01-01 and whole seconds since
    /// midnight; fractional seconds are dropped by design.
    pub fn from_unix_days(days: i64, seconds_of_day: u32) -> Self {
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: seconds_of_day / 3600,
            minute: seconds_of_day % 3600 / 60,
            second: seconds_of_day % 60,
        }
    }
}

impl std::fmt::Display for DateTimeParts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Days between 0001-01-01 and 1970-01-01 in the proleptic Gregorian calendar.
pub(crate) const DAYS_FROM_YEAR_ONE_TO_UNIX_EPOCH: i64 = 719_162;
/// Days between 1900-01-01 and 1970-01-01.
pub(crate) const DAYS_FROM_1900_TO_UNIX_EPOCH: i64 = 25_567;
/// Days between 1970-01-01 and 2000-01-01.
pub(crate) const DAYS_FROM_UNIX_EPOCH_TO_2000: i64 = 10_957;

/// Converts days since 1970-01-01 into a civil date (Howard Hinnant's
/// `civil_from_days`).
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (
        year,
        u32::try_from(month).unwrap_or(0),
        u32::try_from(day).unwrap_or(0),
    )
}

/// Formats bytes as `0x` followed by upper-case hexadecimal digits.
pub(crate) fn format_binary(value: &[u8]) -> String {
    let mut output = String::with_capacity(2 + value.len() * 2);
    output.push_str("0x");
    for byte in value {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
}

/// Formats a UUID in canonical lower-case `8-4-4-4-12` form.
pub(crate) fn format_uuid(bytes: &[u8; 16]) -> String {
    let mut output = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            output.push('-');
        }
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

/// Formats a scaled integer (`value / 10^scale`) as decimal text.
pub(crate) fn format_scaled_integer(value: i128, scale: u32) -> String {
    if scale == 0 {
        return value.to_string();
    }
    let divisor = 10_i128.pow(scale);
    let magnitude = value.unsigned_abs();
    let integer = magnitude / divisor.unsigned_abs();
    let fraction = magnitude % divisor.unsigned_abs();
    format!(
        "{}{integer}.{fraction:0width$}",
        if value < 0 { "-" } else { "" },
        width = scale as usize
    )
}

/// Decodes the PostgreSQL binary `numeric` representation: four big-endian
/// 16-bit headers (digit count, weight, sign, display scale) followed by
/// base-10000 digits.
pub(crate) fn decode_postgres_numeric(raw: &[u8]) -> Result<String, String> {
    let header = |offset: usize| -> Result<u16, String> {
        raw.get(offset..offset + 2)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
            .ok_or_else(|| "numeric value is truncated".to_owned())
    };
    let digit_count = usize::from(header(0)?);
    let weight = i32::from(header(2)? as i16);
    let sign = header(4)?;
    let display_scale = usize::from(header(6)?);
    match sign {
        0xC000 => return Ok("NaN".to_owned()),
        0xD000 => return Ok("Infinity".to_owned()),
        0xF000 => return Ok("-Infinity".to_owned()),
        0x0000 | 0x4000 => {}
        other => return Err(format!("numeric sign {other:#06x} is unknown")),
    }
    let digits = (0..digit_count)
        .map(|index| header(8 + index * 2))
        .collect::<Result<Vec<_>, _>>()?;
    let digit_at = |index: i32| -> u16 {
        usize::try_from(index)
            .ok()
            .and_then(|index| digits.get(index).copied())
            .unwrap_or(0)
    };

    let mut output = String::new();
    if sign == 0x4000 {
        output.push('-');
    }
    if weight < 0 {
        output.push('0');
    } else {
        for index in 0..=weight {
            let digit = digit_at(index);
            if index == 0 {
                write!(output, "{digit}").expect("writing to String cannot fail");
            } else {
                write!(output, "{digit:04}").expect("writing to String cannot fail");
            }
        }
    }
    if display_scale > 0 {
        output.push('.');
        let mut fraction = String::with_capacity(display_scale + 4);
        let mut index = weight + 1;
        while fraction.len() < display_scale {
            write!(fraction, "{:04}", digit_at(index)).expect("writing to String cannot fail");
            index += 1;
        }
        fraction.truncate(display_scale);
        output.push_str(&fraction);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "tests/cells.rs"]
mod tests;
