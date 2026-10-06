//! Полный состав точек из нативной карты маршрута версии 5.

use super::{BusinessProcessRoutePoint, Guid, Synonym, Value};
use std::collections::HashSet;

pub(super) fn parse(value: &Value, owner: &Guid) -> Option<Vec<BusinessProcessRoutePoint>> {
    let root = value.as_list()?;
    if root.first()?.as_u32()? != 5 || root.get(1)?.as_list()?.len() != 1 {
        return None;
    }
    let count = usize::try_from(root.get(2)?.as_u32()?).ok()?;
    if root.len() != count.checked_mul(2)?.checked_add(4)? {
        return None;
    }
    root.last()?.as_u32()?;
    let mut points = Vec::new();
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for pair in root[3..root.len() - 1].chunks_exact(2) {
        let kind = pair[0].as_u32()?;
        let body = pair[1].as_list()?;
        if matches!(kind, 0 | 1) {
            // Соединительная линия и декорация имеют собственные имена, но не GUID точки.
            let header = body.first()?.as_list()?;
            if body.len() != (if kind == 1 { 7 } else { 3 })
                || header.len() != 5
                || header[0].as_u32()? != 4
            {
                return None;
            }
            continue;
        }
        if !matches!(kind, 2..=10) {
            return None;
        }
        let expected_len = match kind {
            2 | 3 | 4 | 9 => 4,
            5 => 8,
            6 => usize::try_from(body.get(3)?.as_u32()?)
                .ok()?
                .checked_add(5)?,
            7 | 8 => 3,
            10 => 6,
            _ => return None,
        };
        if body.len() != expected_len {
            return None;
        }
        if kind == 6
            && (!body[4..body.len() - 1]
                .iter()
                .all(|branch| branch.as_string().is_some())
                || body.last()?.as_list().is_none())
        {
            return None;
        }
        let entry = body.first()?.as_list()?;
        if entry.len() != 4 || entry[1].as_u32()? != 4 || entry[3].as_u32()? != 0 {
            return None;
        }
        let header = entry[0].as_list()?;
        if header.len() != 5 || header[0].as_u32()? != 4 {
            return None;
        }
        let name = header[3].as_string()?;
        let id: Guid = entry[2].as_scalar()?.parse().ok()?;
        if name.is_empty()
            || id.is_nil()
            || !ids.insert(id.clone())
            || !names.insert(name.to_owned())
        {
            return None;
        }
        let labels = header[2].as_list()?;
        let label_count = usize::try_from(labels.get(1)?.as_u32()?).ok()?;
        if labels.first()?.as_u32()? != 1 || labels.len() != label_count.checked_add(2)? {
            return None;
        }
        let mut descriptor = BusinessProcessRoutePoint {
            owner_guid: owner.clone(),
            point_guid: id,
            name: name.to_owned(),
            synonyms: Vec::new(),
        };
        for label in &labels[2..] {
            let fields = label.as_list()?;
            if fields.len() != 2 {
                return None;
            }
            descriptor.synonyms.push(Synonym {
                language: fields[0].as_string()?.to_owned(),
                text: fields[1].as_string()?.to_owned(),
            });
        }
        points.push(descriptor);
    }
    Some(points)
}

#[cfg(test)]
#[path = "route_points_tests.rs"]
mod tests;
