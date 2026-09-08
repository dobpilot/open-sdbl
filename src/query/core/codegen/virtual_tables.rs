use std::collections::BTreeSet;

use super::context::{CompilationContext, SourceScope};
use super::expression::{compile_expression, single_column, single_column_at};
use super::sources::CompiledSourceRelation;
use crate::Token;
use crate::metadata::{
    ConfigFieldPurpose, FieldId, LiveColumn, LiveTable, MetadataKind, MetadataObject,
    MetadataSnapshot, ObjectId,
};
use crate::query::core::ast::{AccumulationAst, AccumulationKind, Expression, SourceAst};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    CompilationCatalog, PresentationExpression, PresentationPlan, QueryableColumn, QueryableField,
    logical_column_name,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_accumulation_relation(
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    live_table: &LiveTable,
    fields: &[QueryableField],
    dialect: SqlDialect,
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
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
    if virtual_table.kind == AccumulationKind::Balance && record_kind.is_none() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            "Balance is unavailable for a turnover-only accumulation register",
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
            &live_table.name,
            active_column,
            period_column,
            record_kind.expect("a balance register has RecordKind"),
            dialect,
        );
    }

    if virtual_table
        .arguments
        .get(2)
        .and_then(Option::as_ref)
        .is_some()
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            "Turnovers periodicity is not supported yet; omit the third argument",
        ));
    }
    let begin = virtual_table.arguments.first().and_then(Option::as_ref);
    let end = virtual_table.arguments.get(1).and_then(Option::as_ref);
    let condition = virtual_table.arguments.get(3).and_then(Option::as_ref);

    let begin = begin
        .map(|expression| {
            compile_virtual_period_literal(expression, virtual_table, "begin period", dialect)
        })
        .transpose()?;
    let end = end
        .map(|expression| {
            compile_virtual_period_literal(expression, virtual_table, "period boundary", dialect)
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
    if let Some(sql) = compile_accumulation_condition(
        condition,
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        &dimension_fields,
        "__aggregate_base",
        dialect,
    )? {
        predicates.push(sql);
    }

    let mut projections = Vec::new();
    let mut grouping = Vec::new();
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
        let value = record_kind.map_or(value.clone(), |record_kind| {
            format!(
                "CASE WHEN {} = 0 THEN {value} ELSE -{value} END",
                dialect.qualified_column(Some("__aggregate_base"), &record_kind.physical_name)
            )
        });
        let aggregate = format!("SUM({value})");
        projections.push(format!(
            "{aggregate} AS {}",
            dialect.quote_identifier(&column.physical_name)
        ));
        virtual_resources.push(accumulation_resource_field(field, virtual_table.kind));
    }

    let mut relation = format!(
        "(SELECT {} FROM {} AS {} WHERE {}",
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

    dimension_fields.extend(virtual_resources);
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: dimension_fields.into(),
    })
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
    movement_table: &str,
    active_column: &QueryableColumn,
    movement_period: &QueryableColumn,
    record_kind: &QueryableColumn,
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
            compile_virtual_period_literal(expression, virtual_table, "period boundary", dialect)
        })
        .transpose()?;
    let totals_condition = compile_accumulation_condition(
        condition,
        source,
        virtual_table,
        snapshot,
        catalog,
        object,
        dimension_fields,
        "__totals_base",
        dialect,
    )?;

    let relation = if let Some(boundary) = boundary {
        let movement_condition = compile_accumulation_condition(
            condition,
            source,
            virtual_table,
            snapshot,
            catalog,
            object,
            dimension_fields,
            "__movement_base",
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
            movement_table,
            totals_condition.as_deref(),
            movement_condition.as_deref(),
            dialect,
        )?
    } else {
        compile_current_balance_sql(
            totals,
            dimension_fields,
            resource_fields,
            totals_condition.as_deref(),
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
fn compile_accumulation_condition(
    condition: Option<&Expression<'_, '_>>,
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    alias: &str,
    dialect: SqlDialect,
) -> Result<Option<String>, QueryDiagnostic> {
    let Some(condition) = condition else {
        return Ok(None);
    };
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
            reference_joins: Vec::new(),
        }],
        dialect,
    };
    let sql = compile_expression(condition, &mut context)?;
    if !context.sources[0].reference_joins.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(virtual_table.token),
            format!(
                "{} condition supports direct dimensions and separators only",
                virtual_table.kind.name()
            ),
        ));
    }
    Ok(Some(sql))
}

fn compile_current_balance_sql(
    totals: BalanceTotals<'_>,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    condition: Option<&str>,
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
        "(SELECT {} FROM {} AS {} WHERE {}",
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
    movement_condition: Option<&str>,
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

    let anchor = match dialect {
        SqlDialect::Postgres => format!(
            "COALESCE(MAX({totals_period}) FILTER (WHERE {totals_period} <= {boundary}), MAX({totals_period}))"
        ),
        SqlDialect::MsSql { .. } => format!(
            "COALESCE(MAX(CASE WHEN {totals_period} <= {boundary} THEN {totals_period} END), MAX({totals_period}))"
        ),
    };
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
            "(WITH {balance_anchor} AS (SELECT {anchor} AS {period_alias} FROM {totals_table} AS {anchor_totals}), {balance_parts} AS (SELECT {} FROM {totals_table} AS {totals_base} CROSS JOIN {balance_anchor} WHERE {} UNION ALL SELECT {} FROM {movement_table} AS {movement_base} CROSS JOIN {balance_anchor} WHERE {}) SELECT {} FROM {balance_parts}",
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
                "(SELECT {} FROM (SELECT {} FROM {totals_table} AS {totals_base} CROSS JOIN {anchor_relation} WHERE {} UNION ALL SELECT {} FROM {movement_table} AS {movement_base} CROSS JOIN {anchor_relation} WHERE {}) AS {balance_parts}",
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

fn compile_virtual_period_literal(
    expression: &Expression<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    argument: &str,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Literal(token) => dialect.datetime_literal(token),
        Expression::DateTime { .. } | Expression::BeginOfPeriod { .. } => {
            compile_constant_date_expression(expression, dialect)
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

pub(super) fn compile_constant_date_expression(
    expression: &Expression<'_, '_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::DateTime { token, value } => dialect.datetime_expression(*value, true, token),
        Expression::BeginOfPeriod {
            token: _,
            value,
            period,
        } => {
            let value = compile_constant_date_expression(value, dialect)?;
            Ok(dialect.begin_of_period(&value, *period))
        }
        _ => Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Metadata,
            "date expression must be a constant DATETIME or BEGINOFPERIOD value",
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
