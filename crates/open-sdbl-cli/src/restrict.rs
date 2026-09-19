//! Console access restrictions: `\restrict`.
//!
//! A restriction is stored per metadata table (or tabular section) with its
//! condition text; before every query the console passes only the
//! restrictions whose target the prepared batch requested, so a stored
//! restriction for a table the query does not read is never an error.
//! A restriction is either typed by the operator or derived by `\as` from
//! the roles of the current user: a typed one always wins, and the derived
//! ones are forgotten when the current user changes.

use open_sdbl::metadata::{MetadataSnapshot, ObjectId};
use open_sdbl::query::{AccessRestriction, RestrictionRequest, find_metadata_object};

use crate::error::CliError;

const RESTRICT_USAGE: &str = "usage: \\restrict [<Вид>.<Объект>[.<ТабличнаяЧасть>] <condition> | clear]  (\\restrict alone lists the restrictions)";

/// Where a stored restriction comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestrictionOrigin {
    /// Typed with `\restrict`.
    Typed,
    /// Expanded by `\as` from the roles of the current user.
    Derived,
}

/// One restriction kept for the console session.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredRestriction {
    /// The target as typed, for listings.
    name: String,
    object: ObjectId,
    table_part: Option<String>,
    condition: String,
    origin: RestrictionOrigin,
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

    /// Whether a restriction the operator typed covers the object.
    fn typed_covers(&self, object: ObjectId) -> bool {
        self.items.iter().any(|item| {
            item.origin == RestrictionOrigin::Typed
                && item.object == object
                && item.table_part.is_none()
        })
    }

    /// Replaces the derived restrictions with these, keeping every
    /// restriction the operator typed. Returns how many were stored.
    pub(crate) fn derive(&mut self, derived: Vec<(String, ObjectId, String)>) -> usize {
        self.forget_derived();
        let mut stored = 0;
        for (name, object, condition) in derived {
            if self.typed_covers(object) {
                continue;
            }
            stored += 1;
            self.items.push(StoredRestriction {
                name,
                object,
                table_part: None,
                condition,
                origin: RestrictionOrigin::Derived,
            });
        }
        stored
    }

    /// Forgets the restrictions `\as` derived.
    pub(crate) fn forget_derived(&mut self) {
        self.items
            .retain(|item| item.origin == RestrictionOrigin::Typed);
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
            .map(|item| {
                let mark = match item.origin {
                    RestrictionOrigin::Typed => ' ',
                    RestrictionOrigin::Derived => '*',
                };
                format!("{mark} {:<width$}  {}\n", item.name, item.condition)
            })
            .chain(
                self.items
                    .iter()
                    .any(|item| item.origin == RestrictionOrigin::Derived)
                    .then(|| "# * derived from the roles of the current user\n".to_owned()),
            )
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
                origin: RestrictionOrigin::Typed,
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
#[path = "tests/restrict.rs"]
mod tests;
