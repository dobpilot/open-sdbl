//! A connection's virtual system catalog. It holds only proven readable metadata.
mod model;
mod relations;
pub(crate) mod sql;
#[cfg(test)]
mod tests;
mod wire;

pub(crate) use model::{Candidate, candidates, finish, synonym};
use sql::{Table, Value};
use std::collections::BTreeMap;
pub(crate) use wire::{decode, encode};

pub(crate) struct Catalog {
    pub(crate) revision: u32,
    pub(crate) _build_failures: usize,
    pub(crate) tables: BTreeMap<String, Table>,
    pub(crate) user: String,
    pub(crate) database: String,
    pub(crate) schemas: Vec<String>,
    pub(crate) views: BTreeMap<i64, String>,
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
}
