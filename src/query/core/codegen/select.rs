use super::constants::{constants_source_scope, finalize_constants_relation};
use super::context::{
    CompilationContext, CompiledBranch, JoinPlan, OrderKey, OuterScope, ProjectedMember,
    ResolvedPath, ScopeId, SelectedProjection, SourceScope, attach_outer_scopes,
    compile_presentation, projected_members,
};
use super::expression::{
    compile_aggregate, compile_composite_projection, compile_expression, compile_predicate,
    expression_kind, reference_column, reference_type_column, single_column, spread_over_members,
    widen_reference,
};
use super::nested::{PendingSection, resolve_section};
use super::orchestrate::{PresentationCompilation, compile_query_ast};
use super::params::{
    object_type_number, parameter_kind, reference_constant_of_bytes, reference_constant_of_value,
    render_scalar_parameter,
};
use super::separators::separator_predicates;
use super::sources::{
    SourceRestriction, compile_source_free_branch, compile_source_free_expression,
    compile_source_relation, contains_aggregate, expression_children, projection_is_aggregated,
    projection_token, references_a_field, validate_aggregate_projection,
};
use super::virtual_tables::finalize_aggregate_relation;
use crate::metadata::{Guid, MetadataSnapshot, ObjectId};
use crate::query::core::ast::{
    AggregateArgument, CastTarget, Expression, FieldReference, JoinAst, JoinKind, OrderKeyAst,
    OrderTerm, PresentationArgument, Projection, ProjectionItem, SelectAst, SourceAst, TypeName,
};
use crate::query::core::dialect::{OutputLabelAllocator, SqlDialect, decode_binary_literal};
use crate::query::core::names::names_equal;
use crate::query::core::params::{ParameterColumn, ParameterValue};
use crate::query::core::temp_tables::cte_name;
use std::str::FromStr;

use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableColumn, QueryableField,
    resolve_source_metadata, restriction_label,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// How a branch is rendered within its statement.
#[derive(Clone, Copy)]
pub(super) struct BranchMode<'a> {
    /// Output positions whose fixed references must be widened to payloads.
    pub(super) widen: &'a BTreeSet<usize>,
    /// Projected values that other UNION branches carry as a composite,
    /// with the members every branch must spread that value over.
    pub(super) expand: &'a BTreeMap<usize, Vec<&'static str>>,
    /// Whether the statement is nested and keeps values in the storage
    /// domain (no MSSQL year-offset correction on projections).
    pub(super) storage_domain: bool,
    /// Whether a totals wrapper follows: order keys that are source
    /// expressions are projected as hidden `__order_<n>` columns, and the
    /// branch emits its own `ORDER BY` only when `ПЕРВЫЕ` depends on it.
    pub(super) totals: bool,
    /// The sources of the enclosing statement, visible to a correlated
    /// subquery by their qualifier.
    pub(super) outer: &'a [OuterScope],
}

pub(super) fn compile_branch(
    ast: &SelectAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    order_terms: &[OrderTerm<'_, '_>],
    union_order: bool,
    presentations: &mut PresentationCompilation<'_>,
    mode: BranchMode<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    let BranchMode {
        widen,
        expand: _,
        storage_domain,
        totals,
        outer,
    } = mode;
    let dialect = presentations.dialect;
    validate_aggregate_projection(ast)?;
    let Some(source) = ast.source.as_ref() else {
        return compile_source_free_branch(
            ast,
            order_terms,
            snapshot,
            dialect,
            widen,
            catalog.parameters(),
            storage_domain,
        );
    };
    let joins = ast.joins.as_slice();
    validate_join_projection(ast, joins)?;
    let grouped = !ast.group.is_empty() || ast.having.is_some();
    let parameter_columns = parameter_source_columns(ast);
    let mut context = compile_branch_context(
        outer,
        source,
        joins,
        &parameter_columns,
        snapshot,
        catalog,
        dialect,
        presentations,
    )?;
    context.aggregates_allowed = grouped
        || ast
            .projection
            .iter()
            .any(|projection| projection_is_aggregated(&projection.expression));
    // A projected tabular section is answered by a statement of its own,
    // so it leaves the list of projections before they compile and comes
    // back as a nested result once the main statement is rendered.
    let sections = take_projected_sections(ast, &context)?;
    let mut selected = compile_branch_projections(
        ast,
        source,
        joins.first(),
        &mut context,
        presentations,
        storage_domain,
    )?;
    let owner_keys = if sections.is_empty() {
        0
    } else {
        push_owner_key(&sections, &mut selected, &context)?
    };
    let mut group_by = compile_group_keys(ast, &selected, &mut context)?;
    context.aggregates_allowed = grouped;
    let having = ast
        .having
        .as_ref()
        .map(|having| compile_predicate(having, &mut context))
        .transpose()?;
    context.aggregates_allowed = false;

    let RenderedProjections {
        columns,
        sql: mut projections,
        deferred_presentations,
        logical_columns,
    } = render_selected_projections(&selected, &context, widen, mode.expand, storage_domain)?;
    // A composite value occupies several output columns, so the owner keys
    // are found by counting back from the end, where they were appended.
    let service_columns = (columns.len() - owner_keys..columns.len()).collect::<Vec<_>>();
    if projections.is_empty() {
        return Err(empty_projection_diagnostic(source, joins.first()));
    }

    let conditions = joins
        .iter()
        .enumerate()
        .map(|(index, join)| match &join.condition {
            Some(condition) => compile_join_condition(
                condition,
                &mut context,
                join.token,
                join.kind,
                ScopeId(index + 1),
            ),
            None => Ok(JoinCondition { sql: String::new() }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let filter = ast
        .filter
        .as_ref()
        .map(|filter| compile_predicate(filter, &mut context))
        .transpose()?;
    let mut derived_grouping = Vec::new();
    let mut order = compile_order_terms(
        order_terms,
        ast,
        &selected,
        &mut context,
        // `РАЗЛИЧНЫЕ` keeps only the projected values, so SQL orders such
        // a statement by its projected columns and nothing else.
        union_order || !joins.is_empty() || grouped || ast.distinct,
        grouped,
        // A joined statement orders by a field it does not project as
        // well, by the column itself; a union, a grouping or РАЗЛИЧНЫЕ
        // has only its projected columns to order by.
        !joins.is_empty() && !union_order && !grouped && !ast.distinct,
        &mut derived_grouping,
        if grouped {
            "GROUP BY ORDER BY field must be a key or a projection alias"
        } else if !joins.is_empty() {
            "JOIN ORDER BY field must occur in the projection"
        } else if ast.distinct {
            "DISTINCT ORDER BY field must occur in the projection"
        } else {
            "UNION ORDER BY field must occur in the first branch projection"
        },
    )?;
    // A dereference of a grouping key in the ordering reads joined
    // columns, which the grouping must cover as well.
    for expression in derived_grouping {
        if !group_by.contains(&expression) {
            group_by.push(expression);
        }
    }
    if totals {
        // A totals wrapper re-orders the rows by these keys, so expression
        // keys must be visible as columns of the wrapped statement.
        for (index, key) in order.iter_mut().enumerate() {
            if key.position.is_none() {
                let label = format!("__order_{}", index + 1);
                projections.push(format!(
                    "{} AS {}",
                    key.sql,
                    dialect.quote_identifier(&label)
                ));
                key.sql = dialect.quote_identifier(&label);
            }
        }
    }

    for (scope, source) in context
        .sources
        .iter_mut()
        .zip(std::iter::once(source).chain(joins.iter().map(|join| &join.source)))
    {
        finalize_constants_relation(scope, source.object, dialect)?;
        finalize_aggregate_relation(scope, source.object, dialect)?;
    }
    let mut sql = compile_branch_sql(
        ast,
        joins,
        &context,
        &projections,
        &conditions,
        filter.as_deref(),
    );
    if !group_by.is_empty() {
        sql.push_str(" GROUP BY ");
        sql.push_str(&group_by.join(", "));
    }
    if let Some(having) = having {
        sql.push_str(" HAVING ");
        sql.push_str(&having);
    }
    // A nested result embeds this statement as a subquery, where SQL
    // Server refuses `ORDER BY`; the owners it names do not depend on it.
    let keyed_sql = (!sections.is_empty()).then(|| sql.clone());
    if !order.is_empty() && !union_order && (!totals || ast.top.is_some()) {
        sql.push_str(" ORDER BY ");
        sql.push_str(
            &order
                .iter()
                .map(OrderKey::render)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    dialect.append_limit(&mut sql, ast.top);
    Ok(CompiledBranch {
        sql,
        keyed_sql,
        sections,
        service_columns,
        columns,
        deferred_presentations,
        logical_width: selected.len(),
        logical_columns,
        order,
    })
}

fn validate_join_projection(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
) -> Result<(), QueryDiagnostic> {
    let Some(first) = joins.first() else {
        return Ok(());
    };
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection.expression, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(first.token),
            if first.kind == JoinKind::Cross {
                "wildcard projection over several sources is not supported"
            } else {
                "wildcard projection in JOIN is not supported"
            },
        ));
    }
    Ok(())
}

/// The comma element of every scope: the base source and the sources its
/// joins introduce form element 0, each comma-listed source starts the
/// next element together with its own joins.
fn source_elements(joins: &[JoinAst<'_, '_>]) -> Vec<usize> {
    let mut elements = Vec::with_capacity(joins.len() + 1);
    let mut element = 0;
    elements.push(element);
    for join in joins {
        if join.kind == JoinKind::Cross {
            element += 1;
        }
        elements.push(element);
    }
    elements
}

#[allow(clippy::too_many_arguments)]
fn compile_branch_context<'snapshot, 'catalog>(
    outer: &[OuterScope],
    source: &SourceAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    parameter_columns: &ParameterColumns,
    snapshot: &'snapshot MetadataSnapshot,
    catalog: &'catalog CompilationCatalog<'snapshot>,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompilationContext<'snapshot, 'catalog>, QueryDiagnostic> {
    if !joins.is_empty() {
        let mut sources = vec![resolve_join_source(
            source,
            parameter_columns,
            snapshot,
            catalog,
            "__left",
            dialect,
            presentations,
        )?];
        for (index, join) in joins.iter().enumerate() {
            // Sources are numbered from one: the base is `__left`, the first
            // joined source `__right`, later ones `__join3`, `__join4`, …
            let default_alias = if index == 0 {
                "__right".to_owned()
            } else {
                format!("__join{}", index + 2)
            };
            let scope = resolve_join_source(
                &join.source,
                parameter_columns,
                snapshot,
                catalog,
                &default_alias,
                dialect,
                presentations,
            )?;
            if let Some(previous) = sources
                .iter()
                .find(|previous: &&SourceScope| names_equal(&previous.sql_alias, &scope.sql_alias))
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(join.token),
                    format!(
                        "JOIN sources must have distinct aliases; both resolve to {:?}",
                        previous.sql_alias
                    ),
                ));
            }
            sources.push(scope);
        }
        let local_sources = sources.len();
        let mut context = CompilationContext {
            section_aliases: std::cell::Cell::new(0),
            snapshot,
            catalog,
            sources,
            dialect,
            aggregates_allowed: false,
            compiling_join_condition: false,
            dereference_in_join: false,
            source_elements: source_elements(joins),
            local_sources,
        };
        attach_outer_scopes(&mut context, outer);
        return Ok(context);
    }
    let scope = resolve_join_source(
        source,
        parameter_columns,
        snapshot,
        catalog,
        "__src",
        dialect,
        presentations,
    )?;
    let mut context = CompilationContext {
        snapshot,
        catalog,
        sources: vec![scope],
        dialect,
        aggregates_allowed: false,
        compiling_join_condition: false,
        dereference_in_join: false,
        source_elements: vec![0],
        section_aliases: std::cell::Cell::new(0),
        local_sources: 1,
    };
    attach_outer_scopes(&mut context, outer);
    Ok(context)
}

/// Builds the scope of a `(ВЫБРАТЬ …) КАК alias` source: the nested
/// statement becomes the relation and its columns become the fields.
fn derived_source_scope(
    source: &SourceAst<'_, '_>,
    nested: &crate::query::core::ast::QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<SourceScope, QueryDiagnostic> {
    let compiled = compile_query_ast(
        nested,
        snapshot,
        catalog,
        presentations,
        Some(source.object),
    )?;
    let alias = source
        .alias
        .expect("the parser requires an alias on a nested source")
        .lexeme
        .to_owned();
    let fields = derived_fields(&compiled.columns, snapshot, dialect);
    Ok(SourceScope {
        object: derived_owner(),
        fields: fields.into(),
        relation: format!("({})", compiled.sql),
        sql_alias: alias.clone(),
        object_name: alias.clone(),
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
        separator_predicates: Vec::new(),
        constants: None,
        aggregate: None,
        used_fields: RefCell::new(BTreeSet::new()),
        current_table: false,
    })
}

/// Builds the scope of a temporary-table source: the stored CTE becomes the
/// relation and the stored columns become the fields.
/// The columns a branch reads from each source written as `&Таблица`,
/// keyed by the source's alias: what an unbound compilation — the
/// preparation pass, which sees no values — exposes on such a source.
type ParameterColumns = BTreeMap<String, Vec<String>>;

fn parameter_source_columns(ast: &SelectAst<'_, '_>) -> ParameterColumns {
    let mut columns = ParameterColumns::new();
    let sources = ast
        .source
        .iter()
        .chain(ast.joins.iter().map(|join| &join.source))
        .filter(|source| source.parameter);
    for source in sources {
        let alias = source
            .alias
            .map_or(source.object.lexeme.trim_start_matches('&'), |alias| {
                alias.lexeme
            });
        columns.insert(alias.to_owned(), Vec::new());
    }
    if columns.is_empty() {
        return columns;
    }
    let mut expressions: Vec<&Expression<'_, '_>> = Vec::new();
    let mut references: Vec<&FieldReference<'_, '_>> = Vec::new();
    for item in &ast.projection {
        match &item.expression {
            Projection::Field(reference) => references.push(reference),
            Projection::Scalar(expression) => expressions.push(expression),
            Projection::Aggregate {
                argument: AggregateArgument::Expression(expression),
                ..
            } => expressions.push(expression),
            Projection::Presentation { argument, .. } => match argument {
                PresentationArgument::Field(reference) => references.push(reference),
                PresentationArgument::Expression(expression) => expressions.push(expression),
                PresentationArgument::Literal(_) => {}
            },
            Projection::Aggregate { .. } | Projection::All | Projection::TabularSection { .. } => {}
        }
    }
    expressions.extend(ast.joins.iter().filter_map(|join| join.condition.as_ref()));
    expressions.extend(ast.filter.iter());
    expressions.extend(ast.group.iter().map(|key| &key.expression));
    expressions.extend(ast.having.iter());
    while let Some(expression) = expressions.pop() {
        if let Expression::Field(reference) = expression {
            references.push(reference);
        }
        expressions.extend(expression_children(expression));
    }
    for reference in references {
        let [alias, column] = reference.segments.as_slice() else {
            continue;
        };
        let Some(known) = columns
            .iter_mut()
            .find(|(name, _)| names_equal(name, alias.lexeme))
            .map(|(_, columns)| columns)
        else {
            continue;
        };
        if !known.iter().any(|name| names_equal(name, column.lexeme)) {
            known.push(column.lexeme.to_owned());
        }
    }
    columns
}

/// Builds the scope of a value table passed as a parameter. The rows are
/// inlined as a CTE of the statement — `SELECT 1 AS "__row", <values>
/// UNION ALL SELECT 2, …` — which the source reads by name; an unbound
/// compilation exposes the columns the branch names, of no kind.
fn parameter_source_scope(
    source: &SourceAst<'_, '_>,
    parameter_columns: &ParameterColumns,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
    let token = source.object;
    let name = token.lexeme.trim_start_matches('&');
    let alias = source
        .alias
        .map_or_else(|| name.to_owned(), |alias| alias.lexeme.to_owned());
    let (columns, relation) = if catalog.is_bound() {
        let value = catalog.parameters().lookup(token)?;
        let Some(ParameterValue::Table { columns, rows }) = value else {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Parameter,
                Some(token),
                format!(
                    "parameter {:?} read as a source must be bound to a value table",
                    token.lexeme
                ),
            ));
        };
        let (compiled, body) = render_parameter_table(columns, rows, token, snapshot, dialect)?;
        let cte = catalog.next_cte_name("__param");
        catalog.push_hierarchy_cte(cte.clone(), body);
        (compiled, dialect.quote_identifier(&cte))
    } else {
        let columns = parameter_columns
            .iter()
            .find(|(known, _)| names_equal(known, &alias))
            .map(|(_, columns)| columns.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|column| CompiledColumn::named(column.clone(), column.clone(), ColumnKind::Null))
            .collect::<Vec<_>>();
        (columns, dialect.quote_identifier("__param_unbound"))
    };
    let fields = derived_fields(&columns, snapshot, dialect);
    Ok(SourceScope {
        object: derived_owner(),
        fields: fields.into(),
        relation,
        sql_alias: alias.clone(),
        object_name: name.to_owned(),
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
        separator_predicates: Vec::new(),
        constants: None,
        aggregate: None,
        used_fields: RefCell::new(BTreeSet::new()),
        current_table: false,
    })
}

/// Renders a value table as the body of its CTE and describes its
/// columns. Each column has the kind it declares: the first row is cast
/// to it, so the CTE column carries that type whatever the later rows or
/// the emptiness of the table; every value must fit the kind, and a
/// reference must point at one of the kind's targets.
fn render_parameter_table(
    columns: &[ParameterColumn],
    rows: &[Vec<ParameterValue>],
    token: &Token<'_>,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<(Vec<CompiledColumn>, String), QueryDiagnostic> {
    let invalid = |message: String| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Parameter,
            Some(token),
            format!("table parameter {:?}: {message}", token.lexeme),
        )
    };
    if columns.is_empty() {
        return Err(invalid(
            "a value table needs at least one column".to_owned(),
        ));
    }
    let mut kinds = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        if column.name.is_empty()
            || column
                .name
                .contains(|character: char| character.is_whitespace())
        {
            return Err(invalid(format!("column {index} has no usable name")));
        }
        if columns[..index]
            .iter()
            .any(|earlier| names_equal(&earlier.name, &column.name))
        {
            return Err(invalid(format!("column {:?} is named twice", column.name)));
        }
        let kind = match &column.kind {
            ColumnKind::Reference {
                targets,
                runtime_typed,
            } => ColumnKind::Reference {
                targets: targets.clone(),
                runtime_typed: *runtime_typed || targets.len() != 1,
            },
            ColumnKind::String { .. }
            | ColumnKind::Number { .. }
            | ColumnKind::Boolean
            | ColumnKind::DateTime
            | ColumnKind::Binary { .. } => column.kind.clone(),
            other => {
                return Err(invalid(format!(
                    "column {:?} needs a scalar or reference kind, not {other:?}",
                    column.name
                )));
            }
        };
        kinds.push(kind);
    }
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != columns.len() {
            return Err(invalid(format!(
                "row {} has {} values for {} columns",
                row_index + 1,
                row.len(),
                columns.len()
            )));
        }
        for ((column, kind), value) in columns.iter().zip(&kinds).zip(row) {
            let fits = match (kind, value) {
                (_, ParameterValue::Null) => true,
                (ColumnKind::String { .. }, ParameterValue::String(_))
                | (ColumnKind::Number { .. }, ParameterValue::Number { .. })
                | (ColumnKind::Boolean, ParameterValue::Boolean(_))
                | (ColumnKind::DateTime, ParameterValue::Date(_))
                | (ColumnKind::Binary { .. }, ParameterValue::Binary(_)) => true,
                (
                    ColumnKind::Reference { targets, .. },
                    ParameterValue::Reference { object, .. },
                ) => targets.is_empty() || targets.contains(object),
                _ => false,
            };
            if !fits {
                return Err(invalid(format!(
                    "row {} holds {:?} in column {:?} of kind {kind:?}",
                    row_index + 1,
                    parameter_kind(value),
                    column.name
                )));
            }
        }
    }
    let cast_type = |kind: &ColumnKind| match derived_data_type(kind, dialect).as_str() {
        "varbinary" => "varbinary(max)".to_owned(),
        other => other.to_owned(),
    };
    let render = |kind: &ColumnKind, value: &ParameterValue| -> Result<String, QueryDiagnostic> {
        Ok(match (kind, value) {
            (
                ColumnKind::Reference {
                    runtime_typed: true,
                    ..
                },
                ParameterValue::Reference { object, id },
            ) => dialect.reference_payload(
                &dialect.binary_u32(object_type_number(*object, token, snapshot)?),
                &dialect.binary_literal(id),
            ),
            (_, other) => render_scalar_parameter(other, token, dialect, true)?,
        })
    };
    let labelled = |values: Vec<String>| {
        std::iter::once("__row".to_owned())
            .chain(columns.iter().map(|column| column.name.clone()))
            .zip(values)
            .map(|(label, sql)| format!("{sql} AS {}", dialect.quote_identifier(&label)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The first row types the CTE columns through casts; an empty table
    // is one row of typed `NULL`s that never answers.
    let mut body = match rows.first() {
        Some(first) => {
            let mut values = vec!["1".to_owned()];
            for (kind, value) in kinds.iter().zip(first) {
                values.push(format!(
                    "CAST({} AS {})",
                    render(kind, value)?,
                    cast_type(kind)
                ));
            }
            format!("SELECT {}", labelled(values))
        }
        None => {
            let values = std::iter::once("1".to_owned())
                .chain(
                    kinds
                        .iter()
                        .map(|kind| format!("CAST(NULL AS {})", cast_type(kind))),
                )
                .collect();
            format!("SELECT {} WHERE 1 = 0", labelled(values))
        }
    };
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        let mut values = vec![(row_index + 1).to_string()];
        for (kind, value) in kinds.iter().zip(row) {
            values.push(render(kind, value)?);
        }
        body.push_str(" UNION ALL SELECT ");
        body.push_str(&values.join(", "));
    }
    let compiled = columns
        .iter()
        .zip(kinds)
        .map(|(column, kind)| CompiledColumn::named(column.name.clone(), column.name.clone(), kind))
        .collect();
    Ok((compiled, body))
}

fn temporary_source_scope(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
    let table = catalog.temporary_source(source.object)?;
    let alias = source
        .alias
        .map_or_else(|| table.name.clone(), |token| token.lexeme.to_owned());
    let fields = derived_fields(&table.columns, snapshot, dialect);
    Ok(SourceScope {
        object: derived_owner(),
        fields: fields.into(),
        relation: dialect.quote_identifier(&cte_name(table.id)),
        sql_alias: alias.clone(),
        object_name: table.name,
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
        separator_predicates: Vec::new(),
        constants: None,
        aggregate: None,
        used_fields: RefCell::new(BTreeSet::new()),
        current_table: false,
    })
}

/// The placeholder owner of derived-source fields; no metadata object has
/// the nil GUID.
pub(super) fn derived_owner() -> ObjectId {
    ObjectId::from(&Guid::from_str(Guid::NIL).expect("the nil GUID is well formed"))
}

/// One field of a derived source, addressed by the nested column label.
/// Builds the scope of a filter-criterion source. The platform searches
/// every field the criterion lists and returns the objects holding the
/// value, which is one `SELECT` per field united by `UNION ALL`; the
/// single field `Ссылка` carries the `RTRef ‖ RRRef` payload of the found
/// object, exactly as a derived source does.
fn criterion_source_scope(
    source: &SourceAst<'_, '_>,
    value: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    default_alias: &str,
    dialect: SqlDialect,
) -> Result<SourceScope, QueryDiagnostic> {
    let criterion = snapshot.criterion(source.object.lexeme).ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownObject,
            Some(source.object),
            format!("unknown filter criterion {:?}", source.object.lexeme),
        )
    })?;
    let value_sql = criterion_value_sql(value, snapshot, catalog, dialect)?;
    let mut branches = Vec::new();
    let mut targets = Vec::new();
    for guid in &criterion.content {
        let Some(field) = snapshot.fields().iter().find(|field| &field.guid == guid) else {
            continue;
        };
        for table in &field.owner_tables {
            let Some(live) = snapshot.live_table(table) else {
                continue;
            };
            let Ok(object_id) = snapshot.object_id_by_physical_table(table) else {
                continue;
            };
            let Some(object) = snapshot.object_by_id(object_id) else {
                continue;
            };
            let Some(number) = object.number else {
                continue;
            };
            let Some(identity) = live
                .columns
                .iter()
                .find(|column| names_equal(&column.name, "_IDRRef"))
            else {
                continue;
            };
            let Some(column) = live.columns.iter().find(|column| {
                names_equal(&column.name, &field.physical_name)
                    || names_equal(&column.name, &format!("{}RRef", field.physical_name))
            }) else {
                continue;
            };
            let alias = format!("__criterion{}", branches.len() + 1);
            let mut predicates =
                separator_predicates(catalog, live, &alias, source.object, dialect)?;
            predicates.push(format!(
                "({} = {value_sql})",
                dialect.qualified_column(Some(&alias), &column.name)
            ));
            branches.push(format!(
                "SELECT {} AS {} FROM {} AS {} WHERE {}",
                dialect.reference_payload(
                    &dialect.binary_u32(number),
                    &dialect.qualified_column(Some(&alias), &identity.name),
                ),
                dialect.quote_identifier(CRITERION_FIELD),
                dialect.quote_identifier(&live.name),
                dialect.quote_identifier(&alias),
                predicates.join(" AND "),
            ));
            targets.push(object_id);
        }
    }
    if branches.is_empty() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(source.object),
            format!(
                "filter criterion {:?} searches no live field",
                source.object.lexeme
            ),
        ));
    }
    let alias = source
        .alias
        .map_or_else(|| default_alias.to_owned(), |token| token.lexeme.to_owned());
    let kind = ColumnKind::Reference {
        targets,
        runtime_typed: true,
    };
    let field = derived_field(
        0,
        CRITERION_FIELD,
        CRITERION_FIELD,
        &kind,
        snapshot,
        dialect,
    );
    Ok(SourceScope {
        object: derived_owner(),
        fields: vec![field].into(),
        relation: format!("({})", branches.join(" UNION ALL ")),
        sql_alias: alias.clone(),
        object_name: alias.clone(),
        source_alias: Some(alias),
        identity_is_base: false,
        reference_joins: Vec::new(),
        separator_predicates: Vec::new(),
        constants: None,
        aggregate: None,
        used_fields: RefCell::new(BTreeSet::new()),
        current_table: false,
    })
}

/// The value a criterion searches for, rendered the way its content
/// columns store it: a reference keeps only its 16-byte identifier, the
/// way the platform compares it, and every other value compiles as
/// written.
fn criterion_value_sql(
    value: &Expression<'_, '_>,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    dialect: SqlDialect,
) -> Result<String, QueryDiagnostic> {
    match value {
        Expression::Parameter(token) => {
            if let Some(bound) = catalog.parameters().lookup(token)?
                && let Some(constant) =
                    reference_constant_of_value(bound, token, snapshot, dialect)?
            {
                return Ok(constant.id_sql);
            }
        }
        Expression::Literal(token) if token.kind == TokenKind::Binary => {
            let bytes = decode_binary_literal(token)?;
            if let Some(constant) = reference_constant_of_bytes(&bytes, dialect) {
                return Ok(constant.id_sql);
            }
        }
        _ => {}
    }
    compile_source_free_expression(value, snapshot, dialect, catalog.parameters(), true)
}

/// The only field a filter-criterion source exposes.
const CRITERION_FIELD: &str = "Ссылка";

/// The fields of a derived source: each column under the name the text
/// gave it, except that a name two columns share — `А.Контрагент,
/// Б.Контрагент` without aliases — falls back to the emitted labels, which
/// the allocator keeps distinct.
fn derived_fields(
    columns: &[CompiledColumn],
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Vec<QueryableField> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let shared = columns
                .iter()
                .filter(|other| names_equal(&other.name, &column.name))
                .count()
                > 1;
            let name = if shared { &column.label } else { &column.name };
            derived_field(index, name, &column.label, &column.kind, snapshot, dialect)
        })
        .collect()
}

/// A field of a derived source: named by the alias the text gave the
/// projection, read through the label the SQL emitted.
fn derived_field(
    index: usize,
    name: &str,
    label: &str,
    kind: &ColumnKind,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> QueryableField {
    let physical_table = |id: &ObjectId| {
        snapshot
            .object_by_id(*id)
            .and_then(|object| object.physical_table.clone())
    };
    let (reference_target, reference_targets) = match kind {
        ColumnKind::Reference {
            targets,
            runtime_typed: false,
        } => {
            let tables = targets
                .iter()
                .filter_map(physical_table)
                .collect::<Vec<_>>();
            let single = (tables.len() == 1 && targets.len() == 1).then(|| tables[0].clone());
            (single, tables)
        }
        // A payload column already carries the RTRef: it is presented the
        // way universal references are, deferred to the application.
        ColumnKind::Reference { .. } => (None, vec![String::new()]),
        _ => (None, Vec::new()),
    };
    QueryableField {
        name: name.to_owned(),
        schema_name: format!("__derived{}", index + 1),
        aliases: vec![name.to_owned()],
        columns: vec![QueryableColumn {
            physical_name: label.to_owned(),
            data_type: derived_data_type(kind, dialect),
            output_label: name.to_owned(),
            kind: kind.clone(),
        }],
        reference_target,
        reference_targets,
    }
}

/// A catalog type name that renders literals correctly for a derived
/// column; kinds carry the truth.
pub(super) fn derived_data_type(kind: &ColumnKind, dialect: SqlDialect) -> String {
    let postgres = dialect == SqlDialect::Postgres;
    match kind {
        ColumnKind::String { .. } => {
            if postgres {
                "text"
            } else {
                "nvarchar(max)"
            }
        }
        ColumnKind::Number { .. } => "numeric",
        ColumnKind::Boolean => {
            if postgres {
                "boolean"
            } else {
                "bit"
            }
        }
        ColumnKind::DateTime => {
            if postgres {
                "timestamp without time zone"
            } else {
                "datetime2"
            }
        }
        ColumnKind::Reference { .. } | ColumnKind::Binary { .. } | ColumnKind::Type => {
            if postgres {
                "bytea"
            } else {
                "varbinary"
            }
        }
        ColumnKind::Uuid => {
            if postgres {
                "uuid"
            } else {
                "uniqueidentifier"
            }
        }
        ColumnKind::Null | ColumnKind::Undefined => "",
        ColumnKind::Unknown { data_type } => data_type.as_str(),
    }
    .to_owned()
}

fn compile_branch_projections(
    ast: &SelectAst<'_, '_>,
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
    context: &mut CompilationContext<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
    storage_domain: bool,
) -> Result<Vec<SelectedProjection>, QueryDiagnostic> {
    let wildcard = match ast.projection.as_slice() {
        [
            ProjectionItem {
                expression: Projection::All,
                ..
            },
        ] => true,
        [
            ProjectionItem {
                expression: Projection::TabularSection { path, columns },
                ..
            },
        ] => source_wildcard_scope(context, path, columns).is_some(),
        _ => false,
    };
    if join.is_none() && wildcard {
        let scope = context.source(ScopeId(0));
        return Ok(scope
            .fields
            .iter()
            .enumerate()
            .map(|field| ResolvedPath::from_source(ScopeId(0), scope, field))
            .map(SelectedProjection::Field)
            .collect());
    }
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection.expression, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(source.object),
            "'*' cannot be combined with named fields",
        ));
    }
    compile_selected_projections(ast, context, presentations, storage_domain)
}

/// Whether a projection names a tabular section rather than a value.
fn is_section_projection(
    context: &CompilationContext<'_, '_>,
    projection: &Projection<'_, '_>,
) -> bool {
    match projection {
        Projection::TabularSection { .. } => true,
        Projection::Field(reference) => context.section_scope_of(reference).is_some(),
        _ => false,
    }
}

/// Takes the projected tabular sections out of the projection list. A
/// section written as `Состав.(…)` or `Состав.*` says so outright; the
/// bare `Состав` form looks like a field and only metadata tells them
/// apart, so it is recognized here.
/// The source `Псевдоним.*` names, when it names one rather than a
/// tabular section: no columns listed and the qualifier a source's own.
fn source_wildcard_scope(
    context: &CompilationContext<'_, '_>,
    path: &FieldReference<'_, '_>,
    columns: &[&Token<'_>],
) -> Option<ScopeId> {
    if !columns.is_empty() {
        return None;
    }
    let [qualifier] = path.segments.as_slice() else {
        return None;
    };
    context.qualifier_scope(qualifier).ok().flatten()
}

fn take_projected_sections(
    ast: &SelectAst<'_, '_>,
    context: &CompilationContext<'_, '_>,
) -> Result<Vec<PendingSection>, QueryDiagnostic> {
    let mut sections = Vec::new();
    let mut position = 0;
    for projection in &ast.projection {
        let (path, requested) = match &projection.expression {
            Projection::TabularSection { path, columns } => (path, columns.as_slice()),
            Projection::Field(reference) => {
                let Some(scope) = context.section_scope_of(reference) else {
                    position += 1;
                    continue;
                };
                let _ = scope;
                (reference, [].as_slice())
            }
            _ => {
                position += 1;
                continue;
            }
        };
        let Some(scope) = context.section_scope_of(path) else {
            // `Псевдоним.*` of a source stands for every field of it,
            // taken up with the projections.
            if let Some(source) = source_wildcard_scope(context, path, requested) {
                position += context.source(source).fields.len();
                continue;
            }
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnknownObject,
                Some(path.last()),
                format!(
                    "tabular section {:?} was not found in {}",
                    path.last().lexeme,
                    context.scope_description()
                ),
            ));
        };
        let label = projection.alias.map_or_else(
            || path.last().lexeme.to_owned(),
            |alias| alias.lexeme.to_owned(),
        );
        sections.push(resolve_section(
            context,
            scope,
            path.last(),
            requested,
            label,
            position,
        )?);
        position += 1;
    }
    Ok(sections)
}

/// Adds the owner key of every source a section belongs to, so the nested
/// statement can link its rows to the main ones. The key is a service
/// column: a consumer showing the result leaves it out.
fn push_owner_key(
    sections: &[PendingSection],
    selected: &mut Vec<SelectedProjection>,
    context: &CompilationContext<'_, '_>,
) -> Result<usize, QueryDiagnostic> {
    let mut scopes = sections
        .iter()
        .map(|section| section.owner_scope)
        .collect::<Vec<_>>();
    scopes.dedup();
    let keys = scopes.len();
    for scope in scopes {
        let sql = context.identity_sql(scope)?;
        selected.push(SelectedProjection::Generated {
            sql,
            label: "__owner".to_owned(),
            deferred: false,
            kind: ColumnKind::Binary { length: None },
        });
    }
    Ok(keys)
}

fn empty_projection_diagnostic(
    source: &SourceAst<'_, '_>,
    join: Option<&JoinAst<'_, '_>>,
) -> QueryDiagnostic {
    join.map_or_else(
        || {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(source.object),
                "metadata table has no queryable live columns",
            )
        },
        |join| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(join.token),
                "JOIN has no queryable projected columns",
            )
        },
    )
}

struct JoinCondition {
    sql: String,
}

struct RenderedProjections {
    columns: Vec<CompiledColumn>,
    sql: Vec<String>,
    deferred_presentations: Vec<usize>,
    /// How many columns each projected value occupies.
    logical_columns: Vec<usize>,
}

fn compile_selected_projections(
    ast: &SelectAst<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
    storage_domain: bool,
) -> Result<Vec<SelectedProjection>, QueryDiagnostic> {
    let mut selected = Vec::with_capacity(ast.projection.len());
    for projection in &ast.projection {
        // `Псевдоним.*` stands for every field of that source, in the
        // metadata order, wherever in the list it is written.
        if let Projection::TabularSection { path, columns } = &projection.expression
            && let Some(scope_id) = source_wildcard_scope(context, path, columns)
        {
            let scope = context.source(scope_id);
            selected.extend(
                scope
                    .fields
                    .iter()
                    .enumerate()
                    .map(|field| ResolvedPath::from_source(scope_id, scope, field))
                    .map(SelectedProjection::Field),
            );
            continue;
        }
        // A tabular section is answered by a statement of its own, which
        // `take_projected_sections` has already resolved.
        if is_section_projection(context, &projection.expression) {
            continue;
        }
        match &projection.expression {
            Projection::Field(reference) => {
                let mut resolved = context.resolve(reference)?;
                if let Some(alias) = projection.alias {
                    resolved.path_label = Some(alias.lexeme.to_owned());
                } else if resolved.path_label.is_none() {
                    // The platform names the column by the field as the
                    // text spells it — `Ссылка`, not the schema's `ID`.
                    resolved.path_label = Some(reference.last().lexeme.to_owned());
                }
                selected.push(SelectedProjection::Field(resolved));
            }
            Projection::Presentation {
                token,
                operation,
                argument,
            } => {
                let (sql, label, deferred) =
                    compile_presentation(context, token, *operation, argument, presentations)?;
                selected.push(SelectedProjection::Generated {
                    sql,
                    label: projection
                        .alias
                        .map_or(label, |alias| alias.lexeme.to_owned()),
                    deferred,
                    kind: if deferred {
                        ColumnKind::Reference {
                            targets: Vec::new(),
                            runtime_typed: true,
                        }
                    } else {
                        ColumnKind::String { length: None }
                    },
                });
            }
            Projection::Scalar(expression) => {
                let number = selected.len() + 1;
                // Alternatives of different types are one value of a
                // composite type, which the platform spreads over the
                // members of that type.
                if let Some(members) = compile_composite_projection(expression, context)? {
                    let label = projection.alias.map_or_else(
                        || format!("__expr{number}"),
                        |alias| alias.lexeme.to_owned(),
                    );
                    for member in members {
                        selected.push(SelectedProjection::Generated {
                            sql: member.sql,
                            label: format!("{label}{}", member.suffix),
                            deferred: false,
                            kind: member.kind,
                        });
                    }
                    continue;
                }
                let sql = compile_expression(expression, context)?;
                let kind = expression_kind(expression, context)?;
                selected.push(SelectedProjection::Generated {
                    sql: if kind == ColumnKind::DateTime && !storage_domain {
                        context.dialect.date_scalar(&sql)
                    } else {
                        sql
                    },
                    label: projection.alias.map_or_else(
                        || format!("column{number}"),
                        |alias| alias.lexeme.to_owned(),
                    ),
                    deferred: false,
                    kind,
                });
            }
            Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            } => {
                let (sql, output_kind) = compile_aggregate(context, *kind, *distinct, argument)?;
                selected.push(SelectedProjection::Generated {
                    sql,
                    label: projection
                        .alias
                        .map_or_else(|| token.lexeme.to_owned(), |alias| alias.lexeme.to_owned()),
                    deferred: false,
                    kind: output_kind,
                });
            }
            Projection::TabularSection { .. } => {
                unreachable!("tabular sections are taken out before projections compile")
            }
            Projection::All => unreachable!("wildcard projections are handled by the caller"),
        }
    }
    Ok(selected)
}

/// Renders the selected projections as `expr AS label` pairs. Columns whose
/// position is listed in `widen` are fixed references that another UNION
/// branch projects as a runtime-typed payload; they are widened here so
/// every branch emits the same width.
fn render_selected_projections(
    selected: &[SelectedProjection],
    context: &CompilationContext<'_, '_>,
    widen: &BTreeSet<usize>,
    expand: &BTreeMap<usize, Vec<&'static str>>,
    storage_domain: bool,
) -> Result<RenderedProjections, QueryDiagnostic> {
    let mut columns = Vec::new();
    let mut sql = Vec::new();
    let mut deferred_presentations = Vec::new();
    let mut logical_columns = Vec::with_capacity(selected.len());
    let mut labels = OutputLabelAllocator::new(context.dialect);
    for (logical, selected) in selected.iter().enumerate() {
        let before = columns.len();
        // Another branch of the union carries this value as a composite,
        // so it is spread over the same members here.
        if let Some(members) = expand.get(&logical)
            && let Some((value_sql, kind, label)) = projection_scalar(selected, context)
        {
            for (member, sql_text) in members.iter().zip(spread_over_members(
                &value_sql, &kind, members, None, context,
            )?) {
                context.catalog.charge(1, None)?;
                let requested = format!("{label}{member}");
                let output_label = labels.allocate(&requested);
                sql.push(format!(
                    "{sql_text} AS {}",
                    context.dialect.quote_identifier(&output_label)
                ));
                columns.push(CompiledColumn::named(
                    requested,
                    output_label,
                    composite_member_kind_of(member),
                ));
            }
            logical_columns.push(columns.len() - before);
            continue;
        }
        match selected {
            SelectedProjection::Field(resolved) => {
                for member in projected_members(resolved.field()) {
                    context.catalog.charge(1, None)?;
                    let (expression, requested_label, kind) = match member {
                        ProjectedMember::Single(column) => (
                            if storage_domain {
                                context.dialect.storage_column_projection(
                                    &context.sql_column(resolved, column),
                                    &column.kind,
                                    &column.data_type,
                                )
                            } else {
                                context.dialect.column_projection(
                                    &context.sql_column(resolved, column),
                                    &column.kind,
                                    &column.data_type,
                                )
                            },
                            resolved.output_label(column),
                            column.kind.clone(),
                        ),
                        ProjectedMember::Reference {
                            type_member,
                            value_member,
                        } => (
                            context.dialect.reference_payload(
                                &context.sql_column(resolved, type_member),
                                &context.sql_column(resolved, value_member),
                            ),
                            resolved.field_label(),
                            value_member.kind.clone(),
                        ),
                    };
                    let (expression, kind) = if widen.contains(&columns.len()) {
                        widen_reference(
                            &expression,
                            &kind,
                            None,
                            context.snapshot,
                            context.dialect,
                        )?
                    } else {
                        (expression, kind)
                    };
                    let output_label = labels.allocate(&requested_label);
                    sql.push(format!(
                        "{expression} AS {}",
                        context.dialect.quote_identifier(&output_label)
                    ));
                    columns.push(CompiledColumn::named(
                        requested_label.clone(),
                        output_label,
                        kind,
                    ));
                }
            }
            SelectedProjection::Generated {
                sql: expression,
                label,
                deferred,
                kind,
            } => {
                context.catalog.charge(1, None)?;
                let (expression, kind) = if widen.contains(&columns.len()) {
                    widen_reference(expression, kind, None, context.snapshot, context.dialect)?
                } else {
                    (expression.clone(), kind.clone())
                };
                let expression = context.dialect.expression_projection(&expression, &kind);
                let output_label = labels.allocate(label);
                sql.push(format!(
                    "{expression} AS {}",
                    context.dialect.quote_identifier(&output_label)
                ));
                if *deferred {
                    deferred_presentations.push(columns.len());
                }
                columns.push(CompiledColumn::named(label.clone(), output_label, kind));
            }
        }
        logical_columns.push(columns.len() - before);
    }
    Ok(RenderedProjections {
        columns,
        sql,
        deferred_presentations,
        logical_columns,
    })
}

/// The value a projection carries when it occupies one output column, with
/// the label that column would take. `None` for a projection that is
/// already several columns.
fn projection_scalar(
    selected: &SelectedProjection,
    context: &CompilationContext<'_, '_>,
) -> Option<(String, ColumnKind, String)> {
    match selected {
        SelectedProjection::Generated {
            sql,
            label,
            deferred,
            kind,
        } => (!*deferred).then(|| (sql.clone(), kind.clone(), label.clone())),
        SelectedProjection::Field(resolved) => match projected_members(resolved.field()).as_slice()
        {
            [ProjectedMember::Single(column)] => Some((
                context.sql_column(resolved, column),
                column.kind.clone(),
                resolved.output_label(column),
            )),
            [
                ProjectedMember::Reference {
                    type_member,
                    value_member,
                },
            ] => Some((
                context.dialect.reference_payload(
                    &context.sql_column(resolved, type_member),
                    &context.sql_column(resolved, value_member),
                ),
                ColumnKind::Reference {
                    targets: match &value_member.kind {
                        ColumnKind::Reference { targets, .. } => targets.clone(),
                        _ => Vec::new(),
                    },
                    runtime_typed: true,
                },
                resolved.field_label(),
            )),
            _ => None,
        },
    }
}

/// The kind of one member column of a composite value.
pub(super) fn composite_member_kind_of(member: &str) -> ColumnKind {
    match member {
        "_TYPE" => ColumnKind::Binary { length: Some(1) },
        "_S" => ColumnKind::String { length: None },
        "_N" => ColumnKind::Number {
            precision: None,
            scale: None,
        },
        "_T" => ColumnKind::DateTime,
        "_L" => ColumnKind::Boolean,
        _ => ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        },
    }
}

/// The output position (1-based) of the first column of projection `index`.
fn projection_position(selected: &[SelectedProjection], index: usize) -> usize {
    selected[..index]
        .iter()
        .map(|selected| match selected {
            SelectedProjection::Field(resolved) => projected_members(resolved.field()).len(),
            SelectedProjection::Generated { .. } => 1,
        })
        .sum::<usize>()
        + 1
}

/// The projection whose alias equals a single-segment field reference.
fn aliased_projection(ast: &SelectAst<'_, '_>, field: &FieldReference<'_, '_>) -> Option<usize> {
    let [segment] = field.segments.as_slice() else {
        return None;
    };
    ast.projection.iter().position(|projection| {
        projection
            .alias
            .is_some_and(|alias| names_equal(alias.lexeme, segment.lexeme))
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_order_terms(
    order_terms: &[OrderTerm<'_, '_>],
    ast: &SelectAst<'_, '_>,
    selected: &[SelectedProjection],
    context: &mut CompilationContext<'_, '_>,
    positional: bool,
    grouped: bool,
    unprojected_fields: bool,
    derived_grouping: &mut Vec<String>,
    missing_message: &'static str,
) -> Result<Vec<OrderKey>, QueryDiagnostic> {
    let mut keys = Vec::with_capacity(order_terms.len());
    let key_paths = group_key_paths(ast);
    for term in order_terms {
        let field = match &term.key {
            OrderKeyAst::Field(field) => field,
            OrderKeyAst::Expression(expression) => {
                // A grouped statement orders by an aggregate expression
                // — `МАКСИМУМ(Период) УБЫВ` — as the platform does.
                if positional && !(grouped && contains_aggregate(expression)) {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(term.token),
                        missing_message,
                    ));
                }
                let aggregates_allowed = context.aggregates_allowed;
                context.aggregates_allowed |= grouped;
                let sql = compile_expression(expression, context);
                context.aggregates_allowed = aggregates_allowed;
                keys.push(OrderKey {
                    sql: sql?,
                    position: None,
                    descending: term.descending,
                });
                continue;
            }
        };
        if let Some(index) = aliased_projection(ast, field) {
            if positional {
                keys.push(OrderKey {
                    sql: String::new(),
                    position: Some(projection_position(selected, index)),
                    descending: term.descending,
                });
                continue;
            }
            // A projection alias orders a plain branch by the projected
            // expression, as on the platform.
            match &selected[index] {
                SelectedProjection::Generated { sql, .. } => keys.push(OrderKey {
                    sql: sql.clone(),
                    position: None,
                    descending: term.descending,
                }),
                // A compound field — a point in time, a reference of
                // several types, a composite value — orders by its columns
                // in turn, the type first, as the platform orders values.
                SelectedProjection::Field(resolved) if resolved.field().columns.len() > 1 => {
                    for column in &resolved.field().columns {
                        keys.push(OrderKey {
                            sql: context.sql_column(resolved, column),
                            position: None,
                            descending: term.descending,
                        });
                    }
                }
                SelectedProjection::Field(resolved) => {
                    let column = single_column(resolved.field(), field.last())?;
                    keys.push(OrderKey {
                        sql: context.sql_column(resolved, column),
                        position: None,
                        descending: term.descending,
                    });
                }
            }
            continue;
        }
        let resolved = context.resolve(field)?;
        // A point in time orders by its date and then by its reference,
        // the way the platform spreads the pair over the ordering; any
        // other compound field orders by its columns in turn as well.
        // A projection renders a reference pair as one column, so a
        // positional ordering names the rendered members, not the
        // physical ones.
        let columns = if positional {
            projected_members(resolved.field())
                .into_iter()
                .map(|member| match member {
                    ProjectedMember::Single(column) => column,
                    ProjectedMember::Reference { value_member, .. } => value_member,
                })
                .collect::<Vec<_>>()
        } else {
            resolved.field().columns.iter().collect::<Vec<_>>()
        };
        for column in columns {
            if positional {
                let position = selected_column_position(selected, &resolved, &column.physical_name);
                // A grouping key, or a dereference of one —
                // `Сотрудник.Наименование` over the key `Сотрудник` — orders
                // a grouped statement whether or not it is projected; the
                // dereferenced columns join the grouping.
                if position.is_none()
                    && grouped
                    && (is_group_key_path(field, &key_paths)
                        || extends_group_key(field, &key_paths))
                {
                    let sql = context.sql_column(&resolved, column);
                    if extends_group_key(field, &key_paths) {
                        derived_grouping.push(sql.clone());
                    }
                    keys.push(OrderKey {
                        sql,
                        position: None,
                        descending: term.descending,
                    });
                    continue;
                }
                if position.is_none() && unprojected_fields {
                    keys.push(OrderKey {
                        sql: context.sql_column(&resolved, column),
                        position: None,
                        descending: term.descending,
                    });
                    continue;
                }
                let position = position.ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(term.token),
                        missing_message,
                    )
                })?;
                keys.push(OrderKey {
                    sql: String::new(),
                    position: Some(position),
                    descending: term.descending,
                });
                continue;
            }
            keys.push(OrderKey {
                sql: context.sql_column(&resolved, column),
                position: None,
                descending: term.descending,
            });
        }
    }
    Ok(keys)
}

/// The `СГРУППИРОВАТЬ ПО` keys written as field paths, upper-cased.
fn group_key_paths(ast: &SelectAst<'_, '_>) -> Vec<Vec<String>> {
    ast.group
        .iter()
        .filter_map(|key| match &key.expression {
            Expression::Field(reference) => Some(
                reference
                    .segments
                    .iter()
                    .map(|token| token.lexeme.to_uppercase())
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

fn path_matches(reference: &FieldReference<'_, '_>, path: &[String]) -> bool {
    reference
        .segments
        .iter()
        .zip(path)
        .all(|(token, name)| names_equal(token.lexeme, name))
}

/// Whether the path is one of the grouping keys as written.
fn is_group_key_path(reference: &FieldReference<'_, '_>, paths: &[Vec<String>]) -> bool {
    paths
        .iter()
        .any(|path| reference.segments.len() == path.len() && path_matches(reference, path))
}

/// Whether the path dereferences a grouping key — `Сотрудник.Наименование`
/// over the key `Сотрудник` — which the platform takes as a function of
/// the key; the joined columns it reads must join the grouping.
fn extends_group_key(reference: &FieldReference<'_, '_>, paths: &[Vec<String>]) -> bool {
    paths
        .iter()
        .any(|path| reference.segments.len() > path.len() && path_matches(reference, path))
}

/// One resolved `GROUP BY` key.
enum GroupKeyTarget {
    Path(ResolvedPath),
    Alias(usize),
    Scalar(String),
}

/// Resolves the `GROUP BY` keys, checks that every non-aggregated
/// projection is a key, and renders the physical `GROUP BY` list.
fn compile_group_keys(
    ast: &SelectAst<'_, '_>,
    selected: &[SelectedProjection],
    context: &mut CompilationContext<'_, '_>,
) -> Result<Vec<String>, QueryDiagnostic> {
    if ast.group.is_empty() {
        return Ok(Vec::new());
    }
    let mut keys = Vec::with_capacity(ast.group.len());
    let mut sql = Vec::new();
    let mut push = |expression: String| {
        if !sql.contains(&expression) {
            sql.push(expression);
        }
    };
    let key_paths = group_key_paths(ast);
    for key in &ast.group {
        context.catalog.charge(1, None)?;
        let target = match &key.expression {
            Expression::Field(reference) => match context.resolve(reference) {
                Ok(resolved) => GroupKeyTarget::Path(resolved),
                Err(error) => match aliased_projection(ast, reference) {
                    Some(index) if error.kind() == QueryDiagnosticKind::UnknownField => {
                        GroupKeyTarget::Alias(index)
                    }
                    _ => return Err(error),
                },
            },
            expression => {
                if contains_aggregate(expression) {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(key.token),
                        "GROUP BY key cannot contain aggregate functions",
                    ));
                }
                let mut compiled = compile_expression(expression, context)?;
                // SQL refuses a bare constant as a grouping key, so a value
                // that is `NULL` states the type it stands for.
                if expression_kind(expression, context)? == ColumnKind::Null {
                    compiled = context.dialect.typed_null(&derived_data_type(
                        &ColumnKind::String { length: None },
                        context.dialect,
                    ));
                }
                push(compiled);
                GroupKeyTarget::Scalar(expression_fingerprint(expression))
            }
        };
        match &target {
            GroupKeyTarget::Path(resolved) => {
                for column in &resolved.field().columns {
                    push(context.sql_column(resolved, column));
                }
            }
            GroupKeyTarget::Alias(index) => match &selected[*index] {
                SelectedProjection::Field(resolved) => {
                    for column in &resolved.field().columns {
                        push(context.sql_column(resolved, column));
                    }
                }
                SelectedProjection::Generated {
                    sql: expression, ..
                } => {
                    if projection_is_aggregated(&ast.projection[*index].expression) {
                        return Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::UnsupportedFeature,
                            Some(key.token),
                            "GROUP BY key cannot be an aggregate projection",
                        ));
                    }
                    push(expression.clone());
                }
            },
            GroupKeyTarget::Scalar(_) => {}
        }
        keys.push(target);
    }

    for (index, (item, projection)) in ast.projection.iter().zip(selected).enumerate() {
        if projection_is_aggregated(&item.expression) {
            continue;
        }
        // A projection that reads no field is a constant of the row set;
        // the platform answers it in a grouped statement without listing
        // it among the grouping keys.
        if let Projection::Scalar(expression) = &item.expression
            && !references_a_field(expression)
        {
            continue;
        }
        let matched = keys
            .iter()
            .any(|key| match (key, projection, &item.expression) {
                (GroupKeyTarget::Alias(alias), _, _) => *alias == index,
                (GroupKeyTarget::Path(key), SelectedProjection::Field(resolved), _) => {
                    key.same_path(resolved)
                }
                (
                    GroupKeyTarget::Path(key),
                    SelectedProjection::Generated { .. },
                    Projection::Presentation {
                        argument: PresentationArgument::Field(reference),
                        ..
                    },
                ) => context
                    .resolve(reference)
                    .is_ok_and(|resolved| key.same_path(&resolved)),
                (GroupKeyTarget::Scalar(fingerprint), _, Projection::Scalar(expression)) => {
                    *fingerprint == expression_fingerprint(expression)
                }
                _ => false,
            });
        // An expression over grouped fields — `-Сумма`, `ЕСТЬNULL(Счет,
        // &Пустой) + 1` — is a function of the group, which the platform
        // accepts without listing the expression itself.
        let mut derived = Vec::new();
        let matched = matched
            || match (projection, &item.expression) {
                (SelectedProjection::Field(resolved), Projection::Field(reference))
                    if extends_group_key(reference, &key_paths) =>
                {
                    for column in &resolved.field().columns {
                        derived.push(context.sql_column(resolved, column));
                    }
                    true
                }
                (_, Projection::Scalar(expression)) => {
                    scalar_is_grouped(expression, &keys, &key_paths, context, &mut derived)
                }
                _ => false,
            };
        for expression in derived {
            push(expression);
        }
        if !matched {
            return Err(QueryDiagnostic::at_or_unpositioned(
                QueryDiagnosticKind::UnsupportedFeature,
                projection_token(&item.expression),
                format!(
                    "field {:?} must be grouped or aggregated",
                    projection_label(item, projection)
                ),
            ));
        }
        // An inline presentation of a key reads joined columns that must be
        // grouped as well; grouping by the rendered expression covers them.
        if let (
            SelectedProjection::Generated {
                sql: expression, ..
            },
            Projection::Presentation { .. },
        ) = (projection, &item.expression)
        {
            push(expression.clone());
        }
    }
    Ok(sql)
}

fn projection_label(item: &ProjectionItem<'_, '_>, projection: &SelectedProjection) -> String {
    item.alias.map_or_else(
        || match projection {
            SelectedProjection::Field(resolved) => resolved.field_label(),
            SelectedProjection::Generated { label, .. } => label.clone(),
        },
        |alias| alias.lexeme.to_owned(),
    )
}

/// A case- and whitespace-insensitive structural rendering used to match
/// `GROUP BY` expressions with projected expressions.
/// Whether a scalar expression is a function of the grouping keys: a key
/// itself, a field a key names, a constant, or an expression whose every
/// operand is.
fn scalar_is_grouped(
    expression: &Expression<'_, '_>,
    keys: &[GroupKeyTarget],
    key_paths: &[Vec<String>],
    context: &mut CompilationContext<'_, '_>,
    derived: &mut Vec<String>,
) -> bool {
    let fingerprint = expression_fingerprint(expression);
    if keys
        .iter()
        .any(|key| matches!(key, GroupKeyTarget::Scalar(key) if *key == fingerprint))
    {
        return true;
    }
    match expression {
        Expression::Field(reference) => {
            if keys.iter().any(|key| match key {
                GroupKeyTarget::Path(key) => context
                    .resolve(reference)
                    .is_ok_and(|resolved| key.same_path(&resolved)),
                _ => false,
            }) {
                return true;
            }
            if extends_group_key(reference, key_paths)
                && let Ok(resolved) = context.resolve(reference)
            {
                for column in &resolved.field().columns {
                    derived.push(context.sql_column(&resolved, column));
                }
                return true;
            }
            false
        }
        Expression::Aggregate { .. } => true,
        _ => {
            if !references_a_field(expression) {
                return true;
            }
            let children = expression_children(expression);
            !children.is_empty()
                && children
                    .iter()
                    .all(|child| scalar_is_grouped(child, keys, key_paths, context, derived))
        }
    }
}

fn expression_fingerprint(expression: &Expression<'_, '_>) -> String {
    let mut output = String::new();
    fingerprint_into(expression, &mut output);
    output
}

fn fingerprint_into(expression: &Expression<'_, '_>, output: &mut String) {
    let upper = |token: &Token<'_>| token.lexeme.to_uppercase();
    match expression {
        Expression::Field(reference) => {
            output.push_str("F(");
            for segment in &reference.segments {
                output.push_str(&upper(segment));
                output.push('.');
            }
            output.push(')');
        }
        Expression::Literal(token) | Expression::Parameter(token) => {
            output.push_str("L(");
            output.push_str(&upper(token));
            output.push(')');
        }
        Expression::DateTime { value, .. } => {
            output.push_str(&format!("D({value:?})"));
        }
        Expression::BeginOfPeriod { value, period, .. } => {
            output.push_str(&format!("BOP({period:?},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::EndOfPeriod { value, period, .. } => {
            output.push_str(&format!("EOP({period:?},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::DateAdd {
            value,
            period,
            count,
            ..
        } => {
            output.push_str(&format!("DADD({period:?},"));
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(count, output);
            output.push(')');
        }
        Expression::DateDiff {
            from, to, period, ..
        } => {
            output.push_str(&format!("DDIFF({period:?},"));
            fingerprint_into(from, output);
            output.push(',');
            fingerprint_into(to, output);
            output.push(')');
        }
        Expression::DatePart { part, value, .. } => {
            output.push_str(&format!("PART({part:?},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::Refs {
            value,
            kind,
            object,
            ..
        } => {
            output.push_str(&format!("REFS({}.{},", upper(kind), upper(object)));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::MetadataValue {
            kind,
            object,
            value,
            ..
        } => {
            output.push_str(&format!(
                "V({}.{}.{})",
                upper(kind),
                upper(object),
                upper(value)
            ));
        }
        Expression::Tuple { items, .. } => {
            output.push_str("T(");
            for item in items {
                fingerprint_into(item, output);
                output.push(',');
            }
            output.push(')');
        }
        Expression::SystemValue {
            enumeration, value, ..
        } => {
            output.push_str(&format!("SV({}.{})", upper(enumeration), upper(value)));
        }
        Expression::Uuid { argument, .. } => {
            output.push_str("UUID(");
            fingerprint_into(&Expression::Field(argument.clone()), output);
            output.push(')');
        }
        Expression::Between {
            value,
            low,
            high,
            negated,
            ..
        } => {
            output.push_str(if *negated { "NBTW(" } else { "BTW(" });
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(low, output);
            output.push(',');
            fingerprint_into(high, output);
            output.push(')');
        }
        Expression::ScalarFunction {
            function,
            arguments,
            ..
        } => {
            output.push_str(&format!("F({},", function.name()));
            for argument in arguments {
                fingerprint_into(argument, output);
                output.push(',');
            }
            output.push(')');
        }
        Expression::TypeLiteral { name, .. } => match name {
            TypeName::Primitive(primitive) => {
                output.push_str(&format!("TYPE({primitive:?})"));
            }
            TypeName::Object { kind, object } => {
                output.push_str(&format!("TYPE({}.{})", upper(kind), upper(object)));
            }
        },
        Expression::ValueType { argument, .. } => {
            output.push_str("VTYPE(");
            fingerprint_into(argument, output);
            output.push(')');
        }
        Expression::Cast {
            argument,
            target,
            path,
            ..
        } => {
            output.push_str("CAST(");
            fingerprint_into(argument, output);
            output.push_str(&match target {
                CastTarget::String { length } => format!(",S{length:?}"),
                CastTarget::Number { precision, scale } => format!(",N{precision:?}{scale:?}"),
                CastTarget::Boolean => ",B".to_owned(),
                CastTarget::Date => ",D".to_owned(),
                CastTarget::Reference { kind, object } => {
                    format!(",R{}.{}", upper(kind), upper(object))
                }
            });
            if let Some(path) = path {
                output.push('.');
                output.push_str(&upper(path));
            }
            output.push(')');
        }
        Expression::Unary { operator, value } => {
            output.push_str(&format!("U({},", upper(operator)));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            output.push_str(&format!("B({},", upper(operator)));
            fingerprint_into(left, output);
            output.push(',');
            fingerprint_into(right, output);
            output.push(')');
        }
        Expression::InList {
            value,
            items,
            negated,
            hierarchy,
            ..
        } => {
            output.push_str(&format!("IN({negated},{hierarchy},"));
            fingerprint_into(value, output);
            for item in items {
                output.push(',');
                fingerprint_into(item, output);
            }
            output.push(')');
        }
        Expression::InQuery {
            token,
            value,
            negated,
            hierarchy,
            ..
        } => {
            output.push_str(&format!("INQ({negated},{hierarchy},{},", token.span.start));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::IsNull { value, negated } => {
            output.push_str(&format!("ISNULL({negated},"));
            fingerprint_into(value, output);
            output.push(')');
        }
        Expression::Case {
            branches,
            otherwise,
            ..
        } => {
            output.push_str("CASE(");
            for branch in branches {
                fingerprint_into(&branch.when, output);
                output.push(':');
                fingerprint_into(&branch.then, output);
                output.push(',');
            }
            if let Some(otherwise) = otherwise {
                output.push_str("ELSE:");
                fingerprint_into(otherwise, output);
            }
            output.push(')');
        }
        Expression::IsNullFunction {
            value, fallback, ..
        } => {
            output.push_str("COALESCE(");
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(fallback, output);
            output.push(')');
        }
        Expression::Like {
            value,
            pattern,
            escape,
            negated,
            ..
        } => {
            output.push_str(&format!("LIKE({negated},"));
            fingerprint_into(value, output);
            output.push(',');
            fingerprint_into(pattern, output);
            if let Some(escape) = escape {
                output.push(',');
                fingerprint_into(escape, output);
            }
            output.push(')');
        }
        Expression::Aggregate {
            kind,
            distinct,
            argument,
            ..
        } => {
            output.push_str(&format!("AGG({kind:?},{distinct},"));
            match argument {
                AggregateArgument::All => output.push('*'),
                AggregateArgument::Expression(expression) => fingerprint_into(expression, output),
            }
            output.push(')');
        }
    }
}

fn compile_branch_sql(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    conditions: &[JoinCondition],
    filter: Option<&str>,
) -> String {
    if joins.is_empty() {
        let dialect = context.dialect;
        let mut sql = dialect.select_prefix(ast.distinct, ast.top);
        sql.push_str(&projections.join(", "));
        sql.push_str(" FROM ");
        sql.push_str(&context.sources[0].relation);
        sql.push_str(" AS ");
        sql.push_str(&dialect.quote_identifier(context.base_alias()));
        for reference_join in &context.sources[0].reference_joins {
            append_reference_join(&mut sql, reference_join, dialect);
        }
        append_where(
            &mut sql,
            context.sources[0].separator_predicates.iter().cloned(),
            filter,
        );
        return sql;
    }
    compile_native_join(ast, joins, context, projections, conditions, filter)
}

fn resolve_join_source(
    source: &SourceAst<'_, '_>,
    parameter_columns: &ParameterColumns,
    snapshot: &MetadataSnapshot,
    catalog: &CompilationCatalog<'_>,
    default_alias: &str,
    dialect: SqlDialect,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<SourceScope, QueryDiagnostic> {
    if let Some(nested) = &source.nested {
        return derived_source_scope(source, nested, snapshot, catalog, dialect, presentations);
    }
    if source.temporary {
        return temporary_source_scope(source, snapshot, catalog, dialect);
    }
    if source.parameter {
        return parameter_source_scope(source, parameter_columns, snapshot, catalog, dialect);
    }
    if source.constants {
        return constants_source_scope(source, snapshot, catalog, default_alias, dialect);
    }
    if let Some(value) = &source.criterion {
        return criterion_source_scope(source, value, snapshot, catalog, default_alias, dialect);
    }
    let resolved = resolve_source_metadata(source, snapshot, catalog)?;
    let target = resolved.restriction_target();
    let restriction =
        catalog
            .restriction_for(target.clone())
            .map(|restriction| SourceRestriction {
                restriction,
                label: restriction_label(snapshot, &target),
                identity_is_base: resolved.identity_is_base,
            });
    let sql_alias = source
        .alias
        .map_or_else(|| default_alias.to_owned(), |token| token.lexeme.to_owned());
    let compiled_source = compile_source_relation(
        source,
        snapshot,
        catalog,
        resolved.object,
        resolved.live_table,
        &resolved.fields,
        &sql_alias,
        restriction.as_ref(),
        dialect,
    )?;
    Ok(SourceScope {
        object: ObjectId::from(&resolved.object.guid),
        fields: compiled_source.fields,
        relation: compiled_source.sql,
        aggregate: compiled_source.aggregate,
        sql_alias,
        object_name: resolved.qualifier_name,
        source_alias: source.alias.map(|token| token.lexeme.to_owned()),
        identity_is_base: resolved.identity_is_base,
        reference_joins: Vec::new(),
        separator_predicates: compiled_source.separators,
        constants: None,
        used_fields: RefCell::new(BTreeSet::new()),
        current_table: false,
    })
}

/// Compiles the `ON` condition of the join that introduces scope `joined`.
/// The condition must contain a top-level direct-field equality between the
/// joined source and an earlier one and may not reference later sources.
fn compile_join_condition(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    kind: JoinKind,
    joined: ScopeId,
) -> Result<JoinCondition, QueryDiagnostic> {
    let mut parts = Vec::new();
    let mut left_marker = None;
    context.compiling_join_condition = true;
    let compiled =
        compile_join_condition_parts(expression, context, &mut parts, &mut left_marker, joined);
    context.compiling_join_condition = false;
    compiled?;
    // An inner, left or right join takes any condition — `ПО (ИСТИНА)`,
    // a comparison with a parameter, an inequality of periods — the way
    // the platform and the servers do; the anchor equality is what makes
    // a FULL JOIN plannable, so it stays required there.
    if kind == JoinKind::Full && left_marker.is_none() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "FULL JOIN condition requires at least one top-level direct-field equality between the joined source and an earlier source combined by AND",
        ));
    }
    Ok(JoinCondition {
        sql: parts.join(" AND "),
    })
}

fn compile_join_condition_parts(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    parts: &mut Vec<String>,
    left_marker: &mut Option<String>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let Expression::Binary {
            left,
            operator,
            right,
        } = expression
            && operator.kind == TokenKind::Keyword(Keyword::And)
        {
            pending.push(right);
            pending.push(left);
            continue;
        }

        if let Some((equality, marker)) =
            compile_cross_source_join_equality(expression, context, joined)?
        {
            if left_marker.is_none() {
                *left_marker = Some(marker);
            }
            parts.push(equality);
            continue;
        }

        validate_direct_join_condition_fields(expression, context, joined)?;
        parts.push(compile_predicate(expression, context)?);
    }
    Ok(())
}

fn compile_cross_source_join_equality(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    joined: ScopeId,
) -> Result<Option<(String, String)>, QueryDiagnostic> {
    let Expression::Binary {
        left,
        operator,
        right,
    } = expression
    else {
        return Ok(None);
    };
    if operator.lexeme != "=" {
        return Ok(None);
    }
    let (Expression::Field(left_reference), Expression::Field(right_reference)) =
        (left.as_ref(), right.as_ref())
    else {
        return Ok(None);
    };
    let left_field = context.resolve(left_reference)?;
    let right_field = context.resolve(right_reference)?;
    check_join_scope(context, left_field.scope, left_reference.last(), joined)?;
    check_join_scope(context, right_field.scope, right_reference.last(), joined)?;
    // Only an equality that binds the joined source to an earlier one is the
    // anchor; equalities between earlier sources are ordinary predicates.
    if left_field.scope == right_field.scope
        || (left_field.scope != joined && right_field.scope != joined)
    {
        return Ok(None);
    }
    let equality = compile_join_field_equality(
        context,
        &left_field,
        left_reference.last(),
        &right_field,
        right_reference.last(),
    )?;
    let marker = if left_field.scope == ScopeId(0) {
        equality.left_marker
    } else {
        equality.right_marker
    };
    Ok(Some((equality.sql, marker)))
}

/// A join condition may reference the joined source and earlier ones only.
fn check_join_scope(
    context: &CompilationContext<'_, '_>,
    scope: ScopeId,
    token: &Token<'_>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    if scope.0 > joined.0 {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            format!(
                "JOIN condition cannot reference {:?}, which is joined later",
                token.lexeme
            ),
        ));
    }
    if context.source_elements.get(scope.0) != context.source_elements.get(joined.0) {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownField,
            Some(token),
            format!(
                "field {:?} is not visible from this join; sources listed through commas are joined independently",
                token.lexeme
            ),
        ));
    }
    Ok(())
}

fn validate_direct_join_condition_fields(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_, '_>,
    joined: ScopeId,
) -> Result<(), QueryDiagnostic> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Expression::Field(reference) => {
                let resolved = context.resolve(reference)?;
                check_join_scope(context, resolved.scope, reference.last(), joined)?;
            }
            Expression::Uuid { argument, .. } => {
                let resolved = context.resolve(argument)?;
                check_join_scope(context, resolved.scope, argument.last(), joined)?;
            }
            // A dereference through a cast joins its target the way a
            // dereferenced field does.
            Expression::Cast { argument, .. } => pending.push(argument),
            Expression::Between {
                value, low, high, ..
            } => {
                pending.push(value);
                pending.push(low);
                pending.push(high);
            }
            Expression::TypeLiteral { .. } => {}
            Expression::ScalarFunction { arguments, .. } => pending.extend(arguments),
            Expression::BeginOfPeriod { value, .. }
            | Expression::EndOfPeriod { value, .. }
            | Expression::DatePart { value, .. }
            | Expression::Refs { value, .. }
            | Expression::ValueType {
                argument: value, ..
            }
            | Expression::Unary { value, .. }
            | Expression::IsNull { value, .. } => pending.push(value),
            Expression::DateAdd { value, count, .. } => {
                pending.push(count);
                pending.push(value);
            }
            Expression::DateDiff { from, to, .. } => {
                pending.push(to);
                pending.push(from);
            }
            Expression::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            Expression::InList { value, items, .. } => {
                pending.extend(items.iter().rev());
                pending.push(value);
            }
            Expression::InQuery { value, .. } => pending.push(value),
            Expression::Case {
                branches,
                otherwise,
                ..
            } => {
                if let Some(otherwise) = otherwise {
                    pending.push(otherwise);
                }
                for branch in branches.iter().rev() {
                    pending.push(&branch.then);
                    pending.push(&branch.when);
                }
            }
            Expression::IsNullFunction {
                value, fallback, ..
            } => {
                pending.push(fallback);
                pending.push(value);
            }
            Expression::Like {
                value,
                pattern,
                escape,
                ..
            } => {
                if let Some(escape) = escape {
                    pending.push(escape);
                }
                pending.push(pattern);
                pending.push(value);
            }
            Expression::Aggregate { token, .. } => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "JOIN condition cannot contain aggregate functions",
                ));
            }
            Expression::Literal(_)
            | Expression::Parameter(_)
            | Expression::DateTime { .. }
            | Expression::MetadataValue { .. }
            | Expression::SystemValue { .. } => {}
            Expression::Tuple { items, .. } => pending.extend(items),
        }
    }
    Ok(())
}

struct JoinedFieldEquality {
    sql: String,
    left_marker: String,
    right_marker: String,
}

/// One side of a join equality, classified by its physical shape.
enum JoinOperand<'field> {
    /// A single physical column; `payload` marks a 20-byte
    /// `RTRef ‖ RRRef` value and `fixed` a 16-byte reference.
    Single {
        column: &'field QueryableColumn,
        payload: bool,
        fixed: bool,
    },
    /// A composite reference storing its type and value separately.
    Compound {
        type_column: &'field QueryableColumn,
        value_column: &'field QueryableColumn,
    },
    Unsupported,
}

fn classify_join_operand(field: &QueryableField) -> JoinOperand<'_> {
    if let [column] = field.columns.as_slice() {
        let (payload, fixed) = match &column.kind {
            ColumnKind::Reference { runtime_typed, .. } => (*runtime_typed, !*runtime_typed),
            _ => (false, false),
        };
        return JoinOperand::Single {
            column,
            payload,
            fixed,
        };
    }
    let type_column = field
        .columns
        .iter()
        .find(|column| column.is_reference_type_member());
    let value_column = field
        .columns
        .iter()
        .find(|column| column.is_reference_value_member());
    match (type_column, value_column) {
        (Some(type_column), Some(value_column)) => JoinOperand::Compound {
            type_column,
            value_column,
        },
        _ => JoinOperand::Unsupported,
    }
}

fn compile_join_field_equality(
    context: &CompilationContext<'_, '_>,
    left: &ResolvedPath,
    left_token: &Token<'_>,
    right: &ResolvedPath,
    right_token: &Token<'_>,
) -> Result<JoinedFieldEquality, QueryDiagnostic> {
    let equality = |sql: String, left_marker: String, right_marker: String| JoinedFieldEquality {
        sql,
        left_marker,
        right_marker,
    };
    match (
        classify_join_operand(left.field()),
        classify_join_operand(right.field()),
    ) {
        // Equal widths compare directly: two scalars, two fixed
        // references, or two runtime-typed payloads.
        (
            JoinOperand::Single {
                column: left_column,
                payload: left_payload,
                ..
            },
            JoinOperand::Single {
                column: right_column,
                payload: right_payload,
                ..
            },
        ) if left_payload == right_payload => {
            let mut left_sql = context.sql_column(left, left_column);
            let mut right_sql = context.sql_column(right, right_column);
            // A stored string of the provider's own type has no common type
            // with the text a derived source projects, so the derived side
            // takes the stored type; the stored column keeps its own, or an
            // index over it could not be used.
            let dialect = context.dialect;
            if dialect.is_provider_string_type(&left_column.data_type)
                && !dialect.is_provider_string_type(&right_column.data_type)
            {
                right_sql = format!("CAST({right_sql} AS {})", left_column.data_type.trim());
            } else if dialect.is_provider_string_type(&right_column.data_type)
                && !dialect.is_provider_string_type(&left_column.data_type)
            {
                left_sql = format!("CAST({left_sql} AS {})", right_column.data_type.trim());
            }
            Ok(equality(
                format!("{left_sql} = {right_sql}"),
                left_sql,
                right_sql,
            ))
        }
        // A fixed reference against a payload column is widened to its own
        // payload, so both sides compare as 20-byte values.
        (
            JoinOperand::Single {
                column: fixed_column,
                fixed: true,
                ..
            },
            JoinOperand::Single {
                column: payload_column,
                payload: true,
                ..
            },
        ) => {
            let widened = widened_fixed_reference(context, left, fixed_column, left_token)?;
            let payload_sql = context.sql_column(right, payload_column);
            Ok(equality(
                format!("{widened} = {payload_sql}"),
                widened,
                payload_sql,
            ))
        }
        (
            JoinOperand::Single {
                column: payload_column,
                payload: true,
                ..
            },
            JoinOperand::Single {
                column: fixed_column,
                fixed: true,
                ..
            },
        ) => {
            let widened = widened_fixed_reference(context, right, fixed_column, right_token)?;
            let payload_sql = context.sql_column(left, payload_column);
            Ok(equality(
                format!("{payload_sql} = {widened}"),
                payload_sql,
                widened,
            ))
        }
        // A composite field against a payload column is concatenated.
        (
            JoinOperand::Compound {
                type_column,
                value_column,
            },
            JoinOperand::Single {
                column: payload_column,
                payload: true,
                ..
            },
        ) => {
            let widened = context.dialect.reference_payload(
                &context.sql_column(left, type_column),
                &context.sql_column(left, value_column),
            );
            let payload_sql = context.sql_column(right, payload_column);
            Ok(equality(
                format!("{widened} = {payload_sql}"),
                widened,
                payload_sql,
            ))
        }
        (
            JoinOperand::Single {
                column: payload_column,
                payload: true,
                ..
            },
            JoinOperand::Compound {
                type_column,
                value_column,
            },
        ) => {
            let widened = context.dialect.reference_payload(
                &context.sql_column(right, type_column),
                &context.sql_column(right, value_column),
            );
            let payload_sql = context.sql_column(left, payload_column);
            Ok(equality(
                format!("{payload_sql} = {widened}"),
                payload_sql,
                widened,
            ))
        }
        // Two composite fields compare member by member, which keeps the
        // physical columns available to the optimizer.
        (
            JoinOperand::Compound {
                type_column: left_type,
                value_column: left_value,
            },
            JoinOperand::Compound {
                type_column: right_type,
                value_column: right_value,
            },
        ) => {
            let left_value_sql = context.sql_column(left, left_value);
            let right_value_sql = context.sql_column(right, right_value);
            let sql = format!(
                "({left_value_sql} = {right_value_sql} AND {} = {})",
                context.sql_column(left, left_type),
                context.sql_column(right, right_type),
            );
            Ok(equality(sql, left_value_sql, right_value_sql))
        }
        (
            JoinOperand::Compound { .. },
            JoinOperand::Single {
                fixed: true,
                payload: false,
                ..
            },
        ) => compile_compound_fixed_reference_equality(
            context,
            left,
            left_token,
            right,
            right_token,
            false,
        ),
        (
            JoinOperand::Single {
                fixed: true,
                payload: false,
                ..
            },
            JoinOperand::Compound { .. },
        ) => compile_compound_fixed_reference_equality(
            context,
            right,
            right_token,
            left,
            left_token,
            true,
        ),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(left_token),
            format!(
                "JOIN equality does not support these compound field shapes: {} against {}",
                field_shape(left.field()),
                field_shape(right.field()),
            ),
        )),
    }
}

/// The physical members of a field, for a diagnostic.
fn field_shape(field: &QueryableField) -> String {
    let members = field
        .columns
        .iter()
        .map(|column| column.physical_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{:?} [{members}]", field.name)
}

/// A fixed reference rendered as the `RTRef ‖ RRRef` payload of its target.
fn widened_fixed_reference(
    context: &CompilationContext<'_, '_>,
    resolved: &ResolvedPath,
    column: &QueryableColumn,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    let database_type = fixed_reference_database_type(context.snapshot, resolved.field(), token)?;
    Ok(context.dialect.reference_payload(
        &context.dialect.binary_u32(database_type),
        &context.sql_column(resolved, column),
    ))
}

/// A composite reference compared with a fixed one: the identifiers must
/// match and the composite type must be the fixed target's type number.
fn compile_compound_fixed_reference_equality(
    context: &CompilationContext<'_, '_>,
    compound: &ResolvedPath,
    compound_token: &Token<'_>,
    fixed: &ResolvedPath,
    fixed_token: &Token<'_>,
    fixed_is_left: bool,
) -> Result<JoinedFieldEquality, QueryDiagnostic> {
    let compound_reference = reference_column(compound.field(), compound_token)?;
    let compound_type = reference_type_column(compound.field(), compound_token)?;
    let fixed_reference = reference_column(fixed.field(), fixed_token)?;
    let database_type =
        fixed_reference_database_type(context.snapshot, fixed.field(), fixed_token)?;
    let compound_reference_sql = context.sql_column(compound, compound_reference);
    let compound_type_sql = context.sql_column(compound, compound_type);
    let fixed_reference_sql = context.sql_column(fixed, fixed_reference);
    let sql = format!(
        "({compound_reference_sql} = {fixed_reference_sql} AND {compound_type_sql} = {})",
        context.dialect.binary_u32(database_type)
    );
    let (left_marker, right_marker) = if fixed_is_left {
        (fixed_reference_sql, compound_reference_sql)
    } else {
        (compound_reference_sql, fixed_reference_sql)
    };
    Ok(JoinedFieldEquality {
        sql,
        left_marker,
        right_marker,
    })
}

fn fixed_reference_database_type(
    snapshot: &MetadataSnapshot,
    field: &QueryableField,
    token: &Token<'_>,
) -> Result<u32, QueryDiagnostic> {
    let target = field.reference_target.as_deref().ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!(
                "fixed reference field {:?} has no unique SchemaStorage target",
                field.name
            ),
        )
    })?;
    // SchemaStorage names a table without the leading underscore of its
    // physical name (`Reference347` for `_Reference347`).
    let target = target.trim_start_matches('_');
    let matches = snapshot
        .schema()
        .tables
        .iter()
        .filter(|table| names_equal(table.name.trim_start_matches('_'), target))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [table] => Ok(table.number),
        [] => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::Metadata,
            Some(token),
            format!("reference target {target:?} has no SchemaStorage database type"),
        )),
        _ => Err(QueryDiagnostic::at(
            QueryDiagnosticKind::AmbiguousObject,
            Some(token),
            format!("reference target {target:?} has an ambiguous database type"),
        )),
    }
}

fn compile_native_join(
    ast: &SelectAst<'_, '_>,
    joins: &[JoinAst<'_, '_>],
    context: &CompilationContext<'_, '_>,
    projections: &[String],
    conditions: &[JoinCondition],
    filter: Option<&str>,
) -> String {
    // A condition that dereferences a reference can only see joins written
    // before it, so every source carries its own dereference joins in a
    // parenthesized group. Without such a condition the flat list is kept.
    let grouped = context.dereference_in_join;
    let placement = place_separator_predicates(joins, &context.sources);
    let mut sql = context.dialect.select_prefix(ast.distinct, ast.top);
    sql.push_str(&projections.join(", "));
    sql.push_str(" FROM ");
    sql.push_str(&render_join_source(context, &context.sources[0], grouped));
    // A comma-listed element joins as a whole: the platform answers
    // `A, B ПРАВОЕ СОЕДИНЕНИЕ C` as `A × (B ⟕ C)`, so an element that
    // carries joins of its own is parenthesized. Rendering it flat would
    // make an unmatched row of `C` appear once instead of once per row of
    // `A`, which is a different answer, not a different plan.
    let mut element_open = false;
    for (index, ((join, condition), source)) in joins
        .iter()
        .zip(conditions)
        .zip(&context.sources[1..])
        .enumerate()
    {
        let operator = match join.kind {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Cross => "CROSS JOIN",
            JoinKind::Full => "FULL JOIN",
        };
        if join.kind == JoinKind::Cross && element_open {
            sql.push(')');
            element_open = false;
        }
        sql.push(' ');
        sql.push_str(operator);
        sql.push(' ');
        if join.kind == JoinKind::Cross
            && joins
                .get(index + 1)
                .is_some_and(|next| next.kind != JoinKind::Cross)
        {
            sql.push('(');
            element_open = true;
        }
        sql.push_str(&render_join_source(context, source, grouped));
        if join.kind == JoinKind::Cross {
            debug_assert!(placement.on[index].is_empty());
            continue;
        }
        sql.push_str(" ON ");
        sql.push_str(&condition.sql);
        for predicate in &placement.on[index] {
            sql.push_str(" AND ");
            sql.push_str(predicate);
        }
    }
    if element_open {
        sql.push(')');
    }
    if !grouped {
        append_joined_reference_joins(&mut sql, context);
    }
    append_where(&mut sql, placement.filter.into_iter(), filter);
    sql
}

/// Where the separator predicates of a join chain go: `on[i]` extends the
/// condition of join `i`, `filter` extends the statement `WHERE`.
struct SeparatorPlacement {
    on: Vec<Vec<String>>,
    filter: Vec<String>,
}

/// A source keeps its own rows only until a later `RIGHT JOIN` null-extends
/// it. A source introduced by `INNER`/`LEFT` is filtered in its own `ON`;
/// the base source and a `RIGHT`-joined source are filtered in the `ON` of
/// the next `RIGHT JOIN` that null-extends them, or in `WHERE` when none
/// follows, so rows of other areas never survive as unmatched rows.
fn place_separator_predicates(
    joins: &[JoinAst<'_, '_>],
    sources: &[SourceScope],
) -> SeparatorPlacement {
    let mut on = vec![Vec::new(); joins.len()];
    let mut filter = Vec::new();
    for (position, source) in sources.iter().enumerate() {
        if source.separator_predicates.is_empty() {
            continue;
        }
        let own_join = position.checked_sub(1);
        let target = match own_join.map(|index| joins[index].kind) {
            // A source a FULL JOIN introduces is null-extended like a
            // LEFT-joined one, so its own ON carries the filter.
            Some(JoinKind::Inner | JoinKind::Left | JoinKind::Full) => own_join,
            // A preserved source keeps its rows only until a later join
            // null-extends it, which RIGHT and FULL both do.
            // The search stops at the next comma: a later element is
            // joined as a whole and null-extends nothing of this one.
            _ => joins
                .iter()
                .enumerate()
                .skip(position)
                .take_while(|(_, join)| join.kind != JoinKind::Cross)
                .find(|(_, join)| matches!(join.kind, JoinKind::Right | JoinKind::Full))
                .map(|(index, _)| index),
        };
        match target {
            Some(index) => on[index].extend(source.separator_predicates.iter().cloned()),
            None => filter.extend(source.separator_predicates.iter().cloned()),
        }
    }
    SeparatorPlacement { on, filter }
}

/// Appends `WHERE` with the separator predicates followed by the query's
/// own filter, when any of them is present.
fn append_where(sql: &mut String, separators: impl Iterator<Item = String>, filter: Option<&str>) {
    let mut predicates = separators.collect::<Vec<_>>();
    if let Some(filter) = filter {
        predicates.push(filter.to_owned());
    }
    if !predicates.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&predicates.join(" AND "));
    }
}

/// One source of a native join: bare, or grouped with its dereference
/// joins so that later `ON` clauses can address them.
fn render_join_source(
    context: &CompilationContext<'_, '_>,
    source: &SourceScope,
    grouped: bool,
) -> String {
    let mut sql = format!(
        "{} AS {}",
        source.relation,
        context.dialect.quote_identifier(&source.sql_alias)
    );
    if !grouped || source.reference_joins.is_empty() {
        return sql;
    }
    for join in &source.reference_joins {
        append_reference_join(&mut sql, join, context.dialect);
    }
    format!("({sql})")
}

fn append_joined_reference_joins(sql: &mut String, context: &CompilationContext<'_, '_>) {
    for source in &context.sources {
        for join in &source.reference_joins {
            append_reference_join(sql, join, context.dialect);
        }
    }
}

pub(super) fn append_reference_join(sql: &mut String, join: &JoinPlan, dialect: SqlDialect) {
    sql.push_str(" LEFT JOIN ");
    sql.push_str(&join.target_relation);
    sql.push_str(" AS ");
    sql.push_str(&dialect.quote_identifier(&join.alias));
    sql.push_str(" ON ");
    sql.push_str(&join.source_value_sql.clone().unwrap_or_else(|| {
        dialect.qualified_column(Some(&join.source_alias), &join.source_column)
    }));
    sql.push_str(" = ");
    sql.push_str(&dialect.qualified_column(Some(&join.alias), &join.target_id_column));
    append_type_guard(sql, &join.source_alias, join, dialect);
    for predicate in &join.target_predicates {
        sql.push_str(" AND ");
        sql.push_str(predicate);
    }
}

fn append_type_guard(sql: &mut String, source_alias: &str, join: &JoinPlan, dialect: SqlDialect) {
    let Some(number) = join.database_type else {
        return;
    };
    let Some(type_sql) = join.source_type_sql.clone().or_else(|| {
        join.source_type_column
            .as_ref()
            .map(|column| dialect.qualified_column(Some(source_alias), column))
    }) else {
        return;
    };
    sql.push_str(" AND ");
    sql.push_str(&type_sql);
    sql.push_str(" = ");
    sql.push_str(&dialect.binary_u32(number));
}

fn selected_column_position(
    selected: &[SelectedProjection],
    ordered: &ResolvedPath,
    ordered_column: &str,
) -> Option<usize> {
    let mut position = 1;
    for selected in selected {
        match selected {
            SelectedProjection::Field(resolved) => {
                for member in projected_members(resolved.field()) {
                    let column = match member {
                        ProjectedMember::Single(column) => column,
                        ProjectedMember::Reference { value_member, .. } => value_member,
                    };
                    if resolved.scope == ordered.scope
                        && names_equal(&resolved.sql_alias, &ordered.sql_alias)
                        && names_equal(&column.physical_name, ordered_column)
                    {
                        return Some(position);
                    }
                    position += 1;
                }
            }
            SelectedProjection::Generated { .. } => position += 1,
        }
    }
    None
}
