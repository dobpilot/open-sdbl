//! Console access restrictions: `\restrict`.
//!
//! A restriction is stored per metadata table (or tabular section) with its
//! condition text; before every query the console passes only the
//! restrictions whose target the prepared batch requested, so a stored
//! restriction for a table the query does not read is never an error.

use open_sdbl::metadata::{MetadataSnapshot, ObjectId};
use open_sdbl::query::{AccessRestriction, RestrictionRequest, find_metadata_object};

use super::CliError;

const RESTRICT_USAGE: &str = "usage: \\restrict [<Вид>.<Объект>[.<ТабличнаяЧасть>] <condition> | clear]  (\\restrict alone lists the restrictions)";

/// One restriction kept for the console session.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredRestriction {
    /// The target as typed, for listings.
    name: String,
    object: ObjectId,
    table_part: Option<String>,
    condition: String,
}

/// The session restriction store, in insertion order.
#[derive(Debug, Default)]
pub(crate) struct RestrictionStore {
    items: Vec<StoredRestriction>,
}

impl RestrictionStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Stores or replaces the restriction of one target, keeping its
    /// original position.
    fn set(&mut self, entry: StoredRestriction) {
        match self.items.iter_mut().find(|item| {
            item.object == entry.object && same_part(&item.table_part, &entry.table_part)
        }) {
            Some(existing) => *existing = entry,
            None => self.items.push(entry),
        }
    }

    fn clear(&mut self) {
        self.items.clear();
    }

    /// The restrictions of the targets a prepared batch requested.
    pub(crate) fn for_request(&self, request: &RestrictionRequest) -> Vec<AccessRestriction> {
        self.items
            .iter()
            .filter(|item| {
                request.targets.iter().any(|target| {
                    target.object == item.object && same_part(&target.table_part, &item.table_part)
                })
            })
            .map(|item| {
                let restriction = AccessRestriction::new(item.object, item.condition.clone());
                match &item.table_part {
                    Some(section) => restriction.table_part(section.clone()),
                    None => restriction,
                }
            })
            .collect()
    }

    /// One line per restriction: the target as typed and its condition.
    pub(crate) fn listing(&self) -> String {
        if self.items.is_empty() {
            return "No restrictions set.\n".to_owned();
        }
        let width = self
            .items
            .iter()
            .map(|item| item.name.chars().count())
            .max()
            .unwrap_or(0);
        self.items
            .iter()
            .map(|item| format!("{:<width$}  {}\n", item.name, item.condition))
            .collect()
    }
}

fn same_part(left: &Option<String>, right: &Option<String>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => names_equal(left, right),
        _ => false,
    }
}

fn names_equal(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_uppercase)
        .eq(right.chars().flat_map(char::to_uppercase))
}

/// A recognized restriction command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestrictionCommand<'line> {
    Set {
        name: &'line str,
        condition: &'line str,
    },
    List,
    Clear,
    Usage,
}

/// Recognizes `\restrict`; other lines return `None`.
pub(crate) fn parse_restriction_command(line: &str) -> Option<RestrictionCommand<'_>> {
    let (command, rest) = line
        .split_once(char::is_whitespace)
        .map_or((line, ""), |(command, rest)| (command, rest.trim()));
    if command != "\\restrict" {
        return None;
    }
    if rest.is_empty() {
        return Some(RestrictionCommand::List);
    }
    if rest.eq_ignore_ascii_case("clear") {
        return Some(RestrictionCommand::Clear);
    }
    let (name, condition) = rest
        .split_once(char::is_whitespace)
        .map_or((rest, ""), |(name, condition)| (name, condition.trim()));
    if condition.is_empty() || !(2..=3).contains(&name.split('.').count()) {
        return Some(RestrictionCommand::Usage);
    }
    Some(RestrictionCommand::Set { name, condition })
}

/// Applies a restriction command and returns the text to print.
pub(crate) fn apply_restriction_command(
    store: &mut RestrictionStore,
    command: RestrictionCommand<'_>,
    snapshot: &MetadataSnapshot,
) -> Result<String, CliError> {
    match command {
        RestrictionCommand::Set { name, condition } => {
            let mut segments = name.split('.');
            let kind = segments.next().unwrap_or_default();
            let object_name = segments.next().unwrap_or_default();
            let table_part = segments.next().map(str::to_owned);
            if kind.is_empty() || object_name.is_empty() || table_part.as_deref() == Some("") {
                return Err(CliError::Data(RESTRICT_USAGE.to_owned()));
            }
            let object = find_metadata_object(snapshot, &format!("{kind}.{object_name}"))
                .map_err(|error| CliError::Data(error.to_string()))?;
            store.set(StoredRestriction {
                name: name.to_owned(),
                object: ObjectId::from(&object.guid),
                table_part,
                condition: condition.to_owned(),
            });
            Ok(format!("Restriction of {name} set.\n"))
        }
        RestrictionCommand::List => Ok(store.listing()),
        RestrictionCommand::Clear => {
            store.clear();
            Ok("Restrictions cleared.\n".to_owned())
        }
        RestrictionCommand::Usage => Err(CliError::Data(RESTRICT_USAGE.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use open_sdbl::query::RestrictionTarget;

    use super::*;
    use crate::params::tests::enumeration_snapshot;

    #[test]
    fn parses_restriction_commands() {
        assert_eq!(
            parse_restriction_command("\\restrict Справочник.Номенклатура Организация = &Орг"),
            Some(RestrictionCommand::Set {
                name: "Справочник.Номенклатура",
                condition: "Организация = &Орг"
            })
        );
        assert_eq!(
            parse_restriction_command("\\restrict Документ.Реализация.Товары Сумма > 0"),
            Some(RestrictionCommand::Set {
                name: "Документ.Реализация.Товары",
                condition: "Сумма > 0"
            })
        );
        assert_eq!(
            parse_restriction_command("\\restrict"),
            Some(RestrictionCommand::List)
        );
        assert_eq!(
            parse_restriction_command("\\restrict Clear"),
            Some(RestrictionCommand::Clear)
        );
        assert_eq!(
            parse_restriction_command("\\restrict Номенклатура Код = 1"),
            Some(RestrictionCommand::Usage)
        );
        assert_eq!(
            parse_restriction_command("\\restrict Справочник.Номенклатура"),
            Some(RestrictionCommand::Usage)
        );
        assert_eq!(parse_restriction_command("\\set x 1"), None);
    }

    #[test]
    fn stores_lists_and_filters_restrictions_by_request() {
        let snapshot = enumeration_snapshot();
        let object = ObjectId::from(
            &find_metadata_object(&snapshot, "Перечисление.бит_ВидыСтатусовОбъектов")
                .unwrap()
                .guid,
        );
        let mut store = RestrictionStore::new();
        let output = apply_restriction_command(
            &mut store,
            RestrictionCommand::Set {
                name: "Перечисление.бит_ВидыСтатусовОбъектов",
                condition: "Порядок > 0",
            },
            &snapshot,
        )
        .unwrap();
        assert_eq!(
            output,
            "Restriction of Перечисление.бит_ВидыСтатусовОбъектов set.\n"
        );
        apply_restriction_command(
            &mut store,
            RestrictionCommand::Set {
                name: "перечисление.бит_ВидыСтатусовОбъектов",
                condition: "Порядок > 1",
            },
            &snapshot,
        )
        .unwrap();
        apply_restriction_command(
            &mut store,
            RestrictionCommand::Set {
                name: "Перечисление.бит_ВидыСтатусовОбъектов.Строки",
                condition: "Сумма > 0",
            },
            &snapshot,
        )
        .unwrap();
        assert_eq!(
            store.listing(),
            "перечисление.бит_ВидыСтатусовОбъектов         Порядок > 1\nПеречисление.бит_ВидыСтатусовОбъектов.Строки  Сумма > 0\n"
        );

        let unknown = apply_restriction_command(
            &mut store,
            RestrictionCommand::Set {
                name: "Справочник.Нет",
                condition: "Код = 1",
            },
            &snapshot,
        )
        .unwrap_err();
        assert!(unknown.to_string().contains("was not found"));

        let request = RestrictionRequest {
            targets: vec![RestrictionTarget {
                object,
                table_part: Some("строки".to_owned()),
            }],
        };
        let restrictions = store.for_request(&request);
        assert_eq!(restrictions.len(), 1);
        assert_eq!(restrictions[0].condition(), "Сумма > 0");
        assert_eq!(restrictions[0].table_part_name(), Some("Строки"));
        assert!(store.for_request(&RestrictionRequest::default()).is_empty());

        assert_eq!(
            apply_restriction_command(&mut store, RestrictionCommand::Clear, &snapshot).unwrap(),
            "Restrictions cleared.\n"
        );
        assert_eq!(store.listing(), "No restrictions set.\n");
        assert!(
            apply_restriction_command(&mut store, RestrictionCommand::Usage, &snapshot).is_err()
        );
    }
}
