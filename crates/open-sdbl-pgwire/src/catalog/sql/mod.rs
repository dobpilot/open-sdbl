//! In-memory SQL for the virtual catalog. No database handles enter here.
mod decimal;
mod eval;
use decimal::Decimal;
mod func;
mod lex;
mod parse;
mod plan;
pub(super) mod regex;

use super::Catalog;
use crate::codec::{ErrorFields, FieldDescription};
pub(crate) use parse::Query;
use parse::*;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    Reg(i64, String),
    Text(String),
    Array(Vec<Value>),
}
impl Value {
    pub(super) fn text(&self) -> String {
        let mut output = String::new();
        self.write_text(&mut output).expect("writing to a String");
        output
    }
    /// Count UTF-8 bytes without constructing intermediate strings, including
    /// array quoting. Concatenation checks this size before reserving memory.
    fn text_len(&self, limit: usize) -> Result<usize, ErrorFields> {
        struct Counter {
            remaining: usize,
        }
        impl std::fmt::Write for Counter {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                self.remaining = self.remaining.checked_sub(s.len()).ok_or(std::fmt::Error)?;
                Ok(())
            }
        }
        let mut counter = Counter { remaining: limit };
        self.write_text(&mut counter).map_err(|_| {
            ErrorFields::error("54001", "catalog string allocation budget exceeded")
        })?;
        Ok(limit - counter.remaining)
    }
    fn write_text(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match self {
            Self::Null => Ok(()),
            Self::Bool(v) => out.write_str(if *v { "t" } else { "f" }),
            Self::Int(v) => write!(out, "{v}"),
            Self::Decimal(v) => out.write_str(&v.text()),
            Self::Reg(_, name) | Self::Text(name) => out.write_str(name),
            Self::Array(vs) => {
                struct Escaped<'a>(&'a mut dyn std::fmt::Write);
                impl std::fmt::Write for Escaped<'_> {
                    fn write_str(&mut self, s: &str) -> std::fmt::Result {
                        for c in s.chars() {
                            if c == '\\' || c == '"' {
                                self.0.write_char('\\')?;
                            }
                            self.0.write_char(c)?;
                        }
                        Ok(())
                    }
                }
                out.write_char('{')?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        out.write_char(',')?;
                    }
                    if *v == Self::Null {
                        out.write_str("NULL")?;
                    } else {
                        out.write_char('"')?;
                        v.write_text(&mut Escaped(out))?;
                        out.write_char('"')?;
                    }
                }
                out.write_char('}')
            }
        }
    }
    pub(super) fn int(&self) -> Result<i64, ErrorFields> {
        match self {
            Self::Int(n) | Self::Reg(n, _) => Ok(*n),
            Self::Text(s) => s
                .parse()
                .map_err(|_| ErrorFields::error("22P02", "invalid integer")),
            _ => Err(ErrorFields::error("42804", "expected an integer")),
        }
    }
    pub(super) fn truth(&self) -> Result<Option<bool>, ErrorFields> {
        match self {
            Self::Null => Ok(None),
            Self::Bool(v) => Ok(Some(*v)),
            _ => Err(ErrorFields::error("42804", "expected a boolean")),
        }
    }
    pub(super) fn cmp_sql(&self, other: &Self) -> Result<Ordering, ErrorFields> {
        match (self, other) {
            (Self::Reg(n, _), b) => Self::Int(*n).cmp_sql(&match b {
                Self::Reg(n, _) => Self::Int(*n),
                b => b.clone(),
            }),
            (a, Self::Reg(n, _)) => a.cmp_sql(&Self::Int(*n)),
            (Self::Decimal(a), b) => Ok(a.compare(&Decimal::parse(&b.text())?)),
            (a, Self::Decimal(b)) => Ok(Decimal::parse(&a.text())?.compare(b)),
            (Self::Int(a), Self::Int(b)) => Ok(a.cmp(b)),
            (Self::Int(a), Self::Text(b)) => Ok(a.cmp(
                &b.parse()
                    .map_err(|_| ErrorFields::error("22P02", "invalid integer"))?,
            )),
            (Self::Text(a), Self::Int(b)) => Ok(a
                .parse::<i64>()
                .map_err(|_| ErrorFields::error("22P02", "invalid integer"))?
                .cmp(b)),
            (Self::Text(a), Self::Text(b)) => Ok(a.cmp(b)),
            (Self::Bool(a), Self::Bool(b)) => Ok(a.cmp(b)),
            (Self::Array(a), Self::Array(b)) => Ok(a.cmp(b)),
            _ => Err(ErrorFields::error("42883", "incompatible comparison types")),
        }
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Text(s.into())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Column {
    pub(crate) name: String,
    pub(crate) qualifier: String,
    pub(crate) oid: u32,
    pub(crate) modifier: i32,
    pub(super) scope: usize,
    /// Explicitly addressable system columns are excluded from `*`.
    pub(super) system: bool,
    pub(super) using_hidden: bool,
    pub(super) star_order: usize,
}
impl Column {
    pub(crate) fn new(name: &str, oid: u32) -> Self {
        Self {
            name: name.into(),
            qualifier: String::new(),
            oid,
            modifier: -1,
            scope: 0,
            system: false,
            using_hidden: false,
            star_order: 0,
        }
    }
    pub(crate) fn field(&self, format: i16) -> FieldDescription {
        FieldDescription {
            name: self.name.clone(),
            type_oid: self.oid,
            type_length: match self.oid {
                16 | 18 => 1,
                19 => 64,
                21 => 2,
                23 | 24 | 26 | 28 | 2203 => 4,
                20 | 701 | 1114 => 8,
                _ => -1,
            },
            type_modifier: self.modifier,
            format,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Table {
    pub(crate) columns: Vec<Column>,
    pub(crate) rows: Vec<Vec<Value>>,
}
impl Table {
    pub(crate) fn new(columns: &[(&str, u32)]) -> Self {
        Self {
            columns: columns.iter().map(|(n, t)| Column::new(n, *t)).collect(),
            rows: Vec::new(),
        }
    }
    pub(crate) fn push(&mut self, row: Vec<Value>) {
        assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
    }
}

pub(super) struct Budget {
    steps: usize,
    bytes: usize,
}
impl Budget {
    pub(super) fn new() -> Self {
        Self {
            steps: 50_000_000,
            bytes: 64 * 1024 * 1024,
        }
    }
    pub(super) fn step(&mut self) -> Result<(), ErrorFields> {
        self.steps = self.steps.checked_sub(1).ok_or_else(|| {
            ErrorFields::error("54001", "catalog evaluation work budget exceeded")
        })?;
        Ok(())
    }
    fn allocate(&mut self, row: &[Value]) -> Result<(), ErrorFields> {
        fn size(v: &Value) -> usize {
            std::mem::size_of::<Value>().saturating_add(match v {
                Value::Text(s) | Value::Reg(_, s) => s.len(),
                Value::Array(a) => a.iter().map(size).fold(0, usize::saturating_add),
                _ => 0,
            })
        }
        let bytes = row.iter().map(size).fold(0, usize::saturating_add);
        self.allocate_bytes(bytes)
    }
    fn allocate_bytes(&mut self, bytes: usize) -> Result<(), ErrorFields> {
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| ErrorFields::error("54001", "catalog allocation budget exceeded"))?;
        Ok(())
    }
    fn rows(&self, n: usize) -> Result<(), ErrorFields> {
        if n > 1_000_000 {
            Err(ErrorFields::error(
                "54001",
                "catalog intermediate row budget exceeded",
            ))
        } else {
            Ok(())
        }
    }
}

/// Compare complete fixed probes by SQL tokens, preserving string values
/// while ignoring keyword case, whitespace and comments.
pub(crate) fn matches_statement(text: &str, expected: &str) -> bool {
    fn kinds(text: &str) -> Option<Vec<lex::Kind>> {
        let mut tokens = lex::lex(text)
            .ok()?
            .into_iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>();
        tokens.pop(); // End token.
        if matches!(tokens.last(), Some(lex::Kind::Symbol(s)) if s == ";") {
            tokens.pop();
        }
        Some(tokens)
    }
    match (kinds(text), kinds(expected)) {
        (Some(text), Some(expected)) => text == expected,
        _ => false,
    }
}

/// Decode a complete single string-literal SELECT without constructing a
/// catalog. Every other shape retains its normal routing.
pub(crate) fn string_probe(text: &str) -> Option<(String, String)> {
    let tokens = lex::lex(text).ok()?;
    if !matches!(tokens.first().map(|t| &t.kind), Some(lex::Kind::Word(w)) if w == "select")
        || !matches!(tokens.get(1).map(|t| &t.kind), Some(lex::Kind::String(_)))
    {
        return None;
    }
    let query = parse::parse(tokens).ok()?;
    if !query.ctes.is_empty()
        || query.union.is_some()
        || !query.order.is_empty()
        || query.limit.is_some()
        || query.offset.is_some()
    {
        return None;
    }
    let Body::Select(select) = query.body else {
        return None;
    };
    if select.distinct
        || !select.from.is_empty()
        || select.filter.is_some()
        || !select.group.is_empty()
        || select.having.is_some()
        || select.columns.len() != 1
    {
        return None;
    }
    let (expression, alias) = select.columns.into_iter().next()?;
    let ExprKind::String(value) = expression.kind else {
        return None;
    };
    Some((value, alias.unwrap_or_else(|| "?column?".into())))
}

/// Token names decide intent; strings and comments cannot divert SDBL.
pub(crate) fn route(text: &str) -> Option<Result<Query, ErrorFields>> {
    let tokens = match lex::lex(text) {
        Ok(tokens) => tokens,
        Err(e) => {
            // Lex the prefix ending before the bad token to determine intent.
            let end = text
                .char_indices()
                .nth(e.position.unwrap_or(1).saturating_sub(1))
                .map_or(text.len(), |(i, _)| i);
            return lex::lex(&text[..end])
                .ok()
                .filter(|tokens| intended(tokens))
                .map(|_| Err(e));
        }
    };
    if !intended(&tokens) {
        return None;
    }
    let q = match parse::parse(tokens) {
        Ok(q) => q,
        Err(e) => return Some(Err(e)),
    };
    if !catalog_only(&q, &[]) {
        return None;
    }
    Some(Ok(q))
}
fn intended(tokens: &[lex::Token]) -> bool {
    tokens.iter().any(|t| matches!(&t.kind, lex::Kind::Word(w) | lex::Kind::Quoted(w) if w == "pg_catalog" || w == "information_schema" || super::relations::known(w) || func::known(w)))
}
fn catalog_only(q: &Query, outer_ctes: &[String]) -> bool {
    let mut ctes = outer_ctes.to_vec();
    ctes.extend(q.ctes.iter().map(|(n, _)| n.clone()));
    if !q.ctes.iter().all(|(_, q)| catalog_only(q, &ctes)) {
        return false;
    }
    if let Body::Select(s) = &q.body {
        for join in &s.from {
            match &join.source.kind {
                SourceKind::Relation(n)
                    if !(n.len() == 1
                        && (super::relations::known(&n[0]) || ctes.contains(&n[0])))
                        && !(n.len() == 2
                            && ["pg_catalog", "information_schema"].contains(&n[0].as_str())) =>
                {
                    return false;
                }
                SourceKind::Query(q) if !catalog_only(q, &ctes) => return false,
                _ => {}
            }
        }
    }
    // Include every expression, including nested subqueries, in routing.
    let mut valid = true;
    walk_query(q, &mut |e| {
        if let ExprKind::Query(q) | ExprKind::Exists(q) | ExprKind::ArrayQuery(q) = &e.kind {
            valid &= catalog_only(q, &ctes);
        }
    });
    valid && q.union.as_ref().is_none_or(|(_, q)| catalog_only(q, &ctes))
}
fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::OrderedAggregate(call, order) => {
            walk_expr(call, f);
            for (key, _, _) in order {
                walk_expr(key, f);
            }
        }
        ExprKind::Call(_, es) | ExprKind::Array(es) => {
            for e in es {
                walk_expr(e, f);
            }
        }
        ExprKind::Unary(_, e) | ExprKind::Cast(e, _) | ExprKind::Is(e, _, _) => walk_expr(e, f),
        ExprKind::Binary(_, a, b) | ExprKind::Subscript(a, b) => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        ExprKind::Case(base, bs, other) => {
            if let Some(e) = base {
                walk_expr(e, f);
            }
            for (a, b) in bs {
                walk_expr(a, f);
                walk_expr(b, f);
            }
            if let Some(e) = other {
                walk_expr(e, f);
            }
        }
        ExprKind::In(e, es, _) => {
            walk_expr(e, f);
            for e in es {
                walk_expr(e, f);
            }
        }
        _ => {}
    }
}
fn walk_query(q: &Query, f: &mut impl FnMut(&Expr)) {
    if let Body::Select(s) = &q.body {
        for (e, _) in &s.columns {
            walk_expr(e, f);
        }
        for e in s.filter.iter().chain(&s.group).chain(&s.having) {
            walk_expr(e, f);
        }
        for j in &s.from {
            if let Some(e) = &j.on {
                walk_expr(e, f);
            }
            if let SourceKind::Function(e) = &j.source.kind {
                walk_expr(e, f);
            }
        }
    }
    for (e, _, _) in &q.order {
        walk_expr(e, f);
    }
}

pub(crate) fn describe(
    q: &Query,
    catalog: &Catalog,
    declared: &[u32],
) -> Result<(Vec<Column>, Vec<u32>), ErrorFields> {
    plan::describe(q, catalog, declared)
}
pub(crate) fn evaluate(
    q: &Query,
    catalog: &Catalog,
    parameters: &[Value],
    parameter_types: &[u32],
) -> Result<Table, ErrorFields> {
    eval::evaluate(q, catalog, parameters, parameter_types)
}

pub(crate) use func::format_type;
pub(crate) fn array_oid(oid: u32) -> Option<u32> {
    func::array_type(oid)
}
pub(crate) fn type_name(oid: u32) -> String {
    match oid {
        16 => "bool",
        17 => "bytea",
        18 => "char",
        19 => "name",
        20 => "int8",
        21 => "int2",
        22 => "int2vector",
        23 => "int4",
        24 => "regproc",
        2203 => "regoper",
        25 => "text",
        26 => "oid",
        28 => "xid",
        30 => "oidvector",
        701 => "float8",
        1033 => "aclitem",
        1043 => "varchar",
        1114 => "timestamp",
        1186 => "interval",
        1700 => "numeric",
        2205 => "regclass",
        2206 => "regtype",
        2950 => "uuid",
        4089 => "regnamespace",
        _ => "unknown",
    }
    .into()
}

// Bound the constructed expression too: a flat stream of operators can
// make a deep AST without any parenthesis nesting.
fn bound_expr(root: &Expr) -> Result<(), ErrorFields> {
    let mut pending = vec![(root, 1_usize)];
    while let Some((e, depth)) = pending.pop() {
        if depth > 32 {
            return Err(lex::error(
                "54001",
                "catalog expression nesting exceeds 32",
                e.position,
            ));
        }
        let d = depth + 1;
        match &e.kind {
            ExprKind::Call(_, es) | ExprKind::Array(es) => {
                pending.extend(es.iter().map(|e| (e, d)))
            }
            ExprKind::Unary(_, a) | ExprKind::Cast(a, _) | ExprKind::Is(a, _, _) => {
                pending.push((a, d))
            }
            ExprKind::Binary(_, a, b) | ExprKind::Subscript(a, b) => {
                pending.push((a, d));
                pending.push((b, d));
            }
            ExprKind::Case(base, branches, other) => {
                if let Some(e) = base {
                    pending.push((e, d));
                }
                for (a, b) in branches {
                    pending.push((a, d));
                    pending.push((b, d));
                }
                if let Some(e) = other {
                    pending.push((e, d));
                }
            }
            ExprKind::In(a, es, _) => {
                pending.push((a, d));
                pending.extend(es.iter().map(|e| (e, d)));
            }
            _ => {}
        }
    }
    Ok(())
}
fn scope(local: &[Column], outer: &[Column]) -> Vec<Column> {
    let mut columns = local.to_vec();
    columns.extend(outer.iter().cloned().map(|mut c| {
        c.scope += 1;
        c
    }));
    columns
}

fn parameter_count(q: &Query) -> usize {
    fn visit(q: &Query, count: &mut usize) {
        walk_query(q, &mut |e| {
            if let ExprKind::Parameter(n) = e.kind {
                *count = (*count).max(n);
            }
            if let ExprKind::Query(q) | ExprKind::Exists(q) | ExprKind::ArrayQuery(q) = &e.kind {
                visit(q, count);
            }
        });
        if let Body::Values(rows) = &q.body {
            for row in rows {
                for e in row {
                    walk_expr(e, &mut |e| {
                        if let ExprKind::Parameter(n) = e.kind {
                            *count = (*count).max(n);
                        }
                    });
                }
            }
        }
        for (_, q) in &q.ctes {
            visit(q, count);
        }
        if let Some((_, q)) = &q.union {
            visit(q, count);
        }
        if let Body::Select(s) = &q.body {
            for j in &s.from {
                if let SourceKind::Query(q) = &j.source.kind {
                    visit(q, count);
                }
            }
        }
        for e in q.limit.iter().chain(&q.offset) {
            walk_expr(e, &mut |e| {
                if let ExprKind::Parameter(n) = e.kind {
                    *count = (*count).max(n);
                }
            });
        }
    }
    let mut count = 0;
    visit(q, &mut count);
    count
}
#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz(text: &str) {
    let catalog = super::finish(vec![], "fuzz", "1c", 3);
    if let Some(Ok(q)) = route(text) {
        let _ = plan::describe(&q, &catalog, &[]);
        let _ = eval::evaluate_bounded(&q, &catalog, &[], &[], 50_000);
    }
}
