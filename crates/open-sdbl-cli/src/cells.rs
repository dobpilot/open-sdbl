//! Typed result cells shared by both database drivers and the table printer.
//!
//! Drivers decode native values into [`Cell`]; rendering to text happens once
//! at print time with one provider-independent policy: bytes as `0x` plus
//! upper-case hexadecimal, booleans as `true`/`false`, date-times as
//! `YYYY-MM-DD HH:MM:SS` without fractional seconds, numbers with their
//! declared scale, and UUIDs in canonical lower-case form.

use std::borrow::Cow;
use std::fmt::Write as _;

/// One decoded result value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cell {
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
    pub(crate) fn render(&self) -> Cow<'_, str> {
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
    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the bytes of a binary cell.
    pub(crate) fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Whether the cell is SQL `NULL`.
    pub(crate) const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Broken-down date and time in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DateTimeParts {
    pub(crate) year: i64,
    pub(crate) month: u32,
    pub(crate) day: u32,
    pub(crate) hour: u32,
    pub(crate) minute: u32,
    pub(crate) second: u32,
}

impl DateTimeParts {
    /// Builds parts from days since 1970-01-01 and whole seconds since
    /// midnight; fractional seconds are dropped by design.
    pub(crate) fn from_unix_days(days: i64, seconds_of_day: u32) -> Self {
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
mod tests {
    use super::{
        Cell, DateTimeParts, civil_from_days, decode_postgres_numeric, format_binary,
        format_scaled_integer, format_uuid,
    };

    fn numeric(digits: &[u16], weight: i16, sign: u16, scale: u16) -> Vec<u8> {
        let mut raw = Vec::new();
        raw.extend_from_slice(&(digits.len() as u16).to_be_bytes());
        raw.extend_from_slice(&weight.to_be_bytes());
        raw.extend_from_slice(&sign.to_be_bytes());
        raw.extend_from_slice(&scale.to_be_bytes());
        for digit in digits {
            raw.extend_from_slice(&digit.to_be_bytes());
        }
        raw
    }

    #[test]
    fn renders_cells_with_one_policy() {
        assert_eq!(Cell::Null.render(), "NULL");
        assert_eq!(Cell::Bytes(vec![0, 0x7d, 0xd6]).render(), "0x007DD6");
        assert_eq!(Cell::Bool(true).render(), "true");
        assert_eq!(Cell::Bool(false).render(), "false");
        assert_eq!(Cell::Number("15.50".to_owned()).render(), "15.50");
        assert_eq!(
            Cell::DateTime(DateTimeParts::from_unix_days(19_782, 45_296)).render(),
            "2024-02-29 12:34:56"
        );
        let uuid = [
            0xd2, 0xf8, 0xbd, 0xe9, 0xfa, 0xdd, 0x4b, 0xe8, 0x90, 0x22, 0x24, 0x9e, 0x3a, 0x1a,
            0xc4, 0xb9,
        ];
        assert_eq!(
            Cell::Uuid(uuid).render(),
            "d2f8bde9-fadd-4be8-9022-249e3a1ac4b9"
        );
        assert_eq!(
            format_uuid(&[0; 16]),
            "00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(format_binary(&[]), "0x");
    }

    #[test]
    fn converts_days_to_civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(-719_162), (1, 1, 1));
        assert_eq!(civil_from_days(-25_567), (1900, 1, 1));
        assert_eq!(civil_from_days(2_932_896), (9999, 12, 31));
    }

    #[test]
    fn formats_scaled_integers_with_declared_scale() {
        assert_eq!(format_scaled_integer(1550, 2), "15.50");
        assert_eq!(format_scaled_integer(-5, 1), "-0.5");
        assert_eq!(format_scaled_integer(15, 0), "15");
        assert_eq!(format_scaled_integer(0, 3), "0.000");
    }

    #[test]
    fn decodes_postgres_numeric_wire_format() {
        assert_eq!(
            decode_postgres_numeric(&numeric(&[15, 5000], 0, 0, 2)).unwrap(),
            "15.50"
        );
        assert_eq!(
            decode_postgres_numeric(&numeric(&[12, 3456], 1, 0x4000, 0)).unwrap(),
            "-123456"
        );
        assert_eq!(
            decode_postgres_numeric(&numeric(&[5], -1, 0, 4)).unwrap(),
            "0.0005"
        );
        assert_eq!(
            decode_postgres_numeric(&numeric(&[1], 0, 0, 3)).unwrap(),
            "1.000"
        );
        assert_eq!(
            decode_postgres_numeric(&numeric(&[], 0, 0, 0)).unwrap(),
            "0"
        );
        assert_eq!(
            decode_postgres_numeric(&numeric(&[], 0, 0xC000, 0)).unwrap(),
            "NaN"
        );
        assert!(decode_postgres_numeric(&[0, 1]).is_err());
    }
}
