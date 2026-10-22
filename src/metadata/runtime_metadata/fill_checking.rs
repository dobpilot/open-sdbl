//! Measured fill-checking settings for objects and attributes.

use super::{
    CatalogStandardAttributeFillChecking, ConfigDescriptor, FillChecking, Value, at, atom, guid,
    list, object_header,
};

pub(super) fn fill_checking(value: &Value) -> Option<FillChecking> {
    match atom(value)? {
        "0" => Some(FillChecking::DontCheck),
        "1" => Some(FillChecking::ShowError),
        _ => None,
    }
}

pub(super) fn standard_fill_checking(
    class: &[Value],
) -> Option<Vec<CatalogStandardAttributeFillChecking>> {
    standard_fill_checking_at(class, 45, 9, false)
}

pub(super) fn standard_fill_checking_at(
    class: &[Value],
    index: usize,
    count: usize,
    process: bool,
) -> Option<Vec<CatalogStandardAttributeFillChecking>> {
    let outer = list(class.get(index)?)?;
    if outer.len() != 2 || outer.first().and_then(atom) != Some("1") {
        return None;
    }
    let rows = list(&outer[1])?;
    // Полные таблицы из девяти записей справочника и семи бизнес-процесса
    // независимо сверены с нативными XML; ключи двух классов различаются.
    // Неполную или разреженную форму не выдаём как полный набор.
    if rows.len() != 2 + count * 3
        || rows.first().and_then(atom) != Some("1")
        || rows.get(1)?.as_u32()? as usize != count
    {
        return None;
    }
    let mut result = Vec::new();
    for row in rows[2..].chunks_exact(3) {
        let key = list(&row[0])?;
        if key.len() != 1 || atom(&row[1]) != Some("510405d3-2a0c-4fea-960a-7fee59b32f9b") {
            return None;
        }
        let name = match atom(&key[0])? {
            "-9" if process => "Started",
            "-13" if !process => "PredefinedDataName",
            "-10" if !process => "Predefined",
            "-8" if process => "HeadTask",
            "-8" => "Ref",
            "-7" if process => "Completed",
            "-7" => "DeletionMark",
            "-6" if !process => "IsFolder",
            "-5" if process => "Ref",
            "-5" => "Owner",
            "-4" if process => "DeletionMark",
            "-4" => "Parent",
            "-3" if process => "Date",
            "-3" => "Description",
            "-2" if process => "Number",
            "-2" => "Code",
            _ => return None,
        };
        if result
            .iter()
            .any(|entry: &CatalogStandardAttributeFillChecking| entry.name == name)
        {
            return None;
        }
        let bag = list(&row[2])?;
        if !matches!(
            (bag.first().and_then(atom), bag.get(1).and_then(atom)),
            (Some("13"), Some("24")) | (Some("14"), Some("25"))
        ) || bag.get(1)?.as_u32()? as usize * 2 != bag.len().checked_sub(2)?
        {
            return None;
        }
        let mut keys = Vec::new();
        let mut setting = None;
        for pair in bag[2..].chunks_exact(2) {
            let id = guid(&pair[0])?;
            if keys.contains(&id) {
                return None;
            }
            keys.push(id);
            if atom(&pair[0]) == Some("2723eb98-b4c1-498a-a6f3-70444757902f") {
                let value = list(&pair[1])?;
                if value.len() != 3
                    || value[0].as_string() != Some("#")
                    || atom(&value[1]) != Some("98ea8e5a-b586-442b-b944-6e3447734aa7")
                {
                    return None;
                }
                setting = fill_checking(&value[2]);
            }
        }
        result.push(CatalogStandardAttributeFillChecking {
            name: name.into(),
            fill_checking: setting?,
        });
    }
    Some(result)
}

pub(super) fn attribute_fill_checking(value: &Value, descriptors: &mut [ConfigDescriptor]) {
    let Some(values) = list(value) else {
        return;
    };
    if values.len() == 23
        && values.first().and_then(atom) == Some("27")
        && let Some(id) = at(value, &[1, 1]).and_then(object_header)
        && let Some(d) = descriptors.iter_mut().find(|d| d.object_guid == id)
    {
        // Позиция и оба кода сверены по UUID собственных, табличных
        // и общих реквизитов с нативным FillChecking, без анализа BSL.
        d.fill_checking = fill_checking(&values[13]);
    }
    for value in values {
        attribute_fill_checking(value, descriptors);
    }
}
