use std::collections::BTreeMap;

use super::DbNames;

/// One logical field and the physical PostgreSQL columns that implement it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalField {
    /// Logical field name without a leading underscore or compound suffix.
    pub name: String,
    /// Canonical physical members in input order.
    pub physical_columns: Vec<String>,
}

/// Restores canonical 1C casing for a lowercase PostgreSQL catalog identifier.
///
/// Known tokens are always matched longest-first. Unknown characters are
/// preserved, except single-letter compound tails after `_`, which are uppercased.
#[must_use]
pub fn recase_postgres_identifier(identifier: &str) -> String {
    const TOKENS: [(&str, &str); 38] = [
        ("usersworkhistory", "UsersWorkHistory"),
        ("description", "Description"),
        ("predefinedid", "PredefinedID"),
        ("numberprefix", "NumberPrefix"),
        ("recorder", "Recorder"),
        ("reference", "Reference"),
        ("document", "Document"),
        ("accumrg", "AccumRg"),
        ("inforg", "InfoRg"),
        ("idrref", "IDRRef"),
        ("lineno", "LineNo"),
        ("ckinds", "CKinds"),
        ("accrg", "AccRg"),
        ("rrref", "RRRef"),
        ("rtref", "RTRef"),
        ("chngr", "ChngR"),
        ("version", "Version"),
        ("marked", "Marked"),
        ("const", "Const"),
        ("seqb", "SeqB"),
        ("tref", "TRef"),
        ("rref", "RRef"),
        ("type", "TYPE"),
        ("chrc", "Chrc"),
        ("number", "Number"),
        ("period", "Period"),
        ("posted", "Posted"),
        ("active", "Active"),
        ("crg", "CRg"),
        ("acc", "Acc"),
        ("enum", "Enum"),
        ("node", "Node"),
        ("task", "Task"),
        ("code", "Code"),
        ("bpr", "BPr"),
        ("seq", "Seq"),
        ("fld", "Fld"),
        ("vt", "VT"),
    ];
    let lower_storage;
    let lower = if identifier.bytes().any(|byte| byte.is_ascii_uppercase()) {
        lower_storage = identifier.to_ascii_lowercase();
        lower_storage.as_str()
    } else {
        identifier
    };
    let mut output = String::with_capacity(identifier.len());
    let mut characters = identifier.char_indices().peekable();
    while let Some((offset, character)) = characters.next() {
        let token = if character.is_ascii() {
            let byte = lower.as_bytes()[offset];
            TOKENS.iter().find(|(token, _)| {
                token.as_bytes()[0] == byte && token_matches(lower, offset, token)
            })
        } else {
            None
        };
        if let Some((token, canonical)) = token {
            output.push_str(canonical);
            let token_end = offset + token.len();
            while characters
                .peek()
                .is_some_and(|(next_offset, _)| *next_offset < token_end)
            {
                characters.next();
            }
            continue;
        }
        if character == '_'
            && let Some(next) = lower.as_bytes().get(offset + 1).copied()
            && matches!(next, b's' | b'r' | b'l' | b'n' | b't')
            && lower
                .as_bytes()
                .get(offset + 2)
                .is_none_or(|following| *following == b'_')
        {
            output.push('_');
            output.push(char::from(next.to_ascii_uppercase()));
            characters.next();
            continue;
        }
        output.push(character);
    }
    output
}

fn token_matches(identifier: &str, offset: usize, token: &str) -> bool {
    if !identifier[offset..].starts_with(token) {
        return false;
    }
    let boundary_before = offset == 0 || identifier.as_bytes()[offset - 1] == b'_';
    match token {
        "idrref" | "rrref" | "rtref" | "type" => boundary_before,
        "rref" | "tref" => boundary_before || offset + token.len() == identifier.len(),
        "chngr" | "vt" => true,
        _ => boundary_before,
    }
}

/// Collapses physical compound-column suffixes into logical fields.
#[must_use]
pub fn collapse_logical_fields<I, S>(columns: I) -> Vec<LogicalField>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut order = Vec::<String>::new();
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for column in columns {
        let column = column.as_ref();
        let canonical = recase_postgres_identifier(column);
        let logical = normalize_logical_name(&canonical);
        if !groups.contains_key(&logical) {
            order.push(logical.clone());
        }
        groups.entry(logical).or_default().push(canonical);
    }
    order
        .into_iter()
        .map(|name| LogicalField {
            physical_columns: groups.remove(&name).unwrap_or_default(),
            name,
        })
        .collect()
}

pub(crate) fn normalize_logical_name(canonical: &str) -> String {
    let logical = logical_base(canonical);
    logical.strip_prefix('_').unwrap_or(logical).to_owned()
}

pub(crate) fn normalize_standard_field_name(logical: &str) -> &str {
    if logical == "Date_Time" {
        "Date"
    } else {
        logical
    }
}

/// Collapses an index key and removes DBNames data-separation fields.
#[must_use]
pub fn normalize_index_key<I, S>(columns: I, db_names: &DbNames) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    collapse_logical_fields(columns)
        .into_iter()
        .filter(|field| {
            field
                .name
                .strip_prefix("Fld")
                .and_then(|number| number.parse::<u32>().ok())
                .is_none_or(|number| !db_names.is_data_separator(number))
        })
        .map(|field| field.name)
        .collect()
}

fn logical_base(column: &str) -> &str {
    const UNDERSCORE_SUFFIXES: [&str; 10] = [
        "_RRRef", "_RTRef", "_TYPE", "_RRef", "_TRef", "_L", "_N", "_T", "_S", "_R",
    ];
    for suffix in UNDERSCORE_SUFFIXES {
        if let Some(base) = column.strip_suffix(suffix) {
            return base;
        }
    }
    for suffix in ["RRRef", "RTRef", "RRef", "TRef"] {
        if let Some(base) = column.strip_suffix(suffix) {
            return base;
        }
    }
    column
}

#[cfg(test)]
mod tests {
    use super::{
        collapse_logical_fields, normalize_logical_name, normalize_standard_field_name,
        recase_postgres_identifier,
    };

    #[test]
    fn recases_longest_tokens_and_compound_suffixes() {
        assert_eq!(recase_postgres_identifier("_accrg3942"), "_AccRg3942");
        assert_eq!(recase_postgres_identifier("_acc3930"), "_Acc3930");
        assert_eq!(recase_postgres_identifier("_seqb10"), "_SeqB10");
        assert_eq!(recase_postgres_identifier("_fld12_rrref"), "_Fld12_RRRef");
        assert_eq!(recase_postgres_identifier("_fld12_type"), "_Fld12_TYPE");
    }

    #[test]
    fn preserves_non_ascii_identifiers_without_corrupting_utf8() {
        assert_eq!(
            recase_postgres_identifier("_СправочникКонтрагенты"),
            "_СправочникКонтрагенты"
        );
        assert_eq!(
            recase_postgres_identifier("_reference53_Колонка"),
            "_Reference53_Колонка"
        );
        assert_eq!(
            recase_postgres_identifier("_fld12_rrref_Ссылка"),
            "_Fld12_RRRef_Ссылка"
        );
    }

    #[test]
    fn collapses_reference_and_compound_members() {
        let fields = collapse_logical_fields([
            "_fld12_tref",
            "_fld12_rrref",
            "_recorderTRef",
            "_recorderRRef",
            "_value_type",
            "_value_s",
            "_value_rtref",
            "_value_rrref",
            "_idrref",
        ]);
        assert_eq!(fields[0].name, "Fld12");
        assert_eq!(fields[0].physical_columns.len(), 2);
        assert_eq!(fields[1].name, "Recorder");
        assert_eq!(fields[2].name, "value");
        assert_eq!(fields[3].name, "ID");
    }

    #[test]
    fn removes_one_physical_prefix_and_normalizes_the_date_standard_field() {
        assert_eq!(normalize_logical_name("_Date_Time"), "Date_Time");
        assert_eq!(normalize_logical_name("__Date_Time"), "_Date_Time");
        assert_eq!(normalize_standard_field_name("Date_Time"), "Date");
        assert_eq!(normalize_standard_field_name("Period"), "Period");
    }
}
