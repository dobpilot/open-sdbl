//! Type values produced by `ТИП(…)` and `ТИПЗНАЧЕНИЯ(…)`.
//!
//! A type value travels through SQL as five bytes: the tag the platform
//! stores in the `_TYPE` member of a composite field, measured on the
//! platform (`0x01` undefined, `0x02` boolean, `0x03` number, `0x04`
//! date, `0x05` string, `0x08` reference; `0x00` for the type of `NULL`,
//! which has no storage tag) followed by the big-endian `RTRef` table
//! number, zero for anything but a reference.

use crate::metadata::{MetadataKind, MetadataSnapshot};
use crate::query::core::resolve::kind_query_name;

/// The decoded value of a column of kind [`ColumnKind::Type`].
///
/// [`ColumnKind::Type`]: crate::query::ColumnKind::Type
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeValue {
    /// The type of `NULL`.
    Null,
    /// `Неопределено`.
    Undefined,
    /// `Булево`.
    Boolean,
    /// `Число`.
    Number,
    /// `Строка`.
    String,
    /// `Дата`.
    Date,
    /// A reference to the object with this `RTRef` table number.
    Reference(u32),
}

impl TypeValue {
    /// Length of the encoded representation in bytes.
    pub const ENCODED_LEN: usize = 5;

    const TAG_NULL: u8 = 0x00;
    const TAG_UNDEFINED: u8 = 0x01;
    const TAG_BOOLEAN: u8 = 0x02;
    const TAG_NUMBER: u8 = 0x03;
    const TAG_DATE: u8 = 0x04;
    const TAG_STRING: u8 = 0x05;
    /// The tag of a reference; the same byte marks references in the
    /// `_TYPE` member of a composite field.
    pub const TAG_REFERENCE: u8 = 0x08;

    /// The platform's `_TYPE` tag of this type.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Null => Self::TAG_NULL,
            Self::Undefined => Self::TAG_UNDEFINED,
            Self::Boolean => Self::TAG_BOOLEAN,
            Self::Number => Self::TAG_NUMBER,
            Self::String => Self::TAG_STRING,
            Self::Date => Self::TAG_DATE,
            Self::Reference(_) => Self::TAG_REFERENCE,
        }
    }

    /// Encodes the type as its five-byte SQL representation.
    #[must_use]
    pub fn encode(self) -> [u8; Self::ENCODED_LEN] {
        let number = match self {
            Self::Reference(number) => number,
            _ => 0,
        };
        let mut bytes = [0; Self::ENCODED_LEN];
        bytes[0] = self.tag();
        bytes[1..].copy_from_slice(&number.to_be_bytes());
        bytes
    }

    /// Decodes a five-byte SQL representation; `None` for any other
    /// length or an unknown tag.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let [tag, rest @ ..] = bytes else {
            return None;
        };
        let number = u32::from_be_bytes(rest.try_into().ok()?);
        match *tag {
            Self::TAG_NULL => Some(Self::Null),
            Self::TAG_UNDEFINED => Some(Self::Undefined),
            Self::TAG_BOOLEAN => Some(Self::Boolean),
            Self::TAG_NUMBER => Some(Self::Number),
            Self::TAG_STRING => Some(Self::String),
            Self::TAG_DATE => Some(Self::Date),
            Self::TAG_REFERENCE => Some(Self::Reference(number)),
            _ => None,
        }
    }

    /// The name of the type as written in a query: `Строка`, `Число`,
    /// `Дата`, `Булево`, `Неопределено`, `Null`, or `<Вид>.<Имя>` of the
    /// referenced object. A reference whose object the snapshot does not
    /// know is named by its table number.
    #[must_use]
    pub fn query_name(self, snapshot: &MetadataSnapshot) -> String {
        match self {
            Self::Null => "Null".to_owned(),
            Self::Undefined => "Неопределено".to_owned(),
            Self::Boolean => "Булево".to_owned(),
            Self::Number => "Число".to_owned(),
            Self::String => "Строка".to_owned(),
            Self::Date => "Дата".to_owned(),
            Self::Reference(number) => snapshot
                .object_id_by_database_type(number)
                .ok()
                .and_then(|id| snapshot.object_by_id(id))
                .and_then(|object| {
                    let kind: MetadataKind = object.kind?;
                    let name = object.name.as_deref()?;
                    Some(format!("{}.{name}", kind_query_name(Some(kind))))
                })
                .unwrap_or_else(|| format!("Ссылка.{number}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TypeValue;

    #[test]
    fn round_trips_every_tag() {
        for value in [
            TypeValue::Null,
            TypeValue::Undefined,
            TypeValue::Boolean,
            TypeValue::Number,
            TypeValue::String,
            TypeValue::Date,
            TypeValue::Reference(0x35),
        ] {
            assert_eq!(TypeValue::decode(&value.encode()), Some(value));
        }
        assert_eq!(
            TypeValue::Reference(0x35).encode(),
            [0x08, 0x00, 0x00, 0x00, 0x35]
        );
        assert_eq!(TypeValue::decode(&[0x04, 0, 0, 0]), None);
        assert_eq!(TypeValue::decode(&[0x09, 0, 0, 0, 0]), None);
    }
}
