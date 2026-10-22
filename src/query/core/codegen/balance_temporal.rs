//! Измеренная граница остатков через прежние литералы и каталог компиляции.
use super::*;
use crate::query::{BoundaryKind, ParameterBoundaryValue, ParameterPointInTime};

pub(super) struct Cut {
    pub(super) date: String,
    pub(super) before: String,
}

pub(super) fn compile(
    expression: Option<&Expression<'_, '_>>,
    catalog: &CompilationCatalog<'_>,
    snapshot: &MetadataSnapshot,
    fields: &[QueryableField],
    period: &QueryableColumn,
    dialect: SqlDialect,
) -> Result<Option<Cut>, QueryDiagnostic> {
    let Some(Expression::Parameter(token)) = expression else {
        return Ok(None);
    };
    let Some(value) = catalog.parameters().lookup(token)? else {
        return Ok(None);
    };
    let (date, reference, including) = match value {
        ParameterValue::PointInTime(ParameterPointInTime { date, reference }) => {
            (*date, *reference, false)
        }
        ParameterValue::Boundary(boundary) => {
            let (date, reference) = match boundary.value {
                ParameterBoundaryValue::Date(date) => (date, None),
                ParameterBoundaryValue::PointInTime(point) => (point.date, point.reference),
            };
            (date, reference, boundary.kind == BoundaryKind::Including)
        }
        _ => return Ok(None),
    };
    let date = render_scalar_parameter(&ParameterValue::Date(date), token, dialect, true)?;
    let period = dialect.qualified_column(Some("__movement_base"), &period.physical_name);
    let diagnostic = |message| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            message,
        )
    };
    // Тип сохраняется и проверяется даже у пустой ссылки. Нативный замер
    // temporal-balance-native-1019: пустой RRRef задаёт границу всей секунды.
    let reference = reference
        .map(|reference| {
            let object = snapshot
                .object_by_id(reference.object)
                .ok_or_else(|| diagnostic("point reference metadata is missing"))?;
            if object.kind != Some(crate::metadata::MetadataKind::Document) {
                return Err(diagnostic(
                    "Balance point references support only acquired documents",
                ));
            }
            let number =
                super::super::params::object_type_number(reference.object, token, snapshot)?;
            Ok((reference, number))
        })
        .transpose()?;
    let Some((reference, number)) = reference.filter(|(reference, _)| reference.id != [0; 16])
    else {
        return Ok(Some(Cut {
            before: format!("{period} {} {date}", if including { "<=" } else { "<" }),
            date,
        }));
    };
    let recorder = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "Recorder"))
        .ok_or_else(|| diagnostic("Balance point requires a live Recorder field"))?;
    let id = recorder
        .columns
        .iter()
        .find(|column| column.is_reference_value_member())
        .ok_or_else(|| diagnostic("Balance point requires a live Recorder RRRef"))?;
    let id = dialect.qualified_column(Some("__movement_base"), &id.physical_name);
    let value = dialect.binary_literal(&reference.id);
    let comparison = if including { "<=" } else { "<" };
    let recorder_before = if let Some(type_column) = recorder
        .columns
        .iter()
        .find(|column| column.is_reference_type_member())
    {
        let type_sql =
            dialect.qualified_column(Some("__movement_base"), &type_column.physical_name);
        let type_value = dialect.binary_u32(number);
        format!(
            "({type_sql} < {type_value} OR ({type_sql} = {type_value} AND {id} {comparison} {value}))"
        )
    } else {
        let target = recorder
            .reference_target
            .as_ref()
            .or(match recorder.reference_targets.as_slice() {
                [target] => Some(target),
                _ => None,
            })
            .ok_or_else(|| diagnostic("Balance recorder has no unambiguous acquired type"))?;
        let target = snapshot
            .object_id_by_physical_table(target)
            .map_err(|_| diagnostic("Balance recorder target is not acquired"))?;
        let row_number = super::super::params::object_type_number(target, token, snapshot)?;
        match row_number.cmp(&number) {
            std::cmp::Ordering::Less => "1 = 1".to_owned(),
            std::cmp::Ordering::Greater => "1 = 0".to_owned(),
            std::cmp::Ordering::Equal => format!("{id} {comparison} {value}"),
        }
    };
    Ok(Some(Cut {
        before: format!("({period} < {date} OR ({period} = {date} AND ({recorder_before})))"),
        date,
    }))
}
