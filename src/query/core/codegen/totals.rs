//! `ИТОГИ … ПО …`: the rows of the platform's linear traversal.
//!
//! The statement is wrapped into a CTE that numbers its rows in the user
//! order; the overall row, one aggregated `SELECT` per control-point level,
//! and the detail rows are combined with `UNION ALL` and ordered so that a
//! total precedes the rows it covers, groups following the first appearance
//! of their value.

use super::context::OrderKey;
use super::params::render_scalar_parameter;
use super::select::derived_data_type;
use crate::metadata::{LiveTable, MetadataSnapshot};
use crate::query::core::ast::{
    AggregateArgument, AggregateKind, ControlPoint, Expression, Projection, QueryAst, TotalsAst,
    TotalsField,
};
use crate::query::core::dialect::{SqlDialect, compile_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::{ParameterValue, Parameters};
use crate::query::core::resolve::{ColumnKind, CompiledColumn, CompiledQuery};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

/// The CTE holding the numbered rows of the wrapped statement.
const ROWS: &str = "__totals_rows";
/// Alias of the wrapped statement inside the CTE.
const SOURCE: &str = "__totals_source";
/// Alias of the `UNION ALL` the final ordering reads.
const RESULT: &str = "__totals";
/// Row number of a detail row in the user order.
const ROW_NUMBER: &str = "__rn";
/// The platform's `Уровень()` of a row.
const LEVEL: &str = "__level";
/// The hierarchy key of a row: the hierarchical control point value, or
/// its parent under `ТОЛЬКО ИЕРАРХИЯ`.
const HIERARCHY_KEY: &str = "__hk";
/// Alias of the catalog table inside the hierarchy CTEs.
const CATALOG: &str = "__totals_catalog";
/// Recursive CTE of `(leaf, ancestor node, steps)` pairs.
const ANCESTORS: &str = "__totals_ancestors";
const LEAF: &str = "__leaf";
const NODE: &str = "__node";
const STEPS: &str = "__steps";
/// CTE of the depth of every leaf.
const DEPTHS: &str = "__totals_depths";
const DEPTH: &str = "__depth";
/// CTE of every node with its rank, depth, and parent.
const NODES: &str = "__totals_nodes";
const RANK: &str = "__rank";
const PARENT: &str = "__parent";
/// `1` when the node also has a hierarchy row (it is an ancestor of some
/// key), which pushes its own group row one level down, as on the
/// platform.
const HIER: &str = "__hier";
/// Recursive CTE of the materialized rank path of every node.
const PATHS: &str = "__totals_paths";
const PATH: &str = "__path";

/// The kind a totals field expression produces.
enum FieldKind {
    /// A count, sum, average, or arithmetic: a number.
    Number,
    /// `МИНИМУМ`/`МАКСИМУМ` of the result column at this position.
    Column(usize),
}

/// A control point with `[ТОЛЬКО] ИЕРАРХИЯ`: the catalog whose parent
/// chain the hierarchy rows follow.
struct Hierarchy {
    /// Index into the control points.
    index: usize,
    only: bool,
    /// Quoted live table of the catalog.
    table: String,
    /// Quoted `_IDRRef` and `_ParentIDRRef` columns of the catalog.
    id: String,
    parent: String,
}

/// Wraps a compiled statement into the totals rows.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn wrap_totals(
    ast: &QueryAst<'_, '_>,
    totals: &TotalsAst<'_, '_>,
    compiled: CompiledQuery,
    order: &[OrderKey],
    level_column: bool,
    parameters: Parameters<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let names = column_names(ast, &compiled.columns);
    let (points, hierarchy) = resolve_points(
        totals,
        &compiled.columns,
        &names,
        parameters,
        snapshot,
        dialect,
    )?;
    let overall = totals.overall.is_some();
    if points.is_empty() && !overall {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(totals.token),
            "TOTALS requires OVERALL or at least one control point",
        ));
    }
    let aggregates = resolve_fields(totals, &compiled.columns, &names, parameters, dialect)?;

    let quote = |name: &str| dialect.quote_identifier(name);
    let labels = compiled
        .columns
        .iter()
        .map(|column| quote(&column.label))
        .collect::<Vec<_>>();
    let rows = quote(ROWS);
    let source = quote(SOURCE);
    let rn = quote(ROW_NUMBER);
    let level = quote(LEVEL);
    let hk = quote(HIERARCHY_KEY);
    let path = quote(PATH);
    let empty_reference = dialect.binary_literal(&[0; 16]);
    let keys = if order.is_empty() {
        "(SELECT 1)".to_owned()
    } else {
        order
            .iter()
            .map(|key| {
                format!(
                    "{}{}",
                    key.position.map_or_else(
                        || key.sql.clone(),
                        |position| format!("{source}.{}", labels[position - 1])
                    ),
                    if key.descending { " DESC" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // PostgreSQL 1C string columns may still be `mvarchar` after functions
    // such as `ЕСТЬNULL`; the CTE normalises them to `text` so that the
    // typed NULL placeholders and text-cast counts unite with them.
    let mut cte_columns = compiled
        .columns
        .iter()
        .zip(&labels)
        .map(|(column, label)| {
            if dialect == SqlDialect::Postgres && matches!(column.kind, ColumnKind::String { .. }) {
                format!("{source}.{label}::text AS {label}")
            } else {
                format!("{source}.{label} AS {label}")
            }
        })
        .collect::<Vec<_>>();
    cte_columns.push(format!("ROW_NUMBER() OVER (ORDER BY {keys}) AS {rn}"));
    let mut rows_from = format!("({}) AS {source}", compiled.sql);
    if let Some(hierarchy) = &hierarchy {
        let point = &labels[points[hierarchy.index]];
        if hierarchy.only {
            cte_columns.push(format!(
                "COALESCE({}.{}, {empty_reference}) AS {hk}",
                quote(CATALOG),
                hierarchy.parent
            ));
            rows_from.push_str(&format!(
                " LEFT JOIN {} AS {} ON {}.{} = {source}.{point}",
                hierarchy.table,
                quote(CATALOG),
                quote(CATALOG),
                hierarchy.id
            ));
        } else {
            cte_columns.push(format!("{source}.{point} AS {hk}"));
        }
    }
    let mut ctes = vec![format!(
        "{rows} AS (SELECT {} FROM {rows_from})",
        cte_columns.join(", ")
    )];
    if let Some(hierarchy) = &hierarchy {
        ctes.extend(hierarchy_ctes(hierarchy, &empty_reference, dialect));
    }

    let overall_offset = usize::from(overall);
    let detail_base = points.len() + overall_offset;
    let group_column = |index: usize| quote(&format!("__g{}", index + 1));
    let flag_column = |index: usize| quote(&format!("__f{}", index + 1));
    let hierarchy_index = hierarchy.as_ref().map(|hierarchy| hierarchy.index);
    // Groups of every plain level are ordered by the first appearance of
    // their own value in the ordered result, independently of the
    // enclosing group, as the platform does.
    let rank_partition = |index: usize| labels[points[index]].clone();
    let cell = |position: usize| -> String {
        let label = &labels[position];
        match &aggregates[position] {
            Some(sql) => format!("{sql} AS {label}"),
            None => format!(
                "CAST(NULL AS {}) AS {label}",
                derived_data_type(&compiled.columns[position].kind, dialect)
            ),
        }
    };
    // The column a control point renders as in a branch: the hierarchy key
    // stands in for the hierarchical point from its level downwards.
    let key_of = |index: usize| -> String {
        if hierarchy_index == Some(index) {
            hk.clone()
        } else {
            labels[points[index]].clone()
        }
    };
    let nodes = quote(NODES);
    let paths = quote(PATHS);
    let hierarchy_joins = |key: &str| {
        format!(
            " JOIN {nodes} ON {nodes}.{} = {key} JOIN {paths} ON {paths}.{} = {key}",
            quote(NODE),
            quote(NODE)
        )
    };
    let depth = format!("{nodes}.{}", quote(DEPTH));
    let hier = format!("{nodes}.{}", quote(HIER));
    // Rows keyed by a node sit one level below the node's own hierarchy row
    // when it has one.
    let key_depth = format!("({depth} + {hier})");
    let path_value = format!("{paths}.{path}");
    let empty_path = dialect.string_literal("");

    let mut branches = Vec::with_capacity(points.len() + 3);
    // Sort keys of a branch: one rank (or path) and one flag per control
    // point, then the row number.
    let sort_keys = |branch: &mut Vec<String>,
                     ranks: &dyn Fn(usize) -> String,
                     flags: &dyn Fn(usize) -> String,
                     row_number: &str| {
        for index in 0..points.len() {
            if hierarchy_index == Some(index) {
                branch.push(format!("{} AS {path}", ranks(index)));
            } else {
                branch.push(format!("{} AS {}", ranks(index), group_column(index)));
            }
            branch.push(format!("{} AS {}", flags(index), flag_column(index)));
        }
        branch.push(format!("{row_number} AS {rn}"));
    };
    if overall {
        let mut projection = (0..labels.len()).map(cell).collect::<Vec<_>>();
        projection.push(format!("0 AS {level}"));
        sort_keys(
            &mut projection,
            &|index| {
                if hierarchy_index == Some(index) {
                    empty_path.clone()
                } else {
                    "0".to_owned()
                }
            },
            &|_| "0".to_owned(),
            "0",
        );
        branches.push(format!(
            "SELECT {} FROM {rows} HAVING COUNT(*) > 0",
            projection.join(", ")
        ));
    }
    for depth_index in 0..points.len() {
        let below_hierarchy = hierarchy_index.is_some_and(|h| depth_index >= h);
        let base = depth_index + overall_offset;
        // A hierarchy row per ancestor folder precedes the group rows of its
        // level.
        if hierarchy_index == Some(depth_index) {
            let mut projection = (0..labels.len())
                .map(|position| {
                    if points[..depth_index].contains(&position) {
                        labels[position].clone()
                    } else if points[depth_index] == position {
                        format!("{nodes}.{} AS {}", quote(NODE), labels[position])
                    } else {
                        cell(position)
                    }
                })
                .collect::<Vec<_>>();
            projection.push(format!("{base} + {depth} AS {level}"));
            sort_keys(
                &mut projection,
                &|index| {
                    if index < depth_index {
                        format!(
                            "MIN(MIN({rn})) OVER (PARTITION BY {})",
                            rank_partition(index)
                        )
                    } else if index == depth_index {
                        path_value.clone()
                    } else {
                        "0".to_owned()
                    }
                },
                &|index| if index < depth_index { "1" } else { "0" }.to_owned(),
                "0",
            );
            let mut group_by = points[..depth_index]
                .iter()
                .map(|point| labels[*point].clone())
                .collect::<Vec<_>>();
            group_by.push(format!("{nodes}.{}", quote(NODE)));
            group_by.push(depth.clone());
            group_by.push(path_value.clone());
            // A folder's row aggregates everything beneath it: the rows keyed
            // by the folder itself and the rows of every descendant key.
            branches.push(format!(
                "SELECT {} FROM {rows} JOIN {nodes} ON {hier} = 1 AND ({nodes}.{node} = {hk} OR EXISTS (SELECT 1 FROM {ancestors} a WHERE a.{leaf} = {hk} AND a.{node} = {nodes}.{node})) JOIN {paths} ON {paths}.{node} = {nodes}.{node} GROUP BY {}",
                projection.join(", "),
                group_by.join(", "),
                node = quote(NODE),
                ancestors = quote(ANCESTORS),
                leaf = quote(LEAF),
            ));
        }
        let mut projection = (0..labels.len())
            .map(|position| {
                if let Some(index) = points[..=depth_index]
                    .iter()
                    .position(|point| *point == position)
                {
                    format!("{} AS {}", key_of(index), labels[position])
                } else {
                    cell(position)
                }
            })
            .collect::<Vec<_>>();
        if below_hierarchy {
            projection.push(format!("{base} + {key_depth} AS {level}"));
        } else {
            projection.push(format!("{base} AS {level}"));
        }
        sort_keys(
            &mut projection,
            &|index| {
                if hierarchy_index == Some(index) {
                    if index <= depth_index {
                        path_value.clone()
                    } else {
                        empty_path.clone()
                    }
                } else if index <= depth_index {
                    format!(
                        "MIN(MIN({rn})) OVER (PARTITION BY {})",
                        rank_partition(index)
                    )
                } else {
                    "0".to_owned()
                }
            },
            &|index| {
                if hierarchy_index == Some(index) {
                    match index.cmp(&depth_index) {
                        std::cmp::Ordering::Less => "2",
                        std::cmp::Ordering::Equal => "1",
                        std::cmp::Ordering::Greater => "0",
                    }
                } else if index < depth_index {
                    "1"
                } else {
                    "0"
                }
                .to_owned()
            },
            "0",
        );
        let mut group_by = (0..=depth_index).map(key_of).collect::<Vec<_>>();
        let mut from = rows.clone();
        if below_hierarchy {
            from.push_str(&hierarchy_joins(&hk));
            group_by.push(depth.clone());
            group_by.push(hier.clone());
            group_by.push(path_value.clone());
        }
        branches.push(format!(
            "SELECT {} FROM {from} GROUP BY {}",
            projection.join(", "),
            group_by.join(", ")
        ));
    }
    let mut projection = labels.clone();
    if hierarchy.is_some() {
        projection.push(format!("{detail_base} + {key_depth} AS {level}"));
    } else {
        projection.push(format!("{detail_base} AS {level}"));
    }
    sort_keys(
        &mut projection,
        &|index| {
            if hierarchy_index == Some(index) {
                path_value.clone()
            } else {
                format!("MIN({rn}) OVER (PARTITION BY {})", rank_partition(index))
            }
        },
        &|index| {
            if hierarchy_index == Some(index) {
                "2"
            } else {
                "1"
            }
            .to_owned()
        },
        &rn,
    );
    let mut from = rows.clone();
    if hierarchy.is_some() {
        from.push_str(&hierarchy_joins(&hk));
    }
    branches.push(format!("SELECT {} FROM {from}", projection.join(", ")));

    let mut ordering = Vec::with_capacity(points.len() * 2 + 1);
    for index in 0..points.len() {
        ordering.push(if hierarchy_index == Some(index) {
            path.clone()
        } else {
            group_column(index)
        });
        ordering.push(flag_column(index));
    }
    ordering.push(rn);
    let mut output = labels.clone();
    if level_column {
        output.push(level);
    }
    let sql = format!(
        "{} {} SELECT {} FROM ({}) AS {} ORDER BY {}",
        if hierarchy.is_some() && dialect == SqlDialect::Postgres {
            "WITH RECURSIVE"
        } else {
            "WITH"
        },
        ctes.join(", "),
        output.join(", "),
        branches.join(" UNION ALL "),
        quote(RESULT),
        ordering.join(", ")
    );
    let mut columns = compiled.columns;
    if level_column {
        columns.push(CompiledColumn::new(
            LEVEL.to_owned(),
            ColumnKind::Number {
                precision: None,
                scale: None,
            },
        ));
    }
    Ok(CompiledQuery {
        sql,
        columns,
        deferred_presentations: compiled.deferred_presentations,
    })
}

/// The recursive ancestor, node, and path CTEs of a hierarchical control
/// point; every hierarchy key of the rows and every ancestor becomes a
/// node ranked by the first appearance of any row beneath it.
fn hierarchy_ctes(
    hierarchy: &Hierarchy,
    empty_reference: &str,
    dialect: SqlDialect,
) -> Vec<String> {
    let quote = |name: &str| dialect.quote_identifier(name);
    let (rows, hk, rn) = (quote(ROWS), quote(HIERARCHY_KEY), quote(ROW_NUMBER));
    let (ancestors, leaf, node, steps) = (quote(ANCESTORS), quote(LEAF), quote(NODE), quote(STEPS));
    let (depths, depth, nodes, rank, parent, hier) = (
        quote(DEPTHS),
        quote(DEPTH),
        quote(NODES),
        quote(RANK),
        quote(PARENT),
        quote(HIER),
    );
    let (paths, path, catalog) = (quote(PATHS), quote(PATH), quote(CATALOG));
    let (table, id, parent_column) = (&hierarchy.table, &hierarchy.id, &hierarchy.parent);
    let segment = |rank: &str| match dialect {
        SqlDialect::Postgres => format!("LPAD(CAST({rank} AS text), 12, '0')"),
        SqlDialect::MsSql { .. } => {
            format!("RIGHT('000000000000' + CAST({rank} AS varchar(12)), 12)")
        }
    };
    let (root_path, child_path) = match dialect {
        SqlDialect::Postgres => (
            segment(&format!("n.{rank}")),
            format!("p.{path} || '/' || {}", segment(&format!("c.{rank}"))),
        ),
        SqlDialect::MsSql { .. } => (
            format!("CAST({} AS varchar(4000))", segment(&format!("n.{rank}"))),
            format!(
                "CAST(p.{path} + '/' + {} AS varchar(4000))",
                segment(&format!("c.{rank}"))
            ),
        ),
    };
    vec![
        format!(
            "{ancestors} AS (SELECT DISTINCT r.{hk} AS {leaf}, {catalog}.{parent_column} AS {node}, 1 AS {steps} FROM {rows} r JOIN {table} AS {catalog} ON {catalog}.{id} = r.{hk} WHERE {catalog}.{parent_column} <> {empty_reference} UNION ALL SELECT h.{leaf}, {catalog}.{parent_column}, h.{steps} + 1 FROM {ancestors} h JOIN {table} AS {catalog} ON {catalog}.{id} = h.{node} WHERE {catalog}.{parent_column} <> {empty_reference})"
        ),
        format!(
            "{depths} AS (SELECT {leaf}, MAX({steps}) AS {depth} FROM {ancestors} GROUP BY {leaf})"
        ),
        format!(
            "{nodes} AS (SELECT x.{node}, MIN(x.{rn}) AS {rank}, MIN(x.{depth}) AS {depth}, MAX(x.{hier}) AS {hier}, {catalog}.{parent_column} AS {parent} FROM (SELECT r.{hk} AS {node}, r.{rn} AS {rn}, COALESCE(d.{depth}, 0) AS {depth}, 0 AS {hier} FROM {rows} r LEFT JOIN {depths} d ON d.{leaf} = r.{hk} UNION ALL SELECT h.{node}, r.{rn}, d.{depth} - h.{steps}, 1 FROM {ancestors} h JOIN {rows} r ON r.{hk} = h.{leaf} JOIN {depths} d ON d.{leaf} = h.{leaf}) x LEFT JOIN {table} AS {catalog} ON {catalog}.{id} = x.{node} GROUP BY x.{node}, {catalog}.{parent_column})"
        ),
        format!(
            "{paths} AS (SELECT n.{node}, {root_path} AS {path} FROM {nodes} n WHERE NOT EXISTS (SELECT 1 FROM {nodes} p WHERE p.{node} = n.{parent}) UNION ALL SELECT c.{node}, {child_path} FROM {paths} p JOIN {nodes} c ON c.{parent} = p.{node})"
        ),
    ]
}

/// The names a result column answers to: its label, the projection alias,
/// and the field name of an unaliased field projection.
fn column_names(ast: &QueryAst<'_, '_>, columns: &[CompiledColumn]) -> Vec<Vec<String>> {
    let projection = &ast.branches[0].projection;
    columns
        .iter()
        .enumerate()
        .map(|(position, column)| {
            let mut names = vec![column.label.clone()];
            if projection.len() == columns.len()
                && let Some(item) = projection.get(position)
            {
                if let Some(alias) = item.alias {
                    names.push(alias.lexeme.to_owned());
                }
                if let Projection::Field(reference) = &item.expression {
                    names.push(reference.last().lexeme.to_owned());
                }
            }
            names
        })
        .collect()
}

fn resolve_column(names: &[Vec<String>], token: &Token<'_>) -> Option<usize> {
    names.iter().position(|candidates| {
        candidates
            .iter()
            .any(|name| names_equal(name, token.lexeme))
    })
}

/// Resolves the control points to result column positions, validates
/// their modifiers, and describes the hierarchical point when present.
fn resolve_points(
    totals: &TotalsAst<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(Vec<usize>, Option<Hierarchy>), QueryDiagnostic> {
    let mut points = Vec::with_capacity(totals.points.len());
    let mut hierarchy: Option<Hierarchy> = None;
    for point in &totals.points {
        let position = resolve_point(point, columns, names, parameters)?;
        if let Some(modifier) = &point.hierarchy {
            if hierarchy.is_some() {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(modifier.token),
                    "only one hierarchical control point is supported",
                ));
            }
            // `ПО Товар, Товар ИЕРАРХИЯ` is the hierarchy alone on the platform.
            if points.last() == Some(&position) {
                points.pop();
            }
            let (table, id, parent) =
                hierarchical_catalog(&columns[position], point, snapshot, dialect)?;
            hierarchy = Some(Hierarchy {
                index: points.len(),
                only: modifier.only,
                table,
                id,
                parent,
            });
        }
        points.push(position);
    }
    Ok((points, hierarchy))
}

/// The quoted live table, `_IDRRef`, and `_ParentIDRRef` of the catalog a
/// hierarchical control point references.
fn hierarchical_catalog(
    column: &CompiledColumn,
    point: &ControlPoint<'_, '_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(String, String, String), QueryDiagnostic> {
    let token = point.field.last();
    let target = match &column.kind {
        ColumnKind::Reference {
            targets,
            runtime_typed: false,
        } if targets.len() == 1 => Some(targets[0]),
        _ => None,
    };
    let physical = target
        .and_then(|target| snapshot.object_by_id(target))
        .and_then(|object| object.physical_table.as_deref());
    let live = physical.and_then(|table| snapshot.live_table(table));
    let find = |live: &LiveTable, name: &str| {
        live.columns
            .iter()
            .find(|column| names_equal(&column.name, name))
            .map(|column| dialect.quote_identifier(&column.name))
    };
    let (Some(physical), Some(live)) = (physical, live) else {
        return Err(hierarchy_diagnostic(token));
    };
    let (Some(id), Some(parent)) = (find(live, "_IDRRef"), find(live, "_ParentIDRRef")) else {
        return Err(hierarchy_diagnostic(token));
    };
    // Records of a catalog extended with data live in the extension tables
    // too, so the lookup reads the same `UNION ALL` a source does.
    let mut branches = snapshot
        .extension_live_tables(physical)
        .filter_map(|table| {
            let (table_id, table_parent) = (find(table, "_IDRRef")?, find(table, "_ParentIDRRef")?);
            Some(format!(
                "SELECT {table_id} AS {id}, {table_parent} AS {parent} FROM {}",
                dialect.quote_identifier(&table.name)
            ))
        })
        .collect::<Vec<_>>();
    let table = if branches.is_empty() {
        dialect.quote_identifier(&live.name)
    } else {
        branches.insert(
            0,
            format!(
                "SELECT {id} AS {id}, {parent} AS {parent} FROM {}",
                dialect.quote_identifier(&live.name)
            ),
        );
        format!("({})", branches.join(" UNION ALL "))
    };
    Ok((table, id, parent))
}

fn hierarchy_diagnostic(token: &Token<'_>) -> QueryDiagnostic {
    QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(token),
        format!(
            "hierarchy totals need a control point {:?} that references one hierarchical catalog",
            token.lexeme
        ),
    )
}

fn resolve_point(
    point: &ControlPoint<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
) -> Result<usize, QueryDiagnostic> {
    let token = point.field.last();
    let position = match point.field.segments.as_slice() {
        [single] => resolve_column(names, single),
        _ => None,
    }
    .ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "control point {:?} must name a result column",
                point
                    .field
                    .segments
                    .iter()
                    .map(|segment| segment.lexeme)
                    .collect::<Vec<_>>()
                    .join(".")
            ),
        )
    })?;
    if let Some(periods) = &point.periods {
        if columns[position].kind != ColumnKind::DateTime {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(periods.token),
                format!(
                    "PERIODS control point {:?} must be a date column",
                    token.lexeme
                ),
            ));
        }
        for bound in [&periods.begin, &periods.end].into_iter().flatten() {
            let valid = match bound {
                Expression::DateTime { .. } => true,
                Expression::Parameter(parameter) => matches!(
                    parameters.lookup(parameter)?,
                    None | Some(ParameterValue::Date(_))
                ),
                _ => false,
            };
            if !valid {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(periods.token),
                    format!(
                        "PERIODS({}) bounds must be DATETIME literals or date parameters",
                        periods.period.display_name()
                    ),
                ));
            }
        }
    }
    Ok(position)
}

/// Compiles the totals fields into one aggregate expression per targeted
/// result column; a later field naming the same column replaces the
/// earlier one, as on the platform.
fn resolve_fields(
    totals: &TotalsAst<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<Vec<Option<String>>, QueryDiagnostic> {
    let mut aggregates = vec![None; columns.len()];
    for field in &totals.fields {
        let (sql, kind) = compile_field(&field.expression, columns, names, parameters, dialect)?;
        let target = field_target(field, &kind, names)?;
        let sql = match (&kind, &columns[target].kind) {
            (FieldKind::Column(source), target_kind)
                if *source == target || columns[*source].kind.is_compatible_with(target_kind) =>
            {
                sql
            }
            (FieldKind::Number, ColumnKind::Number { .. } | ColumnKind::Unknown { .. }) => sql,
            (FieldKind::Number, ColumnKind::String { .. }) => dialect.scalar_text(&sql),
            _ => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(field.token),
                    format!(
                        "totals field cannot be written into the {:?} result column of another kind",
                        columns[target].label
                    ),
                ));
            }
        };
        aggregates[target] = Some(sql);
    }
    Ok(aggregates)
}

/// The result column a totals field writes: its alias, or the argument
/// column of a bare aggregate.
fn field_target(
    field: &TotalsField<'_, '_>,
    kind: &FieldKind,
    names: &[Vec<String>],
) -> Result<usize, QueryDiagnostic> {
    if let Some(alias) = field.alias {
        return resolve_column(names, alias).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(alias),
                format!(
                    "totals field alias {:?} names no result column",
                    alias.lexeme
                ),
            )
        });
    }
    if let Expression::Aggregate {
        argument: AggregateArgument::Expression(argument),
        ..
    } = &field.expression
        && let Expression::Field(reference) = argument.as_ref()
        && let [single] = reference.segments.as_slice()
        && let Some(position) = resolve_column(names, single)
    {
        return Ok(position);
    }
    if let FieldKind::Column(position) = kind {
        return Ok(*position);
    }
    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::Syntax,
        Some(field.token),
        "cannot determine the result column of the totals field; add AS <column>",
    ))
}

/// Compiles a totals field over the CTE columns: aggregates of result
/// columns, arithmetic, numeric literals, and parameters.
fn compile_field(
    expression: &Expression<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<(String, FieldKind), QueryDiagnostic> {
    match expression {
        Expression::Aggregate {
            token,
            kind,
            distinct,
            argument,
        } => {
            let (argument_sql, position) = match argument {
                AggregateArgument::All => ("*".to_owned(), None),
                AggregateArgument::Expression(argument) => {
                    let Expression::Field(reference) = argument.as_ref() else {
                        return Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::UnsupportedFeature,
                            Some(token),
                            "totals aggregate argument must be a result column",
                        ));
                    };
                    let position = match reference.segments.as_slice() {
                        [single] => resolve_column(names, single),
                        _ => None,
                    }
                    .ok_or_else(|| {
                        QueryDiagnostic::at(
                            QueryDiagnosticKind::UnknownField,
                            Some(reference.last()),
                            format!(
                                "totals aggregate argument {:?} names no result column",
                                reference.last().lexeme
                            ),
                        )
                    })?;
                    (
                        dialect.quote_identifier(&columns[position].label),
                        Some(position),
                    )
                }
            };
            let sql = format!(
                "{}({}{argument_sql})",
                kind.sql_name(),
                if *distinct { "DISTINCT " } else { "" }
            );
            let field_kind = match (kind, position) {
                (AggregateKind::Min | AggregateKind::Max, Some(position)) => {
                    FieldKind::Column(position)
                }
                _ => FieldKind::Number,
            };
            Ok((sql, field_kind))
        }
        Expression::Binary {
            left,
            operator,
            right,
        } if operator.kind == TokenKind::Operator
            && matches!(operator.lexeme, "+" | "-" | "*" | "/") =>
        {
            let (left, _) = compile_field(left, columns, names, parameters, dialect)?;
            let (right, _) = compile_field(right, columns, names, parameters, dialect)?;
            Ok((
                format!("({left} {} {right})", operator.lexeme),
                FieldKind::Number,
            ))
        }
        Expression::Unary { operator, value }
            if operator.kind != TokenKind::Keyword(Keyword::Not) =>
        {
            let (value, _) = compile_field(value, columns, names, parameters, dialect)?;
            Ok((format!("({}{value})", operator.lexeme), FieldKind::Number))
        }
        Expression::Literal(token) if token.kind == TokenKind::Number => {
            Ok((compile_literal(token, dialect)?, FieldKind::Number))
        }
        Expression::Parameter(token) => Ok((
            match parameters.lookup(token)? {
                Some(value) => render_scalar_parameter(value, token, dialect, false)?,
                None => "NULL".to_owned(),
            },
            FieldKind::Number,
        )),
        other => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            super::expression::operand_token(other),
            "totals fields support aggregates of result columns, arithmetic, numeric literals, and parameters",
        )),
    }
}
