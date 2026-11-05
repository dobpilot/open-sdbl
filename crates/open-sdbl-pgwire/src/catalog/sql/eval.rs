//! Relational evaluation over immutable virtual tables, under one budget.
use super::*;
use super::{func, plan, regex};
use std::collections::BTreeMap;

fn indexable(oid: u32) -> bool {
    matches!(
        oid,
        16 | 18 | 19 | 20 | 21 | 23 | 24 | 25 | 26 | 28 | 1043 | 2205 | 2206 | 4089
    )
}

fn index_key(value: &Value, oid: u32) -> Result<Option<Value>, ()> {
    match (value, oid) {
        (Value::Null, _) => Ok(None),
        (Value::Int(n) | Value::Reg(n, _), 20 | 21 | 23 | 24 | 26 | 28 | 2205 | 2206 | 4089) => {
            Ok(Some(Value::Int(*n)))
        }
        (Value::Text(s), 18 | 19 | 25 | 1043) => Ok(Some(Value::Text(s.clone()))),
        (Value::Bool(b), 16) => Ok(Some(Value::Bool(*b))),
        _ => Err(()),
    }
}

/// Necessary equalities, including OR alternatives sharing one left key.
fn join_key(expr: &Expr, columns: &[Column], width: usize) -> Option<(usize, Vec<usize>)> {
    let ExprKind::Binary(op, a, b) = &expr.kind else {
        return None;
    };
    if op == "and" {
        return join_key(a, columns, width).or_else(|| join_key(b, columns, width));
    }
    if op == "or" {
        let (left_a, mut right_a) = join_key(a, columns, width)?;
        let (left_b, right_b) = join_key(b, columns, width)?;
        if left_a != left_b {
            return None;
        }
        right_a.extend(right_b);
        right_a.sort_unstable();
        right_a.dedup();
        return Some((left_a, right_a));
    }
    if op != "=" {
        return None;
    }
    let (ExprKind::Column(a), ExprKind::Column(b)) = (&a.kind, &b.kind) else {
        return None;
    };
    let a = plan::resolve(a, columns, expr.position).ok()?;
    let b = plan::resolve(b, columns, expr.position).ok()?;
    if a.oid != b.oid || !indexable(a.oid) {
        return None;
    }
    let a = columns.iter().position(|c| std::ptr::eq(c, a))?;
    let b = columns.iter().position(|c| std::ptr::eq(c, b))?;
    if a < width && b >= width {
        Some((a, vec![b]))
    } else if b < width && a >= width {
        Some((b, vec![a]))
    } else {
        None
    }
}

// Find a necessary numeric equality on one specific resolved column.
fn constrained_number(expr: &Expr, columns: &[Column], target: usize) -> Option<i64> {
    let ExprKind::Binary(op, a, b) = &expr.kind else {
        return None;
    };
    if op == "and" {
        return constrained_number(a, columns, target)
            .or_else(|| constrained_number(b, columns, target));
    }
    if op != "=" {
        return None;
    }
    for (column, value) in [(a.as_ref(), b.as_ref()), (b.as_ref(), a.as_ref())] {
        if let (ExprKind::Column(name), ExprKind::Number(number)) = (&column.kind, &value.kind) {
            let Ok(resolved) = plan::resolve(name, columns, column.position) else {
                continue;
            };
            if std::ptr::eq(resolved, &columns[target]) {
                return number.parse::<i64>().ok();
            }
        }
    }
    None
}

pub(super) fn evaluate(
    q: &Query,
    catalog: &Catalog,
    params: &[Value],
    parameter_types: &[u32],
) -> Result<Table, ErrorFields> {
    evaluate_bounded(q, catalog, params, parameter_types, 50_000_000)
}
pub(super) fn evaluate_bounded(
    q: &Query,
    catalog: &Catalog,
    params: &[Value],
    parameter_types: &[u32],
    steps: usize,
) -> Result<Table, ErrorFields> {
    let (_, parameter_types) = plan::describe(q, catalog, parameter_types)?;
    let mut ctx = Context {
        catalog,
        params,
        parameter_types,
        budget: Budget::new(),
    };
    ctx.budget.steps = steps;
    ctx.query(q, &[], &[], &BTreeMap::new())
}
struct Context<'a> {
    catalog: &'a Catalog,
    params: &'a [Value],
    parameter_types: Vec<u32>,
    budget: Budget,
}
impl Context<'_> {
    fn query(
        &mut self,
        q: &Query,
        outer_cols: &[Column],
        outer: &[Value],
        ctes: &BTreeMap<String, Table>,
    ) -> Result<Table, ErrorFields> {
        self.budget.step()?;
        let mut ctes = ctes.clone();
        for (n, q) in &q.ctes {
            let t = self.query(q, outer_cols, outer, &ctes)?;
            ctes.insert(n.clone(), t);
        }
        let columns = plan::query(
            q,
            self.catalog,
            outer_cols,
            &ctes,
            &mut self.parameter_types,
        )?;
        let mut retain_context = false;
        for (key, _, _) in &q.order {
            walk_expr(key, &mut |e| match &e.kind {
                ExprKind::Column(n) => {
                    retain_context |= plan::resolve(n, &columns, e.position).is_err()
                }
                ExprKind::Query(_) | ExprKind::Exists(_) | ExprKind::ArrayQuery(_) => {
                    retain_context = true
                }
                _ => {}
            });
        }
        let mut records: Vec<(Vec<Value>, Vec<Value>)> = Vec::new();
        let mut context_columns = outer_cols.to_vec();
        match &q.body {
            Body::Values(rows) => {
                for row in rows {
                    let values = row
                        .iter()
                        .map(|e| self.expr(e, outer_cols, outer, None, &ctes))
                        .collect::<Result<Vec<_>, _>>()?;
                    records.push((values, outer.to_vec()));
                }
            }
            Body::Select(s) => {
                // An empty independent INNER source makes the entire join empty.
                // Planning above still validates every name/expression. Avoid
                // repeatedly copying pg_description for empty UNION inventories.
                let empty = s.from.iter().any(|j| {
                    !j.left
                        && match &j.source.kind {
                            SourceKind::Relation(n) => plan::relation(n, self.catalog, &ctes)
                                .is_ok_and(|t| t.rows.is_empty()),
                            _ => false,
                        }
                });
                let mut input = Table {
                    columns: vec![],
                    rows: if empty { vec![] } else { vec![vec![]] },
                };
                // Retain only referenced source columns in intermediate rows.
                // Stars, natural joins and correlated subqueries keep full shapes.
                let mut references = Vec::new();
                let mut full_width = s
                    .from
                    .iter()
                    .any(|j| j.natural || matches!(j.source.kind, SourceKind::Query(_)));
                full_width |= s
                    .columns
                    .iter()
                    .any(|(e, _)| matches!(e.kind, ExprKind::Star(_)));
                walk_query(q, &mut |e| match &e.kind {
                    ExprKind::Column(n) => references.push(n.clone()),
                    ExprKind::Query(_) | ExprKind::Exists(_) | ExprKind::ArrayQuery(_) => {
                        full_width = true
                    }
                    _ => {}
                });
                for (join_index, join) in s.from.iter().enumerate() {
                    let cs = scope(&input.columns, outer_cols);
                    let schema = plan::source(
                        &join.source,
                        self.catalog,
                        &cs,
                        &ctes,
                        &mut self.parameter_types,
                    )?;
                    let projection = schema
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| {
                            full_width
                                || matches!(join.source.kind, SourceKind::Function(_))
                                || references.iter().any(|n| {
                                    n.last() == Some(&c.name)
                                        && (n.len() == 1 || n[n.len() - 2] == c.qualifier)
                                })
                                || s.from.iter().any(|j| j.using.contains(&c.name))
                        })
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    // Push an OID constant through the first inner equijoin. This
                    // avoids retaining an entire wide first relation before the
                    // WHERE predicate on the second relation becomes available.
                    let first_restriction = if outer_cols.is_empty()
                        && join_index == 0
                        && s.from.len() >= 2
                        && !s.from[1].left
                        && !s.from[1].natural
                        && s.from[1].using.is_empty()
                        && matches!(join.source.kind, SourceKind::Relation(_))
                        && matches!(s.from[1].source.kind, SourceKind::Relation(_))
                    {
                        let mut combined = schema.clone();
                        let right = plan::source(
                            &s.from[1].source,
                            self.catalog,
                            &schema,
                            &ctes,
                            &mut self.parameter_types,
                        )?;
                        let width = combined.len();
                        combined.extend(right);
                        s.from[1]
                            .on
                            .as_ref()
                            .and_then(|on| join_key(on, &combined, width))
                            .and_then(|(left, right)| {
                                if right.len() != 1 || combined[left].oid != 26 {
                                    return None;
                                }
                                let value =
                                    constrained_number(s.filter.as_ref()?, &combined, right[0])?;
                                Some((left, value))
                            })
                    } else {
                        None
                    };
                    let schema = projection.iter().map(|i| schema[*i].clone()).collect();
                    let (joined_columns, using_pairs) = plan::join_columns(
                        input.columns.clone(),
                        schema,
                        &join.using,
                        join.natural,
                    )?;
                    let ctx_cols = scope(&joined_columns, outer_cols);
                    let mut rows = vec![];
                    // Independent sources are evaluated once; set-returning
                    // functions may reference the preceding joined row.
                    let independent = match &join.source.kind {
                        SourceKind::Relation(n) => {
                            let table = plan::relation(n, self.catalog, &ctes)?;
                            let mut projected = Vec::new();
                            for row in &table.rows {
                                if let Some((column, value)) = first_restriction {
                                    self.budget.step()?;
                                    let retain = match row.get(column) {
                                        Some(Value::Int(n) | Value::Reg(n, _)) => *n == value,
                                        Some(Value::Null) => false,
                                        // Preserve unusual values for the regular evaluator.
                                        _ => true,
                                    };
                                    if !retain {
                                        continue;
                                    }
                                }
                                projected
                                    .push(projection.iter().map(|i| row[*i].clone()).collect());
                            }
                            Some(Table {
                                columns: vec![],
                                rows: projected,
                            })
                        }
                        SourceKind::Query(q) => Some(self.query(q, outer_cols, outer, &ctes)?),
                        SourceKind::Function(e) => match &e.kind {
                            ExprKind::Call(n, _) => func::table_function(n.last().unwrap()),
                            _ => None,
                        },
                    };
                    let width = input.columns.len();
                    let key = using_pairs
                        .iter()
                        .copied()
                        .find(|(a, _)| indexable(joined_columns[*a].oid))
                        .map(|(a, b)| (a, vec![b]))
                        .or_else(|| {
                            join.on
                                .as_ref()
                                .and_then(|e| join_key(e, &joined_columns, width))
                        })
                        .or_else(|| {
                            (!join.left)
                                .then(|| {
                                    s.filter
                                        .as_ref()
                                        .and_then(|e| join_key(e, &joined_columns, width))
                                })
                                .flatten()
                        });
                    let mut index = None;
                    if let (Some((_, keys)), Some(table)) = (&key, &independent)
                        && !input.rows.is_empty()
                    {
                        let mut entries = BTreeMap::<Value, Vec<usize>>::new();
                        let mut valid = true;
                        for (i, row) in table.rows.iter().enumerate() {
                            self.budget.step()?;
                            for b in keys {
                                let Ok(value) = index_key(&row[b - width], joined_columns[*b].oid)
                                else {
                                    valid = false;
                                    break;
                                };
                                if let Some(value) = value {
                                    if !entries.contains_key(&value) {
                                        self.budget.allocate(std::slice::from_ref(&value))?;
                                        self.budget.allocate_bytes(
                                            std::mem::size_of::<Vec<usize>>()
                                                + 3 * std::mem::size_of::<usize>(),
                                        )?;
                                    }
                                    self.budget
                                        .allocate_bytes(2 * std::mem::size_of::<usize>())?;
                                    let bucket = entries.entry(value).or_default();
                                    if bucket.last() != Some(&i) {
                                        bucket.push(i);
                                    }
                                }
                            }
                        }
                        if valid {
                            index = Some(entries);
                        }
                    }
                    for row in &input.rows {
                        let source = if let Some(t) = &independent {
                            std::borrow::Cow::Borrowed(t)
                        } else {
                            let mut context = row.clone();
                            context.extend_from_slice(outer);
                            let SourceKind::Function(e) = &join.source.kind else {
                                unreachable!()
                            };
                            let value = self.expr(e, &cs, &context, None, &ctes)?;
                            let vs = match value {
                                Value::Array(v) => v,
                                Value::Null => vec![],
                                v => vec![v],
                            };
                            std::borrow::Cow::Owned(Table {
                                columns: vec![],
                                rows: vs.into_iter().map(|v| vec![v]).collect(),
                            })
                        };
                        let mut matched = false;
                        let candidates: Box<dyn Iterator<Item = &Vec<Value>> + '_> =
                            if let (Some((a, _)), Some(entries)) = (&key, &index) {
                                match index_key(&row[*a], joined_columns[*a].oid) {
                                    Ok(Some(value)) => Box::new(
                                        entries
                                            .get(&value)
                                            .into_iter()
                                            .flatten()
                                            .map(|i| &source.rows[*i]),
                                    ),
                                    Ok(None) => Box::new(std::iter::empty()),
                                    Err(()) => Box::new(source.rows.iter()),
                                }
                            } else {
                                Box::new(source.rows.iter())
                            };
                        for right in candidates {
                            self.budget.step()?;
                            let mut both = row.clone();
                            both.extend_from_slice(right);
                            let mut context = both.clone();
                            context.extend_from_slice(outer);
                            let mut valid = true;
                            for &(a, b) in &using_pairs {
                                valid &= both[a] != Value::Null
                                    && both[b] != Value::Null
                                    && both[a].cmp_sql(&both[b])?.is_eq();
                            }
                            if valid && let Some(e) = &join.on {
                                valid = self.expr(e, &ctx_cols, &context, None, &ctes)?.truth()?
                                    == Some(true);
                            }
                            if valid {
                                matched = true;
                                if let Some(filter) = &s.filter
                                    && !self.early_filter(filter, &ctx_cols, &context, &ctes)?
                                {
                                    continue;
                                }
                                self.budget.rows(rows.len() + 1)?;
                                self.budget.allocate(&both)?;
                                rows.push(both);
                            }
                        }
                        if join.left && !matched {
                            let mut both = row.clone();
                            both.resize(joined_columns.len(), Value::Null);
                            let mut context = both.clone();
                            context.extend_from_slice(outer);
                            if let Some(filter) = &s.filter
                                && !self.early_filter(filter, &ctx_cols, &context, &ctes)?
                            {
                                continue;
                            }
                            self.budget.rows(rows.len() + 1)?;
                            self.budget.allocate(&both)?;
                            rows.push(both);
                        }
                    }
                    input = Table {
                        columns: joined_columns,
                        rows,
                    };
                }
                context_columns = scope(&input.columns, outer_cols);
                let mut filtered = vec![];
                for mut row in input.rows {
                    self.budget.step()?;
                    row.extend_from_slice(outer);
                    if let Some(e) = &s.filter
                        && self.expr(e, &context_columns, &row, None, &ctes)?.truth()? != Some(true)
                    {
                        continue;
                    }
                    filtered.push(row);
                }
                let aggregate = s.columns.iter().any(|(e, _)| has_aggregate(e))
                    || s.having.as_ref().is_some_and(has_aggregate)
                    || !s.group.is_empty();
                let groups = if aggregate {
                    let mut groups: BTreeMap<Vec<Value>, Vec<Vec<Value>>> = BTreeMap::new();
                    if s.group.is_empty() {
                        groups.insert(vec![], vec![]);
                    }
                    for row in filtered {
                        let keys = s
                            .group
                            .iter()
                            .map(|e| self.expr(e, &context_columns, &row, None, &ctes))
                            .collect::<Result<Vec<_>, _>>()?;
                        groups.entry(keys).or_default().push(row);
                    }
                    groups.into_values().collect::<Vec<_>>()
                } else {
                    filtered.into_iter().map(|r| vec![r]).collect()
                };
                for group in groups {
                    if s.columns.iter().any(|(e,_)|matches!(&e.kind,ExprKind::Call(n,_) if n.last().is_some_and(|n|n=="pg_partition_ancestors"))) {continue;}
                    let fallback = vec![Value::Null; context_columns.len()];
                    let row = group.first().unwrap_or(&fallback);
                    let group_opt = aggregate.then_some(group.as_slice());
                    if let Some(e) = &s.having
                        && self
                            .expr(e, &context_columns, row, group_opt, &ctes)?
                            .truth()?
                            != Some(true)
                    {
                        continue;
                    }
                    let mut output = vec![];
                    for (e, _) in &s.columns {
                        if let ExprKind::Star(qualifier) = &e.kind {
                            for i in plan::star_indices(&input.columns, qualifier.as_deref()) {
                                output.push(row[i].clone());
                            }
                        } else {
                            output.push(self.expr(e, &context_columns, row, group_opt, &ctes)?);
                        }
                    }
                    let outputs = if s.columns.len() == 1
                        && matches!(&s.columns[0].0.kind, ExprKind::Call(n, _) if n.last().is_some_and(|n| n == "unnest"))
                    {
                        match output.pop().unwrap() {
                            Value::Null => vec![],
                            Value::Array(values) => values.into_iter().map(|v| vec![v]).collect(),
                            _ => {
                                return Err(ErrorFields::error(
                                    "42804",
                                    "unnest requires an array",
                                ));
                            }
                        }
                    } else {
                        vec![output]
                    };
                    for output in outputs {
                        self.budget.rows(records.len() + 1)?;
                        self.budget.allocate(&output)?;
                        let source = if retain_context {
                            self.budget.allocate(row)?;
                            row.clone()
                        } else {
                            vec![]
                        };
                        records.push((output, source));
                    }
                }
                if s.distinct {
                    let mut seen = std::collections::BTreeSet::new();
                    records.retain(|(r, _)| seen.insert(r.clone()));
                }
            }
        }
        if let Some((all, right)) = &q.union {
            let result = self.query(right, outer_cols, outer, &ctes)?;
            records.extend(result.rows.into_iter().map(|r| (r, outer.to_vec())));
            self.budget.rows(records.len())?;
            if !all {
                let mut seen = std::collections::BTreeSet::new();
                records.retain(|(r, _)| seen.insert(r.clone()));
            }
        }
        if !q.order.is_empty() {
            self.sort(&mut records, &q.order, &columns, &context_columns, &ctes)?;
        }
        let offset = if let Some(e) = &q.offset {
            usize::try_from(self.expr(e, outer_cols, outer, None, &ctes)?.int()?)
                .map_err(|_| ErrorFields::error("2201X", "negative OFFSET"))?
        } else {
            0
        };
        let limit = if let Some(e) = &q.limit {
            usize::try_from(self.expr(e, outer_cols, outer, None, &ctes)?.int()?)
                .map_err(|_| ErrorFields::error("2201W", "negative LIMIT"))?
        } else {
            usize::MAX
        };
        Ok(Table {
            columns,
            rows: records
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|(r, _)| r)
                .collect(),
        })
    }
    fn early_filter(
        &mut self,
        filter: &Expr,
        columns: &[Column],
        row: &[Value],
        ctes: &BTreeMap<String, Table>,
    ) -> Result<bool, ErrorFields> {
        if let ExprKind::Binary(op, a, b) = &filter.kind
            && op == "and"
        {
            return Ok(self.early_filter(a, columns, row, ctes)?
                && self.early_filter(b, columns, row, ctes)?);
        }
        let mut local = true;
        walk_expr(filter, &mut |expr| match &expr.kind {
            ExprKind::Column(name) => local &= plan::resolve(name, columns, expr.position).is_ok(),
            ExprKind::Query(_)
            | ExprKind::Exists(_)
            | ExprKind::ArrayQuery(_)
            | ExprKind::OrderedAggregate(_, _) => local = false,
            _ => {}
        });
        if local {
            Ok(self.expr(filter, columns, row, None, ctes)?.truth()? == Some(true))
        } else {
            Ok(true)
        }
    }
    fn sort(
        &mut self,
        records: &mut Vec<(Vec<Value>, Vec<Value>)>,
        order: &[(Expr, bool, bool)],
        columns: &[Column],
        context: &[Column],
        ctes: &BTreeMap<String, Table>,
    ) -> Result<(), ErrorFields> {
        let cs = scope(columns, context);
        let mut keyed = vec![];
        for (row, source) in records.drain(..) {
            let mut values = row.clone();
            values.extend(source.clone());
            let mut key = vec![];
            for (e, _, _) in order {
                let v = if let ExprKind::Number(n) = &e.kind {
                    let idx = n
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .filter(|i| *i < row.len())
                        .ok_or_else(|| {
                            ErrorFields::error("42P10", "ORDER BY position is not in select list")
                        })?;
                    row[idx].clone()
                } else {
                    self.expr(e, &cs, &values, None, ctes)?
                };
                key.push(v);
            }
            keyed.push((key, row, source));
        }
        // Comparator work is charged separately so sorting cannot escape
        // the statement's budget.
        let estimated = keyed
            .len()
            .saturating_mul(keyed.len().max(1).ilog2() as usize + 1)
            .saturating_mul(order.len());
        if estimated > self.budget.steps {
            return Err(ErrorFields::error(
                "54001",
                "catalog sort work budget exceeded",
            ));
        }
        self.budget.steps -= estimated;
        let mut failure = None;
        keyed.sort_by(|(a, _, _), (b, _, _)| {
            for (i, (_, desc, nulls_first)) in order.iter().enumerate() {
                let cmp = match (&a[i], &b[i]) {
                    (Value::Null, Value::Null) => Ordering::Equal,
                    (Value::Null, _) => {
                        if *nulls_first {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    }
                    (_, Value::Null) => {
                        if *nulls_first {
                            Ordering::Greater
                        } else {
                            Ordering::Less
                        }
                    }
                    (a, b) => match a.cmp_sql(b) {
                        Ok(o) => {
                            if *desc {
                                o.reverse()
                            } else {
                                o
                            }
                        }
                        Err(e) => {
                            failure = Some(e);
                            Ordering::Equal
                        }
                    },
                };
                if !cmp.is_eq() {
                    return cmp;
                }
            }
            Ordering::Equal
        });
        if let Some(e) = failure {
            return Err(e);
        }
        records.extend(keyed.into_iter().map(|(_, r, s)| (r, s)));
        Ok(())
    }
    fn expr(
        &mut self,
        e: &Expr,
        columns: &[Column],
        row: &[Value],
        group: Option<&[Vec<Value>]>,
        ctes: &BTreeMap<String, Table>,
    ) -> Result<Value, ErrorFields> {
        self.budget.step()?;
        Ok(match &e.kind {
            ExprKind::Null => Value::Null,
            ExprKind::Bool(v) => Value::Bool(*v),
            ExprKind::String(v) => Value::Text(v.clone()),
            ExprKind::Number(v) => {
                if v.contains('.') {
                    Value::Decimal(Decimal::parse(v)?)
                } else {
                    match v.parse::<i64>() {
                        Ok(n) => Value::Int(n),
                        Err(_) => Value::Decimal(Decimal::parse(v)?),
                    }
                }
            }
            ExprKind::Parameter(n) => self.params.get(n - 1).cloned().unwrap_or(Value::Null),
            ExprKind::Column(n) => {
                let c = plan::resolve(n, columns, e.position)?;
                let index = columns
                    .iter()
                    .position(|known| std::ptr::eq(known, c))
                    .unwrap();
                row.get(index).cloned().unwrap_or(Value::Null)
            }
            ExprKind::Star(_) => {
                return Err(ErrorFields::error(
                    "42601",
                    "star outside projection or count",
                ));
            }
            ExprKind::Unary(op, a) => {
                let v = self.expr(a, columns, row, group, ctes)?;
                if v == Value::Null {
                    Value::Null
                } else if op == "not" {
                    Value::Bool(!v.truth()?.unwrap())
                } else if op == "-" {
                    if let Value::Decimal(n) = v {
                        return Ok(Value::Decimal(n.negative()));
                    }
                    Value::Int(
                        v.int()?
                            .checked_neg()
                            .ok_or_else(|| ErrorFields::error("22003", "integer out of range"))?,
                    )
                } else {
                    v
                }
            }
            ExprKind::Binary(op, a, b) => {
                let a = self.expr(a, columns, row, group, ctes)?;
                if op == "and" && a == Value::Bool(false) {
                    return Ok(a);
                }
                if op == "or" && a == Value::Bool(true) {
                    return Ok(a);
                }
                if let ExprKind::Call(name, args) = &b.kind
                    && ["any", "all"].contains(&name.last().unwrap().as_str())
                {
                    let value = self.expr(&args[0], columns, row, group, ctes)?;
                    let Value::Array(vs) = value else {
                        return Ok(Value::Null);
                    };
                    let all = name.last().unwrap() == "all";
                    let mut null = false;
                    for b in vs {
                        let r = self.binary(op, a.clone(), b)?;
                        if r == Value::Null {
                            null = true;
                        } else if r == Value::Bool(!all) {
                            return Ok(r);
                        }
                    }
                    if null { Value::Null } else { Value::Bool(all) }
                } else {
                    let b = self.expr(b, columns, row, group, ctes)?;
                    self.binary(op, a, b)?
                }
            }
            ExprKind::Cast(a, n) => {
                let v = self.expr(a, columns, row, group, ctes)?;
                func::cast(v, func::type_oid(n.last().unwrap()).unwrap(), self.catalog)?
            }
            ExprKind::Is(a, what, not) => {
                let v = self.expr(a, columns, row, group, ctes)?;
                Value::Bool(
                    (match what.as_str() {
                        "null" | "unknown" => v == Value::Null,
                        "true" => v == Value::Bool(true),
                        "false" => v == Value::Bool(false),
                        _ => unreachable!(),
                    }) != *not,
                )
            }
            ExprKind::Case(base, branches, other) => {
                let base = base
                    .as_ref()
                    .map(|e| self.expr(e, columns, row, group, ctes))
                    .transpose()?;
                let mut chosen = None;
                for (a, b) in branches {
                    let test = self.expr(a, columns, row, group, ctes)?;
                    let yes = if let Some(base) = &base {
                        *base != Value::Null && test != Value::Null && base.cmp_sql(&test)?.is_eq()
                    } else {
                        test.truth()? == Some(true)
                    };
                    if yes {
                        chosen = Some(b);
                        break;
                    }
                }
                if let Some(e) = chosen.or(other.as_deref()) {
                    self.expr(e, columns, row, group, ctes)?
                } else {
                    Value::Null
                }
            }
            ExprKind::In(a, es, not) => {
                let a = self.expr(a, columns, row, group, ctes)?;
                let mut null = a == Value::Null;
                let mut matched = false;
                for e in es {
                    let values = if let ExprKind::Query(q) = &e.kind {
                        self.query(q, columns, row, ctes)?
                            .rows
                            .into_iter()
                            .map(|r| r[0].clone())
                            .collect()
                    } else {
                        vec![self.expr(e, columns, row, group, ctes)?]
                    };
                    for b in values {
                        if b == Value::Null {
                            null = true;
                        } else if a != Value::Null && a.cmp_sql(&b)?.is_eq() {
                            matched = true;
                        }
                    }
                }
                if matched {
                    Value::Bool(!not)
                } else if null {
                    Value::Null
                } else {
                    Value::Bool(*not)
                }
            }
            ExprKind::Exists(q) => Value::Bool(!self.query(q, columns, row, ctes)?.rows.is_empty()),
            ExprKind::Query(q) => {
                let t = self.query(q, columns, row, ctes)?;
                if t.columns.len() != 1 {
                    return Err(ErrorFields::error(
                        "42601",
                        "subquery must return one column",
                    ));
                }
                if t.rows.len() > 1 {
                    return Err(ErrorFields::error(
                        "21000",
                        "scalar subquery returned more than one row",
                    ));
                }
                t.rows.first().map_or(Value::Null, |r| r[0].clone())
            }
            ExprKind::ArrayQuery(q) => Value::Array(
                self.query(q, columns, row, ctes)?
                    .rows
                    .into_iter()
                    .map(|r| r[0].clone())
                    .collect(),
            ),
            ExprKind::Array(es) => {
                let ty = plan::expr(e, self.catalog, columns, ctes, &mut self.parameter_types)?;
                let element = func::element_type(ty).unwrap();
                Value::Array(
                    es.iter()
                        .map(|e| {
                            let v = self.expr(e, columns, row, group, ctes)?;
                            func::cast(v, element, self.catalog)
                        })
                        .collect::<Result<_, _>>()?,
                )
            }
            ExprKind::Subscript(a, b) => {
                let a = self.expr(a, columns, row, group, ctes)?;
                let b = self.expr(b, columns, row, group, ctes)?;
                if b == Value::Null {
                    return Ok(Value::Null);
                }
                if let Value::Array(vs) = a {
                    usize::try_from(b.int()?)
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .and_then(|i| vs.get(i).cloned())
                        .unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            ExprKind::OrderedAggregate(call, order) => {
                let group = group
                    .ok_or_else(|| ErrorFields::error("42803", "aggregate outside a group"))?;
                let mut records = Vec::new();
                for source in group {
                    let mut keys = Vec::new();
                    for (key, _, _) in order {
                        keys.push(self.expr(key, columns, source, None, ctes)?);
                    }
                    self.budget.allocate(&keys)?;
                    self.budget.allocate(source)?;
                    records.push((keys, source.clone()));
                }
                let key_columns = order
                    .iter()
                    .enumerate()
                    .map(|(i, _)| Column::new(&format!("key{i}"), 25))
                    .collect::<Vec<_>>();
                let key_order = order
                    .iter()
                    .enumerate()
                    .map(|(i, (_, desc, nulls))| {
                        (
                            Expr {
                                kind: ExprKind::Number((i + 1).to_string()),
                                position: call.position,
                            },
                            *desc,
                            *nulls,
                        )
                    })
                    .collect::<Vec<_>>();
                self.sort(&mut records, &key_order, &key_columns, columns, ctes)?;
                let sorted = records.into_iter().map(|(_, row)| row).collect::<Vec<_>>();
                self.expr(call, columns, row, Some(&sorted), ctes)?
            }
            ExprKind::Call(n, es) => {
                let name = n.last().unwrap();
                if ["count", "min", "max", "sum", "string_agg", "array_agg"]
                    .contains(&name.as_str())
                {
                    let group = group
                        .ok_or_else(|| ErrorFields::error("42803", "aggregate outside a group"))?;
                    let mut values = vec![];
                    for r in group {
                        let v = if matches!(es[0].kind, ExprKind::Star(_)) {
                            Value::Bool(true)
                        } else {
                            self.expr(&es[0], columns, r, None, ctes)?
                        };
                        if name == "array_agg" {
                            self.budget.allocate(std::slice::from_ref(&v))?;
                        }
                        if v != Value::Null || name == "array_agg" {
                            values.push(v);
                        }
                    }
                    match name.as_str() {
                        "count" => Value::Int(values.len() as i64),
                        "array_agg" => {
                            if values.is_empty() {
                                Value::Null
                            } else {
                                Value::Array(values)
                            }
                        }
                        "min" | "max" => {
                            let mut best = Value::Null;
                            for v in values {
                                if best == Value::Null
                                    || v.cmp_sql(&best)?.is_lt() == (name == "min")
                                {
                                    best = v;
                                }
                            }
                            best
                        }
                        "sum" => {
                            if values.is_empty() {
                                Value::Null
                            } else {
                                let mut sum = 0_i64;
                                for v in values {
                                    sum = sum.checked_add(v.int()?).ok_or_else(|| {
                                        ErrorFields::error("22003", "integer sum out of range")
                                    })?;
                                }
                                Value::Int(sum)
                            }
                        }
                        _ => {
                            let separator = self.expr(&es[1], columns, row, None, ctes)?.text();
                            if values.is_empty() {
                                Value::Null
                            } else {
                                let strings = values.iter().map(Value::text).collect::<Vec<_>>();
                                let size = strings
                                    .iter()
                                    .map(String::len)
                                    .fold(0, usize::saturating_add)
                                    .saturating_add(
                                        separator
                                            .len()
                                            .saturating_mul(strings.len().saturating_sub(1)),
                                    );
                                if size > self.budget.bytes {
                                    return Err(ErrorFields::error(
                                        "54001",
                                        "catalog string allocation budget exceeded",
                                    ));
                                }
                                strings.join(&separator).into()
                            }
                        }
                    }
                } else if name == "coalesce" {
                    let mut value = Value::Null;
                    for e in es {
                        value = self.expr(e, columns, row, group, ctes)?;
                        if value != Value::Null {
                            break;
                        }
                    }
                    value
                } else {
                    let a = es
                        .iter()
                        .map(|e| {
                            let value = self.expr(e, columns, row, group, ctes)?;
                            if name == "concat" {
                                // Bound the retained arguments too: repeated column
                                // references clone strings before concat is called.
                                self.budget.allocate(std::slice::from_ref(&value))?;
                            }
                            Ok(value)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    func::call(name, a, self.catalog, &mut self.budget)?
                }
            }
        })
    }
    fn binary(&mut self, op: &str, a: Value, b: Value) -> Result<Value, ErrorFields> {
        if op == "and" || op == "or" {
            let (a, b) = (a.truth()?, b.truth()?);
            return Ok(match (op, a, b) {
                ("and", Some(false), _) | ("and", _, Some(false)) => Value::Bool(false),
                ("or", Some(true), _) | ("or", _, Some(true)) => Value::Bool(true),
                (_, Some(a), Some(b)) => Value::Bool(if op == "and" { a && b } else { a || b }),
                _ => Value::Null,
            });
        }
        if op == "is distinct" || op == "is not distinct" {
            let eq = if a == Value::Null || b == Value::Null {
                a == b
            } else {
                a.cmp_sql(&b)?.is_eq()
            };
            return Ok(Value::Bool(eq == (op == "is not distinct")));
        }
        if a == Value::Null || b == Value::Null {
            return Ok(Value::Null);
        }
        Ok(match op {
            "=" | "<>" | "!=" | "<" | ">" | "<=" | ">=" => {
                let c = a.cmp_sql(&b)?;
                Value::Bool(match op {
                    "=" => c.is_eq(),
                    "<>" | "!=" => !c.is_eq(),
                    "<" => c.is_lt(),
                    ">" => c.is_gt(),
                    "<=" => !c.is_gt(),
                    _ => !c.is_lt(),
                })
            }
            "~" | "~*" | "!~" | "!~*" => Value::Bool(
                regex::matches(&b.text(), &a.text(), op.ends_with('*'), &mut self.budget)?
                    != op.starts_with('!'),
            ),
            "like" | "ilike" => Value::Bool(regex::like(
                &b.text(),
                &a.text(),
                op == "ilike",
                &mut self.budget,
            )?),
            "||" => match (a, b) {
                (Value::Array(mut a), Value::Array(b)) => {
                    a.extend(b);
                    Value::Array(a)
                }
                (a, b) => {
                    let (a, b) = (a.text(), b.text());
                    if a.len().saturating_add(b.len()) > self.budget.bytes {
                        return Err(ErrorFields::error(
                            "54001",
                            "catalog string allocation budget exceeded",
                        ));
                    }
                    format!("{a}{b}").into()
                }
            },
            "+" | "-" | "*" | "/" | "%" => {
                if matches!(a, Value::Decimal(_)) || matches!(b, Value::Decimal(_)) {
                    return Err(ErrorFields::error(
                        "0A000",
                        "decimal arithmetic is not in the catalog SQL subset",
                    ));
                }
                let (a, b) = (a.int()?, b.int()?);
                if (op == "/" || op == "%") && b == 0 {
                    return Err(ErrorFields::error("22012", "division by zero"));
                }
                Value::Int(
                    match op {
                        "+" => a.checked_add(b),
                        "-" => a.checked_sub(b),
                        "*" => a.checked_mul(b),
                        "/" => a.checked_div(b),
                        _ => a.checked_rem(b),
                    }
                    .ok_or_else(|| ErrorFields::error("22003", "integer out of range"))?,
                )
            }
            _ => {
                return Err(ErrorFields::error(
                    "0A000",
                    format!("unsupported catalog operator {op}"),
                ));
            }
        })
    }
}
fn has_aggregate(e: &Expr) -> bool {
    let mut found = false;
    walk_expr(e, &mut |e| {
        if let ExprKind::Call(n, _) = &e.kind {
            found |= ["count", "min", "max", "sum", "string_agg", "array_agg"]
                .contains(&n.last().unwrap().as_str());
        }
    });
    found
}
