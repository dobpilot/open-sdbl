use super::context::CompiledBranch;
use super::select::compile_branch;
use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::{OrderTerm, QueryAst};
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery, PresentationPlan,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind, SqlDialect};
use std::collections::BTreeSet;

/// Positions of reference columns that UNION branches project with different
/// targets or widths. A fixed reference without exactly one target cannot be
/// widened and is reported at the union token.
fn widened_positions(
    branches: &[CompiledBranch],
    token: &Token<'_>,
) -> Result<BTreeSet<usize>, QueryDiagnostic> {
    let width = branches.first().map_or(0, |branch| branch.columns.len());
    let mut widen = BTreeSet::new();
    for position in 0..width {
        let kinds = branches
            .iter()
            .filter_map(|branch| branch.columns.get(position))
            .filter_map(|column| match &column.kind {
                ColumnKind::Reference {
                    targets,
                    runtime_typed,
                } => Some((targets, *runtime_typed)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let uniform = kinds.iter().all(|(targets, runtime_typed)| {
            !runtime_typed && targets.len() == 1 && targets[0] == kinds[0].0[0]
        });
        if kinds.is_empty() || uniform {
            continue;
        }
        if kinds
            .iter()
            .any(|(targets, runtime_typed)| !runtime_typed && targets.len() != 1)
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!(
                    "UNION column {} mixes reference widths that cannot be widened",
                    position + 1
                ),
            ));
        }
        if kinds.iter().any(|(_, runtime_typed)| !runtime_typed) {
            widen.insert(position);
        }
    }
    Ok(widen)
}

pub(super) struct PresentationCompilation<'plans> {
    plans: &'plans [PresentationPlan],
    pub(super) requested: BTreeSet<ObjectId>,
    collect_only: bool,
    pub(super) dialect: SqlDialect,
}

impl<'plans> PresentationCompilation<'plans> {
    pub(super) fn strict(plans: &'plans [PresentationPlan], dialect: SqlDialect) -> Self {
        Self {
            plans,
            requested: BTreeSet::new(),
            collect_only: false,
            dialect,
        }
    }

    pub(super) fn collect(dialect: SqlDialect) -> Self {
        Self {
            plans: &[],
            requested: BTreeSet::new(),
            collect_only: true,
            dialect,
        }
    }

    pub(super) fn plan(
        &mut self,
        object: ObjectId,
        token: &Token<'_>,
    ) -> Result<Option<&'plans PresentationPlan>, QueryDiagnostic> {
        self.requested.insert(object);
        let mut matches = self.plans.iter().filter(|plan| plan.object == object);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::PresentationPlan,
                Some(token),
                format!("duplicate presentation plans for object {object}"),
            ));
        }
        if first.is_none() && !self.collect_only {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::PresentationPlan,
                Some(token),
                format!("missing presentation plan for object {object}"),
            ));
        }
        Ok(first)
    }
}

pub(super) fn compile(
    ast: QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let dialect = presentations.dialect;
    let catalog = CompilationCatalog::new(snapshot);
    let unioned = !ast.unions.is_empty();
    let compile_branches = |widen: &BTreeSet<usize>,
                            presentations: &mut PresentationCompilation<'_>|
     -> Result<Vec<CompiledBranch>, QueryDiagnostic> {
        let mut branches = Vec::with_capacity(ast.branches.len());
        for (index, branch) in ast.branches.iter().enumerate() {
            catalog.charge(
                1usize.saturating_add(branch.projection.len()),
                branch.source.as_ref().map(|source| source.object),
            )?;
            let order: &[OrderTerm<'_, '_>] = if index == 0 { &ast.order } else { &[] };
            branches.push(compile_branch(
                branch,
                snapshot,
                &catalog,
                order,
                unioned && index == 0,
                presentations,
                widen,
            )?);
        }
        Ok(branches)
    };
    let mut branches = compile_branches(&BTreeSet::new(), presentations)?;

    let first = branches.first().expect("a query has at least one branch");
    for (index, branch) in branches.iter().enumerate().skip(1) {
        if branch.logical_width != first.logical_width
            || branch.columns.len() != first.columns.len()
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(ast.unions[index - 1].token),
                format!(
                    "UNION branch {} projects {} logical fields and {} SQL columns; expected {} logical fields and {} SQL columns",
                    index + 1,
                    branch.logical_width,
                    branch.columns.len(),
                    first.logical_width,
                    first.columns.len(),
                ),
            ));
        }
        if branch.deferred_presentations != first.deferred_presentations {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(ast.unions[index - 1].token),
                format!(
                    "UNION branch {} defers presentation columns {:?}; expected {:?}",
                    index + 1,
                    branch.deferred_presentations,
                    first.deferred_presentations,
                ),
            ));
        }
        for (position, (column, first_column)) in
            branch.columns.iter().zip(&first.columns).enumerate()
        {
            if !column.kind.is_compatible_with(&first_column.kind) {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(ast.unions[index - 1].token),
                    format!(
                        "UNION branch {} projects {:?} at column {} where branch 1 projects {:?}",
                        index + 1,
                        column.kind,
                        position + 1,
                        first_column.kind,
                    ),
                ));
            }
        }
    }

    if !unioned {
        let branch = branches.pop().expect("a query has one branch");
        return Ok(CompiledQuery {
            sql: branch.sql,
            columns: branch.columns,
            deferred_presentations: branch.deferred_presentations,
        });
    }

    // Reference columns whose branches disagree on target or width are
    // widened to one runtime-typed payload; branches projecting a fixed
    // reference there are compiled again with the widening instruction.
    let widen = widened_positions(&branches, ast.unions[0].token)?;
    if !widen.is_empty() {
        branches = compile_branches(&widen, presentations)?;
    }
    let first = branches.first().expect("a query has at least one branch");

    // The merged kind of each column is the first non-wildcard kind across
    // branches, so `NULL` branches do not hide the real type; widened
    // reference columns carry the union of the branch targets.
    let columns = first
        .columns
        .iter()
        .enumerate()
        .map(|(position, column)| {
            let kind = if widen.contains(&position) {
                ColumnKind::Reference {
                    targets: branches
                        .iter()
                        .filter_map(|branch| branch.columns.get(position))
                        .filter_map(|column| match &column.kind {
                            ColumnKind::Reference { targets, .. } => Some(targets.iter().copied()),
                            _ => None,
                        })
                        .flatten()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    runtime_typed: true,
                }
            } else {
                branches
                    .iter()
                    .filter_map(|branch| branch.columns.get(position))
                    .map(|column| &column.kind)
                    .find(|kind| !kind.is_wildcard())
                    .unwrap_or(&column.kind)
                    .clone()
            };
            CompiledColumn::new(column.label.clone(), kind)
        })
        .collect::<Vec<_>>();

    let mut sql = if dialect == SqlDialect::Postgres {
        format!("({})", first.sql)
    } else {
        first.sql.clone()
    };
    for (link, branch) in ast.unions.iter().zip(branches.iter().skip(1)) {
        match dialect {
            SqlDialect::Postgres => {
                sql.push_str(if link.all { " UNION ALL (" } else { " UNION (" });
                sql.push_str(&branch.sql);
                sql.push(')');
            }
            SqlDialect::MsSql { .. } => {
                sql.push_str(if link.all { " UNION ALL " } else { " UNION " });
                sql.push_str(&branch.sql);
            }
        }
    }
    if !first.order.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(&first.order.join(", "));
    }
    Ok(CompiledQuery {
        sql,
        columns,
        deferred_presentations: first.deferred_presentations.clone(),
    })
}
