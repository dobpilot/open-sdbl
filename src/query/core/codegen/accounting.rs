//! The virtual tables of an accounting register, read from its movements.
//!
//! A record of a register with correspondence carries two accounts, so an
//! aggregating table folds each record into two rows — the debit side and
//! the credit side — before grouping by `Счет`, the dimensions and the
//! period: a `UNION ALL` of two branches over the main table. A balance
//! field has one value per record; a non-balance field has one per side,
//! and each branch reads its own (`_Fld414DtRRef` on the debit side,
//! `_Fld414CtRRef` on the credit side) under one unified column name, so
//! the outer aggregation sees one `Валюта`. The debit and credit parts of
//! a balance are the positive and the negated negative part of the sum at
//! the grain the statement reads, so they are derived after the unread
//! dimensions are summed away. The staging of the tables is the OpenSpec
//! change `accounting-register-virtual-tables`.

use super::expression::operand_token;
use super::sources::{CompiledSourceRelation, SourceRestriction};
use super::virtual_tables::{
    AUTO_PERIOD_LEVELS, ConditionSql, DerivedResource, TurnoverPeriodicity, aggregate_source,
    auto_period_level_field, balance_and_turnover_field, compile_accumulation_condition,
    compile_virtual_period_literal, conjunction, register_standard_field, turnovers_period_field,
    turnovers_periodicity,
};
use super::windowed::{
    BucketSum, Grain, RelationVariant, RunningSum, WindowedSpec, auto_grains, windowed_relation,
};
use crate::metadata::{
    ConfigFieldPurpose, LiveTable, MetadataField, MetadataObject, MetadataSnapshot,
};
use crate::query::core::ast::{
    AccumulationAst, AccumulationKind, Expression, PeriodKind, SourceAst,
};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{CompilationCatalog, QueryableColumn, QueryableField};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Token, TokenKind};

/// One side of a record: which account it takes and which `Dt`/`Ct`
/// column of a non-balance field.
#[derive(Clone, Copy)]
struct Side {
    /// `0` for debit, `1` for credit — the code the aggregation switches on.
    code: u8,
    /// The schema name of the side's account field.
    account: &'static str,
    /// The schema suffix of the side's columns of a non-balance field.
    suffix: &'static str,
}

const DEBIT: Side = Side {
    code: 0,
    account: "AccountDt",
    suffix: "Dt",
};
const CREDIT: Side = Side {
    code: 1,
    account: "AccountCt",
    suffix: "Ct",
};

/// The unified alias of the account column of the folded relation.
const ACCOUNT_COLUMN: &str = "_account";
/// The unified alias of the other side's account in `Обороты`.
const CORRESPONDING_ACCOUNT_COLUMN: &str = "__cor_account";
const SIDE_COLUMN: &str = "__side";
const SIDES_ALIAS: &str = "__sides";
/// The raw record period the branches carry for the balances beside the
/// bucketed `_period` of a split.
const RECORD_PERIOD_COLUMN: &str = "__record_period";
const BASE_ALIAS: &str = "__aggregate_base";

/// The message of the balance columns a split table cannot answer.
const NO_BALANCE_WHEN_SPLIT: &str =
    "a periodic BalanceAndTurnovers answers no balance column on this server";

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_accounting_relation(
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    live_table: &LiveTable,
    fields: &[QueryableField],
    restriction: Option<&SourceRestriction<'_>>,
    dialect: SqlDialect,
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    let arguments = Arguments::read(snapshot, object, virtual_table)?;
    let table = Table {
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        live_table,
        fields,
        restriction,
        dialect,
    };
    match virtual_table.kind {
        AccumulationKind::Turnovers => table.turnovers(&arguments),
        AccumulationKind::Balance => table.balance(&arguments),
        AccumulationKind::BalanceAndTurnovers => table.balance_and_turnovers(&arguments),
        AccumulationKind::DrCrTurnovers => table.dr_cr_turnovers(&arguments),
        AccumulationKind::RecordsWithExtDimensions => table.records_with_ext_dimensions(&arguments),
    }
}

/// The staged diagnostic: what a later stage of the plan brings.
fn not_yet(virtual_table: &AccumulationAst<'_, '_>, what: &str) -> QueryDiagnostic {
    QueryDiagnostic::at(
        QueryDiagnosticKind::UnsupportedFeature,
        Some(virtual_table.token),
        format!(
            "accounting-register virtual table {}: {what} is not supported yet",
            virtual_table.kind.name()
        ),
    )
}

/// The arguments of a table by name. The platform omits the
/// extra-dimension arguments for a register whose chart of accounts has
/// no extra dimensions — measured on the UNF configuration, which writes
/// `Обороты(&Н, &К, МЕСЯЦ, , СценарийПланирования = …)` with the condition
/// fifth — so the positions depend on whether the register keeps an
/// `AccRgED` table.
#[derive(Default)]
struct Arguments<'ast, 'tokens, 'source> {
    /// `Период` of `Остатки`, `Начало` of the others.
    begin: Option<&'ast Expression<'tokens, 'source>>,
    end: Option<&'ast Expression<'tokens, 'source>>,
    periodicity: Option<&'ast Expression<'tokens, 'source>>,
    completion: Option<&'ast Expression<'tokens, 'source>>,
    account_condition: Option<&'ast Expression<'tokens, 'source>>,
    extra_dimensions: Option<&'ast Expression<'tokens, 'source>>,
    condition: Option<&'ast Expression<'tokens, 'source>>,
    balanced_account: Option<&'ast Expression<'tokens, 'source>>,
    balanced_extra_dimensions: Option<&'ast Expression<'tokens, 'source>>,
    /// `Порядок` and `Первые` of `ДвиженияССубконто`.
    order: Option<&'ast Expression<'tokens, 'source>>,
    top: Option<&'ast Expression<'tokens, 'source>>,
}

impl<'ast, 'tokens, 'source> Arguments<'ast, 'tokens, 'source> {
    fn read(
        snapshot: &MetadataSnapshot,
        object: &MetadataObject,
        virtual_table: &'ast AccumulationAst<'tokens, 'source>,
    ) -> Result<Self, QueryDiagnostic> {
        let at = |index: usize| virtual_table.arguments.get(index).and_then(Option::as_ref);
        let extra = has_extra_dimensions(snapshot, object);
        // The slot of every named argument with and without extra
        // dimensions; `None` where the table has no such argument.
        let (layout, count): (ArgumentLayout, usize) = match virtual_table.kind {
            AccumulationKind::Balance => (
                &[
                    ("begin", Some(0), Some(0)),
                    ("account", Some(1), Some(1)),
                    ("extra", Some(2), None),
                    ("condition", Some(3), Some(2)),
                ],
                3,
            ),
            AccumulationKind::Turnovers => (
                &[
                    ("begin", Some(0), Some(0)),
                    ("end", Some(1), Some(1)),
                    ("periodicity", Some(2), Some(2)),
                    ("account", Some(3), Some(3)),
                    ("extra", Some(4), None),
                    ("condition", Some(5), Some(4)),
                    ("balanced_account", Some(6), Some(5)),
                    ("balanced_extra", Some(7), None),
                ],
                6,
            ),
            AccumulationKind::BalanceAndTurnovers => (
                &[
                    ("begin", Some(0), Some(0)),
                    ("end", Some(1), Some(1)),
                    ("periodicity", Some(2), Some(2)),
                    ("completion", Some(3), Some(3)),
                    ("account", Some(4), Some(4)),
                    ("extra", Some(5), None),
                    ("condition", Some(6), Some(5)),
                ],
                6,
            ),
            AccumulationKind::DrCrTurnovers => (
                &[
                    ("begin", Some(0), Some(0)),
                    ("end", Some(1), Some(1)),
                    ("periodicity", Some(2), Some(2)),
                    ("account", Some(3), Some(3)),
                    ("extra", Some(4), None),
                    ("balanced_account", Some(5), Some(4)),
                    ("balanced_extra", Some(6), None),
                    ("condition", Some(7), Some(5)),
                ],
                6,
            ),
            AccumulationKind::RecordsWithExtDimensions => (
                &[
                    ("begin", Some(0), Some(0)),
                    ("end", Some(1), Some(1)),
                    ("condition", Some(2), Some(2)),
                    ("order", Some(3), Some(3)),
                    ("top", Some(4), Some(4)),
                ],
                5,
            ),
        };
        if !extra && virtual_table.arguments.len() > count {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(virtual_table.token),
                format!(
                    "{} accepts at most {count} arguments for a register without extra dimensions",
                    virtual_table.kind.name()
                ),
            ));
        }
        let mut arguments = Self::default();
        for (name, with, without) in layout {
            let slot = if extra { *with } else { *without };
            let value = slot.and_then(at);
            match *name {
                "begin" => arguments.begin = value,
                "end" => arguments.end = value,
                "periodicity" => arguments.periodicity = value,
                "completion" => arguments.completion = value,
                "account" => arguments.account_condition = value,
                "extra" => arguments.extra_dimensions = value,
                "condition" => arguments.condition = value,
                "balanced_account" => arguments.balanced_account = value,
                "balanced_extra" => arguments.balanced_extra_dimensions = value,
                "order" => arguments.order = value,
                "top" => arguments.top = value,
                _ => {}
            }
        }
        Ok(arguments)
    }
}

/// The slot of every named argument of a table: with and without extra
/// dimensions, `None` where the table has no such argument.
type ArgumentLayout = &'static [(&'static str, Option<usize>, Option<usize>)];

/// Whether the register keeps extra-dimension values: its DBNames carry
/// an `AccRgED` table for the register's GUID.
fn has_extra_dimensions(snapshot: &MetadataSnapshot, object: &MetadataObject) -> bool {
    snapshot
        .db_names()
        .entries()
        .iter()
        .any(|entry| entry.guid == object.guid && entry.alias == "AccRgED")
}

/// A dimension or resource of the register with the columns each side
/// reads: one queryable field per side, and the unified field the folded
/// relation exposes.
struct FoldedField {
    /// The field as the relation exposes it, with unified column names.
    unified: QueryableField,
    /// The field each side reads: the same field for a balance one, the
    /// `Дт`/`Кт` field for a non-balance one.
    sides: [QueryableField; 2],
    /// The expression each side projects per unified column when it is
    /// not the side field's own column: an extra dimension picked by its
    /// listed kind.
    side_sql: Option<[Vec<String>; 2]>,
}

/// Everything one table compilation reads.
struct Table<'a, 'tokens, 'source> {
    source: &'a SourceAst<'tokens, 'source>,
    virtual_table: &'a AccumulationAst<'tokens, 'source>,
    snapshot: &'a MetadataSnapshot,
    catalog: &'a CompilationCatalog<'a>,
    object: &'a MetadataObject,
    live_table: &'a LiveTable,
    fields: &'a [QueryableField],
    restriction: Option<&'a SourceRestriction<'a>>,
    dialect: SqlDialect,
}

/// How the folded rows are split beyond the account and the dimensions.
enum Split {
    /// One row per combination of the dimensions over the whole interval.
    None,
    /// One row per calendar period of the records.
    Calendar(PeriodKind),
    /// One row per recorder (`Регистратор`) or per record (`Запись`).
    Record { line_number: bool },
    /// `Авто`: the record period, its calendar levels, the recorder and the
    /// line number as dimensions that split only when read.
    Auto,
}

/// The two branches of the fold with what the outer aggregation groups by.
struct Fold {
    /// The debit and the credit branch, each a complete `SELECT`.
    branches: Vec<String>,
    /// `Счет`, the dimensions, and under `Авто` the split fields, in the
    /// order the outer relation exposes them.
    dimension_fields: Vec<QueryableField>,
    /// The fields a calendar or record split adds, kept even when unread.
    split_fields: Vec<QueryableField>,
    /// Indexes into `dimension_fields` of the `Авто` split fields.
    auto_dimensions: Vec<usize>,
    /// The resources under their unified names.
    resources: Vec<FoldedField>,
    /// The unified column of the record period in the branches.
    period: QueryableColumn,
}

impl Table<'_, '_, '_> {
    fn split(&self, arguments: &Arguments<'_, '_, '_>) -> Result<Split, QueryDiagnostic> {
        let periodicity = arguments
            .periodicity
            .map(|expression| turnovers_periodicity(expression, self.virtual_table))
            .transpose()?
            .flatten();
        Ok(match periodicity {
            None => Split::None,
            Some(TurnoverPeriodicity::Calendar(unit)) => Split::Calendar(unit),
            Some(TurnoverPeriodicity::Recorder) => Split::Record { line_number: false },
            Some(TurnoverPeriodicity::Record) => Split::Record { line_number: true },
            Some(TurnoverPeriodicity::Auto) => Split::Auto,
        })
    }

    fn period_bound(
        &self,
        expression: Option<&Expression<'_, '_>>,
        argument: &str,
    ) -> Result<Option<String>, QueryDiagnostic> {
        expression
            .map(|expression| {
                compile_virtual_period_literal(
                    expression,
                    self.virtual_table,
                    argument,
                    self.catalog,
                    self.dialect,
                )
            })
            .transpose()
    }

    /// Folds the register's records into their two sides: each branch
    /// projects the side code, the side's account, the unified dimensions
    /// and resources, and the split columns, and filters active records by
    /// the period predicates, the account condition and the condition.
    fn fold(
        &self,
        arguments: &Arguments<'_, '_, '_>,
        split: &Split,
        carry_period: bool,
        period_predicates: &dyn Fn(&str) -> Vec<String>,
    ) -> Result<Fold, QueryDiagnostic> {
        let dialect = self.dialect;
        let mut register = Register::classify(self)?;
        let corresponding = self.virtual_table.kind == AccumulationKind::Turnovers;
        if corresponding {
            register.correspondence();
        }
        register.extra_dimensions(self, arguments, corresponding)?;
        let period = register_standard_field(self.fields, "Period", self.virtual_table)?;
        let period_column = single(period, self.virtual_table)?.clone();
        let active = register_standard_field(self.fields, "Active", self.virtual_table)?;
        let active_column = single(active, self.virtual_table)?.clone();
        let recorder = register_standard_field(self.fields, "Recorder", self.virtual_table)?;
        let line_number = register_standard_field(self.fields, "LineNo", self.virtual_table)?;

        // What a split adds to every branch: the columns and their
        // expressions over the base alias, with the field they expose.
        let mut split_fields: Vec<QueryableField> = Vec::new();
        let mut auto_fields: Vec<QueryableField> = Vec::new();
        let mut split_columns: Vec<(String, String)> = Vec::new();
        let qualified_period =
            dialect.qualified_column(Some(BASE_ALIAS), &period_column.physical_name);
        let raw = |field: &QueryableField| -> Vec<(String, String)> {
            field
                .columns
                .iter()
                .map(|column| {
                    (
                        dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name),
                        column.physical_name.clone(),
                    )
                })
                .collect()
        };
        // The opening balance of `ОстаткиИОбороты` tells the records before
        // the interval by their period, which no split projects: the
        // branches carry it, ungrouped, when asked.
        let mut carried_columns: Vec<(String, String)> = Vec::new();
        if carry_period {
            // The raw record period under its own name, whatever the
            // split projects as `_period`; the recorder and the line
            // number unless the split projects them itself.
            carried_columns.push((
                dialect.qualified_column(Some(BASE_ALIAS), &period_column.physical_name),
                RECORD_PERIOD_COLUMN.to_owned(),
            ));
            if !matches!(split, Split::Record { .. } | Split::Auto) {
                carried_columns.extend(raw(recorder));
            }
            if !matches!(split, Split::Record { line_number: true } | Split::Auto) {
                carried_columns.extend(raw(line_number));
            }
        }
        match split {
            Split::None => {}
            Split::Calendar(unit) => {
                split_columns.push((
                    dialect.begin_of_period(&qualified_period, *unit),
                    period_column.physical_name.clone(),
                ));
                split_fields.push(turnovers_period_field(period));
            }
            Split::Record {
                line_number: by_line,
            } => {
                split_columns.extend(raw(period));
                split_fields.push(turnovers_period_field(period));
                split_columns.extend(raw(recorder));
                split_fields.push(recorder.clone());
                if *by_line {
                    split_columns.extend(raw(line_number));
                    split_fields.push(line_number.clone());
                }
            }
            Split::Auto => {
                split_columns.extend(raw(period));
                auto_fields.push(turnovers_period_field(period));
                for (unit, russian, english) in AUTO_PERIOD_LEVELS {
                    let field = auto_period_level_field(period, russian, english);
                    split_columns.push((
                        dialect.begin_of_period(&qualified_period, unit),
                        field.columns[0].physical_name.clone(),
                    ));
                    auto_fields.push(field);
                }
                for field in [recorder, line_number] {
                    split_columns.extend(raw(field));
                    auto_fields.push(field.clone());
                }
            }
        }

        let mut branches = Vec::with_capacity(2);
        for (index, side) in [DEBIT, CREDIT].into_iter().enumerate() {
            let qualified = |column: &QueryableColumn| {
                dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name)
            };
            let mut projections = vec![
                format!("{} AS {}", side.code, dialect.quote_identifier(SIDE_COLUMN)),
                format!(
                    "{} AS {}",
                    qualified(&register.accounts[index].columns[0]),
                    dialect.quote_identifier(ACCOUNT_COLUMN)
                ),
            ];
            for folded in register.dimensions.iter().chain(&register.resources) {
                for (position, (unified, own)) in folded
                    .unified
                    .columns
                    .iter()
                    .zip(&folded.sides[index].columns)
                    .enumerate()
                {
                    let expression = folded
                        .side_sql
                        .as_ref()
                        .map_or_else(|| qualified(own), |sides| sides[index][position].clone());
                    projections.push(format!(
                        "{expression} AS {}",
                        dialect.quote_identifier(&unified.physical_name)
                    ));
                }
            }
            for (expression, column) in split_columns.iter().chain(&carried_columns) {
                projections.push(format!(
                    "{expression} AS {}",
                    dialect.quote_identifier(column)
                ));
            }
            let mut predicates = vec![format!(
                "{} = {}",
                qualified(&active_column),
                dialect.boolean_literal(true)
            )];
            predicates.extend(period_predicates(&qualified_period));
            predicates.extend(register.extra_predicates[index].iter().cloned());
            // The account condition and the condition see the side's view
            // of the record; a dereference in either joins its target to
            // the branch.
            let side_fields = register.side_fields(index, self.fields, self.virtual_table)?;
            let conditions = [
                arguments.account_condition,
                arguments.balanced_account,
                arguments.condition,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            let condition = self.condition(&conditions, &side_fields, None)?;
            if let Some(sql) = &condition.predicate {
                predicates.push(sql.clone());
            }
            branches.push(format!(
                "SELECT {} FROM {} AS {}{} WHERE {}",
                projections.join(", "),
                dialect.quote_identifier(&self.live_table.name),
                dialect.quote_identifier(BASE_ALIAS),
                condition.joins_sql(dialect),
                conjunction(predicates).expect("the activity predicate is always present")
            ));
        }

        let mut dimension_fields = vec![register.account_field()];
        dimension_fields.extend(
            register
                .dimensions
                .iter()
                .map(|folded| folded.unified.clone()),
        );
        let auto_dimensions = (dimension_fields.len()..dimension_fields.len() + auto_fields.len())
            .collect::<Vec<_>>();
        dimension_fields.extend(auto_fields);
        Ok(Fold {
            branches,
            dimension_fields,
            split_fields,
            auto_dimensions,
            resources: register.resources,
            period: period_column,
        })
    }

    /// The conditions compiled against one side's fields, with the
    /// separator predicates and the access restriction.
    fn condition(
        &self,
        conditions: &[&Expression<'_, '_>],
        fields: &[QueryableField],
        mirror: Option<&[QueryableField]>,
    ) -> Result<ConditionSql, QueryDiagnostic> {
        compile_accumulation_condition(
            conditions,
            self.source,
            self.virtual_table,
            self.snapshot,
            self.catalog,
            self.object,
            fields,
            mirror,
            self.live_table,
            BASE_ALIAS,
            self.restriction,
            self.dialect,
        )
    }

    /// Builds the outer aggregation over the folded rows: the dimensions
    /// and split fields grouped, the resource columns as given, then the
    /// derived columns computed from the group sums.
    fn aggregate(
        &self,
        fold: Fold,
        resources: Vec<ResourceColumn>,
        derived: Vec<(DerivedRule, QueryableField)>,
        having: Option<String>,
    ) -> CompiledSourceRelation {
        let dialect = self.dialect;
        let mut projections = Vec::new();
        let mut grouping = Vec::new();
        for field in fold.dimension_fields.iter().chain(&fold.split_fields) {
            for column in &field.columns {
                let sql = dialect.qualified_column(Some(SIDES_ALIAS), &column.physical_name);
                projections.push(format!(
                    "{sql} AS {}",
                    dialect.quote_identifier(&column.physical_name)
                ));
                grouping.push(sql);
            }
        }
        let mut resource_fields = Vec::new();
        for resource in &resources {
            projections.push(format!(
                "{} AS {}",
                resource.projection(),
                dialect.quote_identifier(&resource.field.columns[0].physical_name)
            ));
            resource_fields.push(resource.field.clone());
        }
        for (rule, _) in &derived {
            let sum = format!("SUM({})", rule.base_aggregate);
            projections.push(format!(
                "{} AS {}",
                rule.resource.expression(&sum),
                dialect.quote_identifier(&rule.resource.column)
            ));
        }
        let mut relation = format!(
            "(SELECT {} FROM ({}) AS {}",
            projections.join(", "),
            fold.branches.join(" UNION ALL "),
            dialect.quote_identifier(SIDES_ALIAS)
        );
        if !grouping.is_empty() {
            relation.push_str(" GROUP BY ");
            relation.push_str(&grouping.join(", "));
        }
        if let Some(having) = having {
            relation.push_str(" HAVING ");
            relation.push_str(&having);
        }
        relation.push(')');
        let mut aggregate = aggregate_source(&fold.dimension_fields, &resource_fields);
        aggregate.split = fold
            .split_fields
            .iter()
            .flat_map(|field| field.columns.iter())
            .map(|column| column.physical_name.clone())
            .collect();
        aggregate.split_dimensions = fold.auto_dimensions.clone();
        aggregate.derived = derived
            .iter()
            .map(|(rule, _)| rule.resource.clone())
            .collect();
        let mut fields = fold.dimension_fields;
        fields.extend(fold.split_fields);
        fields.extend(resource_fields);
        fields.extend(derived.into_iter().map(|(_, field)| field));
        CompiledSourceRelation {
            sql: relation,
            fields: fields.into(),
            aggregate: Some(aggregate),
            separators: Vec::new(),
        }
    }

    /// `Обороты`: `<Ресурс>Оборот` (debit minus credit), `<Ресурс>ОборотДт`
    /// and `<Ресурс>ОборотКт` over the active records of `[Начало, Конец)`.
    fn turnovers(
        &self,
        arguments: &Arguments<'_, '_, '_>,
    ) -> Result<CompiledSourceRelation, QueryDiagnostic> {
        let split = self.split(arguments)?;
        let begin = self.period_bound(arguments.begin, "begin period")?;
        let end = self.period_bound(arguments.end, "period boundary")?;
        let fold = self.fold(arguments, &split, false, &|period| {
            let mut predicates = Vec::new();
            if let Some(begin) = &begin {
                predicates.push(format!("({period} >= {begin})"));
            }
            if let Some(end) = &end {
                predicates.push(format!("({period} < {end})"));
            }
            predicates
        })?;
        let side = self
            .dialect
            .qualified_column(Some(SIDES_ALIAS), SIDE_COLUMN);
        let mut resources = Vec::new();
        for folded in &fold.resources {
            let column = &folded.unified.columns[0];
            let value = self
                .dialect
                .qualified_column(Some(SIDES_ALIAS), &column.physical_name);
            resources.extend([
                ResourceColumn::new(
                    folded,
                    ("Оборот", "Turnover"),
                    &column.physical_name,
                    format!("CASE WHEN {side} = 0 THEN {value} ELSE -{value} END"),
                ),
                ResourceColumn::new(
                    folded,
                    ("ОборотДт", "TurnoverDr"),
                    &format!("{}TurnoverDt", column.physical_name),
                    format!("CASE WHEN {side} = 0 THEN {value} ELSE 0 END"),
                ),
                ResourceColumn::new(
                    folded,
                    ("ОборотКт", "TurnoverCr"),
                    &format!("{}TurnoverCt", column.physical_name),
                    format!("CASE WHEN {side} = 1 THEN {value} ELSE 0 END"),
                ),
            ]);
        }
        Ok(self.aggregate(fold, resources, Vec::new(), None))
    }

    /// `Остатки`: `<Ресурс>Остаток` (debit minus credit over the active
    /// records before `Период`), `<Ресурс>ОстатокДт` and `<Ресурс>ОстатокКт`
    /// as the positive and the negated negative part at the grain read;
    /// combinations whose every balance is zero are dropped.
    fn balance(
        &self,
        arguments: &Arguments<'_, '_, '_>,
    ) -> Result<CompiledSourceRelation, QueryDiagnostic> {
        let boundary = self.period_bound(arguments.begin, "period boundary")?;
        let fold = self.fold(arguments, &Split::None, false, &|period| {
            boundary
                .iter()
                .map(|boundary| format!("({period} < {boundary})"))
                .collect()
        })?;
        let side = self
            .dialect
            .qualified_column(Some(SIDES_ALIAS), SIDE_COLUMN);
        let mut resources = Vec::new();
        let mut derived = Vec::new();
        let mut nonzero = Vec::new();
        for folded in &fold.resources {
            let column = &folded.unified.columns[0];
            let value = self
                .dialect
                .qualified_column(Some(SIDES_ALIAS), &column.physical_name);
            let signed = format!("CASE WHEN {side} = 0 THEN {value} ELSE -{value} END");
            nonzero.push(format!("SUM({signed}) <> 0"));
            resources.push(ResourceColumn::new(
                folded,
                ("Остаток", "Balance"),
                &column.physical_name,
                signed.clone(),
            ));
            resources.extend(expanded_parts(
                folded,
                &column.physical_name,
                &signed,
                ("РазвернутыйОстатокДт", "ExpandedBalanceDr"),
                ("РазвернутыйОстатокКт", "ExpandedBalanceCr"),
            ));
            derived.extend(balance_parts(
                folded,
                &column.physical_name,
                &signed,
                ("ОстатокДт", "BalanceDr"),
                ("ОстатокКт", "BalanceCr"),
            ));
        }
        let having = Some(format!("({})", nonzero.join(" OR ")));
        Ok(self.aggregate(fold, resources, derived, having))
    }

    /// `ОстаткиИОбороты`: the opening balance (records before `Начало`),
    /// the turnovers of `[Начало, Конец)` and the closing balance, with
    /// the debit and credit parts of both balances derived at the grain
    /// read. A calendar or record split answers no balance column; under
    /// `Авто` the balances are refused only when a split field is read.
    fn balance_and_turnovers(
        &self,
        arguments: &Arguments<'_, '_, '_>,
    ) -> Result<CompiledSourceRelation, QueryDiagnostic> {
        let split = self.split(arguments)?;
        let periodic = !matches!(split, Split::None | Split::Auto);
        if let Some(completion) = arguments.completion {
            let token =
                super::expression::operand_token(completion).unwrap_or(self.virtual_table.token);
            let name = match completion {
                Expression::Field(reference) if reference.segments.len() == 1 => reference.last(),
                _ => token,
            };
            if ![
                "Движения",
                "Movements",
                "ДвиженияИГраницыПериода",
                "MovementsAndBoundaries",
            ]
            .iter()
            .any(|known| names_equal(known, name.lexeme))
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    format!(
                        "BalanceAndTurnovers period completion method {:?} is not supported",
                        name.lexeme
                    ),
                ));
            }
            // Without a periodicity the method has nothing to complete;
            // the platform accepts it there, and real configurations
            // write it.
        }
        let begin = self.period_bound(arguments.begin, "begin period")?;
        let end = self.period_bound(arguments.end, "period boundary")?;
        let fold = self.fold(arguments, &split, true, &|period| {
            let mut predicates = Vec::new();
            // A split table reads the interval alone: it answers no
            // balance, and every period it reports has movements.
            if periodic && let Some(begin) = &begin {
                predicates.push(format!("({period} >= {begin})"));
            }
            if let Some(end) = &end {
                predicates.push(format!("({period} < {end})"));
            }
            predicates
        })?;
        let dialect = self.dialect;
        let side = dialect.qualified_column(Some(SIDES_ALIAS), SIDE_COLUMN);
        let period = dialect.qualified_column(Some(SIDES_ALIAS), RECORD_PERIOD_COLUMN);
        // The record period is projected by the split or, without one,
        // by nobody — the opening balance needs it, so the branches carry
        // it under its own name when no split does.
        let mut resources = Vec::new();
        let mut derived = Vec::new();
        for folded in &fold.resources {
            let column = &folded.unified.columns[0];
            let value = dialect.qualified_column(Some(SIDES_ALIAS), &column.physical_name);
            let signed = format!("CASE WHEN {side} = 0 THEN {value} ELSE -{value} END");
            let (before, inside) = match &begin {
                Some(begin) => (
                    format!("CASE WHEN {period} < {begin} THEN {signed} ELSE 0 END"),
                    format!("CASE WHEN {period} >= {begin} THEN {signed} ELSE 0 END"),
                ),
                None => ("0".to_owned(), signed.clone()),
            };
            let inside_side = |code: u8| match &begin {
                Some(begin) => format!(
                    "CASE WHEN {period} >= {begin} AND {side} = {code} THEN {value} ELSE 0 END"
                ),
                None => format!("CASE WHEN {side} = {code} THEN {value} ELSE 0 END"),
            };
            let base = column.physical_name.clone();
            resources.extend([
                ResourceColumn::new(
                    folded,
                    ("НачальныйОстаток", "OpeningBalance"),
                    &format!("{base}OpeningBalance"),
                    before.clone(),
                ),
                ResourceColumn::new(folded, ("Оборот", "Turnover"), &base, inside.clone()),
                ResourceColumn::new(
                    folded,
                    ("ОборотДт", "TurnoverDr"),
                    &format!("{base}TurnoverDt"),
                    inside_side(0),
                ),
                ResourceColumn::new(
                    folded,
                    ("ОборотКт", "TurnoverCr"),
                    &format!("{base}TurnoverCt"),
                    inside_side(1),
                ),
                ResourceColumn::new(
                    folded,
                    ("КонечныйОстаток", "ClosingBalance"),
                    &format!("{base}ClosingBalance"),
                    signed.clone(),
                ),
            ]);
            resources.extend(expanded_parts(
                folded,
                &format!("{base}OpeningBalance"),
                &before,
                ("НачальныйРазвернутыйОстатокДт", "OpeningExpandedBalanceDr"),
                ("НачальныйРазвернутыйОстатокКт", "OpeningExpandedBalanceCr"),
            ));
            resources.extend(expanded_parts(
                folded,
                &format!("{base}ClosingBalance"),
                &signed,
                ("КонечныйРазвернутыйОстатокДт", "ClosingExpandedBalanceDr"),
                ("КонечныйРазвернутыйОстатокКт", "ClosingExpandedBalanceCr"),
            ));
            derived.extend(balance_parts(
                folded,
                &format!("{base}OpeningBalance"),
                &before,
                ("НачальныйОстатокДт", "OpeningBalanceDr"),
                ("НачальныйОстатокКт", "OpeningBalanceCr"),
            ));
            derived.extend(balance_parts(
                folded,
                &format!("{base}ClosingBalance"),
                &signed,
                ("КонечныйОстатокДт", "ClosingBalanceDr"),
                ("КонечныйОстатокКт", "ClosingBalanceCr"),
            ));
        }
        let auto = matches!(split, Split::Auto);
        // What the windowed variants read: the folded rows and the record
        // columns the branches carry.
        let rows = format!(
            "FROM ({}) AS {}",
            fold.branches.join(" UNION ALL "),
            dialect.quote_identifier(SIDES_ALIAS)
        );
        let dimension_columns = fold
            .dimension_fields
            .iter()
            .enumerate()
            .filter(|(index, _)| !fold.auto_dimensions.contains(index))
            .flat_map(|(_, field)| field.columns.iter())
            .map(|column| column.physical_name.clone())
            .collect::<Vec<_>>();
        let period_column = fold.period.physical_name.clone();
        let recorder = register_standard_field(self.fields, "Recorder", self.virtual_table)?
            .columns
            .iter()
            .map(|column| column.physical_name.clone())
            .collect::<Vec<_>>();
        let line = register_standard_field(self.fields, "LineNo", self.virtual_table)
            .ok()
            .and_then(|field| field.columns.first())
            .map(|column| column.physical_name.clone());
        let mut bucket_sums = Vec::new();
        let mut running = Vec::new();
        for folded in &fold.resources {
            let column = &folded.unified.columns[0];
            let value = dialect.qualified_column(Some(SIDES_ALIAS), &column.physical_name);
            let signed = format!("CASE WHEN {side} = 0 THEN {value} ELSE -{value} END");
            let base = column.physical_name.clone();
            bucket_sums.push(BucketSum {
                column: base.clone(),
                expression: signed.clone(),
            });
            bucket_sums.push(BucketSum {
                column: format!("{base}TurnoverDt"),
                expression: format!("CASE WHEN {side} = 0 THEN {value} ELSE 0 END"),
            });
            bucket_sums.push(BucketSum {
                column: format!("{base}TurnoverCt"),
                expression: format!("CASE WHEN {side} = 1 THEN {value} ELSE 0 END"),
            });
            running.push(RunningSum {
                column: format!("{base}OpeningBalance"),
                expression: signed.clone(),
                exclusive: true,
            });
            running.push(RunningSum {
                column: format!("{base}ClosingBalance"),
                expression: signed,
                exclusive: false,
            });
        }
        let derived_resources = derived
            .iter()
            .map(|(rule, _)| rule.resource.clone())
            .collect::<Vec<_>>();
        let mut relation = self.aggregate(fold, resources, derived, None);
        if periodic || auto {
            let aggregate = relation
                .aggregate
                .as_mut()
                .expect("an aggregating relation");
            let balances = relation
                .fields
                .iter()
                .enumerate()
                .filter(|(_, field)| {
                    ["Остаток", "Balance"].iter().any(|suffix| {
                        field.name.ends_with(suffix) || field.schema_name.ends_with(suffix)
                    }) || ["ОстатокДт", "ОстатокКт", "BalanceDr", "BalanceCr"]
                        .iter()
                        .any(|suffix| {
                            field.name.ends_with(suffix) || field.schema_name.ends_with(suffix)
                        })
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            // The expanded balances are parts of the finest-grain sums,
            // which a running sum over buckets does not carry.
            let expanded = relation
                .fields
                .iter()
                .enumerate()
                .filter(|(_, field)| {
                    field.name.contains("РазвернутыйОстаток")
                        || field.schema_name.contains("ExpandedBalance")
                })
                .map(|(index, _)| {
                    (
                        index,
                        "a periodic BalanceAndTurnovers answers no expanded balance column",
                    )
                })
                .collect::<Vec<_>>();
            if dialect.running_sums() {
                if auto {
                    aggregate.forbidden_with_split = expanded;
                } else {
                    aggregate.forbidden = expanded;
                }
                let spec = WindowedSpec {
                    rows: &rows,
                    alias: SIDES_ALIAS,
                    dimension_columns,
                    period: period.clone(),
                    period_column: &period_column,
                    recorder,
                    line,
                    begin: begin.clone(),
                    bucket_sums,
                    running,
                    derived: derived_resources,
                    auto_levels: auto,
                };
                aggregate.variants = match split {
                    Split::Calendar(unit) => vec![RelationVariant {
                        grain: None,
                        balances: true,
                        sql: windowed_relation(&spec, Grain::Calendar(unit), dialect),
                    }],
                    Split::Record { line_number } => vec![RelationVariant {
                        grain: None,
                        balances: true,
                        sql: windowed_relation(
                            &spec,
                            if line_number {
                                Grain::Record
                            } else {
                                Grain::Recorder
                            },
                            dialect,
                        ),
                    }],
                    _ => auto_grains()
                        .into_iter()
                        .map(|grain| RelationVariant {
                            grain: Some(grain),
                            balances: true,
                            sql: windowed_relation(&spec, grain, dialect),
                        })
                        .collect(),
                };
                aggregate.balance_fields = balances;
            } else {
                let forbidden = balances
                    .into_iter()
                    .map(|index| (index, NO_BALANCE_WHEN_SPLIT))
                    .collect();
                if auto {
                    aggregate.forbidden_with_split = forbidden;
                } else {
                    aggregate.forbidden = forbidden;
                }
            }
        }
        Ok(relation)
    }
}

impl Table<'_, '_, '_> {
    /// The main-table fields of one side under the names the two
    /// record-level tables expose: `СубконтоДт<k>`, `ВидСубконтоДт<k>` and
    /// the credit twins, read from the inline columns.
    fn side_extra_dimension_fields(&self) -> Vec<QueryableField> {
        let mut result = Vec::new();
        for (side, russian, english) in [(0usize, "Дт", "Dr"), (1, "Кт", "Cr")] {
            for (level, (value, kind)) in self.side_levels(side).iter().enumerate() {
                let level = level + 1;
                result.push(renamed(
                    value,
                    &format!("Субконто{russian}{level}"),
                    &format!("ExtDimension{english}{level}"),
                ));
                result.push(renamed(
                    kind,
                    &format!("ВидСубконто{russian}{level}"),
                    &format!("ExtDimensionType{english}{level}"),
                ));
            }
        }
        result
    }

    /// The main-table fields a record-level table exposes: everything but
    /// the inline extra-dimension columns and the hashes, plus the extra
    /// dimensions under their query names.
    fn record_fields(&self) -> Vec<QueryableField> {
        let mut result = self
            .fields
            .iter()
            .filter(|field| {
                let name = field.schema_name.as_str();
                !(name.starts_with("ValueDt")
                    || name.starts_with("ValueCt")
                    || name.starts_with("KindDt")
                    || name.starts_with("KindCt")
                    || name.starts_with("EDHash"))
            })
            .cloned()
            .collect::<Vec<_>>();
        result.extend(self.side_extra_dimension_fields());
        result
    }

    /// `ОборотыДтКт`: one row per pair of accounts, the dimensions of both
    /// sides and the extra dimensions of both sides in use, over the
    /// active records of `[Начало, Конец)`; per balance resource
    /// `<Ресурс>Оборот`, per non-balance one `<Ресурс>ОборотДт` and
    /// `<Ресурс>ОборотКт`. The record is not folded: it is already the
    /// correspondence.
    fn dr_cr_turnovers(
        &self,
        arguments: &Arguments<'_, '_, '_>,
    ) -> Result<CompiledSourceRelation, QueryDiagnostic> {
        let dialect = self.dialect;
        let split = self.split(arguments)?;
        let begin = self.period_bound(arguments.begin, "begin period")?;
        let end = self.period_bound(arguments.end, "period boundary")?;
        let register = Register::classify(self)?;
        let period = register_standard_field(self.fields, "Period", self.virtual_table)?;
        let period_column = single(period, self.virtual_table)?.clone();
        let active = register_standard_field(self.fields, "Active", self.virtual_table)?;
        let active_column = single(active, self.virtual_table)?.clone();
        let qualified = |column: &QueryableColumn| {
            dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name)
        };
        let qualified_period = qualified(&period_column);

        let mut dimension_fields = vec![
            renamed(&register.accounts[0], "СчетДт", "AccountDr"),
            renamed(&register.accounts[1], "СчетКт", "AccountCr"),
        ];
        for folded in &register.dimensions {
            if folded.sides[0] == folded.sides[1] {
                dimension_fields.push(folded.unified.clone());
            } else {
                dimension_fields.push(folded.sides[0].clone());
                dimension_fields.push(folded.sides[1].clone());
            }
        }
        let mut projections = Vec::new();
        let mut grouping = Vec::new();
        for field in &dimension_fields {
            project_dimension(field, dialect, &mut projections, &mut grouping);
        }
        let mut predicates = vec![format!(
            "{} = {}",
            qualified(&active_column),
            dialect.boolean_literal(true)
        )];
        if let Some(begin) = &begin {
            predicates.push(format!("({qualified_period} >= {begin})"));
        }
        if let Some(end) = &end {
            predicates.push(format!("({qualified_period} < {end})"));
        }
        // The extra dimensions of each side, positional or by the listed
        // kinds of that side; a listed kind excludes the records whose
        // account on that side lacks it.
        for (side, listed, russian, english) in [
            (0usize, arguments.extra_dimensions, "Дт", "Dr"),
            (1, arguments.balanced_extra_dimensions, "Кт", "Cr"),
        ] {
            let levels = self.side_levels(side);
            if levels.is_empty() {
                continue;
            }
            let Some(kinds) = self.listed_kinds(listed)? else {
                for (level, (value, kind)) in levels.iter().enumerate() {
                    let level = level + 1;
                    let value = renamed(
                        value,
                        &format!("Субконто{russian}{level}"),
                        &format!("ExtDimension{english}{level}"),
                    );
                    let kind = renamed(
                        kind,
                        &format!("ВидСубконто{russian}{level}"),
                        &format!("ExtDimensionType{english}{level}"),
                    );
                    project_dimension(&value, dialect, &mut projections, &mut grouping);
                    project_dimension(&kind, dialect, &mut projections, &mut grouping);
                    dimension_fields.push(value);
                    dimension_fields.push(kind);
                }
                continue;
            };
            if kinds.len() > levels.len() {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(self.virtual_table.token),
                    format!(
                        "{} lists {} kinds of extra dimensions, the register keeps {}",
                        self.virtual_table.kind.name(),
                        kinds.len(),
                        levels.len()
                    ),
                ));
            }
            let kind_of = |level: usize| qualified(&levels[level].1.columns[0]);
            for (position, kind_sql) in kinds.iter().enumerate() {
                let (value, kind) = &levels[position];
                let position = position + 1;
                let value = renamed(
                    value,
                    &format!("Субконто{russian}{position}"),
                    &format!("ExtDimension{english}{position}"),
                );
                for (member, column) in value.columns.iter().enumerate() {
                    let branches = levels
                        .iter()
                        .enumerate()
                        .map(|(level, (value, _))| {
                            format!(
                                "WHEN {} = {kind_sql} THEN {}",
                                kind_of(level),
                                qualified(&value.columns[member])
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let sql = format!("CASE {branches} END");
                    projections.push(format!(
                        "{sql} AS {}",
                        dialect.quote_identifier(&column.physical_name)
                    ));
                    grouping.push(sql);
                }
                dimension_fields.push(value);
                let kind = renamed(
                    kind,
                    &format!("ВидСубконто{russian}{position}"),
                    &format!("ExtDimensionType{english}{position}"),
                );
                let present = (0..levels.len())
                    .map(|level| format!("{} = {kind_sql}", kind_of(level)))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let sql = format!("CASE WHEN {present} THEN {kind_sql} END");
                projections.push(format!(
                    "{sql} AS {}",
                    dialect.quote_identifier(&kind.columns[0].physical_name)
                ));
                grouping.push(sql);
                dimension_fields.push(kind);
                predicates.push(format!("({present})"));
            }
        }
        // The split: a calendar period, the record's recorder and line,
        // or the `Авто` levels as prunable dimensions.
        let mut split_fields = Vec::new();
        let auto_start = dimension_fields.len();
        match &split {
            Split::None => {}
            Split::Calendar(unit) => {
                let truncated = dialect.begin_of_period(&qualified_period, *unit);
                projections.push(format!(
                    "{truncated} AS {}",
                    dialect.quote_identifier(&period_column.physical_name)
                ));
                grouping.push(truncated);
                split_fields.push(turnovers_period_field(period));
            }
            Split::Record { line_number } => {
                let mut fields = vec![
                    turnovers_period_field(period),
                    register_standard_field(self.fields, "Recorder", self.virtual_table)?.clone(),
                ];
                if *line_number {
                    fields.push(
                        register_standard_field(self.fields, "LineNo", self.virtual_table)?.clone(),
                    );
                }
                for field in &fields {
                    project_dimension(field, dialect, &mut projections, &mut grouping);
                }
                split_fields = fields;
            }
            Split::Auto => {
                let period_field = turnovers_period_field(period);
                project_dimension(&period_field, dialect, &mut projections, &mut grouping);
                dimension_fields.push(period_field);
                for (unit, russian, english) in AUTO_PERIOD_LEVELS {
                    let field = auto_period_level_field(period, russian, english);
                    let truncated = dialect.begin_of_period(&qualified_period, unit);
                    projections.push(format!(
                        "{truncated} AS {}",
                        dialect.quote_identifier(&field.columns[0].physical_name)
                    ));
                    grouping.push(truncated);
                    dimension_fields.push(field);
                }
                for schema_name in ["Recorder", "LineNo"] {
                    let field =
                        register_standard_field(self.fields, schema_name, self.virtual_table)?;
                    project_dimension(field, dialect, &mut projections, &mut grouping);
                    dimension_fields.push(field.clone());
                }
            }
        }
        let auto_dimensions = (auto_start..dimension_fields.len()).collect::<Vec<_>>();
        // The conditions see the record as the main table shows it, the
        // extra dimensions under their query names.
        let record_fields = self.record_fields();
        let conditions = [
            arguments.account_condition,
            arguments.balanced_account,
            arguments.condition,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let condition = self.condition(&conditions, &record_fields, None)?;
        if let Some(sql) = &condition.predicate {
            predicates.push(sql.clone());
        }
        let mut resource_fields = Vec::new();
        for folded in &register.resources {
            let sides: Vec<(usize, (&str, &str))> = if folded.sides[0] == folded.sides[1] {
                vec![(0, ("Оборот", "Turnover"))]
            } else {
                vec![
                    (0, ("ОборотДт", "TurnoverDr")),
                    (1, ("ОборотКт", "TurnoverCr")),
                ]
            };
            for (side, suffix) in sides {
                let column = &folded.sides[side].columns[0];
                projections.push(format!(
                    "SUM({}) AS {}",
                    qualified(column),
                    dialect.quote_identifier(&column.physical_name)
                ));
                resource_fields.push(balance_and_turnover_field(
                    &folded.unified,
                    column,
                    suffix,
                    &column.physical_name,
                ));
            }
        }
        let mut relation = format!(
            "(SELECT {} FROM {} AS {}{} WHERE {}",
            projections.join(", "),
            dialect.quote_identifier(&self.live_table.name),
            dialect.quote_identifier(BASE_ALIAS),
            condition.joins_sql(dialect),
            predicates.join(" AND ")
        );
        if !grouping.is_empty() {
            relation.push_str(" GROUP BY ");
            relation.push_str(&grouping.join(", "));
        }
        relation.push(')');
        let mut aggregate = aggregate_source(&dimension_fields, &resource_fields);
        aggregate.split = split_fields
            .iter()
            .flat_map(|field| field.columns.iter())
            .map(|column| column.physical_name.clone())
            .collect();
        aggregate.split_dimensions = auto_dimensions;
        let mut fields = dimension_fields;
        fields.extend(split_fields);
        fields.extend(resource_fields);
        Ok(CompiledSourceRelation {
            sql: relation,
            fields: fields.into(),
            aggregate: Some(aggregate),
            separators: Vec::new(),
        })
    }

    /// `ДвиженияССубконто`: the records of `[Начало, Конец)` with the
    /// extra dimensions of both sides under their query names.
    fn records_with_ext_dimensions(
        &self,
        arguments: &Arguments<'_, '_, '_>,
    ) -> Result<CompiledSourceRelation, QueryDiagnostic> {
        let dialect = self.dialect;
        let begin = self.period_bound(arguments.begin, "begin period")?;
        let end = self.period_bound(arguments.end, "period boundary")?;
        let period = register_standard_field(self.fields, "Period", self.virtual_table)?;
        let period_column = single(period, self.virtual_table)?.clone();
        let qualified_period =
            dialect.qualified_column(Some(BASE_ALIAS), &period_column.physical_name);
        let mut predicates = Vec::new();
        if let Some(begin) = &begin {
            predicates.push(format!("({qualified_period} >= {begin})"));
        }
        if let Some(end) = &end {
            predicates.push(format!("({qualified_period} < {end})"));
        }
        let record_fields = self.record_fields();
        let conditions = arguments.condition.into_iter().collect::<Vec<_>>();
        // The condition also names the fields without a side — `Счет`,
        // `Субконто1`, a non-balance dimension — and holds when either
        // side satisfies it: the debit reading and the credit reading of
        // each such name.
        let register = Register::classify(self)?;
        let mut readings = [record_fields.clone(), record_fields.clone()];
        for (side, reading) in readings.iter_mut().enumerate() {
            reading.push(renamed(&register.accounts[side], "Счет", "Account"));
            for field in register.dimensions.iter().chain(&register.resources) {
                if field.sides[0].schema_name != field.sides[1].schema_name {
                    reading.push(renamed(
                        &field.sides[side],
                        &field.unified.name,
                        &field.unified.schema_name,
                    ));
                }
            }
            for (level, (value, kind)) in self.side_levels(side).iter().enumerate() {
                let level = level + 1;
                reading.push(renamed(
                    value,
                    &format!("Субконто{level}"),
                    &format!("ExtDimension{level}"),
                ));
                reading.push(renamed(
                    kind,
                    &format!("ВидСубконто{level}"),
                    &format!("ExtDimensionType{level}"),
                ));
            }
        }
        let [debit, credit] = readings;
        let condition = self.condition(&conditions, &debit, Some(&credit))?;
        if let Some(sql) = &condition.predicate {
            predicates.push(sql.clone());
        }
        let projections = record_fields
            .iter()
            .flat_map(|field| field.columns.iter())
            .map(|column| {
                format!(
                    "{} AS {}",
                    dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name),
                    dialect.quote_identifier(&column.physical_name)
                )
            })
            .collect::<Vec<_>>();
        // `Первые` keeps the first N records in the `Порядок` — the record
        // order by default. Without `Первые` the order has no effect on a
        // source, and SQL Server takes no ORDER BY in a subquery, so it
        // is dropped.
        let top = arguments
            .top
            .map(|expression| self.record_limit(expression))
            .transpose()?;
        let mut order = self.record_order(arguments.order, &record_fields)?;
        if top.is_some() && order.is_empty() {
            for schema_name in ["Period", "Recorder", "LineNo"] {
                let field = register_standard_field(self.fields, schema_name, self.virtual_table)?;
                order.extend(field.columns.iter().map(|column| {
                    dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name)
                }));
            }
        }
        let mut relation = format!(
            "({}{} FROM {} AS {}{}",
            dialect.select_prefix(false, top),
            projections.join(", "),
            dialect.quote_identifier(&self.live_table.name),
            dialect.quote_identifier(BASE_ALIAS),
            condition.joins_sql(dialect)
        );
        if let Some(predicate) = conjunction(predicates) {
            relation.push_str(" WHERE ");
            relation.push_str(&predicate);
        }
        if top.is_some() {
            relation.push_str(" ORDER BY ");
            relation.push_str(&order.join(", "));
            dialect.append_limit(&mut relation, top);
        }
        relation.push(')');
        Ok(CompiledSourceRelation {
            sql: relation,
            fields: record_fields.into(),
            aggregate: None,
            separators: Vec::new(),
        })
    }

    /// `Первые`: a number literal or a parameter bound to a number.
    fn record_limit(&self, expression: &Expression<'_, '_>) -> Result<u32, QueryDiagnostic> {
        let refuse = |token: &Token<'_>| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "the Первые argument takes a number literal or a parameter bound to a number",
            )
        };
        match expression {
            Expression::Literal(token) if token.kind == TokenKind::Number => {
                token.lexeme.parse::<u32>().map_err(|_| refuse(token))
            }
            Expression::Parameter(token) => match self.catalog.parameters().lookup(token)? {
                Some(crate::query::core::params::ParameterValue::Number { unscaled, scale: 0 }) => {
                    u32::try_from(*unscaled).map_err(|_| refuse(token))
                }
                None => Ok(0),
                Some(_) => Err(refuse(token)),
            },
            other => Err(refuse(
                operand_token(other).unwrap_or(self.virtual_table.token),
            )),
        }
    }

    /// `Порядок`: record fields, ascending, singly or as a tuple; a
    /// parameter bound to `NULL` (or unbound) names no order.
    fn record_order(
        &self,
        expression: Option<&Expression<'_, '_>>,
        fields: &[QueryableField],
    ) -> Result<Vec<String>, QueryDiagnostic> {
        let Some(expression) = expression else {
            return Ok(Vec::new());
        };
        let refuse = |token: &Token<'_>| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "the Порядок argument takes record fields, ascending, or a parameter bound to NULL",
            )
        };
        let items: Vec<&Expression<'_, '_>> = match expression {
            Expression::Tuple { items, .. } => items.iter().collect(),
            Expression::Parameter(token) => {
                return match self.catalog.parameters().lookup(token)? {
                    None | Some(crate::query::core::params::ParameterValue::Null) => Ok(Vec::new()),
                    Some(_) => Err(refuse(token)),
                };
            }
            single => vec![single],
        };
        let mut order = Vec::new();
        for item in items {
            let Expression::Field(reference) = item else {
                return Err(refuse(
                    operand_token(item).unwrap_or(self.virtual_table.token),
                ));
            };
            let [name] = reference.segments.as_slice() else {
                return Err(refuse(reference.last()));
            };
            let field = fields
                .iter()
                .find(|field| {
                    field
                        .aliases
                        .iter()
                        .any(|alias| names_equal(alias, name.lexeme))
                })
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::UnknownField,
                        Some(name),
                        format!("field {:?} was not found", name.lexeme),
                    )
                })?;
            order.extend(field.columns.iter().map(|column| {
                self.dialect
                    .qualified_column(Some(BASE_ALIAS), &column.physical_name)
            }));
        }
        Ok(order)
    }

    /// The inline value and kind fields of one side per level.
    fn side_levels(&self, side: usize) -> Vec<(QueryableField, QueryableField)> {
        let suffix = if side == 0 { "Dt" } else { "Ct" };
        let mut levels = Vec::new();
        for level in 1.. {
            let value = self
                .fields
                .iter()
                .find(|field| names_equal(&field.schema_name, &format!("Value{suffix}{level}")));
            let kind = self
                .fields
                .iter()
                .find(|field| names_equal(&field.schema_name, &format!("Kind{suffix}{level}")));
            let (Some(value), Some(kind)) = (value, kind) else {
                break;
            };
            levels.push((value.clone(), kind.clone()));
        }
        levels
    }

    /// The listed kinds of an extra-dimension argument as SQL, `None`
    /// when the argument is absent or names a parameter the compiler
    /// cannot see into.
    fn listed_kinds(
        &self,
        listed: Option<&Expression<'_, '_>>,
    ) -> Result<Option<Vec<String>>, QueryDiagnostic> {
        let Some(listed) = listed else {
            return Ok(None);
        };
        let requested: Vec<&Expression<'_, '_>> = match listed {
            Expression::Tuple { items, .. } => items.iter().collect(),
            single => vec![single],
        };
        let unknown = requested.iter().any(|kind| match kind {
            Expression::Parameter(token) => !matches!(
                self.catalog.parameters().lookup(token),
                Ok(Some(
                    crate::query::core::params::ParameterValue::Reference { .. }
                ))
            ),
            _ => false,
        });
        if unknown {
            return Ok(None);
        }
        requested
            .into_iter()
            .map(|kind| {
                super::sources::compile_source_free_expression(
                    kind,
                    self.snapshot,
                    self.dialect,
                    self.catalog.parameters(),
                    false,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}

/// Projects every column of a dimension of the base alias under its own
/// name and groups by it.
fn project_dimension(
    field: &QueryableField,
    dialect: SqlDialect,
    projections: &mut Vec<String>,
    grouping: &mut Vec<String>,
) {
    for column in &field.columns {
        let sql = dialect.qualified_column(Some(BASE_ALIAS), &column.physical_name);
        projections.push(format!(
            "{sql} AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        grouping.push(sql);
    }
}

/// A field under another query name, its columns untouched.
fn renamed(field: &QueryableField, name: &str, schema_name: &str) -> QueryableField {
    let mut result = field.clone();
    result.name = name.to_owned();
    result.schema_name = schema_name.to_owned();
    result.aliases = vec![name.to_owned(), schema_name.to_owned()];
    relabel(&mut result, name);
    result
}

/// Labels the columns of a field under a query name: a compound field's
/// columns carry the member suffix the union spreading and the aliasing
/// read — `Имя_TYPE`, `Имя_S`, and `Имя` itself for the reference member.
fn relabel(field: &mut QueryableField, name: &str) {
    let compound = field.columns.len() > 1;
    for column in &mut field.columns {
        column.output_label = if compound {
            format!("{name}{}", member_label_suffix(&column.physical_name))
        } else {
            name.to_owned()
        };
    }
}

/// The member suffix of a compound column by its physical name.
fn member_label_suffix(physical_name: &str) -> &'static str {
    let lower = physical_name.to_ascii_lowercase();
    [
        ("_type", "_TYPE"),
        ("_s", "_S"),
        ("_n", "_N"),
        ("_t", "_T"),
        ("_l", "_L"),
    ]
    .into_iter()
    .find(|(ending, _)| lower.ends_with(ending))
    .map_or("", |(_, suffix)| suffix)
}

/// One resource column of the outer aggregation: the field it exposes and
/// the row expression it sums.
struct ResourceColumn {
    field: QueryableField,
    aggregate: String,
    /// `Some(true)` for the positive part of the group sum, `Some(false)`
    /// for the negated negative part: the expanded balances, taken per
    /// account, dimensions and extra dimensions before the outer sum.
    part: Option<bool>,
}

impl ResourceColumn {
    fn new(folded: &FoldedField, suffix: (&str, &str), physical: &str, aggregate: String) -> Self {
        Self {
            field: balance_and_turnover_field(
                &folded.unified,
                &folded.unified.columns[0],
                suffix,
                physical,
            ),
            aggregate,
            part: None,
        }
    }

    /// The debit (`positive`) or credit part of the group sum of
    /// `aggregate`: `<Ресурс>РазвернутыйОстатокДт/Кт` and their opening
    /// and closing twins.
    fn part(
        folded: &FoldedField,
        suffix: (&str, &str),
        physical: &str,
        aggregate: String,
        positive: bool,
    ) -> Self {
        Self {
            part: Some(positive),
            ..Self::new(folded, suffix, physical, aggregate)
        }
    }

    /// The projection of the inner aggregation.
    fn projection(&self) -> String {
        let sum = format!("SUM({})", self.aggregate);
        match self.part {
            None => sum,
            Some(true) => format!("CASE WHEN {sum} > 0 THEN {sum} ELSE 0 END"),
            Some(false) => format!("CASE WHEN {sum} < 0 THEN -{sum} ELSE 0 END"),
        }
    }
}

/// The expanded balances of a balance column: its debit and credit parts
/// per account, dimensions and extra dimensions, which the outer sum adds
/// up instead of netting.
fn expanded_parts(
    folded: &FoldedField,
    physical: &str,
    aggregate: &str,
    debit: (&str, &str),
    credit: (&str, &str),
) -> [ResourceColumn; 2] {
    [
        ResourceColumn::part(
            folded,
            debit,
            &format!("{physical}ExpandedDt"),
            aggregate.to_owned(),
            true,
        ),
        ResourceColumn::part(
            folded,
            credit,
            &format!("{physical}ExpandedCt"),
            aggregate.to_owned(),
            false,
        ),
    ]
}

/// A derived column with the row expression its base sums in the inner
/// relation.
struct DerivedRule {
    resource: DerivedResource,
    base_aggregate: String,
}

/// The debit and credit parts of a balance column.
fn balance_parts(
    folded: &FoldedField,
    base: &str,
    base_aggregate: &str,
    debit: (&str, &str),
    credit: (&str, &str),
) -> Vec<(DerivedRule, QueryableField)> {
    [(debit, true), (credit, false)]
        .into_iter()
        .map(|(suffix, positive)| {
            let column = format!("{base}{}", if positive { "Dt" } else { "Ct" });
            (
                DerivedRule {
                    resource: DerivedResource {
                        column: column.clone(),
                        base: base.to_owned(),
                        positive,
                    },
                    base_aggregate: base_aggregate.to_owned(),
                },
                balance_and_turnover_field(
                    &folded.unified,
                    &folded.unified.columns[0],
                    suffix,
                    &column,
                ),
            )
        })
        .collect()
}

/// The register's fields sorted into what the folded relation needs.
struct Register {
    /// The debit and credit account fields of the main table.
    accounts: [QueryableField; 2],
    /// Predicates of the listed extra-dimension kinds, per side.
    extra_predicates: [Vec<String>; 2],
    /// The dimensions and data separators, balance ones and non-balance
    /// ones alike, in Config order.
    dimensions: Vec<FoldedField>,
    /// The resources in Config order.
    resources: Vec<FoldedField>,
}

impl Register {
    fn classify(table: &Table<'_, '_, '_>) -> Result<Self, QueryDiagnostic> {
        let virtual_table = table.virtual_table;
        let fields = table.fields;
        let find = |schema_name: &str| {
            fields
                .iter()
                .find(|field| names_equal(&field.schema_name, schema_name))
                .cloned()
        };
        let Some(debit) = find(DEBIT.account) else {
            // A register without correspondence keeps one account and a
            // record kind per record; its layout is not measured yet.
            return Err(not_yet(virtual_table, "a register without correspondence"));
        };
        let credit = find(CREDIT.account).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(virtual_table.token),
                format!(
                    "{} requires a live credit account field",
                    virtual_table.kind.name()
                ),
            )
        })?;
        let physical_table = table
            .object
            .physical_table
            .as_deref()
            .expect("a live accounting register has a physical table");
        let owned = table
            .snapshot
            .fields()
            .iter()
            .filter(|field| {
                field
                    .owner_tables
                    .iter()
                    .any(|owner| names_equal(owner, physical_table))
            })
            .collect::<Vec<_>>();
        if !owned.iter().any(|field| {
            matches!(
                field.purpose,
                Some(
                    ConfigFieldPurpose::AccountingRegisterDimension
                        | ConfigFieldPurpose::AccountingRegisterResource
                        | ConfigFieldPurpose::AccountingRegisterAttribute
                )
            )
        }) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(virtual_table.token),
                format!(
                    "{} field roles are unavailable in Config metadata",
                    virtual_table.kind.name()
                ),
            ));
        }
        let mut dimensions = Vec::new();
        let mut resources = Vec::new();
        for metadata_field in owned {
            let is_dimension = metadata_field.data_separator
                || metadata_field.purpose == Some(ConfigFieldPurpose::AccountingRegisterDimension);
            let is_resource =
                metadata_field.purpose == Some(ConfigFieldPurpose::AccountingRegisterResource);
            if !is_dimension && !is_resource {
                continue;
            }
            let folded = fold_field(fields, metadata_field, virtual_table)?;
            if is_dimension {
                dimensions.push(folded);
            } else {
                resources.push(folded);
            }
        }
        if resources.is_empty() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(virtual_table.token),
                format!(
                    "{} requires at least one resource",
                    virtual_table.kind.name()
                ),
            ));
        }
        Ok(Self {
            accounts: [debit, credit],
            extra_predicates: [Vec::new(), Vec::new()],
            dimensions,
            resources,
        })
    }

    /// Adds the extra dimensions to the dimensions: `Субконто<k>` and
    /// `ВидСубконто<k>` for every level the main table keeps inline
    /// (`_ValueDt<k>_*`, `_KindDt<k>RRef` and the credit twins, measured
    /// on the demo Бухгалтерия предприятия base). Without the `Субконто`
    /// argument the levels are the account's own positions. With it —
    /// one kind or a parenthesized list — `Субконто<j>` takes the value of
    /// whichever level carries the `j`-th listed kind, and the predicates
    /// returned keep only records whose side carries every listed kind.
    /// `КорСчет` and `<Измерение>Кор` of `Обороты`: the other side's account
    /// and non-balance dimensions, read by each branch through the
    /// opposite side's columns under the `__cor` names.
    fn correspondence(&mut self) {
        let mut account = self.accounts[1].clone();
        account.name = "КорСчет".to_owned();
        account.schema_name = "BalancedAccount".to_owned();
        account.aliases = vec!["КорСчет".to_owned(), "BalancedAccount".to_owned()];
        for column in &mut account.columns {
            column.physical_name = CORRESPONDING_ACCOUNT_COLUMN.to_owned();
        }
        relabel(&mut account, "КорСчет");
        let mut corresponding = vec![FoldedField {
            unified: account,
            sides: [self.accounts[1].clone(), self.accounts[0].clone()],
            side_sql: None,
        }];
        for folded in &self.dimensions {
            if folded.sides[0].schema_name == folded.sides[1].schema_name {
                continue;
            }
            let mut unified = folded.unified.clone();
            unified.name = format!("{}Кор", folded.unified.name);
            unified.schema_name = format!("{}Balanced", folded.unified.schema_name);
            unified.aliases = vec![unified.name.clone(), unified.schema_name.clone()];
            for column in &mut unified.columns {
                column.physical_name = corresponding_physical_name(&column.physical_name);
            }
            let name = unified.name.clone();
            relabel(&mut unified, &name);
            corresponding.push(FoldedField {
                unified,
                sides: [folded.sides[1].clone(), folded.sides[0].clone()],
                side_sql: folded
                    .side_sql
                    .as_ref()
                    .map(|[debit, credit]| [credit.clone(), debit.clone()]),
            });
        }
        self.dimensions.extend(corresponding);
    }

    /// The extra dimensions of both readings: `Субконто<k>` by the
    /// `Субконто` argument and, for a table with a correspondence,
    /// `КорСубконто<k>` by the `КорСубконто` argument.
    fn extra_dimensions(
        &mut self,
        table: &Table<'_, '_, '_>,
        arguments: &Arguments<'_, '_, '_>,
        corresponding: bool,
    ) -> Result<(), QueryDiagnostic> {
        self.extra_dimensions_of(table, arguments.extra_dimensions, false)?;
        if corresponding {
            self.extra_dimensions_of(table, arguments.balanced_extra_dimensions, true)?;
        }
        Ok(())
    }

    /// One reading of the extra dimensions: positional without a list,
    /// otherwise the listed kinds picked by `CASE` over the levels; the
    /// balanced reading swaps the sides and checks the opposite side's
    /// kinds in each branch.
    fn extra_dimensions_of(
        &mut self,
        table: &Table<'_, '_, '_>,
        listed: Option<&Expression<'_, '_>>,
        balanced: bool,
    ) -> Result<(), QueryDiagnostic> {
        let dialect = table.dialect;
        let (value_name, value_english, kind_name, kind_english) = if balanced {
            (
                "КорСубконто",
                "BalancedExtDimension",
                "ВидКорСубконто",
                "BalancedExtDimensionType",
            )
        } else {
            (
                "Субконто",
                "ExtDimension",
                "ВидСубконто",
                "ExtDimensionType",
            )
        };
        let swap = |sides: [QueryableField; 2]| {
            if balanced {
                let [debit, credit] = sides;
                [credit, debit]
            } else {
                sides
            }
        };
        let swap_sql = |sides: [Vec<String>; 2]| {
            if balanced {
                let [debit, credit] = sides;
                [credit, debit]
            } else {
                sides
            }
        };
        let find = |schema_name: &str| {
            table
                .fields
                .iter()
                .find(|field| names_equal(&field.schema_name, schema_name))
                .cloned()
        };
        let mut levels = Vec::new();
        for level in 1.. {
            let Some(value_debit) = find(&format!("ValueDt{level}")) else {
                break;
            };
            let value_credit = find(&format!("ValueCt{level}"));
            let kind_debit = find(&format!("KindDt{level}"));
            let kind_credit = find(&format!("KindCt{level}"));
            let (Some(value_credit), Some(kind_debit), Some(kind_credit)) =
                (value_credit, kind_debit, kind_credit)
            else {
                break;
            };
            levels.push(([value_debit, value_credit], [kind_debit, kind_credit]));
        }
        if levels.is_empty() {
            return Ok(());
        }
        let unify = |field: &QueryableField, name: &str, english: &str| {
            let mut unified = field.clone();
            unified.name = name.to_owned();
            unified.schema_name = english.to_owned();
            unified.aliases = vec![name.to_owned(), english.to_owned()];
            for column in &mut unified.columns {
                column.physical_name = unified_physical_name(&column.physical_name);
                if balanced {
                    column.physical_name = corresponding_physical_name(&column.physical_name);
                }
            }
            relabel(&mut unified, name);
            unified
        };
        let Some(listed) = listed else {
            for (level, (values, kinds)) in levels.iter().enumerate() {
                let level = level + 1;
                self.dimensions.push(FoldedField {
                    unified: unify(
                        &values[0],
                        &format!("{value_name}{level}"),
                        &format!("{value_english}{level}"),
                    ),
                    sides: swap(values.clone()),
                    side_sql: None,
                });
                self.dimensions.push(FoldedField {
                    unified: unify(
                        &kinds[0],
                        &format!("{kind_name}{level}"),
                        &format!("{kind_english}{level}"),
                    ),
                    sides: swap(kinds.clone()),
                    side_sql: None,
                });
            }
            return Ok(());
        };
        let requested: Vec<&Expression<'_, '_>> = match listed {
            Expression::Tuple { items, .. } => items.iter().collect(),
            single => vec![single],
        };
        // A parameter that carries no reference — an unbound one, or an
        // array the compiler cannot see into — says nothing about the
        // kinds, so the levels stay positional, as without the argument.
        let unknown = requested.iter().any(|kind| match kind {
            Expression::Parameter(token) => !matches!(
                table.catalog.parameters().lookup(token),
                Ok(Some(
                    crate::query::core::params::ParameterValue::Reference { .. }
                ))
            ),
            _ => false,
        });
        if unknown {
            return self.extra_dimensions_of(table, None, balanced);
        }
        if requested.len() > levels.len() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(table.virtual_table.token),
                format!(
                    "{} lists {} kinds of extra dimensions, the register keeps {}",
                    table.virtual_table.kind.name(),
                    requested.len(),
                    levels.len()
                ),
            ));
        }
        let mut predicates = Vec::new();
        for (position, kind) in requested.into_iter().enumerate() {
            let kind_sql = super::sources::compile_source_free_expression(
                kind,
                table.snapshot,
                dialect,
                table.catalog.parameters(),
                false,
            )?;
            let mut value_sides: [Vec<String>; 2] = [Vec::new(), Vec::new()];
            let mut kind_sides: [Vec<String>; 2] = [Vec::new(), Vec::new()];
            let mut present = [Vec::new(), Vec::new()];
            for side in 0..2 {
                let kind_of = |level: usize| {
                    dialect.qualified_column(
                        Some(BASE_ALIAS),
                        &levels[level].1[side].columns[0].physical_name,
                    )
                };
                let branches = |member: usize| {
                    levels
                        .iter()
                        .enumerate()
                        .map(|(level, (values, _))| {
                            format!(
                                "WHEN {} = {kind_sql} THEN {}",
                                kind_of(level),
                                dialect.qualified_column(
                                    Some(BASE_ALIAS),
                                    &values[side].columns[member].physical_name
                                )
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                for member in 0..levels[0].0[side].columns.len() {
                    value_sides[side].push(format!("CASE {} END", branches(member)));
                }
                kind_sides[side].push(format!(
                    "CASE WHEN {} THEN {kind_sql} END",
                    (0..levels.len())
                        .map(|level| format!("{} = {kind_sql}", kind_of(level)))
                        .collect::<Vec<_>>()
                        .join(" OR ")
                ));
                present[side] = (0..levels.len())
                    .map(|level| format!("{} = {kind_sql}", kind_of(level)))
                    .collect::<Vec<_>>();
            }
            // The `j`-th listed kind is exposed under the `j`-th level's
            // column names, so the positions never collide.
            let (values, kinds) = &levels[position];
            let position = position + 1;
            self.dimensions.push(FoldedField {
                unified: unify(
                    &values[0],
                    &format!("{value_name}{position}"),
                    &format!("{value_english}{position}"),
                ),
                sides: swap(values.clone()),
                side_sql: Some(swap_sql(value_sides)),
            });
            self.dimensions.push(FoldedField {
                unified: unify(
                    &kinds[0],
                    &format!("{kind_name}{position}"),
                    &format!("{kind_english}{position}"),
                ),
                sides: swap(kinds.clone()),
                side_sql: Some(swap_sql(kind_sides)),
            });
            // Both sides carry the same predicate set; each branch takes
            // its own side's account, so the predicate is per side.
            predicates.push(present);
        }
        // The predicates are per side: the debit branch checks the debit
        // kinds, the credit branch the credit kinds — the other way round
        // for the balanced reading.
        let per_side = |side: usize| {
            predicates
                .iter()
                .map(|present| format!("({})", present[side].join(" OR ")))
                .collect::<Vec<_>>()
        };
        let (debit, credit) = if balanced { (1, 0) } else { (0, 1) };
        self.extra_predicates[0].extend(per_side(debit));
        self.extra_predicates[1].extend(per_side(credit));
        Ok(())
    }

    /// `Счет` of the folded relation: the debit account field under the
    /// unified name and column.
    fn account_field(&self) -> QueryableField {
        let mut field = self.accounts[0].clone();
        field.name = "Счет".to_owned();
        field.schema_name = "Account".to_owned();
        field.aliases = vec!["Счет".to_owned(), "Account".to_owned()];
        for column in &mut field.columns {
            column.physical_name = ACCOUNT_COLUMN.to_owned();
        }
        relabel(&mut field, "Счет");
        field
    }

    /// What a condition sees on one side of the record: `Счет` first, then
    /// the dimensions and resources under their side-less names, and the
    /// record's standard fields.
    fn side_fields(
        &self,
        index: usize,
        fields: &[QueryableField],
        virtual_table: &AccumulationAst<'_, '_>,
    ) -> Result<Vec<QueryableField>, QueryDiagnostic> {
        let mut account = self.accounts[index].clone();
        account.name = "Счет".to_owned();
        account.schema_name = "Account".to_owned();
        account.aliases = vec!["Счет".to_owned(), "Account".to_owned()];
        let mut result = vec![account];
        for folded in self.dimensions.iter().chain(&self.resources) {
            let mut field = folded.sides[index].clone();
            field.name.clone_from(&folded.unified.name);
            field.schema_name.clone_from(&folded.unified.schema_name);
            field.aliases.clone_from(&folded.unified.aliases);
            result.push(field);
        }
        for schema_name in ["Period", "Recorder", "LineNo", "Active"] {
            result.push(register_standard_field(fields, schema_name, virtual_table)?.clone());
        }
        Ok(result)
    }
}

/// Pairs a dimension or resource with the field each side reads. A
/// balance field is read as is; a non-balance one exists as `Fld<N>Dt`
/// and `Fld<N>Ct`, and the unified field takes the debit field's column
/// under the side-less physical name.
fn fold_field(
    fields: &[QueryableField],
    metadata_field: &MetadataField,
    virtual_table: &AccumulationAst<'_, '_>,
) -> Result<FoldedField, QueryDiagnostic> {
    let base = format!("Fld{}", metadata_field.number);
    let find = |schema_name: &str| {
        fields
            .iter()
            .find(|field| names_equal(&field.schema_name, schema_name))
            .cloned()
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::NotLive,
                    Some(virtual_table.token),
                    format!(
                        "{} field {:?} has no live physical representation",
                        virtual_table.kind.name(),
                        metadata_field.name.as_deref().unwrap_or(schema_name)
                    ),
                )
            })
    };
    if metadata_field.balance == Some(false) {
        let debit = find(&format!("{base}{}", DEBIT.suffix))?;
        let credit = find(&format!("{base}{}", CREDIT.suffix))?;
        let mut unified = debit.clone();
        let name = metadata_field.name.clone().unwrap_or_else(|| base.clone());
        unified.name.clone_from(&name);
        unified.schema_name.clone_from(&base);
        unified.aliases = vec![name.clone(), base.clone()];
        for column in &mut unified.columns {
            column.physical_name = unified_physical_name(&column.physical_name);
        }
        relabel(&mut unified, &name);
        return Ok(FoldedField {
            unified,
            sides: [debit, credit],
            side_sql: None,
        });
    }
    let field = find(&base)?;
    Ok(FoldedField {
        unified: field.clone(),
        sides: [field.clone(), field],
        side_sql: None,
    })
}

/// `_fld414rref` → `__cor_fld414rref`: the name of the other side's
/// reading of a unified column.
fn corresponding_physical_name(unified_physical_name: &str) -> String {
    format!("__cor{unified_physical_name}")
}

/// `_fld414dtrref` → `_fld414rref`: the side-less name of a debit column.
fn unified_physical_name(debit_physical_name: &str) -> String {
    let lower = debit_physical_name.to_ascii_lowercase();
    match lower.find("dt") {
        Some(position) => format!(
            "{}{}",
            &debit_physical_name[..position],
            &debit_physical_name[position + 2..]
        ),
        None => debit_physical_name.to_owned(),
    }
}

fn single<'field>(
    field: &'field QueryableField,
    virtual_table: &AccumulationAst<'_, '_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            format!(
                "{} expects field {:?} to be a single column",
                virtual_table.kind.name(),
                field.name
            ),
        )),
    }
}
