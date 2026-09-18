//! The balances of a split `ОстаткиИОбороты`: running sums over the
//! buckets of one grain.
//!
//! The platform accumulates the balance of a period while reading the
//! ordered rows; here it is a window over the bucketed movements — the
//! movements before the interval form one bucket that sorts first and is
//! dropped after the window is taken, so the opening balance of the first
//! period is the balance before the interval. Under `Авто` the grain is
//! whatever split field the statement reads, which is known only once the
//! statement is compiled, so one relation is prepared per grain and the
//! aggregate finalization picks one.

use super::virtual_tables::{AUTO_PERIOD_LEVELS, DerivedResource};
use crate::query::core::ast::PeriodKind;
use crate::query::core::dialect::SqlDialect;

/// The grain of the buckets the balances run over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grain {
    /// One bucket: the whole interval.
    Whole,
    /// One bucket per calendar period of the given unit.
    Calendar(PeriodKind),
    /// One bucket per record period and recorder.
    Recorder,
    /// One bucket per record.
    Record,
}

/// A relation prepared for one grain, with or without the balance
/// columns as running sums.
pub(super) struct RelationVariant {
    /// The grain the relation answers, `None` for a fixed periodicity.
    pub(super) grain: Option<Grain>,
    /// Whether the relation carries the balance columns as running sums.
    pub(super) balances: bool,
    pub(super) sql: String,
}

/// The position of every `Авто` split field in the order
/// `auto_split_fields` adds them: the record period, the ten calendar
/// levels, the recorder, the line number.
pub(super) const AUTO_PERIOD_POSITION: usize = 0;
pub(super) const AUTO_RECORDER_POSITION: usize = 11;
pub(super) const AUTO_LINE_POSITION: usize = 12;

/// The grain the split fields read by a statement ask for: the line
/// number makes it a record, the recorder or the raw period a recorder,
/// otherwise the finest calendar level read; none — the whole interval.
pub(super) fn grain_of(used_positions: impl Iterator<Item = usize>) -> Grain {
    let mut grain = Grain::Whole;
    let mut finest: Option<usize> = None;
    for position in used_positions {
        match position {
            AUTO_LINE_POSITION => return Grain::Record,
            AUTO_RECORDER_POSITION | AUTO_PERIOD_POSITION => grain = Grain::Recorder,
            level => finest = Some(finest.map_or(level, |current| current.min(level))),
        }
    }
    match (grain, finest) {
        (Grain::Recorder, _) => Grain::Recorder,
        (_, Some(level)) => Grain::Calendar(AUTO_PERIOD_LEVELS[level - 1].0),
        _ => Grain::Whole,
    }
}

/// Every grain an `Авто` table may be read at, finest first.
pub(super) fn auto_grains() -> Vec<Grain> {
    let mut grains = vec![Grain::Record, Grain::Recorder];
    grains.extend(
        AUTO_PERIOD_LEVELS
            .iter()
            .map(|(unit, _, _)| Grain::Calendar(*unit)),
    );
    grains
}

/// A column summed per bucket.
pub(super) struct BucketSum {
    pub(super) column: String,
    pub(super) expression: String,
}

/// A column that is a running sum of a row expression over the buckets of
/// the partition, up to the current bucket or up to the previous one.
pub(super) struct RunningSum {
    pub(super) column: String,
    pub(super) expression: String,
    pub(super) exclusive: bool,
}

/// What the windowed relation is built from.
pub(super) struct WindowedSpec<'a> {
    /// `FROM … WHERE …` of the movement rows, the alias included.
    pub(super) rows: &'a str,
    /// The alias the row columns are read from.
    pub(super) alias: &'a str,
    /// The physical dimension columns: projected, grouped, partitioned.
    pub(super) dimension_columns: Vec<String>,
    /// The record period, qualified.
    pub(super) period: String,
    /// The physical name the bucket period is exposed under.
    pub(super) period_column: &'a str,
    /// The physical recorder columns, and the line number.
    pub(super) recorder: Vec<String>,
    pub(super) line: Option<String>,
    /// The begin of the interval; the rows before it form the first bucket.
    pub(super) begin: Option<String>,
    pub(super) bucket_sums: Vec<BucketSum>,
    pub(super) running: Vec<RunningSum>,
    pub(super) derived: Vec<DerivedResource>,
    /// Whether to expose the calendar levels of the bucket period
    /// (`_PeriodMonth`, …) as an `Авто` table does.
    pub(super) auto_levels: bool,
}

/// Renders the relation for one grain.
pub(super) fn windowed_relation(
    spec: &WindowedSpec<'_>,
    grain: Grain,
    dialect: SqlDialect,
) -> String {
    let alias = spec.alias;
    let period = &spec.period;
    let inside = spec.begin.as_ref().map_or_else(
        || "1".to_owned(),
        |begin| format!("CASE WHEN {period} < {begin} THEN 0 ELSE 1 END"),
    );
    let before = |value: &str, zero: &str| -> String {
        spec.begin.as_ref().map_or_else(
            || value.to_owned(),
            |begin| format!("CASE WHEN {period} < {begin} THEN {zero} ELSE {value} END"),
        )
    };
    // The bucket keys: the period at the grain, then the recorder and the
    // line number for the record grains.
    let bucket_period = match grain {
        Grain::Calendar(unit) => before(
            &dialect.begin_of_period(period, unit),
            &dialect.zero_datetime(),
        ),
        _ => before(period, &dialect.zero_datetime()),
    };
    let mut keys = vec![(spec.period_column.to_owned(), bucket_period.clone())];
    if matches!(grain, Grain::Recorder | Grain::Record) {
        for column in &spec.recorder {
            keys.push((
                column.clone(),
                before(&dialect.qualified_column(Some(alias), column), "NULL"),
            ));
        }
    }
    if grain == Grain::Record
        && let Some(line) = &spec.line
    {
        keys.push((
            line.clone(),
            before(&dialect.qualified_column(Some(alias), line), "NULL"),
        ));
    }
    let mut levels = Vec::new();
    if spec.auto_levels {
        let from = match grain {
            Grain::Calendar(unit) => AUTO_PERIOD_LEVELS
                .iter()
                .position(|(level, _, _)| *level == unit)
                .unwrap_or(0),
            _ => 0,
        };
        for (unit, _, english) in &AUTO_PERIOD_LEVELS[from..] {
            levels.push((
                format!("_Period{english}"),
                dialect.begin_of_period(&bucket_period, *unit),
            ));
        }
    }

    let mut projections = Vec::new();
    let mut grouping = Vec::new();
    let mut partition = Vec::new();
    for column in &spec.dimension_columns {
        let sql = dialect.qualified_column(Some(alias), column);
        projections.push(format!("{sql} AS {}", dialect.quote_identifier(column)));
        grouping.push(sql.clone());
        partition.push(sql);
    }
    let mut order = vec![inside.clone()];
    for (column, expression) in keys.iter().chain(&levels) {
        projections.push(format!(
            "{expression} AS {}",
            dialect.quote_identifier(column)
        ));
        grouping.push(expression.clone());
    }
    order.extend(keys.iter().map(|(_, expression)| expression.clone()));
    grouping.push(inside.clone());
    for sum in &spec.bucket_sums {
        projections.push(format!(
            "SUM({}) AS {}",
            sum.expression,
            dialect.quote_identifier(&sum.column)
        ));
    }
    let window = |expression: &str, exclusive: bool| {
        let frame = if exclusive {
            "ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING"
        } else {
            "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW"
        };
        let partition_by = if partition.is_empty() {
            String::new()
        } else {
            format!("PARTITION BY {} ", partition.join(", "))
        };
        let sum = format!(
            "SUM(SUM({expression})) OVER ({partition_by}ORDER BY {} {frame})",
            order.join(", ")
        );
        if exclusive {
            format!("COALESCE({sum}, 0)")
        } else {
            sum
        }
    };
    let mut running = Vec::new();
    for sum in &spec.running {
        let sql = window(&sum.expression, sum.exclusive);
        projections.push(format!(
            "{sql} AS {}",
            dialect.quote_identifier(&sum.column)
        ));
        running.push((sum.column.clone(), sql));
    }
    for derived in &spec.derived {
        let base = running
            .iter()
            .find(|(column, _)| *column == derived.base)
            .map(|(_, sql)| sql.clone())
            .unwrap_or_else(|| format!("SUM({})", dialect.quote_identifier(&derived.base)));
        projections.push(format!(
            "{} AS {}",
            derived.expression(&base),
            dialect.quote_identifier(&derived.column)
        ));
    }
    projections.push(format!(
        "{inside} AS {}",
        dialect.quote_identifier("__inside")
    ));
    let grouped = format!(
        "SELECT {} {} GROUP BY {}",
        projections.join(", "),
        spec.rows,
        grouping.join(", ")
    );
    if spec.begin.is_none() {
        return format!("({grouped})");
    }
    let periods = dialect.quote_identifier("__periods");
    format!(
        "(SELECT * FROM ({grouped}) AS {periods} WHERE {} = 1)",
        dialect.qualified_column(Some("__periods"), "__inside")
    )
}
