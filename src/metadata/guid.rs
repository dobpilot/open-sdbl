use std::fmt;
use std::str::FromStr;

use super::MetadataError;

/// A validated, canonical lowercase 1C metadata GUID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Guid(String);

impl Guid {
    /// The all-zero GUID used by platform-owned DBNames entries.
    pub const NIL: &'static str = "00000000-0000-0000-0000-000000000000";

    /// Returns the canonical lowercase textual representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether this is the all-zero GUID.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0 == Self::NIL
    }

    /// Returns the GUID as 16 bytes in canonical textual (network) order.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut bytes = [0_u8; 16];
        let mut high = None;
        let mut output = 0;
        for byte in self.0.bytes().filter(|byte| *byte != b'-') {
            let nibble = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => unreachable!("a Guid is validated at construction"),
            };
            if let Some(high) = high.take() {
                bytes[output] = (high << 4) | nibble;
                output += 1;
            } else {
                high = Some(nibble);
            }
        }
        bytes
    }
}

impl FromStr for Guid {
    type Err = MetadataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 36 {
            return Err(MetadataError::new(format!("invalid GUID {value:?}")));
        }
        for (index, byte) in value.bytes().enumerate() {
            let separator = matches!(index, 8 | 13 | 18 | 23);
            if (separator && byte != b'-') || (!separator && !byte.is_ascii_hexdigit()) {
                return Err(MetadataError::new(format!("invalid GUID {value:?}")));
            }
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Guid;
    use std::str::FromStr;

    #[test]
    fn validates_and_canonicalizes_guid() {
        let guid = Guid::from_str("B56F25D2-72A9-4D80-8998-77AC3097C873").unwrap();
        assert_eq!(guid.as_str(), "b56f25d2-72a9-4d80-8998-77ac3097c873");
        assert_eq!(guid.to_bytes()[..4], [0xb5, 0x6f, 0x25, 0xd2]);
        assert!(Guid::from_str("b56f25d2").is_err());
    }
}
