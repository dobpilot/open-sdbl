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

/// The kind a totals field expression produces.
enum FieldKind {
    /// A count, sum, average, or arithmetic: a number.
    Number,
    /// `МИНИМУМ`/`МАКСИМУМ` of the result column at this position.
    Column(usize),
}

/// Wraps a compiled statement into the totals rows.
pub(super) fn wrap_totals(
    ast: &QueryAst<'_, '_>,
    totals: &TotalsAst<'_, '_>,
    compiled: CompiledQuery,
    order: &[OrderKey],
    level_column: bool,
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let names = column_names(ast, &compiled.columns);
    let points = resolve_points(totals, &compiled.columns, &names, parameters)?;
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
    let rn = quote(ROW_NUMBER);
    let level = quote(LEVEL);
    let keys = if order.is_empty() {
        "(SELECT 1)".to_owned()
    } else {
        order
            .iter()
            .map(|key| {
                format!(
                    "{}{}",
                    key.position
                        .map_or_else(|| key.sql.clone(), |position| labels[position - 1].clone()),
                    if key.descending { " DESC" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // PostgreSQL 1C string columns may still be `mvarchar` after functions
    // such as `ЕСТЬNULL`; the CTE normalises them to `text` so that the
    // typed NULL placeholders and text-cast counts unite with them.
    let cte_columns = compiled
        .columns
        .iter()
        .zip(&labels)
        .map(|(column, label)| {
            if dialect == SqlDialect::Postgres && matches!(column.kind, ColumnKind::String { .. }) {
                format!("{label}::text AS {label}")
            } else {
                label.clone()
            }
        })
        .collect::<Vec<_>>();
    let cte = format!(
        "{} AS (SELECT {}, ROW_NUMBER() OVER (ORDER BY {keys}) AS {rn} FROM ({}) AS {})",
        quote(ROWS),
        cte_columns.join(", "),
        compiled.sql,
        quote(SOURCE)
    );

    let overall_offset = usize::from(overall);
    let detail_level = points.len() + overall_offset;
    let group_column = |index: usize| quote(&format!("__g{}", index + 1));
    // Groups of every level are ordered by the first appearance of their
    // own value in the ordered result, independently of the enclosing
    // group, as the platform does.
    let rank_partition = |index: usize| labels[points[index]].clone();
    let group_keys = |depth: usize| {
        points[..depth]
            .iter()
            .map(|point| labels[*point].clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
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
    let mut branches = Vec::with_capacity(points.len() + 2);
    if overall {
        let mut projection = (0..labels.len()).map(cell).collect::<Vec<_>>();
        projection.push(format!("0 AS {level}"));
        projection.extend((0..points.len()).map(|index| format!("0 AS {}", group_column(index))));
        projection.push(format!("0 AS {rn}"));
        branches.push(format!(
            "SELECT {} FROM {} HAVING COUNT(*) > 0",
            projection.join(", "),
            quote(ROWS)
        ));
    }
    for depth in 1..=points.len() {
        let mut projection = (0..labels.len())
            .map(|position| {
                if points[..depth].contains(&position) {
                    labels[position].clone()
                } else {
                    cell(position)
                }
            })
            .collect::<Vec<_>>();
        projection.push(format!("{} AS {level}", depth - 1 + overall_offset));
        for index in 0..points.len() {
            let group = if index < depth {
                format!(
                    "MIN(MIN({rn})) OVER (PARTITION BY {})",
                    rank_partition(index)
                )
            } else {
                "0".to_owned()
            };
            projection.push(format!("{group} AS {}", group_column(index)));
        }
        projection.push(format!("0 AS {rn}"));
        branches.push(format!(
            "SELECT {} FROM {} GROUP BY {}",
            projection.join(", "),
            quote(ROWS),
            group_keys(depth)
        ));
    }
    let mut projection = labels.clone();
    projection.push(format!("{detail_level} AS {level}"));
    for index in 0..points.len() {
        projection.push(format!(
            "MIN({rn}) OVER (PARTITION BY {}) AS {}",
            rank_partition(index),
            group_column(index)
        ));
    }
    projection.push(rn.clone());
    branches.push(format!(
        "SELECT {} FROM {}",
        projection.join(", "),
        quote(ROWS)
    ));

    let mut ordering = Vec::with_capacity(points.len() * 2 + 1);
    for index in 0..points.len() {
        ordering.push(group_column(index));
        ordering.push(format!(
            "CASE WHEN {level} <= {} THEN 0 ELSE 1 END",
            index + overall_offset
        ));
    }
    ordering.push(rn);
    let mut output = labels.clone();
    if level_column {
        output.push(level);
    }
    let sql = format!(
        "WITH {cte} SELECT {} FROM ({}) AS {} ORDER BY {}",
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

/// Resolves the control points to result column positions and validates
/// their modifiers.
fn resolve_points(
    totals: &TotalsAst<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
) -> Result<Vec<usize>, QueryDiagnostic> {
    totals
        .points
        .iter()
        .map(|point| resolve_point(point, columns, names, parameters))
        .collect()
}

fn resolve_point(
    point: &ControlPoint<'_, '_>,
    columns: &[CompiledColumn],
    names: &[Vec<String>],
    parameters: Parameters<'_>,
) -> Result<usize, QueryDiagnostic> {
    if let Some(hierarchy) = &point.hierarchy {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(hierarchy.token),
            if hierarchy.only {
                "ONLY HIERARCHY totals are not supported yet"
            } else {
                "HIERARCHY totals are not supported yet"
            },
        ));
    }
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
