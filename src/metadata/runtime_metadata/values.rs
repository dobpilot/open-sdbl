//! Strict shared shapes and scalar values of measured Config records.

use super::{ConfigDeclaredType, ConfigFillingValue, Guid, Value};

pub(super) fn list(v: &Value) -> Option<&[Value]> {
    v.as_list()
}

pub(super) fn at<'a>(v: &'a Value, path: &[usize]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, i| list(v)?.get(*i))
}

pub(super) fn atom(v: &Value) -> Option<&str> {
    match v {
        Value::Atom(s) => Some(s),
        _ => None,
    }
}

pub(super) fn guid(v: &Value) -> Option<Guid> {
    atom(v)?.parse().ok()
}

pub(super) fn flag(v: &Value) -> Option<bool> {
    match atom(v)? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn header(v: &Value) -> Option<Guid> {
    let values = list(v)?;
    if values.first().and_then(atom) != Some("3") {
        return None;
    }
    let reference = list(values.get(1)?)?;
    if reference.len() != 3
        || atom(&reference[1]) != Some("0")
        || values.get(2)?.as_string()?.is_empty()
    {
        return None;
    }
    let id = guid(&reference[2])?;
    (!id.is_nil()).then_some(id)
}

pub(super) fn object_header(v: &Value) -> Option<Guid> {
    let values = list(v)?;
    if values.len() == 9
        && values.first().and_then(atom) == Some("3")
        && list(values.get(1)?)?.first().and_then(atom) == Some("1")
    {
        return header(v);
    }
    // Нативные XML подтверждают также заголовок версии 2 без
    // завершающего поля версии 3, с восемью или десятью полями;
    // прочие формы не расширяем.
    if !matches!(values.len(), 8 | 10) || values.first().and_then(atom) != Some("2") {
        return None;
    }
    let reference = list(values.get(1)?)?;
    if reference.len() != 3
        || atom(&reference[0]) != Some("1")
        || atom(&reference[1]) != Some("0")
        || values[2].as_string()?.is_empty()
    {
        return None;
    }
    let id = guid(&reference[2])?;
    (!id.is_nil()).then_some(id)
}

// Измеренная форма заголовка частей текущего документа проверяется локально:
// общий `header` и ранее поддержанные формы `object_header` не расширяем.
pub(super) fn current40_header2(value: &Value) -> Option<Guid> {
    let values = list(value)?;
    if values.len() != 8
        || atom(&values[0]) != Some("2")
        || values[4].as_string() != Some("")
        || atom(&values[5]) != Some("0")
        || atom(&values[6]) != Some("0")
        || !guid(&values[7])?.is_nil()
    {
        return None;
    }
    let synonyms = list(&values[3])?;
    if synonyms.len() != 3
        || atom(&synonyms[0]) != Some("1")
        || synonyms[1].as_string() != Some("ru")
        || synonyms[2].as_string()?.is_empty()
    {
        return None;
    }
    object_header(value)
}

pub(super) fn declaration(v: &Value) -> Option<Vec<ConfigDeclaredType>> {
    let values = list(v)?;
    if values.first()?.as_string() != Some("Pattern") {
        return None;
    }
    values[1..]
        .iter()
        .map(|value| {
            let parts = list(value)?;
            let tag = parts.first()?.as_string()?;
            let (reference, qualifiers) = if tag == "#" {
                if parts.len() != 2 {
                    return None;
                }
                (Some(guid(&parts[1])?), Vec::new())
            } else if matches!(tag, "S" | "N" | "D" | "B" | "U") {
                (None, parts[1..].to_vec())
            } else {
                return None;
            };
            Some(ConfigDeclaredType {
                tag: tag.into(),
                reference,
                qualifiers,
            })
        })
        .collect()
}

pub(super) fn filling(v: &Value) -> Option<ConfigFillingValue> {
    let p = list(v)?;
    match (p.first()?.as_string()?, p.len()) {
        ("U", 1) => Some(ConfigFillingValue::Undefined),
        ("S", 2) => Some(ConfigFillingValue::String(p[1].as_string()?.into())),
        ("B", 2) => Some(ConfigFillingValue::Boolean(flag(&p[1])?)),
        ("N", 2) => Some(ConfigFillingValue::Number(atom(&p[1])?.into())),
        ("#", 3) if atom(&p[1]) == Some("5c14e26f-099b-4d37-84a6-b433d87400da") => {
            let r = list(&p[2])?;
            if r.len() != 3 || atom(&r[0]) != Some("0") {
                return None;
            }
            Some(ConfigFillingValue::Reference {
                type_id: guid(&r[1])?,
                value_id: guid(&r[2])?,
            })
        }
        _ => None,
    }
}

/// Internal recognition keeps missing input distinct from malformed input.
/// Public compatibility fields deliberately map both states to None.
pub(super) enum CollectionState<T> {
    Absent,
    Malformed,
    Known(T),
}

impl<T> CollectionState<T> {
    pub(super) fn known(self) -> Option<T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Absent | Self::Malformed => None,
        }
    }

    pub(super) fn validate<U>(self, parse: impl FnOnce(T) -> Option<U>) -> CollectionState<U> {
        match self {
            Self::Absent => CollectionState::Absent,
            Self::Malformed => CollectionState::Malformed,
            Self::Known(value) => {
                parse(value).map_or(CollectionState::Malformed, CollectionState::Known)
            }
        }
    }
}

pub(super) fn inspect_collection<'a>(root: &'a [Value], id: &str) -> CollectionState<&'a [Value]> {
    let mut matches = root
        .iter()
        .filter_map(list)
        .filter(|v| v.first().and_then(atom) == Some(id));
    let Some(values) = matches.next() else {
        return CollectionState::Absent;
    };
    if matches.next().is_some()
        || values.len() < 2
        || values.get(1).and_then(Value::as_u32).map(|n| n as usize) != values.len().checked_sub(2)
    {
        return CollectionState::Malformed;
    }
    CollectionState::Known(&values[2..])
}

pub(super) fn collection<'a>(root: &'a [Value], id: &str) -> Option<&'a [Value]> {
    inspect_collection(root, id).known()
}

pub(super) fn membership(rows: &[Value], path: &[usize]) -> Option<Vec<Guid>> {
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        let id = header(at(row, path)?)?;
        if ids.contains(&id) {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}

pub(super) fn owner_reference(value: &Value) -> Option<Guid> {
    let reference = list(value)?;
    if reference.len() != 3
        || reference[0].as_string() != Some("#")
        || atom(&reference[1]) != Some("157fa490-4ce9-11d4-9415-008048da11f9")
    {
        return None;
    }
    let target = list(&reference[2])?;
    if target.len() != 2 || atom(&target[0]) != Some("1") {
        return None;
    }
    guid(&target[1]).filter(|id| !id.is_nil())
}

pub(super) fn owners(v: &Value) -> Option<Vec<Guid>> {
    let p = list(v)?;
    if atom(p.first()?) != Some("0") || p.get(1)?.as_u32()? as usize != p.len() - 2 {
        return None;
    }
    let mut ids = Vec::new();
    for entry in &p[2..] {
        let id = owner_reference(entry)?;
        if ids.contains(&id) {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_malformed_and_measured_empty_collections_are_distinct() {
        assert!(matches!(
            inspect_collection(&[], "members"),
            CollectionState::Absent
        ));
        let truncated = Value::List(vec![Value::Atom("members".into())]);
        assert!(matches!(
            inspect_collection(&[truncated], "members"),
            CollectionState::Malformed
        ));
        let empty = Value::List(vec![Value::Atom("members".into()), Value::Atom("0".into())]);
        assert!(matches!(
            inspect_collection(std::slice::from_ref(&empty), "members"),
            CollectionState::Known([])
        ));
        let malformed = Value::List(vec![Value::Atom("members".into()), Value::Atom("1".into())]);
        assert!(matches!(
            inspect_collection(&[malformed], "members"),
            CollectionState::Malformed
        ));
        assert!(matches!(
            inspect_collection(&[empty.clone(), empty], "members"),
            CollectionState::Malformed
        ));
    }
}
