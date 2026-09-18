use super::context::{CompiledBranch, OrderKey, OuterScope};
use super::expression::composite_member_of;
use super::select::{BranchMode, compile_branch};
use super::totals::wrap_totals;
use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::{OrderTerm, Projection, QueryAst};
use crate::query::core::params::Parameters;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery, NestedResult, PresentationPlan,
};
use crate::query::core::restrict::{AccessRestriction, RestrictionTarget};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind, SqlDialect};
use std::collections::{BTreeMap, BTreeSet};

/// The projected values that the branches of a union carry with different
/// shapes, and the members every branch must spread such a value over. A
/// branch that already projects the members keeps them; the others are
/// recompiled to match.
fn composite_positions(branches: &[CompiledBranch]) -> BTreeMap<usize, Vec<&'static str>> {
    let mut expand = BTreeMap::new();
    if branches.len() < 2 {
        return expand;
    }
    let width = branches
        .iter()
        .map(|branch| branch.logical_width)
        .min()
        .unwrap_or(0);
    for logical in 0..width {
        let spans = branches
            .iter()
            .map(|branch| logical_span(branch, logical))
            .collect::<Vec<_>>();
        if spans.iter().any(|span| span.is_empty()) {
            continue;
        }
        if spans.iter().all(|span| span.len() == 1) {
            let first = &spans[0][0].kind;
            if spans
                .iter()
                .all(|span| span[0].kind.is_compatible_with(first))
            {
                continue;
            }
            let Some(members) = scalar_members(&spans) else {
                continue;
            };
            expand.insert(logical, members);
            continue;
        }
        // One branch already carries the members; the rest follow its
        // layout. A wider span that is not a composite value — a field
        // whose members SchemaStorage does not name — keeps its own
        // diagnostic.
        let Some(members) = spans
            .iter()
            .find(|span| span.len() > 1)
            .map(|span| span.iter().map(member_suffix).collect::<Vec<_>>())
            .filter(|members| {
                members.contains(&"_TYPE")
                    && members
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == members.len()
            })
        else {
            continue;
        };
        if spans.iter().all(|span| {
            span.len() == members.len()
                && span.iter().map(member_suffix).eq(members.iter().copied())
        }) {
            continue;
        }
        expand.insert(logical, members);
    }
    expand
}

/// The output columns one projected value occupies in a branch.
fn logical_span(branch: &CompiledBranch, logical: usize) -> &[CompiledColumn] {
    let start: usize = branch.logical_columns[..logical.min(branch.logical_columns.len())]
        .iter()
        .sum();
    let len = branch.logical_columns.get(logical).copied().unwrap_or(0);
    branch.columns.get(start..start + len).unwrap_or_default()
}

/// The member suffix a column carries — read from the requested name,
/// because the output label may be cut to the dialect's identifier limit
/// and lose the suffix.
fn member_suffix(column: &CompiledColumn) -> &'static str {
    ["_TYPE", "_S", "_N", "_T", "_L"]
        .into_iter()
        .find(|suffix| column.name.ends_with(suffix))
        .unwrap_or("")
}

/// The members a group of single-column branches must spread over: the
/// member of every kind present and the discriminator, ordered the way a
/// `ВЫБОР` of alternatives orders them.
fn scalar_members(spans: &[&[CompiledColumn]]) -> Option<Vec<&'static str>> {
    let mut members = Vec::new();
    for span in spans {
        let (suffix, _) = composite_member_of(&span[0].kind)?;
        if !members.contains(&suffix) {
            members.push(suffix);
        }
    }
    members.sort_by_key(|suffix| if suffix.is_empty() { 0 } else { 1 });
    members.push("_TYPE");
    Some(members)
}

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
    pub(super) parameters: Parameters<'plans>,
    /// Access restrictions handed to every statement's catalog.
    pub(super) restrictions: &'plans [AccessRestriction],
    /// Targets read under `РАЗРЕШЕННЫЕ` across the batch.
    pub(super) restriction_targets: BTreeSet<RestrictionTarget>,
    /// Positions in `restrictions` that some statement applied.
    pub(super) used_restrictions: BTreeSet<usize>,
    /// Whether statements with `ИТОГИ` append the `__level` column.
    pub(super) totals_level: bool,
}

impl<'plans> PresentationCompilation<'plans> {
    pub(super) fn strict(
        plans: &'plans [PresentationPlan],
        parameters: Parameters<'plans>,
        dialect: SqlDialect,
    ) -> Self {
        Self {
            plans,
            requested: BTreeSet::new(),
            collect_only: false,
            dialect,
            parameters,
            restrictions: &[],
            restriction_targets: BTreeSet::new(),
            used_restrictions: BTreeSet::new(),
            totals_level: false,
        }
    }

    pub(super) fn with_restrictions(mut self, restrictions: &'plans [AccessRestriction]) -> Self {
        self.restrictions = restrictions;
        self
    }

    pub(super) const fn with_totals_level(mut self, enabled: bool) -> Self {
        self.totals_level = enabled;
        self
    }

    pub(super) fn collect(dialect: SqlDialect) -> Self {
        Self {
            plans: &[],
            requested: BTreeSet::new(),
            collect_only: true,
            dialect,
            parameters: Parameters::unbound(),
            restrictions: &[],
            restriction_targets: BTreeSet::new(),
            used_restrictions: BTreeSet::new(),
            totals_level: false,
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

/// Compiles one statement. `nested` carries the token of a nested query,
/// which stays in the storage domain (no MSSQL year-offset correction on
/// its projections), cannot project `*` or deferred presentations, and may
/// order its rows only together with `ПЕРВЫЕ`.
/// Renders the nested statement of every tabular section the branch
/// projects. A section needs the main statement, which is why it is
/// rendered here and not where the projections compile.
fn render_sections(
    branch: &CompiledBranch,
    dialect: SqlDialect,
    ast: &QueryAst<'_, '_>,
) -> Result<Vec<NestedResult>, QueryDiagnostic> {
    if branch.sections.is_empty() {
        return Ok(Vec::new());
    }
    if ast.totals.is_some() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            ast.branches.first().and_then(|branch| {
                branch
                    .projection
                    .first()
                    .and_then(|projection| super::sources::projection_token(&projection.expression))
            }),
            "a tabular section cannot be projected together with ИТОГИ",
        ));
    }
    let main = branch
        .keyed_sql
        .as_deref()
        .expect("a branch with sections keeps its unordered statement");
    let mut nested = Vec::with_capacity(branch.sections.len());
    for (section, owner_column) in branch.sections.iter().zip(&branch.service_columns) {
        let key_label = branch.columns[*owner_column].label.clone();
        nested.push(section.render(
            dialect,
            &dialect.quote_identifier(&key_label),
            main,
            *owner_column,
        ));
    }
    Ok(nested)
}

pub(super) fn compile_query_ast(
    ast: &QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    presentations: &mut PresentationCompilation<'_>,
    nested: Option<&Token<'_>>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    compile_query_ast_with_outer(ast, snapshot, catalog, presentations, nested, &[])
}

/// Compiles a statement that may reference the sources of an enclosing
/// one, which is how a correlated subquery reads the outer row.
pub(super) fn compile_query_ast_with_outer(
    ast: &QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    presentations: &mut PresentationCompilation<'_>,
    nested: Option<&Token<'_>>,
    outer: &[OuterScope],
) -> Result<CompiledQuery, QueryDiagnostic> {
    let dialect = presentations.dialect;
    if let Some(token) = nested {
        catalog.charge(1, None)?;
        if ast.branches.iter().any(|branch| {
            branch
                .projection
                .iter()
                .any(|item| matches!(item.expression, Projection::All))
        }) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "a nested query cannot project '*'",
            ));
        }
        if let Some(term) = ast.order.first()
            && ast.branches.iter().any(|branch| branch.top.is_none())
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(term.token),
                "ORDER BY inside a nested query requires TOP on every branch",
            ));
        }
    }
    // Each branch of a nested union limits itself, and the union has no
    // limit of its own, so its ordering changes nothing: it is dropped.
    let order_dropped = nested.is_some() && ast.branches.len() > 1;
    if let (Some(into), Some(totals)) = (ast.into.as_ref(), ast.totals.as_ref()) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(totals.token),
            format!(
                "TOTALS cannot be used in a statement that defines the temporary table {:?}",
                into.name.lexeme
            ),
        ));
    }
    if let (Some(token), Some(totals)) = (nested, ast.totals.as_ref()) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(totals.token),
            format!(
                "TOTALS inside the nested query opened at {:?} are not supported",
                token.lexeme
            ),
        ));
    }
    let totals_mode = ast.totals.is_some();
    let unioned = !ast.unions.is_empty();
    let compile_branches = |widen: &BTreeSet<usize>,
                            expand: &BTreeMap<usize, Vec<&'static str>>,
                            presentations: &mut PresentationCompilation<'_>|
     -> Result<Vec<CompiledBranch>, QueryDiagnostic> {
        let mut branches = Vec::with_capacity(ast.branches.len());
        for (index, branch) in ast.branches.iter().enumerate() {
            catalog.charge(
                1usize.saturating_add(branch.projection.len()),
                branch.source.as_ref().map(|source| source.object),
            )?;
            let order: &[OrderTerm<'_, '_>] = if index == 0 && !order_dropped {
                &ast.order
            } else {
                &[]
            };
            branches.push(compile_branch(
                branch,
                snapshot,
                catalog,
                order,
                unioned && index == 0,
                presentations,
                BranchMode {
                    widen,
                    expand,
                    storage_domain: nested.is_some(),
                    totals: totals_mode,
                    outer,
                },
            )?);
        }
        Ok(branches)
    };
    let mut branches = compile_branches(&BTreeSet::new(), &BTreeMap::new(), presentations)?;
    // A value that one branch carries as a composite is spread over the
    // same members in every branch, the way the platform writes a union of
    // values of different types.
    let mut expand = composite_positions(&branches);
    if !expand.is_empty() {
        branches = compile_branches(&BTreeSet::new(), &expand, presentations)?;
    }

    let first = branches.first().expect("a query has at least one branch");
    for (index, branch) in branches.iter().enumerate().skip(1) {
        if branch.logical_width != first.logical_width
            || branch.columns.len() != first.columns.len()
        {
            // The same count of fields rendered over different counts of
            // columns: name the first field whose width differs.
            let differing = (0..first.logical_width.min(branch.logical_width))
                .map(|logical| (logical_span(first, logical), logical_span(branch, logical)))
                .find(|(expected, actual)| expected.len() != actual.len())
                .and_then(|(expected, actual)| {
                    let column = expected.first()?;
                    let label = column
                        .label
                        .strip_suffix(member_suffix(column))
                        .unwrap_or(&column.label);
                    Some(format!(
                        "; field {label:?} renders as {} SQL columns in branch {} and {} in branch 1",
                        actual.len(),
                        index + 1,
                        expected.len()
                    ))
                })
                .unwrap_or_default();
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(ast.unions[index - 1].token),
                format!(
                    "UNION branch {} projects {} logical fields and {} SQL columns; expected {} logical fields and {} SQL columns{differing}",
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

    if let Some(token) = nested
        && branches
            .iter()
            .any(|branch| !branch.deferred_presentations.is_empty())
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "deferred reference presentations inside a nested query are not supported; present the column in the outer query",
        ));
    }
    if !unioned {
        let branch = branches.pop().expect("a query has one branch");
        // A projected tabular section is answered by a statement of its
        // own, filtered by the owners this statement names.
        let sections = render_sections(&branch, dialect, ast)?;
        let compiled = CompiledQuery {
            sql: branch.sql,
            columns: branch.columns,
            deferred_presentations: branch.deferred_presentations,
            nested: sections,
            service_columns: branch.service_columns,
        };
        let compiled = match &ast.totals {
            Some(totals) => wrap_totals(
                ast,
                totals,
                compiled,
                &branch.order,
                presentations.totals_level,
                presentations.parameters,
                snapshot,
                dialect,
            ),
            None => Ok(compiled),
        };
        return attach_hierarchy_ctes(compiled, catalog, nested, dialect);
    }

    // Reference columns whose branches disagree on target or width are
    // widened to one runtime-typed payload; branches projecting a fixed
    // reference there are compiled again with the widening instruction.
    let widen = widened_positions(&branches, ast.unions[0].token)?;
    if !widen.is_empty() {
        branches = compile_branches(&widen, &expand, presentations)?;
    }
    expand.clear();
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
            CompiledColumn::named(column.name.clone(), column.label.clone(), kind)
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
    if !first.order.is_empty() && !totals_mode {
        sql.push_str(" ORDER BY ");
        sql.push_str(
            &first
                .order
                .iter()
                .map(OrderKey::render)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    let compiled = CompiledQuery {
        sql,
        columns,
        deferred_presentations: first.deferred_presentations.clone(),
        nested: Vec::new(),
        service_columns: Vec::new(),
    };
    let compiled = match &ast.totals {
        Some(totals) => wrap_totals(
            ast,
            totals,
            compiled,
            &first.order,
            presentations.totals_level,
            presentations.parameters,
            snapshot,
            dialect,
        ),
        None => Ok(compiled),
    };
    attach_hierarchy_ctes(compiled, catalog, nested, dialect)
}

/// Prefixes the statement with the recursive CTEs its `В ИЕРАРХИИ`
/// predicates registered. A nested query leaves them to the enclosing
/// statement, which shares the catalog; PostgreSQL spells the keyword
/// `WITH RECURSIVE`, SQL Server plain `WITH`.
fn attach_hierarchy_ctes(
    compiled: Result<CompiledQuery, QueryDiagnostic>,
    catalog: &CompilationCatalog<'_>,
    nested: Option<&Token<'_>>,
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let mut compiled = compiled?;
    if nested.is_some() {
        return Ok(compiled);
    }
    let ctes = catalog.take_hierarchy_ctes();
    if ctes.is_empty() {
        return Ok(compiled);
    }
    let definitions = ctes
        .iter()
        .map(|(name, sql)| format!("{} AS ({sql})", dialect.quote_identifier(name)))
        .collect::<Vec<_>>()
        .join(", ");
    let keyword = if dialect == SqlDialect::Postgres {
        "WITH RECURSIVE "
    } else {
        "WITH "
    };
    compiled.sql = match compiled
        .sql
        .strip_prefix("WITH RECURSIVE ")
        .or_else(|| compiled.sql.strip_prefix("WITH "))
    {
        Some(rest) => format!("{keyword}{definitions}, {rest}"),
        None => format!("{keyword}{definitions} {}", compiled.sql),
    };
    Ok(compiled)
}
