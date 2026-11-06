//! Name resolution and result/parameter typing, including empty sources.
use super::func;
use super::lex::error;
use super::*;
use std::collections::BTreeMap;

pub(super) fn star_indices(columns: &[Column], qualifier: Option<&str>) -> Vec<usize> {
    let mut indices = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.system && qualifier.map_or(!c.using_hidden, |q| q == c.qualifier))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if qualifier.is_none() {
        indices.sort_by_key(|i| columns[*i].star_order);
    }
    indices
}

type UsingPairs = Vec<(usize, usize)>;
pub(super) fn join_columns(
    mut left: Vec<Column>,
    right: Vec<Column>,
    using: &[String],
    natural: bool,
) -> Result<(Vec<Column>, UsingPairs), ErrorFields> {
    let natural_keys = if natural {
        star_indices(&left, None)
            .into_iter()
            .filter(|i| {
                right
                    .iter()
                    .any(|c| !c.system && !c.using_hidden && c.name == left[*i].name)
            })
            .map(|i| left[i].name.clone())
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    let using = if natural { &natural_keys } else { using };
    let width = left.len();
    let mut order = Vec::new();
    let mut pairs = Vec::new();
    for name in using {
        if using.iter().filter(|n| *n == name).count() > 1 {
            return Err(ErrorFields::error(
                "42701",
                format!("duplicate USING column {name}"),
            ));
        }
        let find = |columns: &[Column]| -> Result<usize, ErrorFields> {
            let found = columns
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.system && !c.using_hidden && c.name == *name)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            match found.as_slice() {
                [i] => Ok(*i),
                [] => Err(ErrorFields::error(
                    "42703",
                    format!("USING column {name} does not exist"),
                )),
                _ => Err(ErrorFields::error(
                    "42702",
                    format!("USING column {name} is ambiguous"),
                )),
            }
        };
        let a = find(&left)?;
        let b = find(&right)?;
        if left[a].oid != right[b].oid {
            return Err(ErrorFields::error(
                "0A000",
                "USING keys with different types are unsupported",
            ));
        }
        pairs.push((a, width + b));
        order.push(a);
    }
    order.extend(
        star_indices(&left, None)
            .into_iter()
            .filter(|i| !pairs.iter().any(|(a, _)| a == i)),
    );
    order.extend(
        star_indices(&right, None)
            .into_iter()
            .map(|i| width + i)
            .filter(|i| !pairs.iter().any(|(_, b)| b == i)),
    );
    left.extend(right);
    for (_, b) in &pairs {
        left[*b].using_hidden = true;
    }
    for (rank, i) in order.into_iter().enumerate() {
        left[i].star_order = rank;
    }
    Ok((left, pairs))
}

pub(super) fn describe(
    q: &Query,
    catalog: &Catalog,
    declared: &[u32],
) -> Result<(Vec<Column>, Vec<u32>), ErrorFields> {
    let count = parameter_count(q);
    if declared.len() > count {
        return Err(ErrorFields::error(
            "08P01",
            "too many declared catalog parameter types",
        ));
    }
    let mut parameters = declared.to_vec();
    parameters.resize(count, 0);
    query(q, catalog, &[], &BTreeMap::new(), &mut parameters)?;
    for t in &mut parameters {
        if *t == 0 {
            *t = 25;
        }
    }
    // A later predicate may infer the type of an earlier projected parameter.
    let columns = query(q, catalog, &[], &BTreeMap::new(), &mut parameters)?;
    Ok((columns, parameters))
}
pub(super) fn relation<'a>(
    name: &[String],
    catalog: &'a Catalog,
    ctes: &'a BTreeMap<String, Table>,
) -> Result<&'a Table, ErrorFields> {
    if name.len() == 1 && ctes.contains_key(&name[0]) {
        return Ok(&ctes[&name[0]]);
    }
    let key = if name.len() == 1 {
        format!("pg_catalog.{}", name[0])
    } else {
        name.join(".")
    };
    catalog
        .tables
        .get(&key)
        .map(|table| &**table)
        .ok_or_else(|| {
            ErrorFields::error(
                "42P01",
                format!("catalog relation {} does not exist", name.join(".")),
            )
        })
}
pub(super) fn source(
    s: &Source,
    catalog: &Catalog,
    outer: &[Column],
    ctes: &BTreeMap<String, Table>,
    params: &mut Vec<u32>,
) -> Result<Vec<Column>, ErrorFields> {
    let (mut columns, name) = match &s.kind {
        SourceKind::Relation(n) => (
            relation(n, catalog, ctes)?.columns.clone(),
            n.last().unwrap().clone(),
        ),
        SourceKind::Query(q) => (query(q, catalog, outer, ctes, params)?, String::new()),
        SourceKind::Function(e) => {
            let t = expr(e, catalog, outer, ctes, params)?;
            let ExprKind::Call(n, _) = &e.kind else {
                unreachable!()
            };
            (
                if let Some(table) = func::table_function(n.last().unwrap()) {
                    table.columns
                } else {
                    vec![Column::new(
                        n.last().unwrap(),
                        func::element_type(t).unwrap_or(t),
                    )]
                },
                n.last().unwrap().clone(),
            )
        }
    };
    if s.columns.len() > columns.iter().filter(|c| !c.system).count() {
        return Err(ErrorFields::error("42601", "too many column aliases"));
    }
    let single_column = columns.len() == 1;
    let mut alias_index = 0;
    for c in &mut columns {
        c.qualifier = s.alias.as_ref().unwrap_or(&name).clone();
        c.scope = 0;
        if c.system {
            continue;
        }
        if let Some(n) = s.columns.get(alias_index) {
            c.name.clone_from(n);
        } else if single_column
            && matches!(s.kind, SourceKind::Function(_))
            && let Some(alias) = &s.alias
        {
            c.name.clone_from(alias);
        }
        alias_index += 1;
    }
    Ok(columns)
}
pub(super) fn query(
    q: &Query,
    catalog: &Catalog,
    outer: &[Column],
    ctes: &BTreeMap<String, Table>,
    params: &mut Vec<u32>,
) -> Result<Vec<Column>, ErrorFields> {
    let mut ctes = ctes.clone();
    for (n, q) in &q.ctes {
        let columns = query(q, catalog, outer, &ctes, params)?;
        ctes.insert(
            n.clone(),
            Table {
                columns,
                rows: vec![],
            },
        );
    }
    let columns = match &q.body {
        Body::Values(rows) => {
            let Some(first) = rows.first() else {
                return Err(ErrorFields::error("42601", "VALUES needs a row"));
            };
            let mut columns = Vec::new();
            for (i, e) in first.iter().enumerate() {
                columns.push(Column::new(
                    &format!("column{}", i + 1),
                    expr(e, catalog, outer, &ctes, params)?,
                ));
            }
            for row in rows.iter().skip(1) {
                if row.len() != columns.len() {
                    return Err(ErrorFields::error("42601", "VALUES rows differ in length"));
                }
                for e in row {
                    expr(e, catalog, outer, &ctes, params)?;
                }
            }
            columns
        }
        Body::Select(s) => {
            let mut local = Vec::new();
            for j in &s.from {
                let context = scope(&local, outer);
                let cs = source(&j.source, catalog, &context, &ctes, params)?;
                local = join_columns(local, cs, &j.using, j.natural)?.0;
                let context = scope(&local, outer);
                if let Some(e) = &j.on {
                    expr(e, catalog, &context, &ctes, params)?;
                }
            }
            let context = scope(&local, outer);
            let mut columns = Vec::new();
            for (e, alias) in &s.columns {
                let mut set_returning = 0;
                walk_expr(e, &mut |e| {
                    if let ExprKind::Call(n, _) = &e.kind {
                        set_returning += usize::from(
                            [
                                "unnest",
                                "generate_series",
                                "_pg_expandarray",
                                "pg_available_extension_versions",
                            ]
                            .contains(&n.last().unwrap().as_str()),
                        );
                    }
                });
                let projected_unnest = set_returning == 1
                    && s.columns.len() == 1
                    && s.group.is_empty()
                    && s.having.is_none()
                    && matches!(&e.kind, ExprKind::Call(n, _) if n.last().is_some_and(|n| n == "unnest"));
                if set_returning > 0 && !projected_unnest {
                    return Err(error(
                        "0A000",
                        "set-returning functions require FROM",
                        e.position,
                    ));
                }
                if let ExprKind::Star(q) = &e.kind {
                    if q.as_ref()
                        .is_some_and(|q| !local.iter().any(|c| &c.qualifier == q))
                    {
                        return Err(error("42P01", "unknown table qualifier", e.position));
                    }
                    for i in star_indices(&local, q.as_deref()) {
                        let mut column = local[i].clone();
                        column.using_hidden = false;
                        column.star_order = 0;
                        columns.push(column);
                    }
                } else {
                    let oid = expr(e, catalog, &context, &ctes, params)?;
                    let oid = if projected_unnest {
                        func::element_type(oid)
                            .ok_or_else(|| error("42804", "unnest requires an array", e.position))?
                    } else {
                        oid
                    };
                    let name = alias.clone().unwrap_or_else(|| label(e));
                    let mut c = Column::new(&name, oid);
                    if let ExprKind::Column(n) = &e.kind {
                        c.modifier = resolve(n, &context, e.position)?.modifier;
                    }
                    columns.push(c);
                }
            }
            for e in s.filter.iter().chain(&s.group).chain(&s.having) {
                expr(e, catalog, &context, &ctes, params)?;
            }
            let aggregate = s.columns.iter().any(|(e, _)| aggregate(e))
                || s.having.as_ref().is_some_and(aggregate)
                || !s.group.is_empty();
            if aggregate {
                let mut grouped = Vec::new();
                for e in &s.group {
                    match &e.kind {
                        ExprKind::Column(n) => {
                            let c = resolve(n, &context, e.position)?;
                            grouped.push((c.qualifier.clone(), c.name.clone()));
                        }
                        _ => {
                            return Err(error(
                                "0A000",
                                "only column GROUP BY keys are supported",
                                e.position,
                            ));
                        }
                    }
                }
                for e in s.columns.iter().map(|(e, _)| e).chain(s.having.iter()) {
                    grouped_expr(e, &context, &grouped, false)?;
                }
            }
            let order_context = scope(&columns, &context);
            for (e, _, _) in &q.order {
                if !matches!(e.kind, ExprKind::Number(_)) {
                    expr(e, catalog, &order_context, &ctes, params)?;
                }
            }
            columns
        }
    };
    if let Some((_, right)) = &q.union {
        let other = query(right, catalog, outer, &ctes, params)?;
        if other.len() != columns.len() {
            return Err(ErrorFields::error("42601", "UNION column counts differ"));
        }
    }
    for e in q.limit.iter().chain(&q.offset) {
        expr(e, catalog, outer, &ctes, params)?;
    }
    Ok(columns)
}
pub(super) fn label(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Column(n) | ExprKind::Call(n, _) => n.last().unwrap().clone(),
        ExprKind::Cast(e, _) => label(e),
        _ => "?column?".into(),
    }
}
pub(super) fn resolve<'a>(
    n: &[String],
    columns: &'a [Column],
    pos: usize,
) -> Result<&'a Column, ErrorFields> {
    let last = n.last().unwrap();
    let found = columns
        .iter()
        .filter(|c| {
            &c.name == last
                && (if n.len() == 1 {
                    !c.using_hidden
                } else {
                    n.get(n.len() - 2) == Some(&c.qualifier)
                })
        })
        .collect::<Vec<_>>();
    let nearest = found.iter().map(|c| c.scope).min().ok_or_else(|| {
        error(
            "42703",
            format!("column {} does not exist", n.join(".")),
            pos,
        )
    })?;
    let mut found = found.into_iter().filter(|c| c.scope == nearest);
    let first = found.next().unwrap();
    if found.next().is_some() {
        return Err(error(
            "42702",
            format!("column {} is ambiguous", n.join(".")),
            pos,
        ));
    }
    Ok(first)
}
pub(super) fn expr(
    e: &Expr,
    catalog: &Catalog,
    columns: &[Column],
    ctes: &BTreeMap<String, Table>,
    params: &mut Vec<u32>,
) -> Result<u32, ErrorFields> {
    Ok(match &e.kind {
        ExprKind::Null => 25,
        ExprKind::Bool(_) => 16,
        ExprKind::Number(n) => {
            if let Ok(value) = n.parse::<i64>() {
                if i32::try_from(value).is_ok() { 23 } else { 20 }
            } else {
                Decimal::parse(n)?;
                1700
            }
        }
        ExprKind::String(_) => 25,
        ExprKind::Star(_) => 25,
        ExprKind::Column(n) => resolve(n, columns, e.position)?.oid,
        ExprKind::Parameter(n) => {
            params.resize(params.len().max(*n), 0);
            if params[n - 1] == 0 {
                25
            } else {
                params[n - 1]
            }
        }
        ExprKind::OrderedAggregate(call, order) => {
            let result = expr(call, catalog, columns, ctes, params)?;
            for (key, _, _) in order {
                expr(key, catalog, columns, ctes, params)?;
            }
            result
        }
        ExprKind::Call(n, es) => {
            let name = n.last().unwrap();
            if name == "array_agg" && es.iter().any(|arg| matches!(arg.kind, ExprKind::Star(_))) {
                return Err(error(
                    "42601",
                    "array_agg requires an expression",
                    e.position,
                ));
            }
            if n.len() > 1 && !(["pg_catalog", "information_schema"].contains(&n[0].as_str()))
                || !func::known(name)
            {
                return Err(error(
                    "42883",
                    format!("function {} does not exist", n.join(".")),
                    e.position,
                ));
            }
            let types = es
                .iter()
                .map(|e| expr(e, catalog, columns, ctes, params))
                .collect::<Result<Vec<_>, _>>()?;
            if ["coalesce", "nullif", "greatest"].contains(&name.as_str()) {
                func::result_type(name, &types)?;
                let mut common = None;
                for (arg, t) in es.iter().zip(&types) {
                    if !matches!(arg.kind, ExprKind::Null) {
                        common = Some(common_type(common, *t, e.position)?);
                    }
                }
                return Ok(common.unwrap_or(25));
            }
            func::result_type(name, &types).map_err(|mut err| {
                err.position = Some(e.position);
                err
            })?
        }
        ExprKind::Unary(op, a) => {
            let t = expr(a, catalog, columns, ctes, params)?;
            if op == "not" { 16 } else { t }
        }
        ExprKind::Binary(op, a, b) => {
            let ta = expr(a, catalog, columns, ctes, params)?;
            let tb = expr(b, catalog, columns, ctes, params)?;
            if let ExprKind::Parameter(n) = a.kind {
                if params[n - 1] == 0 {
                    params[n - 1] = tb;
                }
            }
            if let ExprKind::Parameter(n) = b.kind {
                if params[n - 1] == 0 {
                    params[n - 1] = ta;
                }
            }
            if ["||", "+", "-", "*", "/", "%"].contains(&op.as_str()) {
                ta
            } else {
                16
            }
        }
        ExprKind::Cast(a, n) => {
            let ty = func::type_oid(n.last().unwrap()).ok_or_else(|| {
                error(
                    "0A000",
                    format!("unsupported catalog type {}", n.join(".")),
                    e.position,
                )
            })?;
            expr(a, catalog, columns, ctes, params)?;
            if let ExprKind::Parameter(n) = a.kind
                && params[n - 1] == 0
            {
                params[n - 1] = ty;
            }
            ty
        }
        ExprKind::Case(base, branches, other) => {
            if let Some(base) = base {
                expr(base, catalog, columns, ctes, params)?;
            }
            let mut ty = 25;
            let mut branches_type = None;
            for (a, b) in branches {
                expr(a, catalog, columns, ctes, params)?;
                let t = expr(b, catalog, columns, ctes, params)?;
                if !matches!(b.kind, ExprKind::Null) {
                    ty = common_type(branches_type, t, e.position)?;
                    branches_type = Some(ty);
                }
            }
            if let Some(e) = other {
                let t = expr(e, catalog, columns, ctes, params)?;
                if !matches!(e.kind, ExprKind::Null) {
                    ty = common_type(branches_type, t, e.position)?;
                }
            }
            ty
        }
        ExprKind::Is(a, _, _) => {
            expr(a, catalog, columns, ctes, params)?;
            16
        }
        ExprKind::In(a, es, _) => {
            expr(a, catalog, columns, ctes, params)?;
            for e in es {
                expr(e, catalog, columns, ctes, params)?;
            }
            16
        }
        ExprKind::Query(q) => query(q, catalog, columns, ctes, params)?
            .first()
            .map_or(25, |c| c.oid),
        ExprKind::Exists(q) => {
            query(q, catalog, columns, ctes, params)?;
            16
        }
        ExprKind::Array(es) => {
            let mut t = 25;
            for e in es {
                let element = expr(e, catalog, columns, ctes, params)?;
                if !matches!(e.kind, ExprKind::Null) {
                    t = element;
                }
            }
            func::array_type(t)
                .ok_or_else(|| error("0A000", "unsupported array type", e.position))?
        }
        ExprKind::ArrayQuery(q) => {
            let t = query(q, catalog, columns, ctes, params)?
                .first()
                .map_or(25, |c| c.oid);
            func::array_type(t).unwrap_or(1009)
        }
        ExprKind::Subscript(a, b) => {
            let t = expr(a, catalog, columns, ctes, params)?;
            expr(b, catalog, columns, ctes, params)?;
            func::element_type(t)
                .ok_or_else(|| error("42804", "subscript requires an array", e.position))?
        }
    })
}

fn aggregate(e: &Expr) -> bool {
    let mut yes = false;
    walk_expr(e, &mut |e| {
        if let ExprKind::Call(n, _) = &e.kind {
            yes |= ["count", "min", "max", "sum", "string_agg", "array_agg"]
                .contains(&n.last().unwrap().as_str());
        }
    });
    yes
}
fn grouped_expr(
    e: &Expr,
    columns: &[Column],
    keys: &[(String, String)],
    inside: bool,
) -> Result<(), ErrorFields> {
    match &e.kind {
        ExprKind::Column(n) if !inside => {
            let c = resolve(n, columns, e.position)?;
            if !keys.contains(&(c.qualifier.clone(), c.name.clone())) {
                return Err(error(
                    "42803",
                    format!("column {} must appear in GROUP BY", n.join(".")),
                    e.position,
                ));
            }
        }
        ExprKind::Star(_) if !inside => {
            return Err(error(
                "0A000",
                "star in grouped projection is unsupported",
                e.position,
            ));
        }
        ExprKind::OrderedAggregate(call, order) => {
            grouped_expr(call, columns, keys, inside)?;
            for (key, _, _) in order {
                grouped_expr(key, columns, keys, true)?;
                if aggregate(key) {
                    return Err(error(
                        "42803",
                        "aggregate calls cannot be nested",
                        key.position,
                    ));
                }
            }
        }
        ExprKind::Call(n, es) => {
            let agg = ["count", "min", "max", "sum", "string_agg", "array_agg"]
                .contains(&n.last().unwrap().as_str());
            if agg && inside {
                return Err(error(
                    "42803",
                    "aggregate calls cannot be nested",
                    e.position,
                ));
            }
            for e in es {
                grouped_expr(e, columns, keys, inside || agg)?;
            }
        }
        ExprKind::Unary(_, a) | ExprKind::Cast(a, _) | ExprKind::Is(a, _, _) => {
            grouped_expr(a, columns, keys, inside)?
        }
        ExprKind::Binary(_, a, b) | ExprKind::Subscript(a, b) => {
            grouped_expr(a, columns, keys, inside)?;
            grouped_expr(b, columns, keys, inside)?;
        }
        ExprKind::Case(base, branches, other) => {
            if let Some(e) = base {
                grouped_expr(e, columns, keys, inside)?;
            }
            for (a, b) in branches {
                grouped_expr(a, columns, keys, inside)?;
                grouped_expr(b, columns, keys, inside)?;
            }
            if let Some(e) = other {
                grouped_expr(e, columns, keys, inside)?;
            }
        }
        ExprKind::Array(es) => {
            for e in es {
                grouped_expr(e, columns, keys, inside)?;
            }
        }
        ExprKind::In(a, es, _) => {
            grouped_expr(a, columns, keys, inside)?;
            for e in es {
                grouped_expr(e, columns, keys, inside)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn common_type(previous: Option<u32>, next: u32, position: usize) -> Result<u32, ErrorFields> {
    let Some(previous) = previous else {
        return Ok(next);
    };
    if previous == next {
        return Ok(next);
    }
    if (previous == 26 && next == 23) || (previous == 23 && next == 26) {
        return Ok(26);
    }
    if [18, 19, 25, 1043].contains(&previous) && [18, 19, 25, 1043].contains(&next) {
        return Ok(25);
    }
    if [20, 21, 23, 1700].contains(&previous) && [20, 21, 23, 1700].contains(&next) {
        return Ok(if previous == 1700 || next == 1700 {
            1700
        } else if previous == 20 || next == 20 {
            20
        } else {
            23
        });
    }
    Err(error(
        "0A000",
        "mixed CASE result types are outside the catalog SQL subset",
        position,
    ))
}
