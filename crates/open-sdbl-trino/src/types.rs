use serde::{Deserialize, Serialize};

use crate::model::PhysicalColumn;

/// Types understood by both the Rust service and the Java Trino adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TrinoType {
    Boolean,
    Integer,
    Bigint,
    Double,
    Decimal { precision: u8, scale: u8 },
    Varchar,
    Date,
    Timestamp { precision: u8 },
    Uuid,
    Varbinary,
    Json,
}

impl TrinoType {
    #[must_use]
    pub fn signature(&self) -> String {
        match self {
            Self::Boolean => "boolean".to_owned(),
            Self::Integer => "integer".to_owned(),
            Self::Bigint => "bigint".to_owned(),
            Self::Double => "double".to_owned(),
            Self::Decimal { precision, scale } => format!("decimal({precision},{scale})"),
            Self::Varchar => "varchar".to_owned(),
            Self::Date => "date".to_owned(),
            Self::Timestamp { precision } => format!("timestamp({precision})"),
            Self::Uuid => "uuid".to_owned(),
            Self::Varbinary => "varbinary".to_owned(),
            Self::Json => "json".to_owned(),
        }
    }
}

/// Determines the public type of one logical field.
#[must_use]
pub fn map_logical_type(columns: &[PhysicalColumn], reference_targets: &[String]) -> TrinoType {
    if columns.len() > 1 {
        let reference_payloads = columns
            .iter()
            .filter(|column| column.name.to_ascii_lowercase().ends_with("rrref"))
            .count();
        let only_reference_members = columns.iter().all(|column| {
            let name = column.name.to_ascii_lowercase();
            name.ends_with("tref") || name.ends_with("rrref")
        });
        if reference_payloads == 1 && only_reference_members {
            return if reference_targets.len() <= 1 {
                TrinoType::Uuid
            } else {
                TrinoType::Varchar
            };
        }
        return TrinoType::Json;
    }

    let Some(column) = columns.first() else {
        return TrinoType::Json;
    };
    let data_type = column.data_type.trim().to_ascii_lowercase();
    if data_type == "boolean" || data_type == "bool" {
        TrinoType::Boolean
    } else if matches!(data_type.as_str(), "smallint" | "integer" | "int2" | "int4") {
        TrinoType::Integer
    } else if matches!(data_type.as_str(), "bigint" | "int8") {
        TrinoType::Bigint
    } else if matches!(
        data_type.as_str(),
        "real" | "double precision" | "float4" | "float8"
    ) {
        TrinoType::Double
    } else if let Some((precision, scale)) = parse_numeric(&data_type) {
        if scale == 0 && precision <= 9 {
            TrinoType::Integer
        } else if scale == 0 && precision <= 18 {
            TrinoType::Bigint
        } else {
            TrinoType::Decimal {
                precision: precision.min(38) as u8,
                scale: scale.min(precision.min(38)) as u8,
            }
        }
    } else if data_type == "date" {
        TrinoType::Date
    } else if data_type.starts_with("timestamp") {
        TrinoType::Timestamp { precision: 3 }
    } else if data_type == "bytea" {
        if !reference_targets.is_empty() || column.name.to_ascii_lowercase().ends_with("rref") {
            TrinoType::Uuid
        } else {
            TrinoType::Varbinary
        }
    } else {
        TrinoType::Varchar
    }
}

/// Maps a PostgreSQL prepared-statement result type to the transport type used
/// by the polymorphic SDBL table function.
#[must_use]
pub fn map_statement_type(data_type: &str, type_modifier: i32) -> TrinoType {
    match data_type.trim().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => TrinoType::Boolean,
        "int2" | "int4" | "smallint" | "integer" => TrinoType::Integer,
        "int8" | "bigint" => TrinoType::Bigint,
        "float4" | "float8" | "real" | "double precision" => TrinoType::Double,
        "numeric" | "decimal" => numeric_statement_type(type_modifier),
        "date" => TrinoType::Date,
        "timestamp" | "timestamp without time zone" => TrinoType::Timestamp { precision: 3 },
        "uuid" => TrinoType::Uuid,
        "bytea" => TrinoType::Varbinary,
        _ => TrinoType::Varchar,
    }
}

fn numeric_statement_type(type_modifier: i32) -> TrinoType {
    let Some(modifier) = type_modifier
        .checked_sub(4)
        .filter(|modifier| *modifier >= 0)
    else {
        // PostgreSQL does not report precision/scale for expressions such as
        // SUM(numeric). Text is exact; guessing a decimal would be lossy.
        return TrinoType::Varchar;
    };
    let precision = u16::try_from((modifier >> 16) & 0xffff).unwrap_or(38);
    let scale = u16::try_from(modifier & 0xffff).unwrap_or_default();
    if precision == 0 || precision > 38 || scale > precision {
        return TrinoType::Varchar;
    }
    TrinoType::Decimal {
        precision: precision as u8,
        scale: scale as u8,
    }
}

fn parse_numeric(data_type: &str) -> Option<(u16, u16)> {
    let arguments = data_type
        .strip_prefix("numeric(")
        .or_else(|| data_type.strip_prefix("decimal("))?
        .strip_suffix(')')?;
    let (precision, scale) = arguments.split_once(',').unwrap_or((arguments, "0"));
    Some((precision.trim().parse().ok()?, scale.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::{TrinoType, map_logical_type, map_statement_type};
    use crate::model::PhysicalColumn;

    fn column(name: &str, data_type: &str) -> PhysicalColumn {
        PhysicalColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
            output_label: name.to_owned(),
        }
    }

    #[test]
    fn maps_scalar_postgres_types_without_varchar_fallback_for_everything() {
        assert_eq!(
            map_logical_type(&[column("_Fld1", "boolean")], &[]),
            TrinoType::Boolean
        );
        assert_eq!(
            map_logical_type(&[column("_Fld1", "numeric(8,0)")], &[]),
            TrinoType::Integer
        );
        assert_eq!(
            map_logical_type(&[column("_Fld1", "numeric(15,0)")], &[]),
            TrinoType::Bigint
        );
        assert_eq!(
            map_logical_type(&[column("_Fld1", "numeric(15,2)")], &[]),
            TrinoType::Decimal {
                precision: 15,
                scale: 2
            }
        );
        assert_eq!(
            map_logical_type(&[column("_Fld1", "timestamp without time zone")], &[]),
            TrinoType::Timestamp { precision: 3 }
        );
        assert_eq!(
            map_logical_type(&[column("_Fld1", "bytea")], &[]),
            TrinoType::Varbinary
        );
    }

    #[test]
    fn distinguishes_fixed_and_polymorphic_references() {
        let members = [
            column("_Fld1_TRef", "bytea"),
            column("_Fld1_RRRef", "bytea"),
        ];
        assert_eq!(
            map_logical_type(&members, &["Reference1".to_owned()]),
            TrinoType::Uuid
        );
        assert_eq!(
            map_logical_type(&members, &["Reference1".to_owned(), "Document2".to_owned()]),
            TrinoType::Varchar
        );
    }

    #[test]
    fn exposes_unrepresentable_compound_values_as_json() {
        let members = [
            column("_Fld1_TYPE", "bytea"),
            column("_Fld1_S", "mvarchar(100)"),
        ];
        assert_eq!(map_logical_type(&members, &[]), TrinoType::Json);
    }

    #[test]
    fn maps_prepared_statement_numeric_typmods_without_guessing_unknown_scale() {
        let numeric_15_2 = 4 + (15 << 16) + 2;
        assert_eq!(
            map_statement_type("numeric", numeric_15_2),
            TrinoType::Decimal {
                precision: 15,
                scale: 2
            }
        );
        assert_eq!(map_statement_type("numeric", -1), TrinoType::Varchar);
    }
}
