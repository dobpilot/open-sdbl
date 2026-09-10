//! Temporary tables emulated with common table expressions.
//!
//! The compiler never issues DDL: a read-only 1C database copy cannot hold
//! real temporary tables. `ПОМЕСТИТЬ` therefore compiles its statement into
//! a common table expression that later statements read, and this manager
//! keeps those definitions between compilations the way 1C's
//! `МенеджерВременныхТаблиц` keeps temporary tables between queries.

use std::collections::BTreeSet;

use crate::Token;
use crate::metadata::SnapshotFingerprint;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::CompiledColumn;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// One temporary table visible to the next statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempTable<'manager> {
    name: &'manager str,
    columns: &'manager [CompiledColumn],
}

impl<'manager> TempTable<'manager> {
    /// The name the query text uses, as written when the table was placed.
    #[must_use]
    pub const fn name(&self) -> &'manager str {
        self.name
    }

    /// The columns of the table in statement order.
    #[must_use]
    pub const fn columns(&self) -> &'manager [CompiledColumn] {
        self.columns
    }
}

/// One compiled definition. Hidden entries were dropped by `УНИЧТОЖИТЬ`
/// but stay in the manager because later definitions may still read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TempTableEntry {
    pub(super) name: String,
    pub(super) id: u32,
    pub(super) sql: String,
    pub(super) columns: Vec<CompiledColumn>,
    /// The transitive closure of the CTEs this definition reads.
    pub(super) dependencies: BTreeSet<u32>,
    pub(super) visible: bool,
}

/// What a resolved temporary-table source contributes to a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TempTableSource {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) columns: Vec<CompiledColumn>,
    pub(super) dependencies: BTreeSet<u32>,
}

/// Compiled temporary tables shared by consecutive batches.
///
/// A manager holds generated SQL and column metadata only; it performs no
/// I/O and is bound to the dialect and metadata snapshot of its first
/// definition.
///
/// ```no_run
/// use open_sdbl::query::{CompileOptions, TempTablesManager};
/// # fn example(compiler: &open_sdbl::query::QueryCompiler<'_, open_sdbl::query::PostgresBackend>) -> Result<(), open_sdbl::query::QueryDiagnostic> {
/// let mut manager = TempTablesManager::new();
/// let options = CompileOptions::new();
/// compiler.compile_batch("ВЫБРАТЬ Код ПОМЕСТИТЬ ВТ ИЗ Справочник.Товары;", &options, &mut manager)?;
/// let query = compiler.compile_batch("ВЫБРАТЬ Т.Код ИЗ ВТ КАК Т;", &options, &mut manager)?;
/// assert!(query.is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TempTablesManager {
    entries: Vec<TempTableEntry>,
    next_id: u32,
    binding: Option<(SqlDialect, SnapshotFingerprint)>,
}

impl TempTablesManager {
    /// The largest number of definitions one manager keeps. `ДОБАВИТЬ`
    /// defines a new entry, so long append chains count toward the bound.
    const MAX_DEFINITIONS: usize = 256;

    /// An empty manager, bound to no dialect or snapshot yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            binding: None,
        }
    }

    /// A shared empty manager for compilations without temporary tables.
    pub(super) fn none() -> &'static Self {
        static EMPTY: TempTablesManager = TempTablesManager::new();
        &EMPTY
    }

    /// The tables a statement can read, in definition order.
    pub fn tables(&self) -> impl Iterator<Item = TempTable<'_>> {
        self.entries
            .iter()
            .filter(|entry| entry.visible)
            .map(|entry| TempTable {
                name: &entry.name,
                columns: &entry.columns,
            })
    }

    /// Whether a table of this name can be read.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.visible(name).is_some()
    }

    /// Whether no table can be read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables().next().is_none()
    }

    /// Forgets every definition and unbinds the manager.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.next_id = 0;
        self.binding = None;
    }

    fn visible(&self, name: &str) -> Option<&TempTableEntry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.visible && names_equal(&entry.name, name))
    }

    pub(super) fn entry(&self, id: u32) -> Option<&TempTableEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Resolves a source name, or reports it as unknown at its token.
    pub(super) fn source(&self, token: &Token<'_>) -> Result<TempTableSource, QueryDiagnostic> {
        let entry = self.visible(token.lexeme).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(token),
                format!("temporary table {:?} does not exist", token.lexeme),
            )
        })?;
        Ok(TempTableSource {
            id: entry.id,
            name: entry.name.clone(),
            columns: entry.columns.clone(),
            dependencies: entry.dependencies.clone(),
        })
    }

    /// Verifies that stored definitions may be used with this compilation.
    pub(super) fn check_binding(
        &self,
        dialect: SqlDialect,
        fingerprint: SnapshotFingerprint,
    ) -> Result<(), QueryDiagnostic> {
        let Some((bound_dialect, bound_fingerprint)) = self.binding else {
            return Ok(());
        };
        if bound_fingerprint != fingerprint {
            return Err(QueryDiagnostic::snapshot_mismatch());
        }
        if bound_dialect != dialect {
            return Err(QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::TemporaryTable,
                "temporary tables were compiled for another SQL dialect",
            ));
        }
        Ok(())
    }

    /// Registers a `ПОМЕСТИТЬ` definition and returns its CTE id.
    pub(super) fn define(
        &mut self,
        name: &Token<'_>,
        sql: String,
        columns: Vec<CompiledColumn>,
        dependencies: BTreeSet<u32>,
        dialect: SqlDialect,
        fingerprint: SnapshotFingerprint,
    ) -> Result<u32, QueryDiagnostic> {
        if self.visible(name.lexeme).is_some() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TemporaryTable,
                Some(name),
                format!("temporary table {:?} already exists", name.lexeme),
            ));
        }
        self.push(
            name.lexeme.to_owned(),
            sql,
            columns,
            dependencies,
            dialect,
            fingerprint,
            Some(name),
        )
    }

    /// Registers a `ДОБАВИТЬ` definition over an existing table.
    pub(super) fn append(
        &mut self,
        name: &Token<'_>,
        sql: String,
        columns: Vec<CompiledColumn>,
        dependencies: BTreeSet<u32>,
        dialect: SqlDialect,
        fingerprint: SnapshotFingerprint,
    ) -> Result<u32, QueryDiagnostic> {
        let previous = self
            .visible(name.lexeme)
            .map(|entry| entry.name.clone())
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::TemporaryTable,
                    Some(name),
                    format!("temporary table {:?} does not exist", name.lexeme),
                )
            })?;
        self.hide(&previous);
        self.push(
            previous,
            sql,
            columns,
            dependencies,
            dialect,
            fingerprint,
            Some(name),
        )
    }

    /// Hides a table so that later statements no longer see it.
    pub(super) fn drop_table(&mut self, name: &Token<'_>) -> Result<String, QueryDiagnostic> {
        let dropped = self
            .visible(name.lexeme)
            .map(|entry| entry.name.clone())
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::TemporaryTable,
                    Some(name),
                    format!("temporary table {:?} does not exist", name.lexeme),
                )
            })?;
        self.hide(&dropped);
        Ok(dropped)
    }

    fn hide(&mut self, name: &str) {
        for entry in &mut self.entries {
            if names_equal(&entry.name, name) {
                entry.visible = false;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        name: String,
        sql: String,
        columns: Vec<CompiledColumn>,
        dependencies: BTreeSet<u32>,
        dialect: SqlDialect,
        fingerprint: SnapshotFingerprint,
        token: Option<&Token<'_>>,
    ) -> Result<u32, QueryDiagnostic> {
        if self.entries.len() == Self::MAX_DEFINITIONS {
            return Err(QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::TemporaryTable,
                token,
                format!(
                    "temporary tables exceed the limit of {} definitions",
                    Self::MAX_DEFINITIONS
                ),
            ));
        }
        let id = self.next_id + 1;
        self.next_id = id;
        self.binding.get_or_insert((dialect, fingerprint));
        self.entries.push(TempTableEntry {
            name,
            id,
            sql,
            columns,
            dependencies,
            visible: true,
        });
        Ok(id)
    }
}

/// The generated name of one temporary-table CTE.
pub(super) fn cte_name(id: u32) -> String {
    format!("vt{id}")
}
