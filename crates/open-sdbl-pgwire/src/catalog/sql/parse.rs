//! SELECT syntax used by catalog clients. Parsing never evaluates SQL.
use super::lex::{Kind, Token, error};
use crate::codec::ErrorFields;

#[derive(Debug, Clone)]
pub(crate) struct Query {
    pub(super) ctes: Vec<(String, Query)>,
    pub(super) body: Body,
    pub(super) union: Option<(bool, Box<Query>)>,
    pub(super) order: Vec<(Expr, bool, bool)>,
    pub(super) limit: Option<Expr>,
    pub(super) offset: Option<Expr>,
}
#[derive(Debug, Clone)]
pub(super) enum Body {
    Select(Select),
    Values(Vec<Vec<Expr>>),
}
#[derive(Debug, Clone)]
pub(super) struct Select {
    pub(super) distinct: bool,
    pub(super) columns: Vec<(Expr, Option<String>)>,
    pub(super) from: Vec<Join>,
    pub(super) filter: Option<Expr>,
    pub(super) group: Vec<Expr>,
    pub(super) having: Option<Expr>,
}
#[derive(Debug, Clone)]
pub(super) struct Join {
    pub(super) source: Source,
    pub(super) left: bool,
    pub(super) on: Option<Expr>,
    pub(super) using: Vec<String>,
    pub(super) natural: bool,
}
#[derive(Debug, Clone)]
pub(super) struct Source {
    pub(super) kind: SourceKind,
    pub(super) alias: Option<String>,
    pub(super) columns: Vec<String>,
}
#[derive(Debug, Clone)]
pub(super) enum SourceKind {
    Relation(Vec<String>),
    Query(Box<Query>),
    Function(Expr),
}
#[derive(Debug, Clone)]
pub(super) struct Expr {
    pub(super) kind: ExprKind,
    pub(super) position: usize,
}
#[derive(Debug, Clone)]
pub(super) enum ExprKind {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Parameter(usize),
    Column(Vec<String>),
    Star(Option<String>),
    Call(Vec<String>, Vec<Expr>),
    OrderedAggregate(Box<Expr>, Vec<(Expr, bool, bool)>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, Vec<String>),
    Case(Option<Box<Expr>>, Vec<(Expr, Expr)>, Option<Box<Expr>>),
    Is(Box<Expr>, String, bool),
    In(Box<Expr>, Vec<Expr>, bool),
    Query(Box<Query>),
    Exists(Box<Query>),
    Array(Vec<Expr>),
    ArrayQuery(Box<Query>),
    Subscript(Box<Expr>, Box<Expr>),
}

pub(super) fn parse(tokens: Vec<Token>) -> Result<Query, ErrorFields> {
    let mut nesting = 0_usize;
    for token in &tokens {
        if matches!(&token.kind,Kind::Symbol(s) if s=="("||s=="[") {
            nesting += 1;
            if nesting > 32 {
                return Err(error("54001", "catalog nesting exceeds 32", token.position));
            }
        }
        if matches!(&token.kind,Kind::Symbol(s) if s==")"||s=="]") {
            nesting = nesting.saturating_sub(1);
        }
    }
    let mut parser = Parser {
        tokens,
        i: 0,
        depth: 0,
    };
    let result = parser.query()?;
    parser.eat(";");
    if parser.token().kind != Kind::End {
        return Err(parser.unsupported());
    }
    Ok(result)
}
struct Parser {
    tokens: Vec<Token>,
    i: usize,
    depth: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.i]
    }
    fn at(&self, s: &str) -> bool {
        matches!(&self.token().kind, Kind::Word(w) | Kind::Symbol(w) if w == s)
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, s: &str) -> Result<(), ErrorFields> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(error(
                "42601",
                format!("expected {s}"),
                self.token().position,
            ))
        }
    }
    fn unsupported(&self) -> ErrorFields {
        error(
            "0A000",
            "unsupported catalog SQL construct",
            self.token().position,
        )
    }
    fn ident(&mut self) -> Result<String, ErrorFields> {
        if let Kind::Word(w) | Kind::Quoted(w) = self.token().kind.clone() {
            self.i += 1;
            Ok(w)
        } else {
            Err(error(
                "42601",
                "expected an identifier",
                self.token().position,
            ))
        }
    }
    fn name(&mut self) -> Result<Vec<String>, ErrorFields> {
        let mut name = vec![self.ident()?];
        while self.eat(".") {
            name.push(self.ident()?);
        }
        Ok(name)
    }
    fn query(&mut self) -> Result<Query, ErrorFields> {
        self.depth += 1;
        if self.depth > 32 {
            return Err(error(
                "54001",
                "catalog nesting exceeds 32",
                self.token().position,
            ));
        }
        let mut ctes = Vec::new();
        if self.eat("with") {
            if self.at("recursive") {
                return Err(self.unsupported());
            }
            loop {
                let name = self.ident()?;
                self.need("as")?;
                self.need("(")?;
                let q = self.query()?;
                self.need(")")?;
                ctes.push((name, q));
                if ctes.len() > 64 {
                    return Err(error(
                        "54001",
                        "catalog CTE count exceeds 64",
                        self.token().position,
                    ));
                }
                if !self.eat(",") {
                    break;
                }
            }
        }
        let body = if self.eat("values") {
            let mut rows = Vec::new();
            loop {
                self.need("(")?;
                rows.push(self.expr_list(")")?);
                if !self.eat(",") {
                    break;
                }
            }
            Body::Values(rows)
        } else {
            self.need("select")?;
            let distinct = self.eat("distinct");
            let mut columns = Vec::new();
            loop {
                let e = self.expr(0)?;
                let alias = if self.eat("as") || self.alias_available() {
                    Some(self.ident()?)
                } else {
                    None
                };
                columns.push((e, alias));
                if columns.len() > 1664 {
                    return Err(error(
                        "54001",
                        "catalog projection exceeds 1664 columns",
                        self.token().position,
                    ));
                }
                if !self.eat(",") {
                    break;
                }
            }
            let mut from = Vec::new();
            if self.eat("from") {
                from.push(Join {
                    source: self.source()?,
                    left: false,
                    on: None,
                    using: vec![],
                    natural: false,
                });
                loop {
                    let comma = self.eat(",");
                    let natural = self.eat("natural");
                    let left = self.eat("left");
                    if left {
                        self.eat("outer");
                    }
                    let qualifier = self.eat("inner") || self.eat("cross");
                    if !comma && !self.eat("join") {
                        if left || qualifier || natural {
                            self.need("join")?;
                        } else {
                            break;
                        }
                    }
                    let source = self.source()?;
                    let on = if self.eat("on") {
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    let using = if self.eat("using") {
                        self.need("(")?;
                        let mut names = vec![self.ident()?];
                        while self.eat(",") {
                            names.push(self.ident()?);
                        }
                        self.need(")")?;
                        names
                    } else {
                        vec![]
                    };
                    if (on.is_some() && !using.is_empty())
                        || (natural && (on.is_some() || !using.is_empty()))
                    {
                        return Err(error(
                            "42601",
                            "JOIN cannot specify both ON and USING",
                            self.token().position,
                        ));
                    }
                    from.push(Join {
                        source,
                        left,
                        on,
                        using,
                        natural,
                    });
                }
            }
            let filter = if self.eat("where") {
                Some(self.expr(0)?)
            } else {
                None
            };
            let group = if self.eat("group") {
                self.need("by")?;
                self.comma_exprs()?
            } else {
                vec![]
            };
            let having = if self.eat("having") {
                Some(self.expr(0)?)
            } else {
                None
            };
            Body::Select(Select {
                distinct,
                columns,
                from,
                filter,
                group,
                having,
            })
        };
        let mut order = Vec::new();
        let mut limit = None;
        let mut offset = None;
        let union = if self.eat("union") {
            let all = self.eat("all");
            if !all {
                self.eat("distinct");
            }
            let mut right = self.query()?;
            // The trailing clauses belong to the complete set operation.
            // Move them through every branch of a UNION chain at parse time.
            order = std::mem::take(&mut right.order);
            limit = right.limit.take();
            offset = right.offset.take();
            Some((all, Box::new(right)))
        } else {
            None
        };
        if self.eat("order") {
            self.need("by")?;
            order = self.order_items()?;
        }
        if self.eat("limit") {
            limit = if self.eat("all") {
                None
            } else {
                Some(self.expr(0)?)
            };
        }
        if self.eat("offset") {
            offset = Some(self.expr(0)?);
        }
        self.depth -= 1;
        Ok(Query {
            ctes,
            body,
            union,
            order,
            limit,
            offset,
        })
    }
    fn alias_available(&self) -> bool {
        if matches!(self.token().kind, Kind::Quoted(_)) {
            return true;
        }
        matches!(&self.token().kind, Kind::Word(w) if ![
            "from","where","group","having","order","limit","offset","union","join","left","right","full","inner","outer","cross","natural","on","using","when","then","else","end","asc","desc","nulls","collate"
        ].contains(&w.as_str()))
    }
    fn order_items(&mut self) -> Result<Vec<(Expr, bool, bool)>, ErrorFields> {
        let mut order = Vec::new();
        loop {
            let e = self.expr(0)?;
            let desc = self.eat("desc");
            if !desc {
                self.eat("asc");
            }
            let nulls_first = if self.eat("nulls") {
                if self.eat("first") {
                    true
                } else {
                    self.need("last")?;
                    false
                }
            } else {
                desc
            };
            order.push((e, desc, nulls_first));
            if !self.eat(",") {
                break;
            }
        }
        Ok(order)
    }
    fn call(&mut self, name: Vec<String>, position: usize) -> Result<ExprKind, ErrorFields> {
        let args = if self.at(")") {
            vec![]
        } else {
            self.comma_exprs()?
        };
        let call = Expr {
            kind: ExprKind::Call(name.clone(), args),
            position,
        };
        let kind = if self.eat("order") {
            if !["array_agg", "string_agg", "count", "min", "max", "sum"]
                .contains(&name.last().unwrap().as_str())
            {
                return Err(self.unsupported());
            }
            self.need("by")?;
            ExprKind::OrderedAggregate(Box::new(call), self.order_items()?)
        } else {
            call.kind
        };
        self.need(")")?;
        Ok(kind)
    }
    fn source(&mut self) -> Result<Source, ErrorFields> {
        let kind = if self.eat("(") {
            let q = self.query()?;
            self.need(")")?;
            SourceKind::Query(Box::new(q))
        } else {
            let pos = self.token().position;
            let name = self.name()?;
            if self.eat("(") {
                SourceKind::Function(Expr {
                    kind: ExprKind::Call(name, self.expr_list(")")?),
                    position: pos,
                })
            } else {
                SourceKind::Relation(name)
            }
        };
        let alias = if self.eat("as") || self.alias_available() {
            Some(self.ident()?)
        } else {
            None
        };
        let mut columns = Vec::new();
        if self.eat("(") {
            columns.push(self.ident()?);
            while self.eat(",") {
                columns.push(self.ident()?);
            }
            self.need(")")?;
        }
        Ok(Source {
            kind,
            alias,
            columns,
        })
    }
    fn comma_exprs(&mut self) -> Result<Vec<Expr>, ErrorFields> {
        let mut es = vec![self.expr(0)?];
        while self.eat(",") {
            es.push(self.expr(0)?);
        }
        Ok(es)
    }
    fn expr_list(&mut self, end: &str) -> Result<Vec<Expr>, ErrorFields> {
        if self.eat(end) {
            return Ok(vec![]);
        }
        let es = self.comma_exprs()?;
        self.need(end)?;
        Ok(es)
    }
    fn expr(&mut self, min: u8) -> Result<Expr, ErrorFields> {
        self.depth += 1;
        if self.depth > 32 {
            return Err(error(
                "54001",
                "catalog expression nesting exceeds 32",
                self.token().position,
            ));
        }
        let position = self.token().position;
        let token = self.token().kind.clone();
        self.i += 1;
        let quoted = matches!(&token, Kind::Quoted(_));
        let kind = match token {
            Kind::String(s) => ExprKind::String(s),
            Kind::Number(n) => ExprKind::Number(n),
            Kind::Parameter(n) => ExprKind::Parameter(n),
            Kind::Word(ref w) if w == "null" => ExprKind::Null,
            Kind::Word(ref w) if w == "true" || w == "false" => ExprKind::Bool(w == "true"),
            Kind::Word(ref w) if w == "not" => ExprKind::Unary(w.clone(), Box::new(self.expr(3)?)),
            Kind::Symbol(ref w) if w == "-" || w == "+" => {
                ExprKind::Unary(w.clone(), Box::new(self.expr(8)?))
            }
            Kind::Symbol(ref w) if w == "*" => ExprKind::Star(None),
            Kind::Symbol(ref w) if w == "(" => {
                if self.at("select") || self.at("with") || self.at("values") {
                    let q = self.query()?;
                    self.need(")")?;
                    ExprKind::Query(Box::new(q))
                } else {
                    let e = self.expr(0)?;
                    self.need(")")?;
                    e.kind
                }
            }
            Kind::Word(ref w) if w == "exists" => {
                self.need("(")?;
                let q = self.query()?;
                self.need(")")?;
                ExprKind::Exists(Box::new(q))
            }
            Kind::Word(ref w) if w == "array" => {
                if self.eat("[") {
                    ExprKind::Array(self.expr_list("]")?)
                } else {
                    self.need("(")?;
                    let q = self.query()?;
                    self.need(")")?;
                    ExprKind::ArrayQuery(Box::new(q))
                }
            }
            Kind::Word(ref w) if w == "cast" => {
                self.need("(")?;
                let e = self.expr(0)?;
                self.need("as")?;
                let name = self.name()?;
                self.need(")")?;
                ExprKind::Cast(Box::new(e), name)
            }
            Kind::Word(ref w) if w == "case" => {
                let base = if !self.at("when") {
                    Some(Box::new(self.expr(0)?))
                } else {
                    None
                };
                let mut branches = Vec::new();
                while self.eat("when") {
                    let c = self.expr(0)?;
                    self.need("then")?;
                    branches.push((c, self.expr(0)?));
                }
                let other = if self.eat("else") {
                    Some(Box::new(self.expr(0)?))
                } else {
                    None
                };
                self.need("end")?;
                ExprKind::Case(base, branches, other)
            }
            Kind::Word(w) | Kind::Quoted(w) => {
                let mut name = vec![w];
                while self.eat(".") {
                    if self.eat("*") {
                        break;
                    }
                    name.push(self.ident()?);
                }
                if self.tokens[self.i - 1].kind == Kind::Symbol("*".into()) {
                    ExprKind::Star(name.pop())
                } else if self.eat("(") {
                    self.call(name, position)?
                } else if !quoted
                    && name.len() == 1
                    && [
                        "current_user",
                        "session_user",
                        "current_role",
                        "current_catalog",
                    ]
                    .contains(&name[0].as_str())
                {
                    ExprKind::Call(name, vec![])
                } else {
                    ExprKind::Column(name)
                }
            }
            _ => {
                self.i -= 1;
                return Err(error("42601", "expected an expression", position));
            }
        };
        let mut lhs = Expr { kind, position };
        let mut operators = 0;
        loop {
            super::bound_expr(&lhs)?;
            operators += 1;
            if operators > 64 {
                return Err(error(
                    "54001",
                    "catalog expression nesting exceeds 32",
                    position,
                ));
            }
            if min <= 9 && self.eat("::") {
                let mut name = self.name()?;
                if self.eat("[") {
                    self.need("]")?;
                    name.last_mut().unwrap().push_str("[]");
                }
                lhs = Expr {
                    kind: ExprKind::Cast(Box::new(lhs), name),
                    position,
                };
                continue;
            }
            if min <= 9 && self.eat("[") {
                let index = self.expr(0)?;
                self.need("]")?;
                lhs = Expr {
                    kind: ExprKind::Subscript(Box::new(lhs), Box::new(index)),
                    position,
                };
                continue;
            }
            if self.eat("collate") {
                self.name()?;
                continue;
            }
            if min <= 4 && self.eat("is") {
                let not = self.eat("not");
                if self.eat("distinct") {
                    self.need("from")?;
                    let rhs = self.expr(5)?;
                    lhs = Expr {
                        kind: ExprKind::Binary(
                            if not {
                                "is not distinct"
                            } else {
                                "is distinct"
                            }
                            .into(),
                            Box::new(lhs),
                            Box::new(rhs),
                        ),
                        position,
                    };
                } else {
                    let what = self.ident()?;
                    if !["null", "true", "false", "unknown"].contains(&what.as_str()) {
                        return Err(self.unsupported());
                    }
                    lhs = Expr {
                        kind: ExprKind::Is(Box::new(lhs), what, not),
                        position,
                    };
                }
                continue;
            }
            let not = min <= 4 && self.at("not") && self.tokens.get(self.i + 1).is_some_and(|t| matches!(&t.kind, Kind::Word(w) if ["in","like","ilike","between"].contains(&w.as_str())));
            if not {
                self.i += 1;
            }
            if min <= 4 && self.eat("in") {
                self.need("(")?;
                let es = if self.at("select") || self.at("with") || self.at("values") {
                    let q = self.query()?;
                    self.need(")")?;
                    vec![Expr {
                        kind: ExprKind::Query(Box::new(q)),
                        position,
                    }]
                } else {
                    self.expr_list(")")?
                };
                lhs = Expr {
                    kind: ExprKind::In(Box::new(lhs), es, not),
                    position,
                };
                continue;
            }
            if min <= 4 && self.eat("between") {
                let lo = self.expr(5)?;
                self.need("and")?;
                let hi = self.expr(5)?;
                let lower = Expr {
                    kind: ExprKind::Binary(">=".into(), Box::new(lhs.clone()), Box::new(lo)),
                    position,
                };
                let upper = Expr {
                    kind: ExprKind::Binary("<=".into(), Box::new(lhs), Box::new(hi)),
                    position,
                };
                lhs = Expr {
                    kind: ExprKind::Binary("and".into(), Box::new(lower), Box::new(upper)),
                    position,
                };
                if not {
                    lhs = Expr {
                        kind: ExprKind::Unary("not".into(), Box::new(lhs)),
                        position,
                    };
                }
                continue;
            }
            let op = match &self.token().kind {
                Kind::Word(w) | Kind::Symbol(w) => w.clone(),
                _ => break,
            };
            let precedence = match op.as_str() {
                "or" => 1,
                "and" => 2,
                "=" | "<>" | "!=" | "<" | ">" | "<=" | ">=" | "like" | "ilike" | "~" | "~*"
                | "!~" | "!~*" | "operator" => 4,
                "||" => 5,
                "+" | "-" => 6,
                "*" | "/" | "%" => 7,
                _ => break,
            };
            if precedence < min {
                break;
            }
            self.i += 1;
            let op = if op == "operator" {
                self.need("(")?;
                let mut op = String::new();
                while !self.at(")") && self.token().kind != Kind::End {
                    if let Kind::Symbol(s) = &self.token().kind {
                        if s != "." {
                            op.clone_from(s);
                        }
                    }
                    self.i += 1;
                }
                self.need(")")?;
                op
            } else {
                op
            };
            let rhs = self.expr(precedence + 1)?;
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                position,
            };
            if not {
                lhs = Expr {
                    kind: ExprKind::Unary("not".into(), Box::new(lhs)),
                    position,
                };
            }
        }
        super::bound_expr(&lhs)?;
        self.depth -= 1;
        Ok(lhs)
    }
}
