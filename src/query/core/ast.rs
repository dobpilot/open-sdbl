//! Tokens-borrowing query AST.

use crate::Token;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

/// A sequence of `;`-separated statements compiled as one SQL statement.
#[derive(Debug)]
pub(super) struct BatchAst<'tokens, 'source> {
    pub(super) statements: Vec<StatementAst<'tokens, 'source>>,
}

/// One statement of a batch.
#[derive(Debug)]
pub(super) enum StatementAst<'tokens, 'source> {
    /// A query, optionally placing its rows into a temporary table.
    Query(QueryAst<'tokens, 'source>),
    /// `УНИЧТОЖИТЬ <Имя>`, reported at its name token.
    Drop { name: &'tokens Token<'source> },
}

#[derive(Debug)]
pub(super) struct QueryAst<'tokens, 'source> {
    pub(super) branches: Vec<SelectAst<'tokens, 'source>>,
    pub(super) unions: Vec<UnionLink<'tokens, 'source>>,
    pub(super) order: Vec<OrderTerm<'tokens, 'source>>,
    /// `ПОМЕСТИТЬ`/`ДОБАВИТЬ` hoisted from the first branch.
    pub(super) into: Option<IntoAst<'tokens, 'source>>,
    /// `РАЗРЕШЕННЫЕ` hoisted from the first branch; applies to every
    /// source the statement reads, nested queries included.
    pub(super) allowed: Option<&'tokens Token<'source>>,
    /// A trailing `ИНДЕКСИРОВАТЬ ПО` clause, validated but not generated.
    pub(super) index: Option<IndexAst<'tokens, 'source>>,
}

/// `ПОМЕСТИТЬ <Имя>` or `ДОБАВИТЬ <Имя>`.
#[derive(Debug)]
pub(super) struct IntoAst<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) name: &'tokens Token<'source>,
    /// `true` for `ДОБАВИТЬ`, which appends to an existing table.
    pub(super) append: bool,
}

/// `ИНДЕКСИРОВАТЬ ПО [НАБОРАМ]` with its field sets. `УНИКАЛЬНО` is parsed
/// and dropped because no index is generated.
#[derive(Debug)]
pub(super) struct IndexAst<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) sets: Vec<Vec<&'tokens Token<'source>>>,
}

#[derive(Debug)]
pub(super) struct UnionLink<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) all: bool,
}

#[derive(Debug)]
pub(super) struct SelectAst<'tokens, 'source> {
    /// `РАЗРЕШЕННЫЕ` of this branch; only the first branch of a statement
    /// may carry one.
    pub(super) allowed: Option<&'tokens Token<'source>>,
    pub(super) distinct: bool,
    /// `ПОМЕСТИТЬ`/`ДОБАВИТЬ` of this branch; only the first branch of a
    /// statement may carry one.
    pub(super) into: Option<IntoAst<'tokens, 'source>>,
    pub(super) top: Option<u32>,
    pub(super) projection: Vec<ProjectionItem<'tokens, 'source>>,
    pub(super) source: Option<SourceAst<'tokens, 'source>>,
    pub(super) joins: Vec<JoinAst<'tokens, 'source>>,
    pub(super) filter: Option<Expression<'tokens, 'source>>,
    pub(super) group: Vec<GroupKey<'tokens, 'source>>,
    pub(super) having: Option<Expression<'tokens, 'source>>,
}

/// One key of `СГРУППИРОВАТЬ ПО`.
#[derive(Debug)]
pub(super) struct GroupKey<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) expression: Expression<'tokens, 'source>,
}

#[derive(Debug)]
pub(super) struct SourceAst<'tokens, 'source> {
    /// The metadata kind token, the opening parenthesis of a nested query,
    /// or the name of a temporary table.
    pub(super) kind: &'tokens Token<'source>,
    /// The object name token, or the opening parenthesis of a nested query.
    pub(super) object: &'tokens Token<'source>,
    pub(super) table_part: Option<&'tokens Token<'source>>,
    pub(super) slice: Option<SliceAst<'tokens, 'source>>,
    pub(super) accumulation: Option<AccumulationAst<'tokens, 'source>>,
    pub(super) alias: Option<&'tokens Token<'source>>,
    /// A nested `(ВЫБРАТЬ …)` used as a derived source.
    pub(super) nested: Option<Box<QueryAst<'tokens, 'source>>>,
    /// A bare identifier naming a temporary table of the batch.
    pub(super) temporary: bool,
}

#[derive(Debug)]
pub(super) struct ProjectionItem<'tokens, 'source> {
    pub(super) expression: Projection<'tokens, 'source>,
    pub(super) alias: Option<&'tokens Token<'source>>,
}

#[derive(Debug)]
pub(super) struct SliceAst<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) kind: SliceKind,
    pub(super) period: Option<Expression<'tokens, 'source>>,
    pub(super) condition: Option<Expression<'tokens, 'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SliceKind {
    First,
    Last,
}

impl SliceKind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::First => "SliceFirst",
            Self::Last => "SliceLast",
        }
    }

    pub(super) const fn period_operator(self) -> &'static str {
        match self {
            Self::First => ">=",
            Self::Last => "<=",
        }
    }

    pub(super) const fn order(self) -> &'static str {
        match self {
            Self::First => "ASC",
            Self::Last => "DESC",
        }
    }
}

#[derive(Debug)]
pub(super) struct AccumulationAst<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) kind: AccumulationKind,
    pub(super) arguments: Vec<Option<Expression<'tokens, 'source>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccumulationKind {
    Balance,
    Turnovers,
}

impl AccumulationKind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Balance => "Balance",
            Self::Turnovers => "Turnovers",
        }
    }

    pub(super) const fn resource_suffix(self) -> (&'static str, &'static str) {
        match self {
            Self::Balance => ("Остаток", "Balance"),
            Self::Turnovers => ("Оборот", "Turnover"),
        }
    }
}

#[derive(Debug)]
pub(super) struct JoinAst<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) kind: JoinKind,
    pub(super) source: SourceAst<'tokens, 'source>,
    pub(super) condition: Expression<'tokens, 'source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug)]
pub(super) enum Projection<'tokens, 'source> {
    All,
    Field(FieldReference<'tokens, 'source>),
    Scalar(Expression<'tokens, 'source>),
    Aggregate {
        token: &'tokens Token<'source>,
        kind: AggregateKind,
        distinct: bool,
        argument: AggregateArgument<'tokens, 'source>,
    },
    Presentation {
        token: &'tokens Token<'source>,
        operation: PresentationOperation,
        argument: PresentationArgument<'tokens, 'source>,
    },
}

#[derive(Debug)]
pub(super) enum AggregateArgument<'tokens, 'source> {
    All,
    Expression(Box<Expression<'tokens, 'source>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AggregateKind {
    Count,
    Sum,
    Min,
    Max,
}

impl AggregateKind {
    pub(super) const fn sql_name(self) -> &'static str {
        match self {
            Self::Count => "COUNT",
            Self::Sum => "SUM",
            Self::Min => "MIN",
            Self::Max => "MAX",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PresentationOperation {
    Reference,
    String,
    Property,
}

#[derive(Debug)]
pub(super) enum PresentationArgument<'tokens, 'source> {
    Field(FieldReference<'tokens, 'source>),
    Literal(&'tokens Token<'source>),
}

#[derive(Debug, Clone)]
pub(super) struct FieldReference<'tokens, 'source> {
    pub(super) segments: Vec<&'tokens Token<'source>>,
}

impl<'tokens, 'source> FieldReference<'tokens, 'source> {
    pub(super) fn last(&self) -> &'tokens Token<'source> {
        self.segments
            .last()
            .copied()
            .expect("field path is non-empty")
    }
}

#[derive(Debug)]
pub(super) struct OrderTerm<'tokens, 'source> {
    pub(super) field: FieldReference<'tokens, 'source>,
    pub(super) descending: bool,
}

#[derive(Debug)]
pub(super) enum Expression<'tokens, 'source> {
    Field(FieldReference<'tokens, 'source>),
    Literal(&'tokens Token<'source>),
    DateTime {
        token: &'tokens Token<'source>,
        value: DateTimeValue,
    },
    BeginOfPeriod {
        token: &'tokens Token<'source>,
        value: Box<Self>,
        period: PeriodKind,
    },
    MetadataValue {
        token: &'tokens Token<'source>,
        kind: &'tokens Token<'source>,
        object: &'tokens Token<'source>,
        value: &'tokens Token<'source>,
    },
    /// `УНИКАЛЬНЫЙИДЕНТИФИКАТОР(<reference field>)`.
    Uuid {
        token: &'tokens Token<'source>,
        argument: FieldReference<'tokens, 'source>,
    },
    /// `ВЫРАЗИТЬ(<expression> КАК <target>)`, optionally followed by one
    /// `.Field` when the target is a metadata object.
    Cast {
        token: &'tokens Token<'source>,
        argument: Box<Self>,
        target: CastTarget<'tokens, 'source>,
        path: Option<&'tokens Token<'source>>,
    },
    Unary {
        operator: &'tokens Token<'source>,
        value: Box<Self>,
    },
    Binary {
        left: Box<Self>,
        operator: &'tokens Token<'source>,
        right: Box<Self>,
    },
    InList {
        value: Box<Self>,
        items: Vec<Self>,
        negated: bool,
    },
    /// `<value> [НЕ] В (<query>)`.
    InQuery {
        token: &'tokens Token<'source>,
        value: Box<Self>,
        query: Box<QueryAst<'tokens, 'source>>,
        negated: bool,
    },
    IsNull {
        value: Box<Self>,
        negated: bool,
    },
    /// `ВЫБОР КОГДА … ТОГДА … [ИНАЧЕ …] КОНЕЦ`.
    Case {
        token: &'tokens Token<'source>,
        branches: Vec<CaseBranch<'tokens, 'source>>,
        otherwise: Option<Box<Self>>,
    },
    /// `ЕСТЬNULL(<value>, <fallback>)`.
    IsNullFunction {
        token: &'tokens Token<'source>,
        value: Box<Self>,
        fallback: Box<Self>,
    },
    /// `<value> [НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ <escape>]`.
    Like {
        token: &'tokens Token<'source>,
        value: Box<Self>,
        pattern: Box<Self>,
        escape: Option<Box<Self>>,
        negated: bool,
    },
    /// A named `&Имя` parameter whose value is supplied at compilation.
    Parameter(&'tokens Token<'source>),
    /// An aggregate function call; valid only where the branch allows
    /// aggregates.
    Aggregate {
        token: &'tokens Token<'source>,
        kind: AggregateKind,
        distinct: bool,
        argument: AggregateArgument<'tokens, 'source>,
    },
}

/// One `КОГДА … ТОГДА …` alternative of a `ВЫБОР` expression.
#[derive(Debug)]
pub(super) struct CaseBranch<'tokens, 'source> {
    pub(super) token: &'tokens Token<'source>,
    pub(super) when: Expression<'tokens, 'source>,
    pub(super) then: Expression<'tokens, 'source>,
}

/// Target of a `ВЫРАЗИТЬ`/`CAST` expression.
#[derive(Debug, Clone, Copy)]
pub(super) enum CastTarget<'tokens, 'source> {
    /// `СТРОКА(n)` / `STRING(n)`; `None` keeps the length unbounded.
    String { length: Option<u32> },
    /// `ЧИСЛО(p, s)` / `NUMBER(p, s)`.
    Number {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    /// `БУЛЕВО` / `BOOLEAN`.
    Boolean,
    /// `ДАТА` / `DATE`.
    Date,
    /// `<Kind>.<Object>` such as `Справочник.Контрагенты`.
    Reference {
        kind: &'tokens Token<'source>,
        object: &'tokens Token<'source>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DateTimeValue {
    pub(super) year: u16,
    pub(super) month: u8,
    pub(super) day: u8,
    pub(super) hour: u8,
    pub(super) minute: u8,
    pub(super) second: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeriodKind {
    Minute,
    Hour,
    Day,
    Week,
    TenDays,
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl PeriodKind {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "МИНУТА" | "MINUTE" => Some(Self::Minute),
            "ЧАС" | "HOUR" => Some(Self::Hour),
            "ДЕНЬ" | "DAY" => Some(Self::Day),
            "НЕДЕЛЯ" | "WEEK" => Some(Self::Week),
            "ДЕКАДА" | "TENDAYS" => Some(Self::TenDays),
            "МЕСЯЦ" | "MONTH" => Some(Self::Month),
            "КВАРТАЛ" | "QUARTER" => Some(Self::Quarter),
            "ПОЛУГОДИЕ" | "HALFYEAR" => Some(Self::HalfYear),
            "ГОД" | "YEAR" => Some(Self::Year),
            _ => None,
        }
    }

    pub(super) const fn postgres_name(self) -> Option<&'static str> {
        match self {
            Self::Minute => Some("minute"),
            Self::Hour => Some("hour"),
            Self::Day => Some("day"),
            Self::Week => Some("week"),
            Self::Month => Some("month"),
            Self::Quarter => Some("quarter"),
            Self::Year => Some("year"),
            Self::TenDays | Self::HalfYear => None,
        }
    }
}

pub(super) fn parse_datetime_value(
    function: &Token<'_>,
    arguments: &[&Token<'_>],
) -> Result<DateTimeValue, QueryDiagnostic> {
    if !(3..=6).contains(&arguments.len()) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(function),
            "DATETIME requires 3 to 6 integer components",
        ));
    }
    let values = arguments
        .iter()
        .map(|argument| {
            argument.lexeme.parse::<u16>().map_err(|_| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(argument),
                    "DATETIME component is out of range",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let year = values[0];
    let month = u8::try_from(values[1]).unwrap_or(u8::MAX);
    let day = u8::try_from(values[2]).unwrap_or(u8::MAX);
    let hour = values
        .get(3)
        .copied()
        .map_or(0, |value| u8::try_from(value).unwrap_or(u8::MAX));
    let minute = values
        .get(4)
        .copied()
        .map_or(0, |value| u8::try_from(value).unwrap_or(u8::MAX));
    let second = values
        .get(5)
        .copied()
        .map_or(0, |value| u8::try_from(value).unwrap_or(u8::MAX));
    if year == 0 || year > 9999 || month == 0 || month > 12 {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(arguments[if year == 0 { 0 } else { 1 }]),
            "DATETIME year must be 1..=9999 and month must be 1..=12",
        ));
    }
    let maximum_day = days_in_month(year, month);
    if day == 0 || day > maximum_day {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Syntax,
            Some(arguments[2]),
            format!("DATETIME day must be 1..={maximum_day} for the selected month"),
        ));
    }
    for (index, value, maximum, name) in [
        (3, hour, 23, "hour"),
        (4, minute, 59, "minute"),
        (5, second, 59, "second"),
    ] {
        if arguments.get(index).is_some() && value > maximum {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(arguments[index]),
                format!("DATETIME {name} must be 0..={maximum}"),
            ));
        }
    }
    Ok(DateTimeValue {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

pub(super) const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

const fn is_leap_year(year: u16) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
