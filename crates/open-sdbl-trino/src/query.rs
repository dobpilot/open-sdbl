use std::fmt::Write;

use crate::error::{ErrorCode, ServiceError};
use crate::filters::{FilterDomain, FilterRange};
use crate::model::{ColumnMetadata, ScanRequest, TableMetadata};
use crate::types::TrinoType;

/// A validated PostgreSQL scan and its owned bind values.
#[derive(Debug, Clone)]
pub struct PreparedScan {
    pub sql: String,
    pub parameters: Vec<String>,
    pub columns: Vec<ColumnMetadata>,
}

/// Compiles a structured connector request. Raw SQL is intentionally absent.
pub fn prepare_scan(
    table: &TableMetadata,
    request: &ScanRequest,
) -> Result<PreparedScan, ServiceError> {
    if request.schema != table.schema || request.table != table.name {
        return Err(ServiceError::new(
            ErrorCode::ObjectNotFound,
            format!(
                "object {:?}.{:?} does not match the resolved table",
                request.schema, request.table
            ),
        ));
    }
    let columns = request
        .columns
        .iter()
        .map(|name| {
            table.column(name).cloned().ok_or_else(|| {
                ServiceError::new(
                    ErrorCode::ColumnNotFound,
                    format!(
                        "column {name:?} does not exist in {}.{}",
                        table.schema, table.name
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let projection = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            Ok(format!(
                "{} AS {}",
                output_expression(column)?,
                quote_identifier(&format!("c{index}"))
            ))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;
    // Trino may legitimately request no columns for count-style scans. Keep a
    // constant projection so PostgreSQL still returns the correct row count.
    let projection = if projection.is_empty() {
        "NULL::text AS \"c0\"".to_owned()
    } else {
        projection.join(", ")
    };

    let mut parameters = Vec::new();
    let predicates = request
        .filters
        .iter()
        .map(|filter| compile_domain(table, filter, &mut parameters))
        .collect::<Result<Vec<_>, _>>()?;

    let mut sql = format!(
        "SELECT {projection} FROM {}",
        quote_identifier(&table.physical_table)
    );
    if !predicates.is_empty() {
        write!(sql, " WHERE {}", predicates.join(" AND ")).expect("writing to String cannot fail");
    }
    if let Some(limit) = request.limit {
        parameters.push(limit.to_string());
        write!(sql, " LIMIT CAST(${}::text AS bigint)", parameters.len())
            .expect("writing to String cannot fail");
    }
    Ok(PreparedScan {
        sql,
        parameters,
        columns,
    })
}

fn output_expression(column: &ColumnMetadata) -> Result<String, ServiceError> {
    if column.physical.is_empty() {
        return Err(ServiceError::new(
            ErrorCode::InvalidMetadata,
            format!("column {:?} has no physical members", column.name),
        ));
    }
    if column.physical.len() > 1 {
        if column.trino_type == TrinoType::Uuid {
            let payload = reference_member(column, "rrref")?;
            return Ok(format!(
                "CASE WHEN {payload} IS NULL THEN NULL ELSE encode({payload}::bytea, 'hex') END"
            ));
        }
        if column.trino_type == TrinoType::Varchar {
            let type_member = reference_member(column, "tref")?;
            let payload = reference_member(column, "rrref")?;
            return Ok(format!(
                "CASE WHEN {payload} IS NULL THEN NULL ELSE encode({type_member}::bytea, 'hex') || ':' || encode({payload}::bytea, 'hex') END"
            ));
        }
        let members = column
            .physical
            .iter()
            .flat_map(|member| {
                let key = quote_literal(&member.output_label);
                let value = quote_identifier(&member.name);
                [key, format!("{value}::text")]
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(format!("jsonb_build_object({members})::text"));
    }

    let physical = quote_identifier(&column.physical[0].name);
    Ok(match column.trino_type {
        TrinoType::Uuid => format!(
            "CASE WHEN {physical} IS NULL THEN NULL ELSE encode({physical}::bytea, 'hex') END"
        ),
        TrinoType::Varbinary => format!(
            "CASE WHEN {physical} IS NULL THEN NULL ELSE encode({physical}::bytea, 'base64') END"
        ),
        _ => format!("{physical}::text"),
    })
}

fn compile_domain(
    table: &TableMetadata,
    domain: &FilterDomain,
    parameters: &mut Vec<String>,
) -> Result<String, ServiceError> {
    let column = table.column(&domain.column).ok_or_else(|| {
        ServiceError::new(
            ErrorCode::ColumnNotFound,
            format!("filter column {:?} does not exist", domain.column),
        )
    })?;
    if !column.predicate_pushdown || column.physical.len() != 1 {
        return Err(ServiceError::new(
            ErrorCode::UnsupportedPredicate,
            format!(
                "predicate pushdown is not supported for column {:?}",
                domain.column
            ),
        ));
    }
    let physical = &column.physical[0];
    let expression = quote_identifier(&physical.name);
    if domain.all {
        return Ok(if domain.null_allowed {
            "TRUE".to_owned()
        } else {
            format!("{expression} IS NOT NULL")
        });
    }
    if domain.ranges.is_empty() {
        return Ok(if domain.null_allowed {
            format!("{expression} IS NULL")
        } else {
            "FALSE".to_owned()
        });
    }

    let mut ranges = Vec::with_capacity(domain.ranges.len());
    for range in &domain.ranges {
        ranges.push(compile_range(
            &expression,
            physical.data_type.as_str(),
            &column.trino_type,
            range,
            parameters,
        )?);
    }
    let union = format!("({})", ranges.join(" OR "));
    Ok(if domain.null_allowed {
        format!("({union} OR {expression} IS NULL)")
    } else {
        union
    })
}

fn compile_range(
    expression: &str,
    postgres_type: &str,
    trino_type: &TrinoType,
    range: &FilterRange,
    parameters: &mut Vec<String>,
) -> Result<String, ServiceError> {
    if let (Some(low), Some(high)) = (&range.low, &range.high)
        && low.inclusive
        && high.inclusive
        && low.value == high.value
    {
        let parameter = bind_parameter(&low.value, postgres_type, trino_type, parameters)?;
        return Ok(format!("{expression} = {parameter}"));
    }
    let mut parts = Vec::with_capacity(2);
    if let Some(low) = &range.low {
        let parameter = bind_parameter(&low.value, postgres_type, trino_type, parameters)?;
        parts.push(format!(
            "{expression} {} {parameter}",
            if low.inclusive { ">=" } else { ">" }
        ));
    }
    if let Some(high) = &range.high {
        let parameter = bind_parameter(&high.value, postgres_type, trino_type, parameters)?;
        parts.push(format!(
            "{expression} {} {parameter}",
            if high.inclusive { "<=" } else { "<" }
        ));
    }
    Ok(if parts.is_empty() {
        "TRUE".to_owned()
    } else {
        format!("({})", parts.join(" AND "))
    })
}

fn bind_parameter(
    value: &str,
    postgres_type: &str,
    trino_type: &TrinoType,
    parameters: &mut Vec<String>,
) -> Result<String, ServiceError> {
    parameters.push(value.to_owned());
    let placeholder = format!("${}", parameters.len());
    match trino_type {
        TrinoType::Uuid => Ok(format!("decode(replace({placeholder}, '-', ''), 'hex')")),
        TrinoType::Varbinary => Ok(format!("decode({placeholder}, 'base64')")),
        TrinoType::Json => Err(ServiceError::new(
            ErrorCode::UnsupportedPredicate,
            "JSON predicates are not pushed to PostgreSQL",
        )),
        _ => {
            if !safe_postgres_type(postgres_type) {
                return Err(ServiceError::new(
                    ErrorCode::InvalidMetadata,
                    format!("unsafe PostgreSQL type name {postgres_type:?}"),
                ));
            }
            Ok(format!("CAST({placeholder}::text AS {postgres_type})"))
        }
    }
}

fn reference_member(column: &ColumnMetadata, suffix: &str) -> Result<String, ServiceError> {
    column
        .physical
        .iter()
        .find(|member| member.name.to_ascii_lowercase().ends_with(suffix))
        .map(|member| quote_identifier(&member.name))
        .ok_or_else(|| {
            ServiceError::new(
                ErrorCode::InvalidMetadata,
                format!(
                    "reference column {:?} has no physical {suffix} member",
                    column.name
                ),
            )
        })
}

fn safe_postgres_type(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, ' ' | '_' | '(' | ')' | ',' | '.')
        })
}

/// Quotes a PostgreSQL identifier without accepting identifier fragments.
#[must_use]
pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use crate::filters::{FilterBound, FilterDomain, FilterRange};
    use crate::model::{ColumnMetadata, PhysicalColumn, ScanRequest, TableMetadata};
    use crate::types::TrinoType;

    use super::{prepare_scan, quote_identifier};

    fn table() -> TableMetadata {
        TableMetadata {
            schema: "Справочник".to_owned(),
            name: "Контрагенты".to_owned(),
            logical_name: "Контрагенты".to_owned(),
            object_guid: "b56f25d2-72a9-4d80-8998-77ac3097c873".to_owned(),
            physical_table: "_Reference35".to_owned(),
            columns: vec![ColumnMetadata {
                name: "ИНН".to_owned(),
                trino_type: TrinoType::Varchar,
                type_signature: "varchar".to_owned(),
                nullable: true,
                predicate_pushdown: true,
                comment: None,
                physical: vec![PhysicalColumn {
                    name: "_Fld36".to_owned(),
                    data_type: "mvarchar(12)".to_owned(),
                    output_label: "ИНН".to_owned(),
                }],
                reference_targets: vec![],
            }],
        }
    }

    #[test]
    fn quotes_identifiers_and_parameterizes_projection_filter_and_limit() {
        let request = ScanRequest {
            schema: "Справочник".to_owned(),
            table: "Контрагенты".to_owned(),
            columns: vec!["ИНН".to_owned()],
            filters: vec![FilterDomain {
                column: "ИНН".to_owned(),
                all: false,
                null_allowed: false,
                ranges: vec![FilterRange {
                    low: Some(FilterBound {
                        value: "7701234567".to_owned(),
                        inclusive: true,
                    }),
                    high: Some(FilterBound {
                        value: "7701234567".to_owned(),
                        inclusive: true,
                    }),
                }],
            }],
            limit: Some(10),
        };
        let scan = prepare_scan(&table(), &request).unwrap();
        assert_eq!(
            scan.sql,
            "SELECT \"_Fld36\"::text AS \"c0\" FROM \"_Reference35\" WHERE (\"_Fld36\" = CAST($1::text AS mvarchar(12))) LIMIT CAST($2::text AS bigint)"
        );
        assert_eq!(scan.parameters, ["7701234567", "10"]);
    }

    #[test]
    fn translates_null_and_not_null_domains() {
        for (all, null_allowed, expected) in [
            (false, true, "\"_Fld36\" IS NULL"),
            (true, false, "\"_Fld36\" IS NOT NULL"),
        ] {
            let request = ScanRequest {
                schema: "Справочник".to_owned(),
                table: "Контрагенты".to_owned(),
                columns: vec!["ИНН".to_owned()],
                filters: vec![FilterDomain {
                    column: "ИНН".to_owned(),
                    all,
                    null_allowed,
                    ranges: vec![],
                }],
                limit: None,
            };
            assert!(
                prepare_scan(&table(), &request)
                    .unwrap()
                    .sql
                    .ends_with(expected)
            );
        }
    }

    #[test]
    fn doubles_embedded_identifier_quotes() {
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
    }
}
