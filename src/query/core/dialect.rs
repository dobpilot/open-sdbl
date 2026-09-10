//! SQL dialect primitives, quoting, literals, and output labels.

use std::collections::HashSet;

use crate::query::core::ast::{CastTarget, DateTimeValue, PeriodKind};
use crate::query::core::resolve::ColumnKind;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::query::mssql::MsSqlDialectLevel;
use crate::{Keyword, Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    MsSql {
        year_offset: i32,
        dialect_level: MsSqlDialectLevel,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum LabelLimit {
    Bytes(usize),
    Utf16Units(usize),
}

pub(super) struct OutputLabelAllocator {
    limit: LabelLimit,
    used: HashSet<String>,
}

impl OutputLabelAllocator {
    pub(super) fn new(dialect: SqlDialect) -> Self {
        Self {
            limit: dialect.output_label_limit(),
            used: HashSet::new(),
        }
    }

    pub(super) fn allocate(&mut self, requested: &str) -> String {
        let candidate = truncate_label(requested, self.limit);
        if self.used.insert(candidate.to_lowercase()) {
            return candidate;
        }
        for number in 2usize.. {
            let suffix = format!("_{number}");
            let prefix_limit = match self.limit {
                LabelLimit::Bytes(limit) => LabelLimit::Bytes(limit.saturating_sub(suffix.len())),
                LabelLimit::Utf16Units(limit) => {
                    LabelLimit::Utf16Units(limit.saturating_sub(suffix.len()))
                }
            };
            let candidate = format!("{}{}", truncate_label(requested, prefix_limit), suffix);
            if self.used.insert(candidate.to_lowercase()) {
                return candidate;
            }
        }
        unreachable!("an unbounded numeric suffix always produces a unique label")
    }
}

pub(super) fn truncate_label(value: &str, limit: LabelLimit) -> String {
    match limit {
        LabelLimit::Bytes(limit) if value.len() > limit => {
            let mut end = limit;
            while !value.is_char_boundary(end) {
                end -= 1;
            }
            value[..end].to_owned()
        }
        LabelLimit::Utf16Units(limit) => {
            let mut units = 0;
            value
                .chars()
                .take_while(|character| {
                    let next = units + character.len_utf16();
                    if next > limit {
                        false
                    } else {
                        units = next;
                        true
                    }
                })
                .collect()
        }
        LabelLimit::Bytes(_) => value.to_owned(),
    }
}

fn binary_literal_digits<'source>(token: &Token<'source>) -> Result<&'source str, QueryDiagnostic> {
    token
        .lexeme
        .get(2..)
        .filter(|digits| {
            !digits.is_empty()
                && digits.len() % 2 == 0
                && digits.chars().all(|digit| digit.is_ascii_hexdigit())
        })
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "invalid binary literal",
            )
        })
}

/// The bytes of a `0x…` literal token.
pub(super) fn decode_binary_literal(token: &Token<'_>) -> Result<Vec<u8>, QueryDiagnostic> {
    let digits = binary_literal_digits(token)?;
    Ok(digits
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex digits");
            u8::from_str_radix(text, 16).expect("validated hex digits")
        })
        .collect())
}

pub(super) fn compile_literal(
    token: &Token<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match token.kind {
        TokenKind::String => {
            let inner = token
                .lexeme
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Syntax,
                        Some(token),
                        "invalid string literal",
                    )
                })?;
            Ok(dialect.string_literal(&inner.replace("\"\"", "\"")))
        }
        TokenKind::Number => Ok(token.lexeme.to_owned()),
        TokenKind::Binary => {
            let digits = binary_literal_digits(token)?;
            match dialect {
                SqlDialect::Postgres => Ok(format!("'\\x{digits}'::bytea")),
                SqlDialect::MsSql { .. } => Ok(format!("0x{digits}")),
            }
        }
        TokenKind::Keyword(Keyword::True) => Ok(dialect.boolean_literal(true).to_owned()),
        TokenKind::Keyword(Keyword::False) => Ok(dialect.boolean_literal(false).to_owned()),
        TokenKind::Keyword(Keyword::Null) => Ok("NULL".to_owned()),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "unsupported literal",
        )),
    }
}

impl SqlDialect {
    pub(crate) const fn mssql(year_offset: i32, dialect_level: MsSqlDialectLevel) -> Self {
        Self::MsSql {
            year_offset,
            dialect_level,
        }
    }

    pub(super) const fn is_mssql(self) -> bool {
        matches!(self, Self::MsSql { .. })
    }

    pub(super) fn quote_identifier(self, identifier: &str) -> String {
        match self {
            Self::Postgres => format!("\"{}\"", identifier.replace('"', "\"\"")),
            Self::MsSql { .. } => format!("[{}]", identifier.replace(']', "]]")),
        }
    }

    pub(super) fn qualified_column(self, alias: Option<&str>, column: &str) -> String {
        alias.map_or_else(
            || self.quote_identifier(column),
            |alias| {
                format!(
                    "{}.{}",
                    self.quote_identifier(alias),
                    self.quote_identifier(column)
                )
            },
        )
    }

    pub(super) const fn output_label_limit(self) -> LabelLimit {
        match self {
            Self::Postgres => LabelLimit::Bytes(63),
            Self::MsSql { .. } => LabelLimit::Utf16Units(128),
        }
    }

    /// Text conversion used only by presentation functions, whose result is
    /// a string by definition.
    fn text(self, expression: &str) -> String {
        match self {
            Self::Postgres => format!("{expression}::text"),
            Self::MsSql { .. } => format!("CONVERT(nvarchar(max), {expression})"),
        }
    }

    /// Converts a scalar presentation argument to text.
    pub(super) fn scalar_text(self, expression: &str) -> String {
        match self {
            Self::Postgres => format!("({expression})::text"),
            Self::MsSql { .. } => self.text(expression),
        }
    }

    /// Returns a projected date expression in the logical 1C domain: MSSQL
    /// subtracts `_YearOffset`, PostgreSQL passes the expression through.
    pub(super) fn date_scalar(self, expression: &str) -> String {
        match self {
            Self::MsSql { year_offset, .. } if year_offset != 0 => {
                format!("DATEADD(year, {}, {expression})", -year_offset)
            }
            _ => expression.to_owned(),
        }
    }

    /// `НАЧАЛОПЕРИОДА` without `DATETIME2FROMPARTS` (SQL Server 2012): every
    /// boundary is `DATEADD`/`DATEDIFF` arithmetic from a `datetime2` base so
    /// that results match the newer rendering byte for byte. The base is
    /// `0001-01-01`, keeping bases without a year offset in range.
    fn begin_of_period_sql_2008(value: &str, period: PeriodKind) -> String {
        const BASE: &str = "CONVERT(datetime2, '00010101', 112)";
        let day = format!("CONVERT(datetime2, CONVERT(date, {value}))");
        match period {
            PeriodKind::Minute => {
                format!("DATEADD(minute, DATEDIFF(minute, {day}, {value}), {day})")
            }
            PeriodKind::Hour => format!("DATEADD(hour, DATEDIFF(hour, {day}, {value}), {day})"),
            PeriodKind::Day => day,
            PeriodKind::Week => format!(
                "DATEADD(day, -(((DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, {value})) % 7) + 7) % 7), {day})"
            ),
            PeriodKind::TenDays => format!(
                "DATEADD(day, CASE WHEN DAY({value}) <= 10 THEN 0 WHEN DAY({value}) <= 20 THEN 10 ELSE 20 END, DATEADD(month, DATEDIFF(month, {BASE}, {value}), {BASE}))"
            ),
            PeriodKind::Month => {
                format!("DATEADD(month, DATEDIFF(month, {BASE}, {value}), {BASE})")
            }
            PeriodKind::Quarter => {
                format!("DATEADD(quarter, DATEDIFF(quarter, {BASE}, {value}), {BASE})")
            }
            PeriodKind::HalfYear => {
                format!("DATEADD(month, (DATEDIFF(month, {BASE}, {value}) / 6) * 6, {BASE})")
            }
            PeriodKind::Year => format!("DATEADD(year, DATEDIFF(year, {BASE}, {value}), {BASE})"),
        }
    }

    /// Projects one physical column in its native type. The only conversions
    /// are the MSSQL year-offset correction for dates and a `text` cast for
    /// the PostgreSQL 1C extension types `mchar`/`mvarchar`, whose binary
    /// wire format is undocumented.
    pub(super) fn column_projection(
        self,
        expression: &str,
        kind: &ColumnKind,
        data_type: &str,
    ) -> String {
        match (self, kind) {
            (Self::MsSql { year_offset, .. }, ColumnKind::DateTime) if year_offset != 0 => {
                format!("DATEADD(year, {}, {expression})", -year_offset)
            }
            (Self::Postgres, ColumnKind::String { .. })
                if matches!(base_type_name(data_type).as_str(), "mchar" | "mvarchar") =>
            {
                format!("{expression}::text")
            }
            _ => expression.to_owned(),
        }
    }

    /// Projects a column inside a nested statement: values stay in the
    /// storage domain (the outer statement corrects MSSQL dates once), only
    /// the PostgreSQL 1C string types are cast to text.
    pub(super) fn storage_column_projection(
        self,
        expression: &str,
        kind: &ColumnKind,
        data_type: &str,
    ) -> String {
        match (self, kind) {
            (Self::Postgres, ColumnKind::String { .. })
                if matches!(base_type_name(data_type).as_str(), "mchar" | "mvarchar") =>
            {
                format!("{expression}::text")
            }
            _ => expression.to_owned(),
        }
    }

    /// Renders a scalar `ВЫРАЗИТЬ`/`CAST`. PostgreSQL uses `substring … for`
    /// rather than `left` so servers back to 9.0 are supported; MSSQL falls
    /// back to `nvarchar(max)` beyond the 4000-character limit and to
    /// `numeric(38, 10)` when no precision is given.
    pub(super) fn cast_scalar(self, inner: &str, target: CastTarget<'_, '_>) -> String {
        match (self, target) {
            (
                Self::Postgres,
                CastTarget::String {
                    length: Some(length),
                },
            ) => {
                format!("substring({inner}::text from 1 for {length})")
            }
            (Self::Postgres, CastTarget::String { length: None }) => format!("{inner}::text"),
            (Self::Postgres, CastTarget::Number { precision, scale }) => match (precision, scale) {
                (Some(precision), Some(scale)) => format!("{inner}::numeric({precision}, {scale})"),
                (Some(precision), None) => format!("{inner}::numeric({precision})"),
                _ => format!("{inner}::numeric"),
            },
            (Self::Postgres, CastTarget::Boolean) => format!("{inner}::boolean"),
            (Self::Postgres, CastTarget::Date) => format!("{inner}::timestamp"),
            (Self::MsSql { .. }, CastTarget::String { length }) => match length {
                Some(length) if length <= 4000 => format!("CONVERT(nvarchar({length}), {inner})"),
                _ => format!("CONVERT(nvarchar(max), {inner})"),
            },
            (Self::MsSql { .. }, CastTarget::Number { precision, scale }) => format!(
                "CONVERT(numeric({}, {}), {inner})",
                precision.unwrap_or(38),
                scale.unwrap_or(10)
            ),
            (Self::MsSql { .. }, CastTarget::Boolean) => format!("CONVERT(bit, {inner})"),
            (Self::MsSql { .. }, CastTarget::Date) => format!("CONVERT(datetime2, {inner})"),
            (_, CastTarget::Reference { .. }) => {
                unreachable!("reference casts are compiled by the expression compiler")
            }
        }
    }

    /// Turns a boolean value into a predicate: SQL Server has no boolean
    /// expressions, so a `bit` must be compared explicitly.
    pub(super) fn boolean_predicate(self, value: &str) -> String {
        match self {
            Self::Postgres => value.to_owned(),
            Self::MsSql { .. } => format!("({value} = 0x01)"),
        }
    }

    /// Renders `ИСТИНА`/`ЛОЖЬ` in a predicate position.
    pub(super) fn boolean_literal_predicate(self, value: bool) -> String {
        match self {
            Self::Postgres => self.boolean_literal(value).to_owned(),
            Self::MsSql { .. } => (if value { "(1 = 1)" } else { "(1 = 0)" }).to_owned(),
        }
    }

    /// The `RRRef` of a runtime-typed reference when its `RTRef` names the
    /// requested table, `NULL` otherwise.
    pub(super) fn narrowed_reference(
        self,
        type_column: &str,
        database_type: u32,
        reference: &str,
    ) -> String {
        format!(
            "CASE WHEN {type_column} = {} THEN {reference} END",
            self.binary_u32(database_type)
        )
    }

    /// Decodes a 16-byte 1C reference (`d + e + c + b + a` field order) into
    /// a native UUID in canonical `a-b-c-d-e` order. `NULL` propagates.
    ///
    /// MSSQL `CAST(binary AS uniqueidentifier)` reads the first three groups
    /// little-endian, so those bytes are reversed before the cast.
    pub(super) fn reference_uuid(self, reference: &str) -> String {
        match self {
            Self::Postgres => format!(
                "encode(substring({reference} from 13 for 4) || substring({reference} from 11 for 2) || substring({reference} from 9 for 2) || substring({reference} from 1 for 8), 'hex')::uuid"
            ),
            Self::MsSql { .. } => {
                let bytes = [16, 15, 14, 13, 12, 11, 10, 9]
                    .iter()
                    .map(|position| format!("SUBSTRING({reference}, {position}, 1)"))
                    .collect::<Vec<_>>()
                    .join(" + ");
                format!("CAST({bytes} + SUBSTRING({reference}, 1, 8) AS uniqueidentifier)")
            }
        }
    }

    /// Concatenates the `RTRef` discriminator and the `RRRef` value into the
    /// 20-byte runtime-typed reference payload.
    /// The 4-byte `RTRef` prefix of an `RTRef ‖ RRRef` payload.
    pub(super) fn payload_type(self, payload: &str) -> String {
        match self {
            Self::Postgres => format!("substring({payload} from 1 for 4)"),
            Self::MsSql { .. } => format!("SUBSTRING({payload}, 1, 4)"),
        }
    }

    /// The 16-byte `RRRef` suffix of an `RTRef ‖ RRRef` payload.
    pub(super) fn payload_reference(self, payload: &str) -> String {
        match self {
            Self::Postgres => format!("substring({payload} from 5 for 16)"),
            Self::MsSql { .. } => format!("SUBSTRING({payload}, 5, 16)"),
        }
    }

    pub(super) fn reference_payload(self, type_value: &str, reference: &str) -> String {
        match self {
            Self::Postgres => format!("({type_value} || {reference})"),
            Self::MsSql { .. } => format!("({type_value} + {reference})"),
        }
    }

    pub(super) fn datetime_expression(
        self,
        value: DateTimeValue,
        storage_domain: bool,
        token: &Token<'_>,
    ) -> Result<String, QueryDiagnostic> {
        let literal = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            value.year, value.month, value.day, value.hour, value.minute, value.second
        );
        match self {
            Self::Postgres => Ok(format!("TIMESTAMP '{}'", literal.replace('T', " "))),
            Self::MsSql { year_offset, .. } => {
                if storage_domain {
                    let physical_year = i32::from(value.year)
                        .checked_add(year_offset)
                        .ok_or_else(|| {
                            QueryDiagnostic::at(
                                QueryDiagnosticKind::Syntax,
                                Some(token),
                                format!(
                                    "DATETIME year {} with MSSQL year offset {year_offset} is outside 1..=9999",
                                    value.year
                                ),
                            )
                        })?;
                    if !(1..=9999).contains(&physical_year) {
                        return Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::Syntax,
                            Some(token),
                            format!(
                                "DATETIME year {} with MSSQL year offset {year_offset} is outside 1..=9999",
                                value.year
                            ),
                        ));
                    }
                }
                let expression = format!("CONVERT(datetime2, '{literal}', 126)");
                if storage_domain && year_offset != 0 {
                    Ok(format!("DATEADD(year, {year_offset}, {expression})"))
                } else {
                    Ok(expression)
                }
            }
        }
    }

    pub(super) fn begin_of_period(self, value: &str, period: PeriodKind) -> String {
        match self {
            Self::Postgres => match period.postgres_name() {
                Some(period) => format!("date_trunc('{period}', {value})"),
                None if period == PeriodKind::TenDays => format!(
                    "(date_trunc('month', {value}) + (LEAST(((EXTRACT(DAY FROM {value})::integer - 1) / 10), 2) * INTERVAL '10 days'))"
                ),
                None => format!(
                    "(date_trunc('year', {value}) + CASE WHEN EXTRACT(MONTH FROM {value}) > 6 THEN INTERVAL '6 months' ELSE INTERVAL '0 months' END)"
                ),
            },
            Self::MsSql {
                dialect_level: MsSqlDialectLevel::Sql2008,
                ..
            } => Self::begin_of_period_sql_2008(value, period),
            Self::MsSql { .. } => match period {
                PeriodKind::Minute => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), MONTH({value}), DAY({value}), DATEPART(hour, {value}), DATEPART(minute, {value}), 0, 0, 0)"
                ),
                PeriodKind::Hour => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), MONTH({value}), DAY({value}), DATEPART(hour, {value}), 0, 0, 0, 0)"
                ),
                PeriodKind::Day => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), MONTH({value}), DAY({value}), 0, 0, 0, 0, 0)"
                ),
                PeriodKind::Week => format!(
                    "DATEADD(day, -(((DATEDIFF(day, CONVERT(date, '19000101', 112), CONVERT(date, {value})) % 7) + 7) % 7), CONVERT(datetime2, CONVERT(date, {value})))"
                ),
                PeriodKind::TenDays => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), MONTH({value}), CASE WHEN DAY({value}) <= 10 THEN 1 WHEN DAY({value}) <= 20 THEN 11 ELSE 21 END, 0, 0, 0, 0, 0)"
                ),
                PeriodKind::Month => {
                    format!("DATETIME2FROMPARTS(YEAR({value}), MONTH({value}), 1, 0, 0, 0, 0, 0)")
                }
                PeriodKind::Quarter => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), (((MONTH({value}) - 1) / 3) * 3) + 1, 1, 0, 0, 0, 0, 0)"
                ),
                PeriodKind::HalfYear => format!(
                    "DATETIME2FROMPARTS(YEAR({value}), CASE WHEN MONTH({value}) <= 6 THEN 1 ELSE 7 END, 1, 0, 0, 0, 0, 0)"
                ),
                PeriodKind::Year => {
                    format!("DATETIME2FROMPARTS(YEAR({value}), 1, 1, 0, 0, 0, 0, 0)")
                }
            },
        }
    }

    /// Converts one presentation template field to text so that it can be
    /// concatenated with literal template parts.
    pub(super) fn presentation_field_text(self, expression: &str, data_type: &str) -> String {
        let base = base_type_name(data_type);
        match self {
            Self::MsSql { .. } if matches!(base.as_str(), "timestamp" | "rowversion") => {
                expression.to_owned()
            }
            Self::MsSql { .. } if matches!(base.as_str(), "binary" | "varbinary" | "image") => {
                format!("CONVERT(varchar(max), {expression}, 1)")
            }
            Self::MsSql { year_offset, .. }
                if year_offset != 0
                    && matches!(
                        base.as_str(),
                        "date" | "datetime" | "datetime2" | "smalldatetime"
                    ) =>
            {
                self.text(&format!("DATEADD(year, {}, {expression})", -year_offset))
            }
            _ => self.text(expression),
        }
    }

    pub(super) fn literal_for_type(
        self,
        token: &Token<'_>,
        data_type: &str,
    ) -> Result<String, QueryDiagnostic> {
        let literal = compile_literal(token, self)?;
        let base = data_type
            .split_once('(')
            .map_or(data_type, |(base, _)| base)
            .trim();
        match self {
            Self::MsSql { year_offset, .. }
                if year_offset != 0
                    && token.kind == TokenKind::String
                    && matches!(
                        base.to_ascii_lowercase().as_str(),
                        "date" | "datetime" | "datetime2" | "smalldatetime"
                    ) =>
            {
                Ok(format!("DATEADD(year, {year_offset}, {literal})"))
            }
            _ => Ok(literal),
        }
    }

    pub(super) fn datetime_literal(self, token: &Token<'_>) -> Result<String, QueryDiagnostic> {
        self.literal_for_type(token, "datetime2")
    }

    pub(super) fn null_text(self) -> &'static str {
        match self {
            Self::Postgres => "NULL::text",
            Self::MsSql { .. } => "CONVERT(nvarchar(max), NULL)",
        }
    }

    /// PostgreSQL output requires `standard_conforming_strings = on` (the
    /// server default): quotes are doubled and backslashes are passed through.
    pub(super) fn string_literal(self, value: &str) -> String {
        let escaped = value.replace('\'', "''");
        match self {
            Self::Postgres => format!("'{escaped}'"),
            Self::MsSql { .. } => format!("N'{escaped}'"),
        }
    }

    pub(super) fn boolean_literal(self, value: bool) -> &'static str {
        match (self, value) {
            (Self::Postgres, true) => "TRUE",
            (Self::Postgres, false) => "FALSE",
            (Self::MsSql { .. }, true) => "0x01",
            (Self::MsSql { .. }, false) => "0x00",
        }
    }

    pub(super) fn binary_u32(self, value: u32) -> String {
        self.binary_literal(&value.to_be_bytes())
    }

    pub(super) fn binary_literal(self, bytes: &[u8]) -> String {
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        match self {
            Self::Postgres => format!("decode('{hex}', 'hex')"),
            Self::MsSql { .. } => format!("0x{hex}"),
        }
    }

    pub(super) fn select_prefix(self, distinct: bool, top: Option<u32>) -> String {
        let mut sql = String::from("SELECT ");
        if distinct {
            sql.push_str("DISTINCT ");
        }
        if self.is_mssql()
            && let Some(top) = top
        {
            use std::fmt::Write as _;
            write!(sql, "TOP ({top}) ").expect("writing to String cannot fail");
        }
        sql
    }

    pub(super) fn append_limit(self, sql: &mut String, top: Option<u32>) {
        if self == Self::Postgres
            && let Some(top) = top
        {
            use std::fmt::Write as _;
            write!(sql, " LIMIT {top}").expect("writing to String cannot fail");
        }
    }
}

/// Lower-case catalog type name without its length or precision suffix.
pub(super) fn base_type_name(data_type: &str) -> String {
    data_type
        .split_once('(')
        .map_or(data_type, |(base, _)| base)
        .trim()
        .to_ascii_lowercase()
}
