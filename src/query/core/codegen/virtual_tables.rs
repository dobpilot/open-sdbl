use std::cell::RefCell;
use std::collections::BTreeSet;

use super::context::{CompilationContext, JoinPlan, SourceScope};
use super::expression::{compile_predicate, operand_token, single_column, single_column_at};
use super::params::render_scalar_parameter;
use super::select::append_reference_join;
use super::separators::separator_predicates;
use super::sources::{CompiledSourceRelation, SourceRestriction, compile_restriction_predicate};
use super::windowed::{
    BucketSum, Grain, RelationVariant, RunningSum, WindowedSpec, auto_grains, grain_of,
    windowed_relation,
};
use crate::metadata::{
    ConfigFieldPurpose, FieldId, LiveColumn, LiveTable, MetadataKind, MetadataObject,
    MetadataSnapshot, ObjectId,
};
use crate::query::core::ast::{
    AccumulationAst, AccumulationKind, Expression, PeriodKind, SourceAst,
};
use crate::query::core::dialect::{SqlDialect, compile_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::{ParameterValue, Parameters};
use crate::query::core::resolve::{
    CompilationCatalog, PresentationExpression, PresentationPlan, QueryableColumn, QueryableField,
    logical_column_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Token, TokenKind};

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_accumulation_relation(
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
    if object.kind == Some(MetadataKind::AccountingRegister) {
        return super::accounting::compile_accounting_relation(
            source,
            virtual_table,
            snapshot,
            catalog,
            object,
            live_table,
            fields,
            restriction,
            dialect,
        );
    }
    if object.kind != Some(MetadataKind::AccumulationRegister) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            format!(
                "{} is supported only for accumulation registers",
                virtual_table.kind.name()
            ),
        ));
    }
    let physical_table = object
        .physical_table
        .as_deref()
        .expect("a live accumulation register has a physical table");
    let owned_fields = snapshot
        .fields()
        .iter()
        .filter(|field| {
            field
                .owner_tables
                .iter()
                .any(|owner| names_equal(owner, physical_table))
        })
        .collect::<Vec<_>>();
    let has_accumulation_roles = owned_fields.iter().any(|field| {
        matches!(
            field.purpose,
            Some(
                ConfigFieldPurpose::AccumulationRegisterDimension
                    | ConfigFieldPurpose::AccumulationRegisterResource
                    | ConfigFieldPurpose::AccumulationRegisterAttribute
            )
        )
    });
    if !owned_fields.is_empty() && !has_accumulation_roles {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(virtual_table.token),
            format!(
                "{} field roles are unavailable in Config metadata",
                virtual_table.kind.name()
            ),
        ));
    }

    let mut dimension_fields = Vec::new();
    for metadata_field in owned_fields.iter().filter(|field| {
        field.data_separator
            || field.purpose == Some(ConfigFieldPurpose::AccumulationRegisterDimension)
    }) {
        dimension_fields.push(accumulation_metadata_field(
            fields,
            metadata_field,
            virtual_table,
        )?);
    }
    let mut resource_fields = Vec::new();
    for metadata_field in owned_fields
        .iter()
        .filter(|field| field.purpose == Some(ConfigFieldPurpose::AccumulationRegisterResource))
    {
        resource_fields.push(accumulation_metadata_field(
            fields,
            metadata_field,
            virtual_table,
        )?);
    }
    if resource_fields.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(virtual_table.token),
            format!(
                "{} requires at least one Config-declared resource",
                virtual_table.kind.name()
            ),
        ));
    }

    let active = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "Active"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(virtual_table.token),
                format!("{} requires a live Active field", virtual_table.kind.name()),
            )
        })?;
    let active_column = single_column(active, virtual_table.token)?;
    let period = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "Period"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(virtual_table.token),
                format!("{} requires a live Period field", virtual_table.kind.name()),
            )
        })?;
    let period_column = single_column(period, virtual_table.token)?;
    let record_kind = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "RecordKind"))
        .map(|field| single_column(field, virtual_table.token))
        .transpose()?;
    if matches!(
        virtual_table.kind,
        AccumulationKind::Balance | AccumulationKind::BalanceAndTurnovers
    ) && record_kind.is_none()
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            format!(
                "{} is unavailable for a turnover-only accumulation register",
                virtual_table.kind.name()
            ),
        ));
    }

    if virtual_table.kind == AccumulationKind::Balance {
        return compile_accumulation_balance_relation(
            source,
            virtual_table,
            snapshot,
            catalog,
            object,
            &dimension_fields,
            &resource_fields,
            live_table,
            active_column,
            period_column,
            record_kind.expect("a balance register has RecordKind"),
            restriction,
            dialect,
        );
    }

    if virtual_table.kind == AccumulationKind::BalanceAndTurnovers {
        return compile_balance_and_turnovers_relation(
            source,
            virtual_table,
            snapshot,
            catalog,
            object,
            fields,
            &dimension_fields,
            &resource_fields,
            live_table,
            active_column,
            period,
            period_column,
            record_kind.expect("a balance register has RecordKind"),
            restriction,
            dialect,
        );
    }

    let periodicity = virtual_table
        .arguments
        .get(2)
        .and_then(Option::as_ref)
        .map(|expression| turnovers_periodicity(expression, virtual_table))
        .transpose()?
        .flatten();
    let begin = virtual_table.arguments.first().and_then(Option::as_ref);
    let end = virtual_table.arguments.get(1).and_then(Option::as_ref);
    let condition = virtual_table.arguments.get(3).and_then(Option::as_ref);

    let begin = begin
        .map(|expression| {
            compile_virtual_period_literal(
                expression,
                virtual_table,
                "begin period",
                catalog,
                dialect,
            )
        })
        .transpose()?;
    let end = end
        .map(|expression| {
            compile_virtual_period_literal(
                expression,
                virtual_table,
                "period boundary",
                catalog,
                dialect,
            )
        })
        .transpose()?;
    let mut predicates = vec![format!(
        "{} = {}",
        dialect.qualified_column(Some("__aggregate_base"), &active_column.physical_name),
        dialect.boolean_literal(true)
    )];
    let qualified_period =
        dialect.qualified_column(Some("__aggregate_base"), &period_column.physical_name);
    if let Some(begin) = begin {
        predicates.push(format!("({qualified_period} >= {begin})"));
    }
    if let Some(end) = end {
        predicates.push(format!("({qualified_period} < {end})"));
    }
    let condition = compile_accumulation_condition(
        condition.as_slice(),
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        &dimension_fields,
        None,
        live_table,
        "__aggregate_base",
        restriction,
        dialect,
    )?;
    if let Some(sql) = &condition.predicate {
        predicates.push(sql.clone());
    }
    let joins = condition.joins_sql(dialect);

    let mut projections = Vec::new();
    let mut grouping = Vec::new();
    // Every periodicity is a grouping level of its own: the platform
    // keeps it even when the statement never reads the column.
    let mut split_fields = Vec::new();
    match periodicity {
        Some(TurnoverPeriodicity::Calendar(unit)) => {
            let truncated = dialect.begin_of_period(&qualified_period, unit);
            projections.push(format!(
                "{truncated} AS {}",
                dialect.quote_identifier(&period_column.physical_name)
            ));
            grouping.push(truncated);
            split_fields.push(turnovers_period_field(period));
        }
        Some(TurnoverPeriodicity::Auto) => {
            auto_split_fields(
                fields,
                period,
                &qualified_period,
                virtual_table,
                dialect,
                &mut projections,
                &mut grouping,
                &mut dimension_fields,
            )?;
        }
        Some(periodicity @ (TurnoverPeriodicity::Recorder | TurnoverPeriodicity::Record)) => {
            projections.push(format!(
                "{qualified_period} AS {}",
                dialect.quote_identifier(&period_column.physical_name)
            ));
            grouping.push(qualified_period.clone());
            split_fields.push(turnovers_period_field(period));
            let mut standard = vec![register_standard_field(fields, "Recorder", virtual_table)?];
            if periodicity == TurnoverPeriodicity::Record {
                standard.push(register_standard_field(fields, "LineNo", virtual_table)?);
            }
            for field in standard {
                for column in &field.columns {
                    let sql =
                        dialect.qualified_column(Some("__aggregate_base"), &column.physical_name);
                    projections.push(format!(
                        "{sql} AS {}",
                        dialect.quote_identifier(&column.physical_name)
                    ));
                    grouping.push(sql);
                }
                split_fields.push(field.clone());
            }
        }
        None => {}
    }
    for field in &dimension_fields {
        for column in &field.columns {
            let sql = dialect.qualified_column(Some("__aggregate_base"), &column.physical_name);
            projections.push(format!(
                "{sql} AS {}",
                dialect.quote_identifier(&column.physical_name)
            ));
            grouping.push(sql);
        }
    }
    let mut virtual_resources = Vec::new();
    for field in &resource_fields {
        let column = single_column(field, virtual_table.token)?;
        let value = dialect.qualified_column(Some("__aggregate_base"), &column.physical_name);
        let signed = record_kind.map_or(value.clone(), |record_kind| {
            format!(
                "CASE WHEN {} = 0 THEN {value} ELSE -{value} END",
                dialect.qualified_column(Some("__aggregate_base"), &record_kind.physical_name)
            )
        });
        projections.push(format!(
            "SUM({signed}) AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        virtual_resources.push(accumulation_resource_field(field, virtual_table.kind));
        // A balance register also answers the receipts and the expenses
        // of the interval, the way `ОстаткиИОбороты` does.
        if let Some(record_kind) = record_kind {
            let kind =
                dialect.qualified_column(Some("__aggregate_base"), &record_kind.physical_name);
            let suffixes = AccumulationKind::balance_and_turnover_suffixes();
            for (code, suffix) in [(0, suffixes[1]), (1, suffixes[2])] {
                let column_name = format!("{}{}", column.physical_name, suffix.1);
                projections.push(format!(
                    "SUM(CASE WHEN {kind} = {code} THEN {value} ELSE 0 END) AS {}",
                    dialect.quote_identifier(&column_name)
                ));
                virtual_resources.push(balance_and_turnover_field(
                    field,
                    column,
                    suffix,
                    &column_name,
                ));
            }
        }
    }

    let mut relation = format!(
        "(SELECT {} FROM {} AS {}{joins} WHERE {}",
        projections.join(", "),
        dialect.quote_identifier(&live_table.name),
        dialect.quote_identifier("__aggregate_base"),
        predicates.join(" AND ")
    );
    if !grouping.is_empty() {
        relation.push_str(" GROUP BY ");
        relation.push_str(&grouping.join(", "));
    }
    relation.push(')');

    let mut aggregate = aggregate_source(&dimension_fields, &virtual_resources);
    aggregate.split = split_fields
        .iter()
        .flat_map(|field| field.columns.iter())
        .map(|column| column.physical_name.clone())
        .collect();
    dimension_fields.extend(split_fields);
    dimension_fields.extend(virtual_resources);
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: dimension_fields.into(),
        aggregate: Some(aggregate),
        separators: Vec::new(),
    })
}

/// Compiles `ОстаткиИОбороты(Начало, Конец, Периодичность, Метод,
/// Условие)`: one row per combination of the dimensions in use with the
/// opening balance, the receipts and expenses of the interval, their
/// turnover, and the closing balance. Everything is read from the
/// movements, which is what the totals table caches.
#[allow(clippy::too_many_arguments)]
fn compile_balance_and_turnovers_relation(
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    all_fields: &[QueryableField],
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    live_table: &LiveTable,
    active_column: &QueryableColumn,
    period_field: &QueryableField,
    period_column: &QueryableColumn,
    record_kind: &QueryableColumn,
    restriction: Option<&SourceRestriction<'_>>,
    dialect: SqlDialect,
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    // A periodicity splits the interval into periods with movements, the
    // way `Обороты` does. The balances of such a split are running sums
    // the platform accumulates while reading the ordered rows rather than
    // in SQL, so they are refused where the statement reads one.
    let mut auto = false;
    let periodicity = virtual_table
        .arguments
        .get(2)
        .and_then(Option::as_ref)
        .map(|expression| {
            let periodicity = turnovers_periodicity(expression, virtual_table)?;
            Ok::<_, QueryDiagnostic>(match periodicity {
                Some(TurnoverPeriodicity::Calendar(unit)) => Some(Grain::Calendar(unit)),
                Some(TurnoverPeriodicity::Recorder) => Some(Grain::Recorder),
                Some(TurnoverPeriodicity::Record) => Some(Grain::Record),
                None => None,
                Some(TurnoverPeriodicity::Auto) => {
                    auto = true;
                    None
                }
            })
        })
        .transpose()?
        .flatten();
    if let Some(argument) = virtual_table.arguments.get(3).and_then(Option::as_ref) {
        let token = operand_token(argument).unwrap_or(virtual_table.token);
        let completion = match argument {
            Expression::Field(reference) if reference.segments.len() == 1 => reference.last(),
            _ => token,
        };
        // Measured: with a periodicity and no balance column the two
        // methods answer the same rows, because a boundary row carries no
        // turnover.
        if !(names_equal(completion.lexeme, "Движения")
            || names_equal(completion.lexeme, "Movements")
            || names_equal(completion.lexeme, "ДвиженияИГраницыПериода")
            || names_equal(completion.lexeme, "MovementsAndBoundaries"))
        {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!(
                    "BalanceAndTurnovers period completion method {:?} is not supported",
                    completion.lexeme
                ),
            ));
        }
        // Without a periodicity the method has nothing to complete; the
        // platform accepts it there, and real configurations write it.
    }
    let begin = virtual_table
        .arguments
        .first()
        .and_then(Option::as_ref)
        .map(|expression| {
            compile_virtual_period_literal(
                expression,
                virtual_table,
                "begin period",
                catalog,
                dialect,
            )
        })
        .transpose()?;
    let end = virtual_table
        .arguments
        .get(1)
        .and_then(Option::as_ref)
        .map(|expression| {
            compile_virtual_period_literal(
                expression,
                virtual_table,
                "period boundary",
                catalog,
                dialect,
            )
        })
        .transpose()?;
    let condition = virtual_table.arguments.get(4).and_then(Option::as_ref);
    let qualified = |column: &QueryableColumn| {
        dialect.qualified_column(Some("__aggregate_base"), &column.physical_name)
    };
    let mut predicates = vec![format!(
        "{} = {}",
        qualified(active_column),
        dialect.boolean_literal(true)
    )];
    let period = qualified(period_column);
    if let Some(end) = &end {
        predicates.push(format!("({period} < {end})"));
    }
    // Without a periodicity the movements before the interval are read to
    // build its opening balance; a periodic table answers no balance, so
    // it reads the interval alone and every period it reports has
    // movements of its own, as on the platform.
    if periodicity.is_some()
        && let Some(begin) = &begin
    {
        predicates.push(format!("({period} >= {begin})"));
    }
    let condition = compile_accumulation_condition(
        condition.as_slice(),
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        dimension_fields,
        None,
        live_table,
        "__aggregate_base",
        restriction,
        dialect,
    )?;
    if let Some(sql) = &condition.predicate {
        predicates.push(sql.clone());
    }
    let joins = condition.joins_sql(dialect);
    let mut projections = Vec::new();
    let mut grouping = Vec::new();
    for field in dimension_fields {
        for column in &field.columns {
            let sql = qualified(column);
            projections.push(format!(
                "{sql} AS {}",
                dialect.quote_identifier(&column.physical_name)
            ));
            grouping.push(sql);
        }
    }
    let kind = qualified(record_kind);
    let mut fields = dimension_fields.to_vec();
    if auto {
        auto_split_fields(
            all_fields,
            period_field,
            &period,
            virtual_table,
            dialect,
            &mut projections,
            &mut grouping,
            &mut fields,
        )?;
    }
    let auto_dimensions = dimension_fields.len()..fields.len();
    // A periodicity groups the movements into calendar periods, exactly
    // as `Обороты` does — or by the recorder (`Регистратор`) or the record
    // (`Запись`); the split fields become fields of the relation and stay
    // grouping levels even when the statement never reads them.
    let mut split = Vec::new();
    match periodicity {
        Some(Grain::Calendar(unit)) => {
            let truncated = dialect.begin_of_period(&period, unit);
            projections.push(format!(
                "{truncated} AS {}",
                dialect.quote_identifier(&period_column.physical_name)
            ));
            grouping.push(truncated);
            let field = turnovers_period_field(period_field);
            split = field
                .columns
                .iter()
                .map(|column| column.physical_name.clone())
                .collect();
            fields.push(field);
        }
        Some(grain @ (Grain::Recorder | Grain::Record)) => {
            let mut split_fields = vec![
                turnovers_period_field(period_field),
                register_standard_field(all_fields, "Recorder", virtual_table)?.clone(),
            ];
            if grain == Grain::Record {
                split_fields
                    .push(register_standard_field(all_fields, "LineNo", virtual_table)?.clone());
            }
            for field in split_fields {
                for column in &field.columns {
                    let sql = qualified(column);
                    projections.push(format!(
                        "{sql} AS {}",
                        dialect.quote_identifier(&column.physical_name)
                    ));
                    grouping.push(sql);
                    split.push(column.physical_name.clone());
                }
                fields.push(field);
            }
        }
        Some(Grain::Whole) | None => {}
    }
    let split_field_count = fields.len() - auto_dimensions.end;
    for field in resource_fields {
        let column = single_column(field, virtual_table.token)?;
        let value = qualified(column);
        // Receipts carry record kind 0 and expenses 1, so the signed
        // movement is the receipt minus the expense.
        let signed = format!("CASE WHEN {kind} = 0 THEN {value} ELSE -{value} END");
        let before = begin.as_ref().map_or_else(
            || "0".to_owned(),
            |begin| format!("CASE WHEN {period} < {begin} THEN {signed} ELSE 0 END"),
        );
        let inside = begin.as_ref().map_or_else(
            || signed.clone(),
            |begin| format!("CASE WHEN {period} >= {begin} THEN {signed} ELSE 0 END"),
        );
        let receipt = begin.as_ref().map_or_else(
            || format!("CASE WHEN {kind} = 0 THEN {value} ELSE 0 END"),
            |begin| format!("CASE WHEN {period} >= {begin} AND {kind} = 0 THEN {value} ELSE 0 END"),
        );
        let expense = begin.as_ref().map_or_else(
            || format!("CASE WHEN {kind} = 1 THEN {value} ELSE 0 END"),
            |begin| format!("CASE WHEN {period} >= {begin} AND {kind} = 1 THEN {value} ELSE 0 END"),
        );
        for (suffix, aggregate) in AccumulationKind::balance_and_turnover_suffixes()
            .into_iter()
            .zip([
                before.clone(),
                receipt,
                expense,
                inside.clone(),
                format!("{before} + {inside}"),
            ])
        {
            let column_name = format!("{}{}", column.physical_name, suffix.1);
            projections.push(format!(
                "SUM({aggregate}) AS {}",
                dialect.quote_identifier(&column_name)
            ));
            fields.push(balance_and_turnover_field(
                field,
                column,
                suffix,
                &column_name,
            ));
        }
    }
    let mut relation = format!(
        "(SELECT {} FROM {} AS {}{joins} WHERE {}",
        projections.join(", "),
        dialect.quote_identifier(&live_table.name),
        dialect.quote_identifier("__aggregate_base"),
        predicates.join(" AND ")
    );
    if !grouping.is_empty() {
        relation.push_str(" GROUP BY ");
        relation.push_str(&grouping.join(", "));
    }
    relation.push(')');
    let mut aggregate = aggregate_source(&fields[..auto_dimensions.end], &[]);
    let resource_start = auto_dimensions.end + split_field_count;
    aggregate.split = split;
    aggregate.split_dimensions = auto_dimensions.collect();
    aggregate.resources = fields
        .iter()
        .skip(resource_start)
        .flat_map(|field| field.columns.iter())
        .map(|column| column.physical_name.clone())
        .collect();
    if periodicity.is_some() || auto {
        let suffixes = AccumulationKind::balance_and_turnover_suffixes();
        let balances = fields
            .iter()
            .enumerate()
            .skip(resource_start)
            .filter(|(_, field)| {
                [suffixes[0], suffixes[4]].iter().any(|(russian, english)| {
                    field.name.ends_with(russian) || field.name.ends_with(english)
                })
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if dialect.running_sums() {
            // The balance of a period is a running sum over the buckets
            // before it: a window over the bucketed movements, prepared
            // per grain under `Авто` and picked once the statement is known.
            let mut row_predicates = vec![format!(
                "{} = {}",
                qualified(active_column),
                dialect.boolean_literal(true)
            )];
            if let Some(end) = &end {
                row_predicates.push(format!("({period} < {end})"));
            }
            if let Some(sql) = &condition.predicate {
                row_predicates.push(sql.clone());
            }
            let rows = format!(
                "FROM {} AS {}{joins} WHERE {}",
                dialect.quote_identifier(&live_table.name),
                dialect.quote_identifier("__aggregate_base"),
                row_predicates.join(" AND ")
            );
            let mut bucket_sums = Vec::new();
            let mut running = Vec::new();
            for field in resource_fields {
                let column = single_column(field, virtual_table.token)?;
                let value = qualified(column);
                let signed = format!("CASE WHEN {kind} = 0 THEN {value} ELSE -{value} END");
                let base = &column.physical_name;
                bucket_sums.push(BucketSum {
                    column: format!("{base}{}", suffixes[1].1),
                    expression: format!("CASE WHEN {kind} = 0 THEN {value} ELSE 0 END"),
                });
                bucket_sums.push(BucketSum {
                    column: format!("{base}{}", suffixes[2].1),
                    expression: format!("CASE WHEN {kind} = 1 THEN {value} ELSE 0 END"),
                });
                bucket_sums.push(BucketSum {
                    column: format!("{base}{}", suffixes[3].1),
                    expression: signed.clone(),
                });
                running.push(RunningSum {
                    column: format!("{base}{}", suffixes[0].1),
                    expression: signed.clone(),
                    exclusive: true,
                });
                running.push(RunningSum {
                    column: format!("{base}{}", suffixes[4].1),
                    expression: signed,
                    exclusive: false,
                });
            }
            let recorder = register_standard_field(all_fields, "Recorder", virtual_table)?;
            let spec = WindowedSpec {
                rows: &rows,
                alias: "__aggregate_base",
                dimension_columns: dimension_fields
                    .iter()
                    .flat_map(|field| field.columns.iter())
                    .map(|column| column.physical_name.clone())
                    .collect(),
                period: period.clone(),
                period_column: &period_column.physical_name,
                recorder: recorder
                    .columns
                    .iter()
                    .map(|column| column.physical_name.clone())
                    .collect(),
                line: register_standard_field(all_fields, "LineNo", virtual_table)
                    .ok()
                    .and_then(|field| field.columns.first())
                    .map(|column| column.physical_name.clone()),
                begin: begin.clone(),
                bucket_sums,
                running,
                derived: Vec::new(),
                auto_levels: auto,
            };
            aggregate.variants = match periodicity {
                Some(grain) => vec![RelationVariant {
                    grain: None,
                    balances: true,
                    sql: windowed_relation(&spec, grain, dialect),
                }],
                None => auto_grains()
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
            // SQL Server 2008 has no window frame: the balances of a split
            // table stay refused there. Under `Авто` the split exists only
            // when the statement reads one of the split fields.
            let forbidden = balances
                .into_iter()
                .map(|index| {
                    (
                        index,
                        "a periodic BalanceAndTurnovers answers no balance column on this server",
                    )
                })
                .collect();
            if periodicity.is_none() {
                aggregate.forbidden_with_split = forbidden;
            } else {
                aggregate.forbidden = forbidden;
            }
        }
    }
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: fields.into(),
        aggregate: Some(aggregate),
        separators: Vec::new(),
    })
}

/// One of the five columns `ОстаткиИОбороты` exposes per resource.
pub(super) fn balance_and_turnover_field(
    field: &QueryableField,
    column: &QueryableColumn,
    (russian, english): (&str, &str),
    physical_name: &str,
) -> QueryableField {
    let russian_name = format!("{}{russian}", field.name);
    let mut result = field.clone();
    result.name = russian_name.clone();
    result.schema_name = format!("{}{english}", field.schema_name);
    result.aliases = vec![russian_name.clone(), format!("{}{english}", field.name)];
    result.columns = vec![QueryableColumn {
        physical_name: physical_name.to_owned(),
        data_type: column.data_type.clone(),
        output_label: russian_name,
        kind: column.kind.clone(),
    }];
    result
}

/// How `Обороты` splits its rows: by a calendar period, by the document
/// that wrote the records, or by the record itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnoverPeriodicity {
    Calendar(PeriodKind),
    Recorder,
    Record,
    /// `Авто`: the table splits by whatever the statement reads of the
    /// record period, its calendar levels, the recorder and the line
    /// number, and sums away the rest.
    Auto,
}

/// Reads the periodicity of `Обороты`: the platform writes a bare name
/// there, which parses as a one-segment field path. `None` for `Период`
/// — the documented default, "only for the period, do not split".
pub(super) fn turnovers_periodicity(
    expression: &Expression<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
) -> Result<Option<TurnoverPeriodicity>, QueryDiagnostic> {
    let token = match expression {
        Expression::Field(reference) if reference.segments.len() == 1 => reference.last(),
        other => operand_token(other).unwrap_or(virtual_table.token),
    };
    if names_equal(token.lexeme, "Период") || names_equal(token.lexeme, "Period") {
        return Ok(None);
    }
    if names_equal(token.lexeme, "Авто") || names_equal(token.lexeme, "Auto") {
        return Ok(Some(TurnoverPeriodicity::Auto));
    }
    if names_equal(token.lexeme, "Регистратор") || names_equal(token.lexeme, "Recorder")
    {
        return Ok(Some(TurnoverPeriodicity::Recorder));
    }
    if names_equal(token.lexeme, "Запись") || names_equal(token.lexeme, "Record") {
        return Ok(Some(TurnoverPeriodicity::Record));
    }
    PeriodKind::from_name(token.lexeme)
        .map(|unit| Some(TurnoverPeriodicity::Calendar(unit)))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                format!(
                    "Turnovers periodicity {:?} is not supported; use a calendar period, Регистратор or Запись",
                    token.lexeme
                ),
            )
        })
}

/// A standard field of the register exposed by a periodic `Обороты`.
pub(super) fn register_standard_field<'fields>(
    fields: &'fields [QueryableField],
    schema_name: &str,
    virtual_table: &AccumulationAst<'_, '_>,
) -> Result<&'fields QueryableField, QueryDiagnostic> {
    fields
        .iter()
        .find(|field| names_equal(&field.schema_name, schema_name))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(virtual_table.token),
                format!(
                    "{} by {schema_name} requires a live {schema_name} field",
                    virtual_table.kind.name()
                ),
            )
        })
}

/// The `Период` field a periodic `Обороты` exposes: the beginning of the
/// period the records fall into.
pub(super) fn turnovers_period_field(period: &QueryableField) -> QueryableField {
    let mut result = period.clone();
    result.name = "Период".to_owned();
    result.schema_name = "Period".to_owned();
    result.aliases = vec!["Период".to_owned(), "Period".to_owned()];
    for column in &mut result.columns {
        column.output_label = "Период".to_owned();
    }
    result
}

/// The calendar levels of the record period an `Авто` table exposes, as
/// `(unit, Russian suffix, English prefix)`: `ПериодМесяц` /
/// `MonthPeriod`.
pub(super) const AUTO_PERIOD_LEVELS: [(PeriodKind, &str, &str); 10] = [
    (PeriodKind::Second, "Секунда", "Second"),
    (PeriodKind::Minute, "Минута", "Minute"),
    (PeriodKind::Hour, "Час", "Hour"),
    (PeriodKind::Day, "День", "Day"),
    (PeriodKind::Week, "Неделя", "Week"),
    (PeriodKind::TenDays, "Декада", "TenDays"),
    (PeriodKind::Month, "Месяц", "Month"),
    (PeriodKind::Quarter, "Квартал", "Quarter"),
    (PeriodKind::HalfYear, "Полугодие", "HalfYear"),
    (PeriodKind::Year, "Год", "Year"),
];

/// One calendar level of the record period: `ПериодМесяц` is the month
/// the record falls into.
pub(super) fn auto_period_level_field(
    period: &QueryableField,
    russian: &str,
    english: &str,
) -> QueryableField {
    let name = format!("Период{russian}");
    let mut result = period.clone();
    result.name = name.clone();
    result.schema_name = format!("{english}Period");
    result.aliases = vec![name.clone(), format!("{english}Period")];
    for column in &mut result.columns {
        column.physical_name = format!("_Period{english}");
        column.output_label = name.clone();
    }
    result
}

/// The split fields of the `Авто` periodicity, documented as "determined
/// by the period fields the query uses": the record period, its ten
/// calendar levels, the recorder and the line number. They are
/// dimensions of the relation, so the ones the statement never reads are
/// summed away, which leaves the whole interval when it reads none.
#[allow(clippy::too_many_arguments)]
fn auto_split_fields(
    fields: &[QueryableField],
    period: &QueryableField,
    qualified_period: &str,
    virtual_table: &AccumulationAst<'_, '_>,
    dialect: SqlDialect,
    projections: &mut Vec<String>,
    grouping: &mut Vec<String>,
    dimension_fields: &mut Vec<QueryableField>,
) -> Result<(), QueryDiagnostic> {
    let period_field = turnovers_period_field(period);
    projections.push(format!(
        "{qualified_period} AS {}",
        dialect.quote_identifier(&period_field.columns[0].physical_name)
    ));
    grouping.push(qualified_period.to_owned());
    dimension_fields.push(period_field);
    for (unit, russian, english) in AUTO_PERIOD_LEVELS {
        let field = auto_period_level_field(period, russian, english);
        let truncated = dialect.begin_of_period(qualified_period, unit);
        projections.push(format!(
            "{truncated} AS {}",
            dialect.quote_identifier(&field.columns[0].physical_name)
        ));
        grouping.push(truncated);
        dimension_fields.push(field);
    }
    for schema_name in ["Recorder", "LineNo"] {
        let field = register_standard_field(fields, schema_name, virtual_table)?;
        for column in &field.columns {
            let sql = dialect.qualified_column(Some("__aggregate_base"), &column.physical_name);
            projections.push(format!(
                "{sql} AS {}",
                dialect.quote_identifier(&column.physical_name)
            ));
            grouping.push(sql);
        }
        dimension_fields.push(field.clone());
    }
    Ok(())
}

/// What an aggregating register table needs to drop the dimensions the
/// statement never reads. The platform aggregates over them, so a query
/// that reads one dimension gets one row per value of it.
pub(super) struct AggregateSource {
    /// Index into the relation fields of every dimension, with its
    /// physical columns.
    pub(super) dimensions: Vec<(usize, Vec<String>)>,
    /// Physical column of every resource; they are summed when a
    /// dimension is dropped.
    pub(super) resources: Vec<String>,
    /// Fields the statement may not read from this relation, by index,
    /// with the reason. A periodic `ОстаткиИОбороты` computes no running
    /// balance, so its balance columns are refused rather than answered
    /// with the balance of the whole interval.
    pub(super) forbidden: Vec<(usize, &'static str)>,
    /// Physical columns a periodicity splits `Обороты` by: the period,
    /// and the recorder and line number of the record periodicities. A
    /// periodicity is an explicit request to split, so the platform keeps
    /// those groupings even when the statement never reads the columns.
    pub(super) split: Vec<String>,
    /// Dimensions the `Авто` periodicity adds: the record period, its
    /// calendar levels, the recorder and the line number. Reading any of
    /// them splits the table.
    pub(super) split_dimensions: Vec<usize>,
    /// Fields refused only when the statement reads one of
    /// `split_dimensions`: the balances of an `Авто` table that is split.
    pub(super) forbidden_with_split: Vec<(usize, &'static str)>,
    /// Columns computed from the sum of another resource once the grain
    /// is known: the debit and credit parts of an accounting balance are
    /// the positive and the negated negative part of the balance at the
    /// grain the statement reads, not sums of finer parts.
    pub(super) derived: Vec<DerivedResource>,
    /// Relations prepared for the grains and balance columns a split
    /// table may be read at; the one matching what the statement reads
    /// replaces the relation before the unread dimensions are dropped.
    pub(super) variants: Vec<RelationVariant>,
    /// Indexes of the balance columns, which pick a variant with running
    /// sums when read.
    pub(super) balance_fields: Vec<usize>,
}

/// A resource column derived from the sum of a base column at the final
/// grain: `CASE WHEN SUM(base) > 0 THEN SUM(base) ELSE 0 END` for the
/// positive part, the negated negative part otherwise.
#[derive(Clone)]
pub(super) struct DerivedResource {
    pub(super) column: String,
    pub(super) base: String,
    pub(super) positive: bool,
}

impl DerivedResource {
    /// The expression over an aggregate of the base column.
    pub(super) fn expression(&self, sum: &str) -> String {
        if self.positive {
            format!("CASE WHEN {sum} > 0 THEN {sum} ELSE 0 END")
        } else {
            format!("CASE WHEN {sum} < 0 THEN -{sum} ELSE 0 END")
        }
    }
}

/// Sums away the dimensions the statement never reads, as the platform
/// does: a register table answers one row per combination of the
/// dimensions the query actually selects, filters, or joins on.
pub(super) fn finalize_aggregate_relation(
    scope: &mut SourceScope,
    token: &Token<'_>,
    dialect: SqlDialect,
) -> Result<(), QueryDiagnostic> {
    let Some(aggregate) = &scope.aggregate else {
        return Ok(());
    };
    let used = scope.used_fields.borrow();
    let split = aggregate
        .split_dimensions
        .iter()
        .any(|index| used.contains(index));
    // A split table prepared per grain takes the relation of the grain
    // the statement reads, with the balances as running sums when a
    // balance column is read.
    if !aggregate.variants.is_empty() {
        let grain = grain_of(
            aggregate
                .split_dimensions
                .iter()
                .enumerate()
                .filter(|(_, index)| used.contains(index))
                .map(|(position, _)| position),
        );
        let balances = aggregate
            .balance_fields
            .iter()
            .any(|index| used.contains(index));
        if let Some(variant) = aggregate.variants.iter().find(|variant| {
            variant.balances == balances && variant.grain.is_none_or(|own| own == grain)
        }) {
            scope.relation = variant.sql.clone();
        }
    }
    for (index, reason) in aggregate
        .forbidden
        .iter()
        .chain(aggregate.forbidden_with_split.iter().filter(|_| split))
    {
        if used.contains(index) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                (*reason).to_owned(),
            ));
        }
    }
    if aggregate
        .dimensions
        .iter()
        .all(|(index, _)| used.contains(index))
    {
        return Ok(());
    }
    let alias = dialect.quote_identifier("__aggregate_used");
    let columns = aggregate
        .dimensions
        .iter()
        .filter(|(index, _)| used.contains(index))
        .flat_map(|(_, columns)| columns.iter())
        .chain(aggregate.split.iter())
        .collect::<Vec<_>>();
    let kept = columns
        .iter()
        .map(|column| dialect.qualified_column(Some("__aggregate_used"), column))
        .collect::<Vec<_>>();
    let mut projection = kept
        .iter()
        .zip(columns.iter())
        .map(|(sql, column)| format!("{sql} AS {}", dialect.quote_identifier(column)))
        .collect::<Vec<_>>();
    for column in &aggregate.resources {
        projection.push(format!(
            "SUM({}) AS {}",
            dialect.qualified_column(Some("__aggregate_used"), column),
            dialect.quote_identifier(column)
        ));
    }
    for derived in &aggregate.derived {
        let sum = format!(
            "SUM({})",
            dialect.qualified_column(Some("__aggregate_used"), &derived.base)
        );
        projection.push(format!(
            "{} AS {}",
            derived.expression(&sum),
            dialect.quote_identifier(&derived.column)
        ));
    }
    let mut relation = format!(
        "(SELECT {} FROM {} AS {alias}",
        projection.join(", "),
        scope.relation
    );
    if !kept.is_empty() {
        relation.push_str(" GROUP BY ");
        relation.push_str(&kept.join(", "));
    }
    relation.push(')');
    drop(used);
    scope.relation = relation;
    Ok(())
}

/// Describes the dimensions and resources of the relation the two
/// aggregating tables build, so unused dimensions can be aggregated away
/// once the statement is known.
pub(super) fn aggregate_source(
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
) -> AggregateSource {
    AggregateSource {
        forbidden: Vec::new(),
        dimensions: dimension_fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                (
                    index,
                    field
                        .columns
                        .iter()
                        .map(|column| column.physical_name.clone())
                        .collect(),
                )
            })
            .collect(),
        resources: resource_fields
            .iter()
            .flat_map(|field| field.columns.iter())
            .map(|column| column.physical_name.clone())
            .collect(),
        split: Vec::new(),
        split_dimensions: Vec::new(),
        forbidden_with_split: Vec::new(),
        derived: Vec::new(),
        variants: Vec::new(),
        balance_fields: Vec::new(),
    }
}

struct BalanceTotals<'snapshot> {
    table: &'snapshot LiveTable,
    period: &'snapshot LiveColumn,
}

#[allow(clippy::too_many_arguments)]
fn compile_accumulation_balance_relation(
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    movement_table: &LiveTable,
    active_column: &QueryableColumn,
    movement_period: &QueryableColumn,
    record_kind: &QueryableColumn,
    restriction: Option<&SourceRestriction<'_>>,
    dialect: SqlDialect,
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    let totals = resolve_balance_totals(
        snapshot,
        object,
        dimension_fields,
        resource_fields,
        virtual_table.token,
    )?;
    let boundary = virtual_table.arguments.first().and_then(Option::as_ref);
    let condition = virtual_table.arguments.get(1).and_then(Option::as_ref);
    let boundary = boundary
        .map(|expression| {
            compile_virtual_period_literal(
                expression,
                virtual_table,
                "period boundary",
                catalog,
                dialect,
            )
        })
        .transpose()?;
    let totals_condition = compile_accumulation_condition(
        condition.as_slice(),
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        dimension_fields,
        None,
        totals.table,
        "__totals_base",
        restriction,
        dialect,
    )?;

    let relation = if let Some(boundary) = boundary {
        let movement_condition = compile_accumulation_condition(
            condition.as_slice(),
            source,
            virtual_table,
            snapshot,
            catalog,
            object,
            dimension_fields,
            None,
            movement_table,
            "__movement_base",
            restriction,
            dialect,
        )?;
        compile_historical_balance_sql(
            &boundary,
            totals,
            dimension_fields,
            resource_fields,
            active_column,
            movement_period,
            record_kind,
            &movement_table.name,
            totals_condition.predicate.as_deref(),
            &totals_condition.joins_sql(dialect),
            movement_condition.predicate.as_deref(),
            &movement_condition.joins_sql(dialect),
            dialect,
        )?
    } else {
        compile_current_balance_sql(
            totals,
            dimension_fields,
            resource_fields,
            totals_condition.predicate.as_deref(),
            &totals_condition.joins_sql(dialect),
            dialect,
        )?
    };

    let mut fields = dimension_fields.to_vec();
    fields.extend(
        resource_fields
            .iter()
            .map(|field| accumulation_resource_field(field, AccumulationKind::Balance)),
    );
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: fields.into(),
        aggregate: Some(aggregate_source(dimension_fields, resource_fields)),
        separators: Vec::new(),
    })
}

fn resolve_balance_totals<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    token: &Token<'_>,
) -> Result<BalanceTotals<'snapshot>, QueryDiagnostic> {
    let entries = snapshot
        .db_names()
        .entries()
        .iter()
        .filter(|entry| entry.guid == object.guid && entry.alias == "AccumRgT")
        .collect::<Vec<_>>();
    let entry = match entries.as_slice() {
        [entry] => *entry,
        [] => {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(token),
                "Balance requires an AccumRgT entry for the register GUID in DBNames",
            ));
        }
        _ => {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::AmbiguousObject,
                Some(token),
                "Balance totals mapping is ambiguous in DBNames",
            ));
        }
    };
    let physical_name = format!("_AccumRgT{}", entry.number);
    let schema_table = snapshot.schema().table(&physical_name).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!("Balance totals table {physical_name} is absent from SchemaStorage"),
        )
    })?;
    let live_table = snapshot.live_table(&physical_name).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::NotLive,
            Some(token),
            format!("Balance totals table {physical_name} is not live"),
        )
    })?;
    let period = live_table
        .columns
        .iter()
        .find(|column| names_equal(&logical_column_name(&column.name), "Period"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(token),
                format!("Balance totals table {physical_name} has no live Period column"),
            )
        })?;
    if !schema_table
        .columns
        .iter()
        .any(|column| names_equal(&logical_column_name(&column.physical_name()), "Period"))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!("Balance totals table {physical_name} has no declared Period column"),
        ));
    }
    for field in dimension_fields.iter().chain(resource_fields) {
        if !schema_table.columns.iter().any(|column| {
            names_equal(
                &logical_column_name(&column.physical_name()),
                &field.schema_name,
            )
        }) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(token),
                format!(
                    "Balance totals table {physical_name} does not declare field {:?}",
                    field.name
                ),
            ));
        }
        for column in &field.columns {
            if !live_table
                .columns
                .iter()
                .any(|live| names_equal(&live.name, &column.physical_name))
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::NotLive,
                    Some(token),
                    format!(
                        "Balance totals table {physical_name} has no live column {:?}",
                        column.physical_name
                    ),
                ));
            }
        }
    }
    Ok(BalanceTotals {
        table: live_table,
        period,
    })
}

#[allow(clippy::too_many_arguments)]
/// The condition of a virtual table as SQL: the predicate over the base
/// alias, and the reference joins a dereference in it needs, which the
/// caller renders after the `FROM` of the relation.
pub(super) struct ConditionSql {
    pub(super) predicate: Option<String>,
    pub(super) joins: Vec<JoinPlan>,
}

impl ConditionSql {
    /// The joins rendered for the `FROM` clause of the relation.
    pub(super) fn joins_sql(&self, dialect: SqlDialect) -> String {
        let mut sql = String::new();
        for join in &self.joins {
            append_reference_join(&mut sql, join, dialect);
        }
        sql
    }
}

/// Compiles the conditions of a virtual table in one context over the
/// given fields of the base alias — the data separators, the access
/// restriction, then each condition — so a dereference in any of them
/// gets a join of its own on the base alias.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_accumulation_condition(
    conditions: &[&Expression<'_, '_>],
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    mirror: Option<&[QueryableField]>,
    table: &LiveTable,
    alias: &str,
    restriction: Option<&SourceRestriction<'_>>,
    dialect: SqlDialect,
) -> Result<ConditionSql, QueryDiagnostic> {
    let mut predicates = separator_predicates(catalog, table, alias, virtual_table.token, dialect)?;
    let mut joins = Vec::new();
    if let Some(restriction) = restriction {
        let predicate = compile_restriction_predicate(
            restriction,
            snapshot,
            catalog,
            object,
            source.object.lexeme,
            dimension_fields,
            alias,
            dialect,
        )?;
        joins.extend(predicate.reference_joins);
        predicates.push(predicate.sql);
    }
    if conditions.is_empty() {
        return Ok(ConditionSql {
            predicate: conjunction(predicates),
            joins,
        });
    }
    let mut context = CompilationContext {
        snapshot,
        catalog,
        sources: vec![SourceScope {
            object: ObjectId::from(&object.guid),
            fields: dimension_fields.to_vec().into(),
            relation: String::new(),
            sql_alias: alias.to_owned(),
            object_name: source.object.lexeme.to_owned(),
            source_alias: Some(alias.to_owned()),
            identity_is_base: true,
            reference_joins: joins,
            separator_predicates: Vec::new(),
            constants: None,
            aggregate: None,
            used_fields: RefCell::new(BTreeSet::new()),
            current_table: false,
        }],
        dialect,
        aggregates_allowed: false,
        compiling_join_condition: false,
        dereference_in_join: false,
        source_elements: vec![0],
        section_aliases: std::cell::Cell::new(0),
        local_sources: 1,
    };
    // The names the mirror reads through other columns (as the join
    // plans key them, by schema name): a condition on one of them holds
    // when either reading does.
    let two_sided = mirror
        .map(|mirror| {
            mirror
                .iter()
                .filter(|field| {
                    dimension_fields.iter().any(|base| {
                        names_equal(&base.name, &field.name)
                            && base
                                .columns
                                .iter()
                                .map(|column| &column.physical_name)
                                .ne(field.columns.iter().map(|column| &column.physical_name))
                    })
                })
                .map(|field| field.schema_name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for condition in conditions {
        let predicate = compile_predicate(condition, &mut context)?;
        let Some(mirror) = mirror else {
            predicates.push(predicate);
            continue;
        };
        retire_joins(&mut context, &two_sided);
        let base = std::mem::replace(&mut context.sources[0].fields, mirror.to_vec().into());
        let mirrored = compile_predicate(condition, &mut context)?;
        context.sources[0].fields = base;
        retire_joins(&mut context, &two_sided);
        predicates.push(if mirrored == predicate {
            predicate
        } else {
            format!("({predicate} OR {mirrored})")
        });
    }
    Ok(ConditionSql {
        predicate: conjunction(predicates),
        joins: std::mem::take(&mut context.sources[0].reference_joins),
    })
}

/// Keeps the reference joins hung on a two-sided name from serving the
/// other side's reading of it: the join stays in the plan under a name no
/// field carries.
fn retire_joins(context: &mut CompilationContext<'_, '_>, two_sided: &[String]) {
    for join in &mut context.sources[0].reference_joins {
        if two_sided
            .iter()
            .any(|name| names_equal(name, &join.source_field))
        {
            join.source_field.insert(0, '\u{1}');
        }
    }
}

/// `None` for no predicate, otherwise the predicates joined by `AND`; each
/// one is already parenthesized by the expression compiler.
pub(super) fn conjunction(predicates: Vec<String>) -> Option<String> {
    (!predicates.is_empty()).then(|| predicates.join(" AND "))
}

fn compile_current_balance_sql(
    totals: BalanceTotals<'_>,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    condition: Option<&str>,
    joins: &str,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    let (dimension_projection, grouping) =
        accumulation_dimensions(dimension_fields, "__totals_base", dialect);
    let mut projections = dimension_projection;
    let mut aggregates = Vec::new();
    for field in resource_fields {
        let column = single_column_without_token(field)?;
        let aggregate = format!(
            "SUM({})",
            dialect.qualified_column(Some("__totals_base"), &column.physical_name)
        );
        projections.push(format!(
            "{aggregate} AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        aggregates.push(aggregate);
    }
    let period = dialect.qualified_column(Some("__totals_base"), &totals.period.name);
    let latest_period = dialect.qualified_column(Some("__totals_latest"), &totals.period.name);
    let mut predicates = vec![format!(
        "{period} = (SELECT MAX({latest_period}) FROM {} AS {})",
        dialect.quote_identifier(&totals.table.name),
        dialect.quote_identifier("__totals_latest")
    )];
    if let Some(condition) = condition {
        predicates.push(condition.to_owned());
    }
    let mut sql = format!(
        "(SELECT {} FROM {} AS {}{joins} WHERE {}",
        projections.join(", "),
        dialect.quote_identifier(&totals.table.name),
        dialect.quote_identifier("__totals_base"),
        predicates.join(" AND ")
    );
    append_balance_grouping(&mut sql, &grouping, &aggregates);
    sql.push(')');
    Ok(sql)
}

#[allow(clippy::too_many_arguments)]
fn compile_historical_balance_sql(
    boundary: &str,
    totals: BalanceTotals<'_>,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    active_column: &QueryableColumn,
    movement_period: &QueryableColumn,
    record_kind: &QueryableColumn,
    movement_table: &str,
    totals_condition: Option<&str>,
    totals_joins: &str,
    movement_condition: Option<&str>,
    movement_joins: &str,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    let totals_period = dialect.qualified_column(Some("__anchor_totals"), &totals.period.name);
    let anchor_period = dialect.qualified_column(Some("__balance_anchor"), "__period");
    let mut totals_parts = accumulation_part_dimensions(dimension_fields, "__totals_base", dialect);
    let mut movement_parts =
        accumulation_part_dimensions(dimension_fields, "__movement_base", dialect);
    let mut outer_projection =
        accumulation_part_dimensions(dimension_fields, "__balance_parts", dialect);
    let grouping = dimension_fields
        .iter()
        .flat_map(|field| field.columns.iter())
        .map(|column| dialect.qualified_column(Some("__balance_parts"), &column.physical_name))
        .collect::<Vec<_>>();
    let mut aggregates = Vec::new();
    for field in resource_fields {
        let column = single_column_without_token(field)?;
        totals_parts.push(format!(
            "{} AS {}",
            dialect.qualified_column(Some("__totals_base"), &column.physical_name),
            dialect.quote_identifier(&column.physical_name)
        ));
        let value = dialect.qualified_column(Some("__movement_base"), &column.physical_name);
        let signed = format!(
            "CASE WHEN {} = 0 THEN {value} ELSE -{value} END",
            dialect.qualified_column(Some("__movement_base"), &record_kind.physical_name)
        );
        movement_parts.push(format!(
            "CASE WHEN {anchor_period} <= {boundary} THEN {signed} ELSE -({signed}) END AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        let aggregate = format!(
            "SUM({})",
            dialect.qualified_column(Some("__balance_parts"), &column.physical_name)
        );
        outer_projection.push(format!(
            "{aggregate} AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        aggregates.push(aggregate);
    }

    let totals_base_period = dialect.qualified_column(Some("__totals_base"), &totals.period.name);
    let mut totals_predicates = vec![format!("{totals_base_period} = {anchor_period}")];
    if let Some(condition) = totals_condition {
        totals_predicates.push(condition.to_owned());
    }
    let movement_period =
        dialect.qualified_column(Some("__movement_base"), &movement_period.physical_name);
    let mut movement_predicates = vec![format!(
        "{} = {}",
        dialect.qualified_column(Some("__movement_base"), &active_column.physical_name),
        dialect.boolean_literal(true)
    )];
    movement_predicates.push(format!(
        "(({anchor_period} <= {boundary} AND {movement_period} >= {anchor_period} AND {movement_period} < {boundary}) OR ({anchor_period} > {boundary} AND {movement_period} >= {boundary} AND {movement_period} < {anchor_period}))"
    ));
    if let Some(condition) = movement_condition {
        movement_predicates.push(condition.to_owned());
    }

    // `MAX(CASE WHEN …)` is the portable spelling of `FILTER (WHERE …)`
    // (PostgreSQL 9.4), so one form serves every supported server.
    let anchor = format!(
        "COALESCE(MAX(CASE WHEN {totals_period} <= {boundary} THEN {totals_period} END), MAX({totals_period}))"
    );
    let totals_table = dialect.quote_identifier(&totals.table.name);
    let movement_table = dialect.quote_identifier(movement_table);
    let balance_anchor = dialect.quote_identifier("__balance_anchor");
    let balance_parts = dialect.quote_identifier("__balance_parts");
    let period_alias = dialect.quote_identifier("__period");
    let anchor_totals = dialect.quote_identifier("__anchor_totals");
    let totals_base = dialect.quote_identifier("__totals_base");
    let movement_base = dialect.quote_identifier("__movement_base");
    let mut sql = match dialect {
        SqlDialect::Postgres => format!(
            "(WITH {balance_anchor} AS (SELECT {anchor} AS {period_alias} FROM {totals_table} AS {anchor_totals}), {balance_parts} AS (SELECT {} FROM {totals_table} AS {totals_base}{totals_joins} CROSS JOIN {balance_anchor} WHERE {} UNION ALL SELECT {} FROM {movement_table} AS {movement_base}{movement_joins} CROSS JOIN {balance_anchor} WHERE {}) SELECT {} FROM {balance_parts}",
            totals_parts.join(", "),
            totals_predicates.join(" AND "),
            movement_parts.join(", "),
            movement_predicates.join(" AND "),
            outer_projection.join(", ")
        ),
        SqlDialect::MsSql { .. } => {
            let anchor_relation = format!(
                "(SELECT {anchor} AS {period_alias} FROM {totals_table} AS {anchor_totals}) AS {balance_anchor}"
            );
            format!(
                "(SELECT {} FROM (SELECT {} FROM {totals_table} AS {totals_base}{totals_joins} CROSS JOIN {anchor_relation} WHERE {} UNION ALL SELECT {} FROM {movement_table} AS {movement_base}{movement_joins} CROSS JOIN {anchor_relation} WHERE {}) AS {balance_parts}",
                outer_projection.join(", "),
                totals_parts.join(", "),
                totals_predicates.join(" AND "),
                movement_parts.join(", "),
                movement_predicates.join(" AND "),
            )
        }
    };
    append_balance_grouping(&mut sql, &grouping, &aggregates);
    sql.push(')');
    Ok(sql)
}

fn accumulation_dimensions(
    fields: &[QueryableField],
    alias: &str,
    dialect: SqlDialect,
) -> (Vec<String>, Vec<String>) {
    let mut projection = Vec::new();
    let mut grouping = Vec::new();
    for field in fields {
        for column in &field.columns {
            let value = dialect.qualified_column(Some(alias), &column.physical_name);
            projection.push(format!(
                "{value} AS {}",
                dialect.quote_identifier(&column.physical_name)
            ));
            grouping.push(value);
        }
    }
    (projection, grouping)
}

fn accumulation_part_dimensions(
    fields: &[QueryableField],
    alias: &str,
    dialect: SqlDialect,
) -> Vec<String> {
    accumulation_dimensions(fields, alias, dialect).0
}

fn append_balance_grouping(sql: &mut String, grouping: &[String], aggregates: &[String]) {
    if !grouping.is_empty() {
        sql.push_str(" GROUP BY ");
        sql.push_str(&grouping.join(", "));
    }
    sql.push_str(" HAVING (");
    sql.push_str(
        &aggregates
            .iter()
            .map(|aggregate| format!("{aggregate} <> 0"))
            .collect::<Vec<_>>()
            .join(" OR "),
    );
    sql.push(')');
}

fn single_column_without_token(
    field: &QueryableField,
) -> Result<&QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ => Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            format!(
                "accumulation resource {:?} must have one physical column",
                field.name
            ),
        )),
    }
}

fn accumulation_metadata_field(
    fields: &[QueryableField],
    metadata_field: &crate::metadata::MetadataField,
    virtual_table: &AccumulationAst<'_, '_>,
) -> Result<QueryableField, QueryDiagnostic> {
    let schema_name = format!("Fld{}", metadata_field.number);
    fields
        .iter()
        .find(|field| names_equal(&field.schema_name, &schema_name))
        .cloned()
        .ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(virtual_table.token),
                format!(
                    "{} field {:?} has no live physical representation",
                    virtual_table.kind.name(),
                    metadata_field.name.as_deref().unwrap_or(&schema_name)
                ),
            )
        })
}

/// A date parameter as a virtual-table period bound, in the storage domain.
pub(super) fn compile_date_parameter(
    token: &Token<'_>,
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match parameters.lookup(token)? {
        None => Ok("NULL".to_owned()),
        Some(value @ ParameterValue::Date(_)) => {
            render_scalar_parameter(value, token, dialect, true)
        }
        Some(_) => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Parameter,
            Some(token),
            format!("parameter {:?} must be a date", token.lexeme),
        )),
    }
}

pub(super) fn compile_virtual_period_literal(
    expression: &Expression<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    argument: &str,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Literal(token) => dialect.datetime_literal(token),
        Expression::DateTime { .. }
        | Expression::BeginOfPeriod { .. }
        | Expression::EndOfPeriod { .. }
        | Expression::DateAdd { .. }
        | Expression::Parameter(_) => {
            compile_constant_date_expression(expression, catalog.parameters(), dialect)
        }
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(virtual_table.token),
            format!(
                "{} {argument} must be a scalar literal",
                virtual_table.kind.name()
            ),
        )),
    }
}

/// Compiles a virtual-table period in the storage date domain: `ДАТАВРЕМЯ`,
/// a date parameter, or `НАЧАЛОПЕРИОДА`/`КОНЕЦПЕРИОДА`/`ДОБАВИТЬКДАТЕ` over
/// those, the count of `ДОБАВИТЬКДАТЕ` being a numeric literal or parameter.
pub(super) fn compile_constant_date_expression(
    expression: &Expression<'_, '_>,
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::DateTime { token, value } => dialect.datetime_expression(*value, true, token),
        Expression::Parameter(token) => compile_date_parameter(token, parameters, dialect),
        Expression::BeginOfPeriod { value, period, .. } => {
            let value = compile_constant_date_expression(value, parameters, dialect)?;
            Ok(dialect.begin_of_period(&value, *period))
        }
        Expression::EndOfPeriod { value, period, .. } => {
            let value = compile_constant_date_expression(value, parameters, dialect)?;
            Ok(dialect.end_of_period(&value, *period))
        }
        Expression::DateAdd {
            token,
            value,
            period,
            count,
        } => {
            let value = compile_constant_date_expression(value, parameters, dialect)?;
            let count = compile_constant_count(count, token, parameters, dialect)?;
            Ok(dialect.date_add(&value, *period, &count))
        }
        _ => Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            "date expression must be a constant DATETIME, BEGINOFPERIOD, ENDOFPERIOD, or DATEADD value or a date parameter",
        )),
    }
}

/// The count of a `ДОБАВИТЬКДАТЕ` inside a virtual-table period: a numeric
/// literal, optionally negated, or a numeric parameter.
fn compile_constant_count(
    count: &Expression<'_, '_>,
    token: &Token<'_>,
    parameters: Parameters<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match count {
        Expression::Literal(literal) if literal.kind == TokenKind::Number => {
            compile_literal(literal, dialect)
        }
        Expression::Unary { operator, value } if operator.lexeme == "-" => {
            let inner = compile_constant_count(value, token, parameters, dialect)?;
            Ok(format!("(-{inner})"))
        }
        Expression::Parameter(parameter) => match parameters.lookup(parameter)? {
            None => Ok("NULL".to_owned()),
            Some(value @ ParameterValue::Number { .. }) => {
                render_scalar_parameter(value, parameter, dialect, true)
            }
            Some(_) => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Parameter,
                Some(parameter),
                format!("parameter {:?} must be a number", parameter.lexeme),
            )),
        },
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            "DATEADD count in a virtual-table period must be a numeric literal or parameter",
        )),
    }
}

fn accumulation_resource_field(field: &QueryableField, kind: AccumulationKind) -> QueryableField {
    let (russian_suffix, english_suffix) = kind.resource_suffix();
    let russian_name = format!("{}{russian_suffix}", field.name);
    let english_name = format!("{}{english_suffix}", field.name);
    let mut result = field.clone();
    result.name = russian_name.clone();
    result.schema_name = format!("{}{english_suffix}", field.schema_name);
    result.aliases = vec![russian_name.clone(), english_name];
    for column in &mut result.columns {
        column.output_label = russian_name.clone();
    }
    result
}

pub(super) fn compile_presentation_plan(
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    target: ObjectId,
    alias: &str,
    plan: &PresentationPlan,
    token: Option<&Token<'_>>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    if plan.object != target {
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::PresentationPlan,
            token,
            "presentation plan target does not match the requested object",
        ));
    }
    let object = snapshot.object_by_id(target).ok_or_else(|| {
        QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::PresentationPlan,
            token,
            "presentation target was not resolved",
        )
    })?;
    let fields = catalog.fields(object, token)?;
    let mut unique = BTreeSet::new();
    for field in &plan.fields {
        if !unique.insert(*field) {
            return Err(QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::PresentationPlan,
                token,
                "presentation plan contains a duplicate field ID",
            ));
        }
        let _ = presentation_field(snapshot, object, &fields, *field, token)?;
    }
    let mut budget = 256_usize;
    compile_presentation_expression(
        snapshot,
        object,
        &fields,
        &plan.fields,
        &plan.expression,
        alias,
        token,
        0,
        &mut budget,
        dialect,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_presentation_expression(
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
    fields: &[QueryableField],
    authorized: &[FieldId],
    expression: &PresentationExpression,
    alias: &str,
    token: Option<&Token<'_>>,
    depth: usize,
    budget: &mut usize,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    if depth > 32 || *budget == 0 {
        return Err(QueryDiagnostic::at_or_unpositioned(
            QueryDiagnosticKind::PresentationPlan,
            token,
            "presentation expression is too complex",
        ));
    }
    *budget -= 1;
    match expression {
        PresentationExpression::Field(id) => {
            if !authorized.contains(id) {
                return Err(QueryDiagnostic::at_or_unpositioned(
                    QueryDiagnosticKind::PresentationPlan,
                    token,
                    "presentation expression uses a field not listed by its plan",
                ));
            }
            let field = presentation_field(snapshot, object, fields, *id, token)?;
            let column = single_column_at(field, token)?;
            Ok(format!(
                "COALESCE({}, {})",
                dialect.presentation_field_text(
                    &dialect.qualified_column(Some(alias), &column.physical_name),
                    &column.data_type
                ),
                dialect.string_literal("")
            ))
        }
        PresentationExpression::Literal(value) => Ok(dialect.string_literal(value)),
        PresentationExpression::Concat(parts) => {
            if parts.is_empty() {
                return Err(QueryDiagnostic::at_or_unpositioned(
                    QueryDiagnosticKind::PresentationPlan,
                    token,
                    "presentation concatenation cannot be empty",
                ));
            }
            let parts = parts
                .iter()
                .map(|part| {
                    compile_presentation_expression(
                        snapshot,
                        object,
                        fields,
                        authorized,
                        part,
                        alias,
                        token,
                        depth + 1,
                        budget,
                        dialect,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("concat({})", parts.join(", ")))
        }
    }
}

fn presentation_field<'field>(
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
    fields: &'field [QueryableField],
    id: FieldId,
    token: Option<&Token<'_>>,
) -> Result<&'field QueryableField, QueryDiagnostic> {
    let schema_name = match id {
        FieldId::Standard(standard) => standard.schema_name().to_owned(),
        FieldId::Metadata(attribute) => {
            let field = snapshot.attribute_by_id(attribute).map_err(|error| {
                QueryDiagnostic::lookup_at(
                    token,
                    error.clone(),
                    format!("invalid presentation field: {error}"),
                )
            })?;
            let owner = object.physical_table.as_deref().ok_or_else(|| {
                QueryDiagnostic::at_or_unpositioned(
                    QueryDiagnosticKind::PresentationPlan,
                    token,
                    "presentation target has no physical table",
                )
            })?;
            if !field
                .owner_tables
                .iter()
                .any(|table| names_equal(table, owner))
            {
                return Err(QueryDiagnostic::at_or_unpositioned(
                    QueryDiagnosticKind::PresentationPlan,
                    token,
                    "presentation attribute does not belong to its target object",
                ));
            }
            format!("Fld{}", field.number)
        }
    };
    fields
        .iter()
        .find(|field| names_equal(&field.schema_name, &schema_name))
        .ok_or_else(|| {
            QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::NotLive,
                token,
                format!("presentation field {schema_name} is not live on its target"),
            )
        })
}
