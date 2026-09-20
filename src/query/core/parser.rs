//! Bounded recursive-descent parser.

use crate::query::core::ast::{
    AccumulationAst, AccumulationKind, AggregateArgument, AggregateKind, BatchAst, CaseBranch,
    CastTarget, ControlPoint, DatePart, Expression, FieldReference, GroupKey, HierarchyTotals,
    IndexAst, IndexField, IntoAst, JoinAst, JoinKind, OrderKeyAst, OrderTerm, PeriodKind,
    PeriodsAst, PresentationArgument, PresentationOperation, PrimitiveType, Projection,
    ProjectionItem, QueryAst, RestrictionAst, ScalarFunction, SelectAst, SliceAst, SliceKind,
    SourceAst, StatementAst, TotalsAst, TotalsField, TypeName, UnionLink, parse_datetime_value,
};
use crate::query::core::diag::SourcePosition;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::kind_from_query_name;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

/// Whether the token is the `ТекущаяТаблица` of a restriction text.
fn is_current_table_token(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Identifier && names_equal(token.lexeme, crate::access::CURRENT_TABLE)
}

pub(super) struct Parser<'tokens, 'source> {
    tokens: &'tokens [Token<'source>],
    offset: usize,
    depth: usize,
    binary_operators: usize,
    eof: SourcePosition,
    nesting: usize,
}

fn is_comparison(operator: &str) -> bool {
    matches!(operator, "=" | "<>" | "<" | ">" | "<=" | ">=")
}

/// The values of a system enumeration `ЗНАЧЕНИЕ` accepts, with the number
/// the platform stores for each: the record kind of an accumulation
/// register (`_RecordKind`), the side of an accounting record
/// (`_Correspond`), and the kind of an account (`_Kind`, measured on a
/// chart of accounts against its predefined accounts).
/// Whether a source kind word names an accounting register, whose virtual
/// tables take more arguments than an accumulation register's.
fn kind_of_source_is_accounting(kind: &str) -> bool {
    ["РегистрБухгалтерии", "AccountingRegister", "AccRg"]
        .iter()
        .any(|name| names_equal(name, kind))
}

fn system_enumeration(name: &str) -> Option<&'static [(&'static [&'static str], u8)]> {
    const ACCUMULATION: &[(&[&str], u8)] =
        &[(&["Приход", "Receipt"], 0), (&["Расход", "Expense"], 1)];
    const ACCOUNTING: &[(&[&str], u8)] = &[(&["Дебет", "Debit"], 0), (&["Кредит", "Credit"], 1)];
    const ACCOUNT: &[(&[&str], u8)] = &[
        (&["Активный", "Active"], 0),
        (&["Пассивный", "Passive"], 1),
        (&["АктивноПассивный", "ActivePassive"], 2),
    ];
    if names_equal(name, "ВидДвиженияНакопления") || names_equal(name, "AccumulationRecordType")
    {
        Some(ACCUMULATION)
    } else if names_equal(name, "ВидДвиженияБухгалтерии")
        || names_equal(name, "AccountingRecordType")
    {
        Some(ACCOUNTING)
    } else if names_equal(name, "ВидСчета") || names_equal(name, "AccountType") {
        Some(ACCOUNT)
    } else {
        None
    }
}

fn is_contextual_identifier(kind: TokenKind) -> bool {
    kind == TokenKind::Identifier
        || matches!(
            kind,
            TokenKind::Keyword(
                Keyword::Count
                    | Keyword::Sum
                    | Keyword::Min
                    | Keyword::Max
                    | Keyword::Avg
                    | Keyword::Refs
                    | Keyword::Between
                    | Keyword::BalanceAndTurnovers
                    | Keyword::DrCrTurnovers
                    | Keyword::RecordsWithExtDimensions
                    | Keyword::Substring
                    | Keyword::StringLength
                    | Keyword::TrimAll
                    | Keyword::TrimLeft
                    | Keyword::TrimRight
                    | Keyword::Upper
                    | Keyword::Lower
                    | Keyword::StrFind
                    | Keyword::StrReplace
                    | Keyword::Round
                    | Keyword::Int
                    | Keyword::Sqrt
                    | Keyword::Exp
                    | Keyword::Log
                    | Keyword::Log10
                    | Keyword::Pow
                    | Keyword::Cos
                    | Keyword::Sin
                    | Keyword::Tan
                    | Keyword::ACos
                    | Keyword::ASin
                    | Keyword::ATan
                    | Keyword::Type
                    | Keyword::ValueType
                    | Keyword::Totals
                    | Keyword::Overall
                    | Keyword::Hierarchy
                    | Keyword::Only
                    | Keyword::Periods
                    | Keyword::Presentation
                    | Keyword::RefPresentation
                    | Keyword::SliceFirst
                    | Keyword::SliceLast
                    | Keyword::Balance
                    | Keyword::Turnovers
                    | Keyword::DateTime
                    | Keyword::BeginOfPeriod
                    | Keyword::EndOfPeriod
                    | Keyword::DateAdd
                    | Keyword::DateDiff
                    | Keyword::Year
                    | Keyword::Quarter
                    | Keyword::Month
                    | Keyword::DayOfYear
                    | Keyword::Day
                    | Keyword::Week
                    | Keyword::WeekDay
                    | Keyword::Hour
                    | Keyword::Minute
                    | Keyword::Second
                    | Keyword::Value
                    | Keyword::Uuid
                    | Keyword::Cast
                    | Keyword::IsNullFunction
                    | Keyword::Add
                    | Keyword::Drop
                    | Keyword::Index
                    | Keyword::Sets
                    | Keyword::Unique
            )
        )
}

/// Whether a word may be an alias written without `КАК`. The platform
/// accepts the short form wherever the `КАК` form is accepted, and accepts
/// a contextual keyword as the name — `ВЫБРАТЬ 1 Сумма` names a column
/// `Сумма`, measured on 8.3.27. A word that opens the next clause is never
/// the alias: `ИТОГИ` and `ИНДЕКСИРОВАТЬ` both follow a projection and a
/// source directly.
fn is_implicit_alias(kind: TokenKind) -> bool {
    is_contextual_identifier(kind)
        && !matches!(kind, TokenKind::Keyword(Keyword::Totals | Keyword::Index))
}

/// Whether a join nested inside another may be flattened into the same
/// chain. Measured on the platform: a group of left joins answers exactly
/// what the flat chain answers, while an inner join inside a left one
/// keeps the outer rows that the flat chain would drop.
fn check_join_group(outer: JoinKind, inner: &JoinAst<'_, '_>) -> Result<(), QueryDiagnostic> {
    let flattens = match outer {
        JoinKind::Inner | JoinKind::Cross => matches!(inner.kind, JoinKind::Inner | JoinKind::Left),
        JoinKind::Left => inner.kind == JoinKind::Left,
        JoinKind::Right | JoinKind::Full => false,
    };
    if flattens {
        return Ok(());
    }
    Err(QueryDiagnostic::at(
        QueryDiagnosticKind::UnsupportedFeature,
        Some(inner.token),
        "a join written inside the source of this join answers differently \
         than the same joins written one after another",
    ))
}

/// Whether a source names a filter criterion, whose value follows the name
/// in parentheses.
fn is_filter_criterion_kind(lexeme: &str) -> bool {
    names_equal(lexeme, "КритерийОтбора") || names_equal(lexeme, "FilterCriterion")
}

fn is_ascending_order(token: &Token<'_>) -> bool {
    names_equal(token.lexeme, "ASC") || names_equal(token.lexeme, "ВОЗР")
}

impl<'tokens, 'source> Parser<'tokens, 'source> {
    /// Nesting levels a query may use before the parser reports
    /// [`QueryDiagnosticKind::TooDeep`]. Measured in a debug build, one
    /// level of nested date functions costs about sixteen kilobytes of
    /// stack, so the budget stays well inside a small thread stack.
    const MAX_DEPTH: usize = 64;
    const MAX_BINARY_OPERATORS: usize = 4_096;
    /// Nested statements compile recursively, so their depth is bounded
    /// separately from expression nesting.
    const MAX_NESTED_QUERIES: usize = 16;
    /// Statements of one batch; every statement compiles independently.
    const MAX_STATEMENTS: usize = 64;

    pub(super) fn new(tokens: &'tokens [Token<'source>], source: &str) -> Self {
        let mut line = 1;
        let mut column = 1;
        for character in source.chars() {
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Self {
            tokens,
            offset: 0,
            depth: 0,
            binary_operators: 0,
            nesting: 0,
            eof: SourcePosition {
                offset: source.len(),
                line,
                column,
            },
        }
    }

    pub(super) fn parse(mut self) -> Result<BatchAst<'tokens, 'source>, QueryDiagnostic> {
        let mut statements = Vec::new();
        loop {
            while self.consume_lexeme(";") {}
            if self.peek().is_none() && !statements.is_empty() {
                break;
            }
            if statements.len() == Self::MAX_STATEMENTS {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::WorkBudgetExceeded,
                    self.peek(),
                    format!("batch exceeds {} statements", Self::MAX_STATEMENTS),
                ));
            }
            statements.push(self.parse_statement()?);
            if let Some(token) = self.peek()
                && token.lexeme != ";"
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    format!("unsupported query syntax starting at {:?}", token.lexeme),
                ));
            }
        }
        Ok(BatchAst { statements })
    }

    /// Parses one statement of a batch without its terminator.
    fn parse_statement(&mut self) -> Result<StatementAst<'tokens, 'source>, QueryDiagnostic> {
        if self.consume_keyword(Keyword::Drop) {
            let name = self.expect_identifier("expected temporary table name after DROP")?;
            return Ok(StatementAst::Drop { name });
        }
        Ok(StatementAst::Query(self.parse_query_body()?))
    }

    /// Parses `SELECT … [UNION …] [ORDER BY …]` without the terminator; used
    /// for the top level and for nested queries.
    fn parse_query_body(&mut self) -> Result<QueryAst<'tokens, 'source>, QueryDiagnostic> {
        let mut branches = vec![self.parse_select()?];
        let mut unions = Vec::new();
        while let Some(token) = self.consume_keyword_token(Keyword::Union) {
            unions.push(UnionLink {
                token,
                all: self.consume_keyword(Keyword::All),
            });
            branches.push(self.parse_select()?);
        }
        let into = branches[0].into.take();
        if let Some(branch) = branches[1..]
            .iter()
            .find(|branch| branch.into.is_some())
            .and_then(|branch| branch.into.as_ref())
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(branch.token),
                "INTO is allowed only in the first branch of a statement",
            ));
        }
        let allowed = branches[0].allowed.take();
        if let Some(token) = branches[1..].iter().find_map(|branch| branch.allowed) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "ALLOWED is allowed only in the first branch of a statement",
            ));
        }
        // The Syntax Assistant lists clauses out of textual order, so the
        // index clause is accepted on either side of the final ordering.
        let mut index = self.parse_index()?;
        let order = self.parse_order()?;
        if index.is_none() {
            index = self.parse_index()?;
        }
        let totals = self.parse_totals()?;
        Ok(QueryAst {
            branches,
            unions,
            order,
            into,
            allowed,
            index,
            totals,
        })
    }

    /// Parses `ИТОГИ [<поля>] ПО [ОБЩИЕ] [<контрольные точки>]`.
    fn parse_totals(&mut self) -> Result<Option<TotalsAst<'tokens, 'source>>, QueryDiagnostic> {
        let Some(token) = self.consume_keyword_token(Keyword::Totals) else {
            return Ok(None);
        };
        self.record_binary_operator(token)?;
        let mut fields = Vec::new();
        while !self
            .peek()
            .is_some_and(|next| next.kind == TokenKind::Keyword(Keyword::By))
        {
            let field_token = self.peek().ok_or_else(|| {
                self.diagnostic(QueryDiagnosticKind::Syntax, None, "expected TOTALS field")
            })?;
            self.record_binary_operator(field_token)?;
            let expression = self.parse_or()?;
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_identifier("expected TOTALS field alias after AS")?)
            } else {
                None
            };
            fields.push(TotalsField {
                token: field_token,
                expression,
                alias,
            });
            if !self.consume_lexeme(",") {
                break;
            }
        }
        self.expect_keyword(Keyword::By)?;
        let overall = self.consume_keyword_token(Keyword::Overall);
        let mut points = Vec::new();
        if overall.is_none() || self.consume_lexeme(",") {
            loop {
                let field = self.parse_field_reference()?;
                let hierarchy = if let Some(only) = self.consume_keyword_token(Keyword::Only) {
                    self.expect_keyword(Keyword::Hierarchy)?;
                    Some(HierarchyTotals {
                        token: only,
                        only: true,
                    })
                } else {
                    self.consume_keyword_token(Keyword::Hierarchy)
                        .map(|token| HierarchyTotals { token, only: false })
                };
                let periods =
                    if let Some(periods_token) = self.consume_keyword_token(Keyword::Periods) {
                        Some(self.parse_periods(periods_token)?)
                    } else {
                        None
                    };
                if self.consume_keyword(Keyword::As) {
                    self.expect_identifier("expected control point alias after AS")?;
                }
                points.push(ControlPoint {
                    field,
                    hierarchy,
                    periods,
                });
                if !self.consume_lexeme(",") {
                    break;
                }
            }
        }
        Ok(Some(TotalsAst {
            token,
            fields,
            overall,
            points,
        }))
    }

    /// Parses `(<период>[, <начало>[, <конец>]])` after `ПЕРИОДАМИ`.
    fn parse_periods(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<PeriodsAst<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let period = self.expect_period_kind(token, &PeriodKind::SHIFT)?;
        let mut bounds = Vec::new();
        while self.consume_lexeme(",") {
            if bounds.len() == 2 {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    "PERIODS accepts a period and at most two bounds",
                ));
            }
            bounds.push(self.parse_or()?);
        }
        self.expect_lexeme(")")?;
        let mut bounds = bounds.into_iter();
        Ok(PeriodsAst {
            token,
            period,
            begin: bounds.next(),
            end: bounds.next(),
        })
    }

    /// Parses the platform's full form of a restriction text: the
    /// leading `ТекущаяТаблица` with its optional alias, the join clauses,
    /// the optional `ГДЕ`, and the condition, which must span the rest.
    pub(super) fn parse_restriction(
        mut self,
    ) -> Result<RestrictionAst<'tokens, 'source>, QueryDiagnostic> {
        let mut alias = None;
        if self
            .peek()
            .is_some_and(|token| is_current_table_token(token))
        {
            self.offset += 1;
            if self.consume_keyword(Keyword::As) {
                alias = Some(self.expect_alias("expected an alias after КАК in the restriction")?);
            }
        }
        let mut joins = Vec::new();
        while let Some(mut group) = self.parse_join()? {
            joins.append(&mut group);
        }
        self.consume_keyword(Keyword::Where);
        if self.peek().is_none() {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                None,
                "restriction condition is empty",
            ));
        }
        let condition = self.parse_or()?;
        if let Some(token) = self.peek() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                format!(
                    "unexpected {:?} after the restriction condition",
                    token.lexeme
                ),
            ));
        }
        Ok(RestrictionAst {
            alias,
            joins,
            condition,
        })
    }

    /// Parses `ИНДЕКСИРОВАТЬ ПО <поля>` and
    /// `ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((<поля>) [УНИКАЛЬНО], …)`. `УНИКАЛЬНО` is
    /// accepted and dropped: common table expressions carry no indexes.
    fn parse_index(&mut self) -> Result<Option<IndexAst<'tokens, 'source>>, QueryDiagnostic> {
        let Some(token) = self.consume_keyword_token(Keyword::Index) else {
            return Ok(None);
        };
        self.expect_keyword(Keyword::By)?;
        let mut sets = Vec::new();
        if self.consume_keyword(Keyword::Sets) {
            self.expect_lexeme("(")?;
            loop {
                self.expect_lexeme("(")?;
                sets.push(self.parse_index_fields()?);
                self.expect_lexeme(")")?;
                self.consume_keyword(Keyword::Unique);
                if !self.consume_lexeme(",") {
                    break;
                }
            }
            self.expect_lexeme(")")?;
        } else {
            sets.push(self.parse_index_fields()?);
            self.consume_keyword(Keyword::Unique);
        }
        Ok(Some(IndexAst { token, sets }))
    }

    /// An index field names a selection-list label; the platform also
    /// accepts it qualified by the source alias (`ИНДЕКСИРОВАТЬ ПО
    /// Т.Поле`), which names the label of the last segment.
    fn parse_index_fields(&mut self) -> Result<Vec<IndexField<'tokens, 'source>>, QueryDiagnostic> {
        let mut fields = Vec::new();
        loop {
            let mut segments = vec![self.expect_identifier("expected index field name")?];
            while self.consume_lexeme(".") {
                segments.push(self.expect_identifier("expected index field name")?);
            }
            fields.push(IndexField { segments });
            if !self.consume_lexeme(",") {
                break;
            }
        }
        Ok(fields)
    }

    /// Parses a parenthesized nested query after its opening parenthesis was
    /// seen but not consumed. Nesting is bounded separately from expression
    /// depth because every nested statement compiles recursively.
    fn parse_nested_query(
        &mut self,
        opening: &'tokens Token<'source>,
    ) -> Result<QueryAst<'tokens, 'source>, QueryDiagnostic> {
        if self.nesting >= Self::MAX_NESTED_QUERIES {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::TooDeep,
                Some(opening),
                format!(
                    "nested query depth exceeds limit of {}",
                    Self::MAX_NESTED_QUERIES
                ),
            ));
        }
        self.nesting += 1;
        self.depth += 1;
        let result = (|| {
            self.expect_lexeme("(")?;
            let query = self.parse_query_body()?;
            if let Some(into) = &query.into {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(into.token),
                    "INTO is not allowed inside a nested query",
                ));
            }
            if let Some(index) = &query.index {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(index.token),
                    "INDEX BY is not allowed inside a nested query",
                ));
            }
            if let Some(allowed) = query.allowed {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(allowed),
                    "ALLOWED applies to the whole statement; write it after the outermost SELECT",
                ));
            }
            self.expect_lexeme(")")?;
            Ok(query)
        })();
        self.depth -= 1;
        self.nesting -= 1;
        result
    }

    /// A bare identifier without a following `.` names a temporary table;
    /// metadata sources are always written as `Вид.Имя`.
    /// `Константы`/`Constants` not followed by `.`: the constants table.
    fn next_is_constants_source(&self) -> bool {
        self.peek().is_some_and(|token| {
            is_contextual_identifier(token.kind)
                && (names_equal(token.lexeme, "Константы")
                    || names_equal(token.lexeme, "Constants"))
        }) && self
            .tokens
            .get(self.offset + 1)
            .is_none_or(|token| token.lexeme != ".")
    }

    fn next_is_temporary_source(&self) -> bool {
        self.peek()
            .is_some_and(|token| is_contextual_identifier(token.kind))
            && self
                .tokens
                .get(self.offset + 1)
                .is_none_or(|token| token.lexeme != ".")
    }

    fn next_is_nested_query(&self) -> bool {
        self.peek().is_some_and(|token| token.lexeme == "(")
            && self
                .tokens
                .get(self.offset + 1)
                .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Select))
    }

    fn repeated_modifier(&self, token: &'tokens Token<'source>) -> QueryDiagnostic {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(token),
            format!("selection modifier {:?} is written twice", token.lexeme),
        )
    }

    fn parse_top_count(&mut self) -> Result<u32, QueryDiagnostic> {
        let token = self.expect_kind(TokenKind::Number, "expected TOP row count")?;
        let value = token.lexeme.parse::<u32>().map_err(|_| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "TOP row count must be an integer",
            )
        })?;
        Ok(value)
    }

    fn parse_select(&mut self) -> Result<SelectAst<'tokens, 'source>, QueryDiagnostic> {
        self.expect_keyword(Keyword::Select)?;
        // The platform accepts the three modifiers in any order, measured
        // on 8.3.27; each of them only once.
        let mut allowed = None;
        let mut distinct = false;
        let mut top = None;
        loop {
            if let Some(token) = self.consume_keyword_token(Keyword::Allowed) {
                if allowed.is_some() {
                    return Err(self.repeated_modifier(token));
                }
                allowed = Some(token);
                continue;
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Distinct) {
                if distinct {
                    return Err(self.repeated_modifier(token));
                }
                distinct = true;
                continue;
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Top) {
                if top.is_some() {
                    return Err(self.repeated_modifier(token));
                }
                top = Some(self.parse_top_count()?);
                continue;
            }
            break;
        }

        let mut projection = Vec::new();
        loop {
            let expression = if self.consume_lexeme("*") {
                Projection::All
            } else {
                self.parse_projection()?
            };
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_alias("expected projection alias after AS")?)
            } else {
                // `КАК` is optional in front of an alias, in a projection
                // as much as after a source.
                self.consume_implicit_alias()
            };
            if alias.is_some() && matches!(expression, Projection::All) {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::UnsupportedFeature,
                    self.peek(),
                    "wildcard projection cannot have an alias",
                ));
            }
            projection.push(ProjectionItem { expression, alias });
            if !self.consume_lexeme(",") {
                break;
            }
        }
        let into = if let Some(token) = self.consume_keyword_token(Keyword::Into) {
            Some(IntoAst {
                token,
                name: self.expect_identifier("expected temporary table name after INTO")?,
                append: false,
            })
        } else if let Some(token) = self.consume_keyword_token(Keyword::Add) {
            Some(IntoAst {
                token,
                name: self.expect_identifier("expected temporary table name after ADD")?,
                append: true,
            })
        } else {
            None
        };
        let source = if self.consume_keyword(Keyword::From) {
            Some(self.parse_source()?)
        } else {
            None
        };
        let mut joins = Vec::new();
        if source.is_some() {
            loop {
                if let Some(group) = self.parse_join()? {
                    for join in group {
                        self.record_binary_operator(join.token)?;
                        joins.push(join);
                    }
                } else if let Some(element) = self.parse_comma_source()? {
                    self.record_binary_operator(element.token)?;
                    joins.push(element);
                } else {
                    break;
                }
            }
        }
        let filter = if self.consume_keyword(Keyword::Where) {
            Some(self.parse_or()?)
        } else {
            None
        };
        let mut group = Vec::new();
        if self.consume_keyword(Keyword::Group) {
            self.expect_keyword(Keyword::By)?;
            loop {
                let token = self.peek().ok_or_else(|| {
                    self.diagnostic(QueryDiagnosticKind::Syntax, None, "expected GROUP BY key")
                })?;
                self.record_binary_operator(token)?;
                let expression = self.parse_or()?;
                group.push(GroupKey { token, expression });
                if !self.consume_lexeme(",") {
                    break;
                }
            }
        }
        let having = if self.consume_keyword(Keyword::Having) {
            Some(self.parse_or()?)
        } else {
            None
        };

        Ok(SelectAst {
            allowed,
            distinct,
            into,
            top,
            projection,
            source,
            joins,
            filter,
            group,
            having,
        })
    }

    /// Parses `<путь>.(Поле, …)` or `<путь>.*`, the two spellings that
    /// name a tabular section outright. Returns `None` when the text is
    /// something else, leaving the cursor for the caller to restore.
    fn parse_tabular_section_projection(
        &mut self,
    ) -> Result<Option<Projection<'tokens, 'source>>, QueryDiagnostic> {
        let Some(first) = self.peek() else {
            return Ok(None);
        };
        if !is_contextual_identifier(first.kind) {
            return Ok(None);
        }
        let mut segments = vec![self.next().expect("peeked token")];
        loop {
            if !self.consume_lexeme(".") {
                return Ok(None);
            }
            let Some(token) = self.peek() else {
                return Ok(None);
            };
            match token.lexeme {
                "*" => {
                    self.next();
                    return Ok(Some(Projection::TabularSection {
                        path: FieldReference { segments },
                        columns: Vec::new(),
                    }));
                }
                "(" => {
                    self.next();
                    let mut columns = Vec::new();
                    loop {
                        columns.push(self.expect_identifier("expected column name")?);
                        // A column of the list may carry an alias, which
                        // names it in the nested result; the section keeps
                        // its own column names, so the alias is read and
                        // dropped, as `Ссылка КАК Ссылка` means nothing
                        // else.
                        if self.consume_keyword(Keyword::As) {
                            self.expect_identifier("expected column alias after AS")?;
                        } else {
                            self.consume_implicit_alias();
                        }
                        if !self.consume_lexeme(",") {
                            break;
                        }
                    }
                    self.expect_lexeme(")")?;
                    return Ok(Some(Projection::TabularSection {
                        path: FieldReference { segments },
                        columns,
                    }));
                }
                _ if is_contextual_identifier(token.kind) => {
                    segments.push(self.next().expect("peeked token"));
                }
                _ => return Ok(None),
            }
        }
    }

    fn parse_projection(&mut self) -> Result<Projection<'tokens, 'source>, QueryDiagnostic> {
        let function = if self.next_lexeme_is("(") {
            self.consume_keyword_token(Keyword::RefPresentation)
                .map(|token| (token, PresentationOperation::Reference))
                .or_else(|| {
                    self.consume_keyword_token(Keyword::Presentation)
                        .map(|token| (token, PresentationOperation::String))
                })
        } else {
            None
        };
        if let Some((token, operation)) = function {
            self.expect_lexeme("(")?;
            let argument = match self.peek() {
                Some(value)
                    if matches!(
                        value.kind,
                        TokenKind::String | TokenKind::Number | TokenKind::Binary
                    ) || matches!(
                        value.kind,
                        TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::Null)
                    ) =>
                {
                    PresentationArgument::Literal(self.next().expect("peeked token"))
                }
                // The platform presents any expression, not only a field.
                _ => {
                    let offset = self.offset;
                    match self.parse_field_reference() {
                        Ok(reference) if self.peek().is_some_and(|token| token.lexeme == ")") => {
                            PresentationArgument::Field(reference)
                        }
                        _ => {
                            self.offset = offset;
                            PresentationArgument::Expression(Box::new(self.parse_or()?))
                        }
                    }
                }
            };
            self.expect_lexeme(")")?;
            return Ok(Projection::Presentation {
                token,
                operation,
                argument,
            });
        }

        // `Состав.(Поле, …)` and `Состав.*` name a tabular section, which
        // the platform answers as a nested result. The bare `Состав` form
        // is indistinguishable from a field here and is recognized during
        // compilation, where metadata is at hand.
        let offset = self.offset;
        match self.parse_tabular_section_projection() {
            Ok(Some(projection)) => return Ok(projection),
            Ok(None) => self.offset = offset,
            Err(error) => return Err(error),
        }

        let expression = self.parse_or()?;
        match expression {
            Expression::Aggregate {
                token,
                kind,
                distinct,
                argument,
            } => Ok(Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            }),
            Expression::Field(mut reference) => {
                if reference.segments.len() > 1
                    && reference.segments.last().is_some_and(|token| {
                        token.kind == TokenKind::Keyword(Keyword::Presentation)
                    })
                {
                    let token = reference.segments.pop().expect("checked last segment");
                    return Ok(Projection::Presentation {
                        token,
                        operation: PresentationOperation::Property,
                        argument: PresentationArgument::Field(reference),
                    });
                }
                Ok(Projection::Field(reference))
            }
            expression => Ok(Projection::Scalar(expression)),
        }
    }

    /// Parses one join and the joins nested inside its source. 1C closes
    /// the conditions in reverse order, so
    /// `A ЛЕВОЕ СОЕДИНЕНИЕ B ЛЕВОЕ СОЕДИНЕНИЕ C ПО <B‑C> ПО <A‑B>` is a
    /// group; it is returned flattened, outer join first, which answers
    /// what the platform answers for the combinations this accepts.
    fn parse_join(&mut self) -> Result<Option<Vec<JoinAst<'tokens, 'source>>>, QueryDiagnostic> {
        let (token, kind) = if let Some(token) = self.consume_keyword_token(Keyword::Inner) {
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Inner)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Left) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Left)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Right) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Right)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Full) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Full)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Join) {
            (token, JoinKind::Inner)
        } else {
            return Ok(None);
        };
        let source = self.parse_source()?;
        let mut nested = Vec::new();
        while let Some(mut group) = self.parse_join()? {
            if let Some(inner) = group.first() {
                check_join_group(kind, inner)?;
            }
            nested.append(&mut group);
        }
        if !self.consume_keyword(Keyword::On) && !self.consume_keyword(Keyword::By) {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek(),
                "expected ON or ПО after JOIN source",
            ));
        }
        let mut group = Vec::with_capacity(nested.len() + 1);
        group.push(JoinAst {
            token,
            kind,
            source,
            condition: Some(self.parse_or()?),
        });
        group.append(&mut nested);
        Ok(Some(group))
    }

    /// Parses `, <source>` after a source element, returning the element as
    /// a condition-less cross join positioned at the comma.
    fn parse_comma_source(&mut self) -> Result<Option<JoinAst<'tokens, 'source>>, QueryDiagnostic> {
        let Some(token) = self.peek().filter(|token| token.lexeme == ",") else {
            return Ok(None);
        };
        self.offset += 1;
        let source = self.parse_source()?;
        Ok(Some(JoinAst {
            token,
            kind: JoinKind::Cross,
            source,
            condition: None,
        }))
    }

    fn parse_source(&mut self) -> Result<SourceAst<'tokens, 'source>, QueryDiagnostic> {
        if self.next_is_nested_query() {
            let opening = self.peek().expect("checked opening parenthesis");
            let nested = self.parse_nested_query(opening)?;
            let alias = if self.consume_keyword(Keyword::As) {
                self.expect_alias("expected source alias after AS")?
            } else if let Some(alias) = self.consume_implicit_alias() {
                alias
            } else {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    "a nested query source requires an alias",
                ));
            };
            return Ok(SourceAst {
                kind: opening,
                object: opening,
                table_part: None,
                slice: None,
                accumulation: None,
                alias: Some(alias),
                nested: Some(Box::new(nested)),
                temporary: false,
                parameter: false,
                constants: false,
                criterion: None,
            });
        }
        if self.next_is_constants_source() {
            let name = self.next().expect("checked identifier");
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_alias("expected source alias after AS")?)
            } else {
                self.consume_implicit_alias()
            };
            return Ok(SourceAst {
                kind: name,
                object: name,
                table_part: None,
                slice: None,
                accumulation: None,
                alias,
                nested: None,
                temporary: false,
                parameter: false,
                constants: true,
                criterion: None,
            });
        }
        if self.next_is_temporary_source() {
            let name = self.next().expect("checked identifier");
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_alias("expected source alias after AS")?)
            } else {
                self.consume_implicit_alias()
            };
            return Ok(SourceAst {
                kind: name,
                object: name,
                table_part: None,
                slice: None,
                accumulation: None,
                alias,
                nested: None,
                temporary: true,
                parameter: false,
                constants: false,
                criterion: None,
            });
        }
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Parameter)
        {
            // `ИЗ &Таблица` reads a value table the application passes.
            let name = self.next().expect("checked parameter");
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_alias("expected source alias after AS")?)
            } else {
                self.consume_implicit_alias()
            };
            return Ok(SourceAst {
                kind: name,
                object: name,
                table_part: None,
                slice: None,
                accumulation: None,
                alias,
                nested: None,
                temporary: false,
                parameter: true,
                constants: false,
                criterion: None,
            });
        }
        let kind = self.expect_identifier("expected metadata kind after FROM")?;
        self.expect_lexeme(".")?;
        let object = self.expect_identifier("expected metadata object name")?;
        // `КритерийОтбора.<Имя>(<значение>)` carries the value it searches
        // for directly after the name.
        if is_filter_criterion_kind(kind.lexeme) {
            self.expect_lexeme("(")?;
            let value = self.parse_or()?;
            self.expect_lexeme(")")?;
            let alias = if self.consume_keyword(Keyword::As) {
                Some(self.expect_alias("expected source alias after AS")?)
            } else {
                self.consume_implicit_alias()
            };
            return Ok(SourceAst {
                kind,
                object,
                table_part: None,
                slice: None,
                accumulation: None,
                alias,
                nested: None,
                temporary: false,
                parameter: false,
                constants: false,
                criterion: Some(value),
            });
        }
        let (table_part, slice, accumulation) = if self.consume_lexeme(".") {
            // The platform writes a virtual table without its argument
            // list when every argument is left out.
            if let Some(token) = self.consume_keyword_token(Keyword::SliceLast) {
                (None, Some(self.parse_slice(token, SliceKind::Last)?), None)
            } else if let Some(token) = self.consume_keyword_token(Keyword::SliceFirst) {
                (None, Some(self.parse_slice(token, SliceKind::First)?), None)
            } else if let Some((token, virtual_kind)) = self.consume_virtual_table_keyword() {
                let accounting = kind_of_source_is_accounting(kind.lexeme);
                (
                    None,
                    None,
                    Some(AccumulationAst {
                        token,
                        kind: virtual_kind,
                        arguments: self.parse_virtual_arguments(
                            virtual_kind.argument_count(accounting),
                            virtual_kind.name(),
                        )?,
                    }),
                )
            } else {
                (
                    Some(self.expect_identifier("expected tabular-section name")?),
                    None,
                    None,
                )
            }
        } else {
            (None, None, None)
        };
        let alias = if self.consume_keyword(Keyword::As) {
            Some(self.expect_alias("expected source alias after AS")?)
        } else {
            self.consume_implicit_alias()
        };
        Ok(SourceAst {
            kind,
            object,
            table_part,
            parameter: false,
            nested: None,
            temporary: false,
            constants: false,
            slice,
            accumulation,
            alias,
            criterion: None,
        })
    }

    /// The keyword of an aggregating virtual table, when the next token
    /// is one.
    fn consume_virtual_table_keyword(
        &mut self,
    ) -> Option<(&'tokens Token<'source>, AccumulationKind)> {
        for (keyword, kind) in [
            (Keyword::Balance, AccumulationKind::Balance),
            (
                Keyword::BalanceAndTurnovers,
                AccumulationKind::BalanceAndTurnovers,
            ),
            (Keyword::Turnovers, AccumulationKind::Turnovers),
            (Keyword::DrCrTurnovers, AccumulationKind::DrCrTurnovers),
            (
                Keyword::RecordsWithExtDimensions,
                AccumulationKind::RecordsWithExtDimensions,
            ),
        ] {
            if let Some(token) = self.consume_keyword_token(keyword) {
                return Some((token, kind));
            }
        }
        None
    }

    fn parse_slice(
        &mut self,
        token: &'tokens Token<'source>,
        kind: SliceKind,
    ) -> Result<SliceAst<'tokens, 'source>, QueryDiagnostic> {
        let mut arguments = self.parse_virtual_arguments(2, kind.name())?.into_iter();
        Ok(SliceAst {
            token,
            kind,
            period: arguments.next().flatten(),
            condition: arguments.next().flatten(),
        })
    }

    fn parse_virtual_arguments(
        &mut self,
        maximum: usize,
        name: &str,
    ) -> Result<Vec<Option<Expression<'tokens, 'source>>>, QueryDiagnostic> {
        let mut arguments = Vec::new();
        if !self.consume_lexeme("(") {
            return Ok(arguments);
        }
        if self.consume_lexeme(")") {
            return Ok(arguments);
        }
        loop {
            if arguments.len() == maximum {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    format!("{name} accepts at most {maximum} arguments"),
                ));
            }
            let argument = if self
                .peek()
                .is_some_and(|token| matches!(token.lexeme, "," | ")"))
            {
                None
            } else {
                Some(self.parse_or()?)
            };
            arguments.push(argument);
            if self.consume_lexeme(",") {
                continue;
            }
            self.expect_lexeme(")")?;
            break;
        }
        Ok(arguments)
    }

    fn parse_order(&mut self) -> Result<Vec<OrderTerm<'tokens, 'source>>, QueryDiagnostic> {
        let mut order = Vec::new();
        if self.consume_keyword(Keyword::Order) {
            self.expect_keyword(Keyword::By)?;
            loop {
                let token = self.peek().ok_or_else(|| {
                    QueryDiagnostic::at_kind(
                        QueryDiagnosticKind::Syntax,
                        None,
                        "expected an ORDER BY key",
                    )
                })?;
                // A bare field path keeps naming a projection alias, so the
                // common form is recognized before anything else.
                let key = match self.parse_or()? {
                    Expression::Field(field) => OrderKeyAst::Field(field),
                    expression => OrderKeyAst::Expression(expression),
                };
                let descending = self.peek().is_some_and(|token| {
                    names_equal(token.lexeme, "DESC") || names_equal(token.lexeme, "УБЫВ")
                });
                if descending || self.peek().is_some_and(is_ascending_order) {
                    self.offset += 1;
                }
                order.push(OrderTerm {
                    token,
                    key,
                    descending,
                });
                if !self.consume_lexeme(",") {
                    break;
                }
            }
        }
        // `АВТОУПОРЯДОЧИВАНИЕ` asks the platform to order the result by
        // the presentations of its references; the compiler orders by
        // the keys written and takes the word for nothing more.
        self.consume_keyword(Keyword::AutoOrder);
        Ok(order)
    }

    fn parse_or(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_and()?;
        while let Some(operator) = self.consume_keyword_token(Keyword::Or) {
            self.record_binary_operator(operator)?;
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_and()?),
            };
        }
        Ok(expression)
    }

    /// `НЕ` binds looser than every comparison and tighter than `И`, so it
    /// negates the comparison, `ПОДОБНО`, `В`, `ССЫЛКА` or `ЕСТЬ NULL`
    /// written to its right and groups before a conjunction. Measured on
    /// the platform, where `ГДЕ НЕ Цена = 10` answers every other price.
    /// The negations are consumed here rather than in a level of their own
    /// so that a deep expression does not spend an extra stack frame per
    /// nesting level.
    fn parse_and(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut negations = Vec::new();
        self.consume_negations(&mut negations)?;
        let mut expression = Self::negate(self.parse_comparison()?, &mut negations);
        while let Some(operator) = self.consume_keyword_token(Keyword::And) {
            self.record_binary_operator(operator)?;
            self.consume_negations(&mut negations)?;
            let right = Self::negate(self.parse_comparison()?, &mut negations);
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn consume_negations(
        &mut self,
        operators: &mut Vec<&'tokens Token<'source>>,
    ) -> Result<(), QueryDiagnostic> {
        while let Some(operator) = self.consume_keyword_token(Keyword::Not) {
            if self.depth + operators.len() >= Self::MAX_DEPTH {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::TooDeep,
                    Some(operator),
                    format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
                ));
            }
            operators.push(operator);
        }
        Ok(())
    }

    fn negate(
        mut expression: Expression<'tokens, 'source>,
        operators: &mut Vec<&'tokens Token<'source>>,
    ) -> Expression<'tokens, 'source> {
        for operator in operators.drain(..).rev() {
            expression = Expression::Unary {
                operator,
                value: Box::new(expression),
            };
        }
        expression
    }

    fn parse_comparison(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_additive()?;
        if self.peek().is_some_and(|token| {
            matches!(token.kind, TokenKind::Keyword(Keyword::Like | Keyword::Not))
        }) && let Some(like) = self.parse_like_tail(&mut expression)?
        {
            return Ok(like);
        }
        if let Some(is) = self.consume_keyword_token(Keyword::Is) {
            return self.parse_is_null_tail(expression, is);
        }
        if let Some(between) = self.parse_between_tail(&mut expression)? {
            return Ok(between);
        }
        if let Some(token) = self.consume_keyword_token(Keyword::Refs) {
            return self.parse_refs_tail(expression, token);
        }
        let negated_in = self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Not))
            && self
                .tokens
                .get(self.offset + 1)
                .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::In));
        if negated_in {
            self.offset += 1;
        }
        if let Some(operator) = self.consume_keyword_token(Keyword::In) {
            return self.parse_in_tail(expression, operator, negated_in);
        }
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Operator && is_comparison(token.lexeme))
        {
            let operator = self.next().expect("peeked token");
            self.record_binary_operator(operator)?;
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_additive()?),
            };
        }
        Ok(expression)
    }

    /// The `[НЕ] МЕЖДУ <low> И <high>` tail, when the next tokens spell
    /// it; otherwise the input is left untouched. The bounds are parsed
    /// below the conjunction so that `И` separates them.
    #[inline(never)]
    fn parse_between_tail(
        &mut self,
        value: &mut Expression<'tokens, 'source>,
    ) -> Result<Option<Expression<'tokens, 'source>>, QueryDiagnostic> {
        let negated = self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Not));
        let offset = if negated {
            self.offset + 1
        } else {
            self.offset
        };
        if !self
            .tokens
            .get(offset)
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Between))
        {
            return Ok(None);
        }
        self.offset = offset;
        let token = self.next().expect("checked BETWEEN keyword");
        self.record_binary_operator(token)?;
        let low = self.parse_additive()?;
        self.expect_keyword(Keyword::And)?;
        let high = self.parse_additive()?;
        let value = std::mem::replace(value, Expression::Literal(token));
        Ok(Some(Expression::Between {
            token,
            value: Box::new(value),
            low: Box::new(low),
            high: Box::new(high),
            negated,
        }))
    }

    /// The `ССЫЛКА <Вид>.<Объект>` tail.
    #[inline(never)]
    fn parse_refs_tail(
        &mut self,
        value: Expression<'tokens, 'source>,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.record_binary_operator(token)?;
        let kind = self.expect_identifier("REFS expects a metadata kind")?;
        self.expect_lexeme(".")?;
        let object = self.expect_identifier("REFS expects a metadata object")?;
        Ok(Expression::Refs {
            token,
            value: Box::new(value),
            kind,
            object,
        })
    }

    /// The `ЕСТЬ [НЕ] NULL` tail. Kept out of `parse_comparison` so that its
    /// locals stay off the frame of the deep expression recursion.
    #[inline(never)]
    fn parse_is_null_tail(
        &mut self,
        expression: Expression<'tokens, 'source>,
        is: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let negated = self.consume_keyword(Keyword::Not);
        if !self.consume_keyword(Keyword::Null) {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek().or(Some(is)),
                "IS only supports NULL in this query subset",
            ));
        }
        Ok(Expression::IsNull {
            value: Box::new(expression),
            negated,
        })
    }

    /// The `[НЕ] В (…)` tail, either a value list or a nested query. Kept out
    /// of `parse_comparison` for the same reason.
    #[inline(never)]
    fn parse_in_tail(
        &mut self,
        expression: Expression<'tokens, 'source>,
        operator: &'tokens Token<'source>,
        negated: bool,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        // `В ИЕРАРХИИ` spells the genitive of the totals keyword, so the
        // word is matched by lexeme the way `ВОЗР`/`УБЫВ` are.
        let hierarchy = self.peek().is_some_and(|token| {
            token.kind == TokenKind::Keyword(Keyword::Hierarchy)
                || names_equal(token.lexeme, "ИЕРАРХИИ")
        });
        if hierarchy {
            self.offset += 1;
        }
        if self.next_is_nested_query() {
            let opening = self.peek().expect("checked opening parenthesis");
            self.record_binary_operator(operator)?;
            let query = self.parse_nested_query(opening)?;
            return Ok(Expression::InQuery {
                token: operator,
                value: Box::new(expression),
                query: Box::new(query),
                negated,
                hierarchy,
            });
        }
        self.expect_lexeme("(")?;
        if self.consume_lexeme(")") {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::Syntax,
                Some(operator),
                "IN list must contain at least one expression",
            ));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.parse_additive()?);
            if !self.consume_lexeme(",") {
                break;
            }
            if self.peek().is_some_and(|token| token.lexeme == ")") {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    "expected expression after ',' in IN list",
                ));
            }
        }
        self.expect_lexeme(")")?;
        Ok(Expression::InList {
            token: operator,
            value: Box::new(expression),
            items,
            negated,
            hierarchy,
        })
    }

    fn parse_additive(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_multiplicative()?;
        while self
            .peek()
            .is_some_and(|token| matches!(token.lexeme, "+" | "-"))
        {
            let operator = self.next().expect("peeked token");
            self.record_binary_operator(operator)?;
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_multiplicative()?),
            };
        }
        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_unary()?;
        while self
            .peek()
            .is_some_and(|token| matches!(token.lexeme, "*" | "/"))
        {
            let operator = self.next().expect("peeked token");
            self.record_binary_operator(operator)?;
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_unary()?),
            };
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut operators = Vec::new();
        loop {
            let operator = self
                .peek()
                .is_some_and(|token| matches!(token.lexeme, "+" | "-"))
                .then(|| self.next().expect("peeked token"));
            let Some(operator) = operator else {
                break;
            };
            if self.depth + operators.len() >= Self::MAX_DEPTH {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::TooDeep,
                    Some(operator),
                    format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
                ));
            }
            operators.push(operator);
        }
        let mut expression = self.parse_primary()?;
        for operator in operators.into_iter().rev() {
            expression = Expression::Unary {
                operator,
                value: Box::new(expression),
            };
        }
        Ok(expression)
    }

    fn parse_primary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if let Some(token) = self.consume_keyword_token(Keyword::Case) {
            return self.parse_case(token);
        }
        if self.peek().is_some_and(|token| token.lexeme == "(") {
            let opening = self.next().expect("peeked token");
            if self.depth >= Self::MAX_DEPTH {
                return Err(QueryDiagnostic::at_kind(
                    QueryDiagnosticKind::TooDeep,
                    Some(opening),
                    format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
                ));
            }
            self.depth += 1;
            let result = (|| {
                let expression = self.parse_or()?;
                if self.peek().is_some_and(|token| token.lexeme == ",") {
                    // `(А, Б) В (ВЫБРАТЬ …)`: a tuple, accepted only as
                    // the left side of a membership test.
                    let mut items = vec![expression];
                    while self.consume_lexeme(",") {
                        items.push(self.parse_or()?);
                    }
                    self.expect_lexeme(")")?;
                    return Ok(Expression::Tuple {
                        token: opening,
                        items,
                    });
                }
                self.expect_lexeme(")")?;
                Ok(expression)
            })();
            self.depth -= 1;
            return result;
        }
        if self.next_lexeme_is("(") {
            if let Some(token) = self.consume_keyword_token(Keyword::DateTime) {
                return self.parse_datetime(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::BeginOfPeriod) {
                return self.parse_period_boundary(token, false);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::EndOfPeriod) {
                return self.parse_period_boundary(token, true);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::DateAdd) {
                return self.parse_date_add(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::DateDiff) {
                return self.parse_date_diff(token);
            }
            if let Some((token, part)) = self.consume_date_part_keyword() {
                return self.parse_date_part(token, part);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Value) {
                return self.parse_metadata_value(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Uuid) {
                return self.parse_uuid(token);
            }
            if let Some((token, function)) = self.consume_scalar_function() {
                return self.parse_scalar_function(token, function);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Type) {
                return self.parse_type_literal(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::ValueType) {
                return self.parse_value_type(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Cast) {
                return self.parse_cast(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::IsNullFunction) {
                return self.parse_is_null_function(token);
            }
            if let Some((token, kind)) = self.consume_aggregate_keyword() {
                return self.parse_aggregate(token, kind);
            }
        }
        let Some(token) = self.peek() else {
            return Err(self.diagnostic(QueryDiagnosticKind::Syntax, None, "expected expression"));
        };
        if token.kind == TokenKind::Parameter {
            return Ok(Expression::Parameter(self.next().expect("peeked token")));
        }
        if matches!(
            token.kind,
            TokenKind::String | TokenKind::Number | TokenKind::Binary
        ) || matches!(
            token.kind,
            TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::Null | Keyword::Undefined)
        ) {
            return Ok(Expression::Literal(self.next().expect("peeked token")));
        }
        Ok(Expression::Field(self.parse_field_reference()?))
    }

    /// The scalar function opened by the next token, if any. `Лев` and
    /// `Прав` share their English spelling with the join keywords, so the
    /// Russian names are matched by lexeme and the keywords are read as
    /// functions only here, where a join can never appear.
    fn consume_scalar_function(&mut self) -> Option<(&'tokens Token<'source>, ScalarFunction)> {
        let token = self.peek()?;
        let function = match token.kind {
            TokenKind::Keyword(Keyword::RecordAutoNumber) => ScalarFunction::RecordAutoNumber,
            TokenKind::Keyword(Keyword::Substring) => ScalarFunction::Substring,
            TokenKind::Keyword(Keyword::StringLength) => ScalarFunction::StringLength,
            TokenKind::Keyword(Keyword::TrimAll) => ScalarFunction::TrimAll,
            TokenKind::Keyword(Keyword::TrimLeft) => ScalarFunction::TrimLeft,
            TokenKind::Keyword(Keyword::TrimRight) => ScalarFunction::TrimRight,
            TokenKind::Keyword(Keyword::Upper) => ScalarFunction::Upper,
            TokenKind::Keyword(Keyword::Lower) => ScalarFunction::Lower,
            TokenKind::Keyword(Keyword::StrFind) => ScalarFunction::StrFind,
            TokenKind::Keyword(Keyword::StrReplace) => ScalarFunction::StrReplace,
            TokenKind::Keyword(Keyword::Round) => ScalarFunction::Round,
            TokenKind::Keyword(Keyword::Int) => ScalarFunction::Int,
            TokenKind::Keyword(Keyword::Sqrt) => ScalarFunction::Sqrt,
            TokenKind::Keyword(Keyword::Exp) => ScalarFunction::Exp,
            TokenKind::Keyword(Keyword::Log) => ScalarFunction::Log,
            TokenKind::Keyword(Keyword::Log10) => ScalarFunction::Log10,
            TokenKind::Keyword(Keyword::Pow) => ScalarFunction::Pow,
            TokenKind::Keyword(Keyword::Cos) => ScalarFunction::Cos,
            TokenKind::Keyword(Keyword::Sin) => ScalarFunction::Sin,
            TokenKind::Keyword(Keyword::Tan) => ScalarFunction::Tan,
            TokenKind::Keyword(Keyword::ACos) => ScalarFunction::ACos,
            TokenKind::Keyword(Keyword::ASin) => ScalarFunction::ASin,
            TokenKind::Keyword(Keyword::ATan) => ScalarFunction::ATan,
            TokenKind::Keyword(Keyword::Left) => ScalarFunction::Left,
            TokenKind::Keyword(Keyword::Right) => ScalarFunction::Right,
            TokenKind::Identifier if names_equal(token.lexeme, "ЛЕВ") => ScalarFunction::Left,
            TokenKind::Identifier if names_equal(token.lexeme, "ПРАВ") => ScalarFunction::Right,
            _ => return None,
        };
        self.next().map(|token| (token, function))
    }

    /// The argument list of a scalar function after its name.
    fn parse_scalar_function(
        &mut self,
        token: &'tokens Token<'source>,
        function: ScalarFunction,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let mut arguments = Vec::new();
        // `АВТОНОМЕРЗАПИСИ()` takes nothing; the others at least one.
        if !self.peek().is_some_and(|token| token.lexeme == ")") {
            loop {
                arguments.push(self.parse_or()?);
                if !self.consume_lexeme(",") {
                    break;
                }
            }
        }
        self.expect_lexeme(")")?;
        let (low, high) = function.arity();
        if arguments.len() < low || arguments.len() > high {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                format!(
                    "{} takes {} arguments, found {}",
                    function.name(),
                    if low == high {
                        low.to_string()
                    } else {
                        format!("{low} or {high}")
                    },
                    arguments.len()
                ),
            ));
        }
        Ok(Expression::ScalarFunction {
            token,
            function,
            arguments,
        })
    }

    /// `ТИП(Строка | Число | Дата | Булево | <Вид>.<Объект>)`.
    fn parse_type_literal(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let first = self.expect_identifier("TYPE expects a type name")?;
        let name = if self.consume_lexeme(".") {
            let object = self.expect_identifier("TYPE expects a metadata object")?;
            TypeName::Object {
                kind: first,
                object,
            }
        } else if let Some(primitive) = PrimitiveType::from_query_name(first.lexeme) {
            TypeName::Primitive(primitive)
        } else {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(first),
                format!(
                    "TYPE expects Строка, Число, Дата, Булево, or <Kind>.<Object>, found {:?}",
                    first.lexeme
                ),
            ));
        };
        self.expect_lexeme(")")?;
        Ok(Expression::TypeLiteral { token, name })
    }

    /// `ТИПЗНАЧЕНИЯ(<выражение>)`.
    fn parse_value_type(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let argument = self.parse_or()?;
        if self.peek().is_some_and(|next| next.lexeme == ",") {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "VALUETYPE expects exactly one argument",
            ));
        }
        self.expect_lexeme(")")?;
        Ok(Expression::ValueType {
            token,
            argument: Box::new(argument),
        })
    }

    /// Parses `[НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ <escape>]` after the left
    /// operand when the next tokens spell it; otherwise leaves the input
    /// untouched. Kept out of `parse_comparison` so deep expression nesting
    /// does not grow that frame.
    #[inline(never)]
    fn parse_like_tail(
        &mut self,
        value: &mut Expression<'tokens, 'source>,
    ) -> Result<Option<Expression<'tokens, 'source>>, QueryDiagnostic> {
        let negated = self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Not));
        let like_offset = if negated {
            self.offset + 1
        } else {
            self.offset
        };
        if !self
            .tokens
            .get(like_offset)
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Like))
        {
            return Ok(None);
        }
        self.offset = like_offset;
        let token = self.next().expect("checked LIKE keyword");
        self.record_binary_operator(token)?;
        let pattern = self.parse_additive()?;
        let escape = if self.consume_keyword(Keyword::Escape) {
            Some(Box::new(self.parse_additive()?))
        } else {
            None
        };
        let value = std::mem::replace(value, Expression::Literal(token));
        Ok(Some(Expression::Like {
            token,
            value: Box::new(value),
            pattern: Box::new(pattern),
            escape,
            negated,
        }))
    }

    fn consume_aggregate_keyword(&mut self) -> Option<(&'tokens Token<'source>, AggregateKind)> {
        let kind = match self.peek()?.kind {
            TokenKind::Keyword(Keyword::Count) => AggregateKind::Count,
            TokenKind::Keyword(Keyword::Sum) => AggregateKind::Sum,
            TokenKind::Keyword(Keyword::Min) => AggregateKind::Min,
            TokenKind::Keyword(Keyword::Max) => AggregateKind::Max,
            TokenKind::Keyword(Keyword::Avg) => AggregateKind::Avg,
            _ => return None,
        };
        self.next().map(|token| (token, kind))
    }

    /// Parses the argument list of an aggregate after its keyword.
    fn parse_aggregate(
        &mut self,
        token: &'tokens Token<'source>,
        kind: AggregateKind,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let distinct = self.consume_keyword(Keyword::Distinct);
        if distinct && kind != AggregateKind::Count {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "DISTINCT aggregate argument is currently supported only by COUNT",
            ));
        }
        let argument = if self.consume_lexeme("*") {
            if kind != AggregateKind::Count {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "wildcard aggregate argument is supported only by COUNT",
                ));
            }
            if distinct {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "COUNT(DISTINCT *) is not supported",
                ));
            }
            AggregateArgument::All
        } else {
            AggregateArgument::Expression(Box::new(self.parse_or()?))
        };
        self.expect_lexeme(")")?;
        Ok(Expression::Aggregate {
            token,
            kind,
            distinct,
            argument,
        })
    }

    /// Parses `ВЫБОР КОГДА <predicate> ТОГДА <value> … [ИНАЧЕ <value>] КОНЕЦ`
    /// after the `ВЫБОР` keyword. Every alternative counts against the
    /// binary-operator budget and the whole expression against the depth
    /// limit.
    fn parse_case(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::TooDeep,
                Some(token),
                format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
            ));
        }
        self.depth += 1;
        let result = (|| {
            // `ВЫБОР <выражение> КОГДА <значение> ТОГДА …` compares every
            // alternative with one value, as the platform does.
            let subject = if self
                .peek()
                .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::When))
            {
                None
            } else {
                Some(Box::new(self.parse_or()?))
            };
            let mut branches = Vec::new();
            while let Some(when_token) = self.consume_keyword_token(Keyword::When) {
                self.record_binary_operator(when_token)?;
                let when = self.parse_or()?;
                self.expect_keyword(Keyword::Then)?;
                let then = self.parse_or()?;
                branches.push(CaseBranch {
                    token: when_token,
                    when,
                    then,
                });
            }
            if branches.is_empty() {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek().or(Some(token)),
                    "CASE requires at least one WHEN alternative",
                ));
            }
            let otherwise = if self.consume_keyword(Keyword::Else) {
                Some(Box::new(self.parse_or()?))
            } else {
                None
            };
            self.expect_keyword(Keyword::End)?;
            Ok(Expression::Case {
                token,
                subject,
                branches,
                otherwise,
            })
        })();
        self.depth -= 1;
        result
    }

    /// Parses `ЕСТЬNULL(<value>, <fallback>)` after the keyword.
    fn parse_is_null_function(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.record_binary_operator(token)?;
        self.expect_lexeme("(")?;
        let value = self.parse_or()?;
        if !self.consume_lexeme(",") {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek(),
                "ISNULL requires two arguments",
            ));
        }
        let fallback = self.parse_or()?;
        self.expect_lexeme(")")?;
        Ok(Expression::IsNullFunction {
            token,
            value: Box::new(value),
            fallback: Box::new(fallback),
        })
    }

    fn parse_datetime(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let mut arguments = Vec::with_capacity(6);
        loop {
            let argument = self.expect_kind(
                TokenKind::Number,
                "DATETIME components must be integer literals",
            )?;
            if argument.lexeme.contains('.') {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(argument),
                    "DATETIME components must be integer literals",
                ));
            }
            arguments.push(argument);
            if !self.consume_lexeme(",") {
                break;
            }
            if arguments.len() == 6 {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    "DATETIME accepts at most 6 components",
                ));
            }
        }
        self.expect_lexeme(")")?;
        let value = parse_datetime_value(token, &arguments)?;
        Ok(Expression::DateTime { token, value })
    }

    /// Parses `НАЧАЛОПЕРИОДА(<date>, <period>)` or `КОНЕЦПЕРИОДА(<date>,
    /// <period>)` after the keyword.
    fn parse_period_boundary(
        &mut self,
        token: &'tokens Token<'source>,
        end: bool,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.enter_function(token)?;
        let result = (|| {
            self.expect_lexeme("(")?;
            let value = self.parse_or()?;
            self.expect_lexeme(",")?;
            let period = self.expect_period_kind(token, &PeriodKind::BOUNDARY)?;
            self.expect_lexeme(")")?;
            let value = Box::new(value);
            Ok(if end {
                Expression::EndOfPeriod {
                    token,
                    value,
                    period,
                }
            } else {
                Expression::BeginOfPeriod {
                    token,
                    value,
                    period,
                }
            })
        })();
        self.depth -= 1;
        result
    }

    /// Parses `ДОБАВИТЬКДАТЕ(<date>, <period>, <count>)` after the keyword.
    fn parse_date_add(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.enter_function(token)?;
        let result = (|| {
            self.expect_lexeme("(")?;
            let value = self.parse_or()?;
            self.expect_lexeme(",")?;
            let period = self.expect_period_kind(token, &PeriodKind::SHIFT)?;
            self.expect_lexeme(",")?;
            let count = self.parse_or()?;
            self.expect_lexeme(")")?;
            Ok(Expression::DateAdd {
                token,
                value: Box::new(value),
                period,
                count: Box::new(count),
            })
        })();
        self.depth -= 1;
        result
    }

    /// Parses `РАЗНОСТЬДАТ(<from>, <to>, <unit>)` after the keyword.
    fn parse_date_diff(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.enter_function(token)?;
        let result = (|| {
            self.expect_lexeme("(")?;
            let from = self.parse_or()?;
            self.expect_lexeme(",")?;
            let to = self.parse_or()?;
            self.expect_lexeme(",")?;
            let period = self.expect_period_kind(token, &PeriodKind::DIFFERENCE)?;
            self.expect_lexeme(")")?;
            Ok(Expression::DateDiff {
                token,
                from: Box::new(from),
                to: Box::new(to),
                period,
            })
        })();
        self.depth -= 1;
        result
    }

    fn consume_date_part_keyword(&mut self) -> Option<(&'tokens Token<'source>, DatePart)> {
        let part = match self.peek()?.kind {
            TokenKind::Keyword(Keyword::Year) => DatePart::Year,
            TokenKind::Keyword(Keyword::Quarter) => DatePart::Quarter,
            TokenKind::Keyword(Keyword::Month) => DatePart::Month,
            TokenKind::Keyword(Keyword::DayOfYear) => DatePart::DayOfYear,
            TokenKind::Keyword(Keyword::Day) => DatePart::Day,
            TokenKind::Keyword(Keyword::Week) => DatePart::Week,
            TokenKind::Keyword(Keyword::WeekDay) => DatePart::WeekDay,
            TokenKind::Keyword(Keyword::Hour) => DatePart::Hour,
            TokenKind::Keyword(Keyword::Minute) => DatePart::Minute,
            TokenKind::Keyword(Keyword::Second) => DatePart::Second,
            _ => return None,
        };
        self.next().map(|token| (token, part))
    }

    /// Parses `ГОД(<date>)` and the other date-part functions after the
    /// keyword.
    fn parse_date_part(
        &mut self,
        token: &'tokens Token<'source>,
        part: DatePart,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.enter_function(token)?;
        let result = (|| {
            self.expect_lexeme("(")?;
            let value = self.parse_or()?;
            self.expect_lexeme(")")?;
            Ok(Expression::DatePart {
                token,
                part,
                value: Box::new(value),
            })
        })();
        self.depth -= 1;
        result
    }

    /// Charges one nesting level for a function whose arguments recurse into
    /// expressions; the caller restores the depth after parsing.
    fn enter_function(&mut self, token: &'tokens Token<'source>) -> Result<(), QueryDiagnostic> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TooDeep,
                Some(token),
                format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
            ));
        }
        self.depth += 1;
        Ok(())
    }

    /// The period-kind argument of a date function: an unknown name is an
    /// `UnsupportedFeature`, a known period the function does not accept is
    /// a `Syntax` diagnostic. Extracted so that its diagnostics do not
    /// enlarge the frame of the deep expression recursion.
    #[inline(never)]
    fn expect_period_kind(
        &mut self,
        function: &Token<'_>,
        allowed: &[PeriodKind],
    ) -> Result<PeriodKind, QueryDiagnostic> {
        let name = match function.kind {
            TokenKind::Keyword(keyword) => keyword.as_str(),
            _ => "date function",
        };
        let token = self.expect_identifier("expected period kind after ','")?;
        let period = PeriodKind::from_name(token.lexeme).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!("unsupported {name} period {:?}", token.lexeme),
            )
        })?;
        if !allowed.contains(&period) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                format!(
                    "{name} does not accept the {} period",
                    period.display_name()
                ),
            ));
        }
        Ok(period)
    }

    fn parse_metadata_value(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let kind = self.expect_metadata_name("VALUE expects a metadata kind")?;
        self.expect_lexeme(".")?;
        if let Some(values) = system_enumeration(kind.lexeme) {
            let value = self.expect_metadata_name("VALUE expects a system enumeration value")?;
            let code = values
                .iter()
                .find(|(names, _)| names.iter().any(|name| names_equal(name, value.lexeme)))
                .map(|(_, code)| *code)
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Syntax,
                        Some(value),
                        format!("{} has no value {:?}", kind.lexeme, value.lexeme),
                    )
                })?;
            self.expect_lexeme(")")?;
            return Ok(Expression::SystemValue {
                token,
                enumeration: kind,
                value,
                code,
            });
        }
        let object = self.expect_metadata_name("VALUE expects a metadata object")?;
        self.expect_lexeme(".")?;
        let value = self.expect_metadata_name("VALUE expects a predefined value")?;
        self.expect_lexeme(")")?;
        Ok(Expression::MetadataValue {
            token,
            kind,
            object,
            value,
        })
    }

    fn parse_uuid(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let argument = self.parse_field_reference()?;
        if self.peek().is_some_and(|next| next.lexeme == ",") {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "UUID expects exactly one reference field",
            ));
        }
        self.expect_lexeme(")")?;
        Ok(Expression::Uuid { token, argument })
    }

    fn parse_cast(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let argument = self.parse_or()?;
        self.expect_keyword(Keyword::As)?;
        let target = self.parse_cast_target()?;
        self.expect_lexeme(")")?;
        let path = if matches!(target, CastTarget::Reference { .. }) && self.consume_lexeme(".") {
            let field = self.expect_identifier("expected field name after '.'")?;
            if let Some(next) = self.peek()
                && next.lexeme == "."
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(next),
                    "reference paths deeper than one hop are not supported after CAST",
                ));
            }
            Some(field)
        } else {
            None
        };
        Ok(Expression::Cast {
            token,
            argument: Box::new(argument),
            target,
            path,
        })
    }

    fn parse_cast_target(&mut self) -> Result<CastTarget<'tokens, 'source>, QueryDiagnostic> {
        let name = self.expect_identifier("CAST expects a target type")?;
        let is = |candidates: [&str; 2]| {
            candidates
                .iter()
                .any(|candidate| names_equal(name.lexeme, candidate))
        };
        if is(["СТРОКА", "STRING"]) {
            let parameters = self.parse_cast_parameters(name, 1)?;
            return Ok(CastTarget::String {
                length: parameters.first().copied(),
            });
        }
        if is(["ЧИСЛО", "NUMBER"]) {
            let parameters = self.parse_cast_parameters(name, 2)?;
            let narrow = |value: Option<&u32>| value.and_then(|value| u8::try_from(*value).ok());
            return Ok(CastTarget::Number {
                precision: narrow(parameters.first()),
                scale: narrow(parameters.get(1)),
            });
        }
        if is(["БУЛЕВО", "BOOLEAN"]) {
            return Ok(CastTarget::Boolean);
        }
        if is(["ДАТА", "DATE"]) {
            return Ok(CastTarget::Date);
        }
        if kind_from_query_name(name.lexeme).is_some() && self.consume_lexeme(".") {
            let object = self.expect_identifier("CAST expects a metadata object")?;
            return Ok(CastTarget::Reference { kind: name, object });
        }
        Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(name),
            format!(
                "unsupported CAST target {:?}; expected STRING(n), NUMBER(p, s), BOOLEAN, DATE, or <Kind>.<Object>",
                name.lexeme
            ),
        ))
    }

    /// Parses an optional `(n[, m])` parameter list of a scalar cast target.
    fn parse_cast_parameters(
        &mut self,
        target: &'tokens Token<'source>,
        limit: usize,
    ) -> Result<Vec<u32>, QueryDiagnostic> {
        let mut parameters = Vec::new();
        if !self.consume_lexeme("(") {
            return Ok(parameters);
        }
        loop {
            let number = self.peek().ok_or_else(|| {
                self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    None,
                    "CAST target expects a numeric parameter",
                )
            })?;
            let value = (number.kind == TokenKind::Number)
                .then(|| number.lexeme.parse::<u32>().ok())
                .flatten()
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Syntax,
                        Some(number),
                        "CAST target expects a non-negative integer parameter",
                    )
                })?;
            self.next();
            parameters.push(value);
            if parameters.len() > limit {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(target),
                    format!(
                        "CAST target {:?} accepts at most {limit} parameters",
                        target.lexeme
                    ),
                ));
            }
            if !self.consume_lexeme(",") {
                break;
            }
        }
        self.expect_lexeme(")")?;
        Ok(parameters)
    }

    fn parse_field_reference(
        &mut self,
    ) -> Result<FieldReference<'tokens, 'source>, QueryDiagnostic> {
        let mut segments = vec![self.expect_identifier("expected field name")?];
        while self.consume_lexeme(".") {
            let token = self.peek().ok_or_else(|| {
                self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    None,
                    "expected field name after '.'",
                )
            })?;
            // `Состав.(Поле, …)` and `Состав.*` ask for the tabular
            // section as a nested result inside one column, which one SQL
            // statement cannot return.
            if matches!(token.lexeme, "(" | "*") {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "a tabular section as a nested result of the selection is not supported",
                ));
            }
            if !is_contextual_identifier(token.kind) {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(token),
                    "expected field name after '.'",
                ));
            }
            segments.push(self.next().expect("peeked token"));
        }
        Ok(FieldReference { segments })
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), QueryDiagnostic> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek(),
                format!("expected {}", keyword.as_str()),
            ))
        }
    }

    /// A name inside `ЗНАЧЕНИЕ(…)`, where nothing but a name may appear,
    /// so a name the lexer reads as a keyword is accepted: real
    /// configurations name an enumeration value `НеОпределено`.
    fn expect_metadata_name(
        &mut self,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        if !is_contextual_identifier(token.kind) && !matches!(token.kind, TokenKind::Keyword(_)) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
        }
        Ok(self.next().expect("peeked token"))
    }

    fn expect_identifier(
        &mut self,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        if !is_contextual_identifier(token.kind) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
        }
        Ok(self.next().expect("peeked token"))
    }

    /// Takes the alias after `КАК`: an identifier, or a keyword that does
    /// not open the next clause — the platform accepts `КАК Конец` once
    /// `КАК` marks the word as a name.
    fn expect_alias(
        &mut self,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        let opens_a_clause = matches!(
            token.kind,
            TokenKind::Keyword(
                Keyword::Select
                    | Keyword::From
                    | Keyword::Where
                    | Keyword::Group
                    | Keyword::Having
                    | Keyword::Order
                    | Keyword::Union
                    | Keyword::Into
                    | Keyword::Join
                    | Keyword::Left
                    | Keyword::Right
                    | Keyword::Full
                    | Keyword::Inner
                    | Keyword::Outer
                    | Keyword::On
                    | Keyword::Totals
                    | Keyword::Index
                    | Keyword::AutoOrder
            )
        );
        if opens_a_clause || !matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_)) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
        }
        Ok(self.next().expect("peeked token"))
    }

    /// Takes an alias written without `КАК`, if the next word may be one.
    fn consume_implicit_alias(&mut self) -> Option<&'tokens Token<'source>> {
        if self
            .peek()
            .is_some_and(|token| is_implicit_alias(token.kind))
        {
            return self.next();
        }
        None
    }

    fn expect_kind(
        &mut self,
        kind: TokenKind,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        if token.kind != kind {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
        }
        Ok(self.next().expect("peeked token"))
    }

    fn expect_lexeme(&mut self, lexeme: &str) -> Result<(), QueryDiagnostic> {
        if self.consume_lexeme(lexeme) {
            Ok(())
        } else {
            Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek(),
                format!("expected {lexeme:?}"),
            ))
        }
    }

    fn consume_keyword(&mut self, keyword: Keyword) -> bool {
        self.consume_keyword_token(keyword).is_some()
    }

    fn consume_keyword_token(&mut self, keyword: Keyword) -> Option<&'tokens Token<'source>> {
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(keyword))
        {
            self.next()
        } else {
            None
        }
    }

    fn consume_lexeme(&mut self, lexeme: &str) -> bool {
        if self.peek().is_some_and(|token| token.lexeme == lexeme) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn record_binary_operator(
        &mut self,
        operator: &'tokens Token<'source>,
    ) -> Result<(), QueryDiagnostic> {
        if self.binary_operators >= Self::MAX_BINARY_OPERATORS {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::TooDeep,
                Some(operator),
                format!(
                    "query nesting depth exceeds limit of {} binary operators",
                    Self::MAX_BINARY_OPERATORS
                ),
            ));
        }
        self.binary_operators += 1;
        Ok(())
    }

    fn diagnostic(
        &self,
        kind: QueryDiagnosticKind,
        token: Option<&Token<'_>>,
        message: impl Into<String>,
    ) -> QueryDiagnostic {
        match token {
            Some(token) => QueryDiagnostic::at(kind, Some(token), message),
            None => QueryDiagnostic::at_position(kind, self.eof, message),
        }
    }

    fn peek(&self) -> Option<&'tokens Token<'source>> {
        self.tokens.get(self.offset)
    }

    fn next_lexeme_is(&self, lexeme: &str) -> bool {
        self.tokens
            .get(self.offset + 1)
            .is_some_and(|token| token.lexeme == lexeme)
    }

    fn next(&mut self) -> Option<&'tokens Token<'source>> {
        let token = self.tokens.get(self.offset)?;
        self.offset += 1;
        Some(token)
    }
}
