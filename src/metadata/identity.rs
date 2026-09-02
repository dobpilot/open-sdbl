use std::fmt;

use super::Guid;

macro_rules! metadata_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Creates an ID from the bytes of the real 1C metadata GUID.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            /// Returns the real 1C metadata GUID bytes.
            #[must_use]
            pub const fn as_bytes(self) -> [u8; 16] {
                self.0
            }
        }

        impl From<&Guid> for $name {
            fn from(guid: &Guid) -> Self {
                Self(guid.to_bytes())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for (index, byte) in self.0.iter().enumerate() {
                    if matches!(index, 4 | 6 | 8 | 10) {
                        formatter.write_str("-")?;
                    }
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

metadata_id!(
    ObjectId,
    "A stable object identity containing its real 1C metadata GUID."
);
metadata_id!(
    AttributeId,
    "A stable custom-attribute identity containing its real 1C metadata GUID."
);

/// Stable numeric IDs for platform standard fields, which have no Config GUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
pub enum StandardFieldId {
    /// Object reference (`Ссылка`).
    Id = 1,
    /// Catalog code (`Код`).
    Code = 2,
    /// Catalog description (`Наименование`).
    Description = 3,
    /// Deletion mark (`ПометкаУдаления`).
    Marked = 4,
    /// Data version (`ВерсияДанных`).
    Version = 5,
    /// Document number (`Номер`).
    Number = 6,
    /// Document date (`Дата`).
    Date = 7,
    /// Posted flag (`Проведен`).
    Posted = 8,
    /// Register recorder (`Регистратор`).
    Recorder = 9,
    /// Tabular-section line number (`НомерСтроки`).
    LineNo = 10,
    /// Register period (`Период`).
    Period = 11,
    /// Register activity flag (`Активность`).
    Active = 12,
}

impl StandardFieldId {
    /// Resolves a canonical, Russian, or English standard-field name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = name.to_lowercase();
        match normalized.as_str() {
            "id" | "ссылка" => Some(Self::Id),
            "code" | "код" => Some(Self::Code),
            "description" | "наименование" => Some(Self::Description),
            "marked" | "пометкаудаления" => Some(Self::Marked),
            "version" | "версияданных" => Some(Self::Version),
            "number" | "номер" => Some(Self::Number),
            "date" | "date_time" | "дата" => Some(Self::Date),
            "posted" | "проведен" | "проведён" => Some(Self::Posted),
            "recorder" | "регистратор" => Some(Self::Recorder),
            "lineno" | "номерстроки" => Some(Self::LineNo),
            "period" | "период" => Some(Self::Period),
            "active" | "активность" => Some(Self::Active),
            _ => None,
        }
    }

    /// Returns the canonical SchemaStorage field name.
    #[must_use]
    pub const fn schema_name(self) -> &'static str {
        match self {
            Self::Id => "ID",
            Self::Code => "Code",
            Self::Description => "Description",
            Self::Marked => "Marked",
            Self::Version => "Version",
            Self::Number => "Number",
            Self::Date => "Date",
            Self::Posted => "Posted",
            Self::Recorder => "Recorder",
            Self::LineNo => "LineNo",
            Self::Period => "Period",
            Self::Active => "Active",
        }
    }
}

/// Stable field identity used by the presentation protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldId {
    /// A Config-declared custom attribute.
    Metadata(AttributeId),
    /// A platform standard field without a Config GUID.
    Standard(StandardFieldId),
}

/// Typed metadata index lookup failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// No matching object exists.
    ObjectNotFound,
    /// More than one matching object exists.
    AmbiguousObject,
    /// No matching field exists on the owner.
    FieldNotFound,
    /// More than one matching field exists on the owner.
    AmbiguousField,
    /// No matching predefined value exists on the owner.
    ValueNotFound,
    /// More than one matching predefined value exists on the owner.
    AmbiguousValue,
    /// The requested field is standard and therefore has no metadata GUID.
    StandardFieldHasNoMetadataGuid(StandardFieldId),
    /// The supplied owner GUID does not identify a resolved object.
    OwnerNotFound,
}

impl fmt::Display for LookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectNotFound => formatter.write_str("metadata object was not found"),
            Self::AmbiguousObject => formatter.write_str("metadata object name is ambiguous"),
            Self::FieldNotFound => formatter.write_str("metadata field was not found"),
            Self::AmbiguousField => formatter.write_str("metadata field name is ambiguous"),
            Self::ValueNotFound => formatter.write_str("metadata value was not found"),
            Self::AmbiguousValue => formatter.write_str("metadata value name is ambiguous"),
            Self::StandardFieldHasNoMetadataGuid(field) => write!(
                formatter,
                "standard field {} has no Config metadata GUID",
                field.schema_name()
            ),
            Self::OwnerNotFound => formatter.write_str("metadata field owner was not found"),
        }
    }
}

impl std::error::Error for LookupError {}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{AttributeId, Guid, ObjectId, StandardFieldId};

    #[test]
    fn ids_preserve_real_guid_bytes() {
        let guid = Guid::from_str("b56f25d2-72a9-4d80-8998-77ac3097c873").unwrap();
        let object = ObjectId::from(&guid);
        let attribute = AttributeId::from(&guid);
        assert_eq!(object.as_bytes(), attribute.as_bytes());
        assert_eq!(object.to_string(), guid.as_str());
    }

    #[test]
    fn standard_names_are_bilingual() {
        assert_eq!(
            StandardFieldId::from_name("Наименование"),
            Some(StandardFieldId::Description)
        );
        assert_eq!(
            StandardFieldId::from_name("Code"),
            Some(StandardFieldId::Code)
        );
        assert_eq!(
            StandardFieldId::from_name("date_time"),
            Some(StandardFieldId::Date)
        );
    }
}
