//! The virtual system catalog, prebuilt in full and filtered per user.
mod model;
mod relations;
pub(crate) mod sql;
#[cfg(test)]
mod tests;
mod wire;

#[cfg(test)]
pub(crate) use model::candidates;
#[cfg(any(test, feature = "fuzzing"))]
pub(crate) use model::finish;
pub(crate) use model::{Candidate, CandidateAccess, build};
use sql::{Table, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
pub(crate) use wire::{decode, encode};

#[derive(Clone)]
pub(crate) struct SharedTable(Arc<Table>);

impl SharedTable {
    fn new(table: Table) -> Self {
        Self(Arc::new(table))
    }

    #[cfg(test)]
    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Deref for SharedTable {
    type Target = Table;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SharedTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

#[derive(Clone)]
pub(crate) struct Catalog {
    pub(crate) revision: u32,
    pub(crate) _build_failures: usize,
    pub(crate) tables: BTreeMap<String, SharedTable>,
    pub(crate) user: String,
    pub(crate) database: String,
    pub(crate) schemas: Vec<String>,
    pub(crate) views: BTreeMap<i64, String>,
    pub(crate) candidates: Arc<[Candidate]>,
    pub(crate) access: Arc<BTreeMap<i64, CandidateAccess>>,
}

#[derive(Default)]
pub(crate) struct Visibility {
    pub(crate) relations: BTreeMap<i64, BTreeSet<i64>>,
    pub(crate) schemas: BTreeSet<String>,
}

impl Catalog {
    pub(crate) fn search_path(&self) -> String {
        let mut names = vec!["\"$user\"".into(), "public".into()];
        names.extend(
            self.schemas
                .iter()
                .filter(|s| !["pg_catalog", "information_schema", "public"].contains(&s.as_str()))
                .map(|s| format!("\"{}\"", s.replace('"', "\"\""))),
        );
        names.join(", ")
    }

    pub(crate) fn settings(&mut self, settings: &[(String, String)]) {
        let path = self.search_path();
        let table = self.tables.get_mut("pg_catalog.pg_settings").unwrap();
        table.rows.clear();
        for (n, v) in settings {
            table.push(vec![n.clone().into(), v.clone().into()]);
        }
        table.push(vec!["search_path".into(), path.into()]);
    }

    pub(crate) fn personalized(&self, user: &str) -> Self {
        let mut catalog = self.clone();
        catalog.user = user.into();
        for row in &mut catalog.table_mut("information_schema.schemata").rows {
            row[2] = user.into();
        }
        for row in &mut catalog.table_mut("pg_catalog.pg_roles").rows {
            row[1] = user.into();
        }
        for row in &mut catalog.table_mut("pg_catalog.pg_user").rows {
            row[0] = user.into();
        }
        catalog
    }

    pub(crate) fn filtered(&self, visibility: &Visibility, user: &str) -> Self {
        let mut catalog = self.personalized(user);
        let mut relation_ids = BTreeMap::<String, BTreeMap<String, i64>>::new();
        let mut full_ids = BTreeSet::new();
        for candidate in self.candidates.iter() {
            relation_ids
                .entry(candidate.schema.clone())
                .or_default()
                .insert(candidate.name.clone(), candidate.oid);
            full_ids.insert(candidate.oid);
        }
        catalog.candidates = self
            .candidates
            .iter()
            .filter_map(|candidate| {
                let columns = visibility.relations.get(&candidate.oid)?;
                let mut candidate = candidate.clone();
                candidate
                    .columns
                    .retain(|(number, _, _)| columns.contains(number));
                Some(candidate)
            })
            .collect::<Vec<_>>()
            .into();
        catalog
            .schemas
            .retain(|schema| is_builtin_schema(schema) || visibility.schemas.contains(schema));
        catalog
            .views
            .retain(|oid, _| visibility.relations.contains_key(oid));
        for row in &mut catalog.table_mut("pg_catalog.pg_class").rows {
            if let Some(oid) = value_int(&row[0])
                && let Some(columns) = visibility.relations.get(&oid)
            {
                row[22] = (columns.len() as i64).into();
            }
        }
        catalog.filter_rows("pg_catalog.pg_class", |row| {
            value_int(&row[0]).is_some_and(|oid| {
                !full_ids.contains(&oid) || visibility.relations.contains_key(&oid)
            })
        });
        catalog.filter_rows("pg_catalog.pg_attribute", |row| {
            match (value_int(&row[0]), value_int(&row[3])) {
                (Some(oid), Some(number)) => visibility
                    .relations
                    .get(&oid)
                    .is_some_and(|columns| columns.contains(&number)),
                _ => false,
            }
        });
        catalog.filter_rows("pg_catalog.pg_type", |row| {
            value_int(&row[10]).is_none_or(|oid| {
                !full_ids.contains(&oid) || visibility.relations.contains_key(&oid)
            })
        });
        catalog.filter_rows("pg_catalog.pg_description", |row| {
            match (value_int(&row[0]), value_int(&row[2])) {
                (Some(oid), Some(0)) => visibility.relations.contains_key(&oid),
                (Some(oid), Some(number)) => visibility
                    .relations
                    .get(&oid)
                    .is_some_and(|columns| columns.contains(&number)),
                _ => false,
            }
        });
        catalog.filter_rows("pg_catalog.pg_namespace", |row| {
            value_text(&row[1]).is_some_and(|schema| {
                is_builtin_schema(schema) || visibility.schemas.contains(schema)
            })
        });
        catalog.filter_rows("information_schema.schemata", |row| {
            value_text(&row[1]).is_some_and(|schema| {
                is_builtin_schema(schema) || visibility.schemas.contains(schema)
            })
        });
        for name in ["information_schema.tables", "information_schema.views"] {
            catalog.filter_rows(name, |row| {
                relation_oid(&relation_ids, row)
                    .is_some_and(|oid| visibility.relations.contains_key(&oid))
            });
        }
        catalog.filter_rows("information_schema.columns", |row| {
            match (relation_oid(&relation_ids, row), value_int(&row[4])) {
                (Some(oid), Some(number)) => visibility
                    .relations
                    .get(&oid)
                    .is_some_and(|columns| columns.contains(&number)),
                _ => false,
            }
        });
        catalog
    }

    fn table_mut(&mut self, name: &str) -> &mut Table {
        self.tables.get_mut(name).unwrap()
    }

    fn filter_rows(&mut self, name: &str, mut retain: impl FnMut(&[Value]) -> bool) {
        self.table_mut(name)
            .rows
            .retain(|row| retain(row.as_slice()));
    }
}

fn is_builtin_schema(schema: &str) -> bool {
    ["pg_catalog", "information_schema", "public"].contains(&schema)
}

fn value_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(number) | Value::Reg(number, _) => Some(*number),
        _ => None,
    }
}

fn value_text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) | Value::Reg(_, text) => Some(text),
        _ => None,
    }
}

fn relation_oid(relations: &BTreeMap<String, BTreeMap<String, i64>>, row: &[Value]) -> Option<i64> {
    let schema = value_text(&row[1])?;
    let name = value_text(&row[2])?;
    relations.get(schema)?.get(name).copied()
}
