//! Логические позиции сохраняются отдельно от прежней раскладки SQL-колонок.

use super::context::{CompiledBranch, ProjectedMember, SelectedProjection, projected_members};
use super::expression::composite_member_of;
use crate::query::{
    ColumnKind, CompiledColumn, CompositeResultMember, LogicalResultColumn, LogicalResultValue,
};
use std::collections::BTreeMap;

// Этот внутренний маркер не является признаком типа и не выходит в API.
pub(super) const UNAVAILABLE_TAG: u8 = u8::MAX;

type MemberRoles = Vec<(usize, Option<u8>)>;
type Group = (String, MemberRoles);

pub(super) fn suffix_tag(suffix: &str) -> Option<u8> {
    match suffix {
        "_L" => Some(crate::query::TypeValue::Boolean.tag()),
        "_N" => Some(crate::query::TypeValue::Number.tag()),
        "_T" => Some(crate::query::TypeValue::Date.tag()),
        "_S" => Some(crate::query::TypeValue::String.tag()),
        "" => Some(crate::query::TypeValue::Reference(0).tag()),
        _ => None,
    }
}

fn recipe(name: String, roles: &[(usize, Option<u8>)]) -> Option<LogicalResultColumn> {
    let mut discriminator = None;
    let mut members = Vec::new();
    for &(column, tag) in roles {
        if let Some(type_tag) = tag {
            if !matches!(type_tag, 2..=5 | 8) {
                return None;
            }
            if members
                .iter()
                .any(|member: &CompositeResultMember| member.type_tag == type_tag)
            {
                return None;
            }
            members.push(CompositeResultMember {
                type_tag,
                column: Some(column),
            });
        } else if discriminator.replace(column).is_some() {
            return None;
        }
    }
    if members.is_empty() {
        return None;
    }
    Some(LogicalResultColumn {
        name,
        value: LogicalResultValue::Composite {
            discriminator: discriminator?,
            members,
        },
    })
}

pub(super) fn selected_projection(
    selected: &[SelectedProjection],
    widths: &[usize],
    columns: &[CompiledColumn],
    expand: &BTreeMap<usize, Vec<&'static str>>,
) -> Option<Vec<LogicalResultColumn>> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut group: Option<Group> = None;
    for (index, item) in selected.iter().enumerate() {
        let width = *widths.get(index)?;
        let finish =
            |group: &mut Option<Group>, result: &mut Vec<LogicalResultColumn>| -> Option<()> {
                if let Some((name, roles)) = group.take() {
                    result.push(recipe(name, &roles)?);
                }
                Some(())
            };
        if let SelectedProjection::Generated {
            composite: Some((name, tag, first)),
            ..
        } = item
        {
            if width != 1 {
                return None;
            }
            if *first {
                finish(&mut group, &mut result)?;
                group = Some((name.clone(), Vec::new()));
            }
            let active = group.as_mut()?;
            if &active.0 != name {
                return None;
            }
            active.1.push((offset, *tag));
            offset += width;
            continue;
        }
        finish(&mut group, &mut result)?;
        let name = match item {
            SelectedProjection::Field(field) => field.field_label(),
            SelectedProjection::Generated {
                label, deferred, ..
            } => {
                if *deferred {
                    return None;
                }
                label.clone()
            }
        };
        let point = matches!(item, SelectedProjection::Field(field)
            if field.field().schema_name == super::context::POINT_IN_TIME_SCHEMA_NAME);
        let value = if point {
            if width != 2
                || !matches!(columns.get(offset)?.kind, ColumnKind::DateTime)
                || !matches!(columns.get(offset + 1)?.kind, ColumnKind::Reference { .. })
            {
                return None;
            }
            LogicalResultValue::PointInTime {
                date: offset,
                reference: offset + 1,
            }
        } else if let Some(members) = expand.get(&index) {
            if let SelectedProjection::Generated {
                kind: ColumnKind::Null | ColumnKind::Undefined,
                ..
            } = item
            {
                return None;
            }
            let roles = members
                .iter()
                .enumerate()
                .map(|(i, suffix)| {
                    if *suffix == "_TYPE" {
                        Some((offset + i, None))
                    } else {
                        Some((offset + i, Some(suffix_tag(suffix)?)))
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            recipe(name.clone(), &roles)?.value
        } else if width == 1 {
            LogicalResultValue::Scalar { column: offset }
        } else if let SelectedProjection::Field(field) = item {
            let roles = projected_members(field.field())
                .iter()
                .enumerate()
                .map(|(i, member)| {
                    let tag = match member {
                        ProjectedMember::Reference { .. } => {
                            Some(crate::query::TypeValue::Reference(0).tag())
                        }
                        ProjectedMember::Single(column)
                            if column.physical_name.to_ascii_lowercase().ends_with("_type")
                                && matches!(
                                    column.kind,
                                    ColumnKind::Binary {
                                        length: None | Some(1)
                                    }
                                ) =>
                        {
                            None
                        }
                        ProjectedMember::Single(column) => match column.kind {
                            ColumnKind::Boolean
                            | ColumnKind::Number { .. }
                            | ColumnKind::DateTime
                            | ColumnKind::String { .. }
                            | ColumnKind::Reference { .. } => {
                                Some(composite_member_of(&column.kind)?.1.tag())
                            }
                            _ => return None,
                        },
                    };
                    Some((offset + i, tag))
                })
                .collect::<Option<Vec<_>>>()?;
            let mut item = recipe(name.clone(), &roles)?;
            // `TypeValue` фиксирует измеренный признак неопределённого значения
            // в составном поле; этот вариант не использует полезную ячейку.
            if let LogicalResultValue::Composite { members, .. } = &mut item.value {
                members.push(CompositeResultMember {
                    type_tag: crate::query::TypeValue::Undefined.tag(),
                    column: None,
                });
            }
            item.value
        } else {
            return None;
        };
        result.push(LogicalResultColumn { name, value });
        offset += width;
    }
    if let Some((name, roles)) = group {
        result.push(recipe(name, &roles)?);
    }
    if offset != columns.len() {
        return None;
    }
    Some(result)
}

pub(super) fn union_projection(branches: &[CompiledBranch]) -> Option<Vec<LogicalResultColumn>> {
    let first = branches.first()?.logical_projection.as_ref()?;
    let mut result = first.clone();
    for branch in branches.iter().skip(1) {
        let other = branch.logical_projection.as_ref()?;
        if result.len() != other.len() {
            return None;
        }
        for (left, right) in result.iter_mut().zip(other) {
            if let LogicalResultValue::Scalar { column } = left.value {
                let kind = &branches.first()?.columns.get(column)?.kind;
                let other_kind = &branch.columns.get(column)?.kind;
                if (kind == &ColumnKind::Undefined || other_kind == &ColumnKind::Undefined)
                    && kind != other_kind
                {
                    return None;
                }
            }
            if left.value != right.value {
                return None;
            }
        }
    }
    Some(result)
}
