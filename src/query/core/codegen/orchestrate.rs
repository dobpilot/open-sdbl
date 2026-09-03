use super::select::compile_branch;
use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::{OrderTerm, QueryAst};
use crate::query::core::resolve::{CompilationCatalog, CompiledQuery, PresentationPlan};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind, SqlDialect};
use std::collections::BTreeSet;

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
    let mut branches = Vec::with_capacity(ast.branches.len());
    for (index, branch) in ast.branches.iter().enumerate() {
        let order: &[OrderTerm<'_, '_>] = if index == 0 { &ast.order } else { &[] };
        branches.push(compile_branch(
            branch,
            snapshot,
            &catalog,
            order,
            unioned && index == 0,
            presentations,
        )?);
    }

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
    }

    if !unioned {
        let branch = branches.pop().expect("a query has one branch");
        return Ok(CompiledQuery {
            sql: branch.sql,
            columns: branch.columns,
            deferred_presentations: branch.deferred_presentations,
        });
    }

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
        columns: first.columns.clone(),
        deferred_presentations: first.deferred_presentations.clone(),
    })
}
