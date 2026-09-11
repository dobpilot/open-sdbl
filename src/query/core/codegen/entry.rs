use std::collections::BTreeSet;

use super::batch::compile_batch_ast;
use super::expression::single_column_at;
use super::orchestrate::PresentationCompilation;
use super::sources::compile_live_relation;
use super::virtual_tables::compile_presentation_plan;
use crate::metadata::MetadataSnapshot;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::params::{CompileOptions, Parameters, QueryParameter, parameter_name};
use crate::query::core::parser::Parser;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery, PresentationPlan,
    PresentationRequest, PresentationTarget, restriction_label,
};
use crate::query::core::restrict::{AccessRestriction, RestrictionRequest, RestrictionTarget};
use crate::query::core::temp_tables::TempTablesManager;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{TokenKind, tokenize};

/// The application callback requests collected by preparation.
pub(crate) struct PreparedRequests {
    pub(crate) presentations: PresentationRequest,
    pub(crate) restrictions: RestrictionRequest,
}

pub(crate) fn prepare_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<PreparedRequests, QueryDiagnostic> {
    prepare_query_with(source, snapshot, dialect, &TempTablesManager::new())
}

/// Collects presentation and restriction targets with the caller's
/// temporary tables visible. Definitions of the batch are applied to a
/// private copy of the manager so that preparation stays free of side
/// effects.
pub(crate) fn prepare_query_with(
    source: &str,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
    manager: &TempTablesManager,
) -> Result<PreparedRequests, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations = PresentationCompilation::collect(dialect);
    let mut scratch = manager.clone();
    let _ = compile_batch_ast(&ast, snapshot, &mut presentations, &mut scratch)?;
    Ok(PreparedRequests {
        presentations: PresentationRequest {
            targets: presentations
                .requested
                .into_iter()
                .map(|object| PresentationTarget { object })
                .collect(),
        },
        restrictions: RestrictionRequest {
            targets: presentations.restriction_targets.into_iter().collect(),
        },
    })
}

pub(crate) fn compile_presentation_lookup(
    snapshot: &MetadataSnapshot,
    plan: &PresentationPlan,
    references: &[[u8; 16]],
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let catalog = CompilationCatalog::new(snapshot, Parameters::unbound());
    let references = references.iter().copied().collect::<BTreeSet<_>>();
    if references.is_empty() {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::PresentationBatch,
            "presentation lookup requires at least one reference",
        ));
    }
    if references.len() > 1_024 {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::PresentationBatch,
            "presentation lookup exceeds 1,024 unique references",
        ));
    }

    let object = snapshot.object_by_id(plan.object).ok_or_else(|| {
        QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::PresentationPlan,
            "presentation target was not resolved",
        )
    })?;
    let target_table = object
        .physical_table
        .as_deref()
        .and_then(|physical| snapshot.live_table(physical))
        .ok_or_else(|| {
            QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::NotLive,
                "presentation target table is not live",
            )
        })?;
    let fields = catalog.fields(object, None)?;
    let id = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "ID"))
        .ok_or_else(|| {
            QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::PresentationPlan,
                "presentation target has no ID",
            )
        })?;
    let id_column = single_column_at(id, None)?;
    let alias = "__presentation_target";
    let qualified_id = dialect.qualified_column(Some(alias), &id_column.physical_name);
    let expression =
        compile_presentation_plan(snapshot, &catalog, plan.object, alias, plan, None, dialect)?;
    let relation = compile_live_relation(snapshot, target_table, &fields, dialect);
    let values = references
        .iter()
        .map(|reference| dialect.binary_literal(reference))
        .collect::<Vec<_>>()
        .join(", ");
    let reference_label = dialect.quote_identifier("__reference");
    let presentation_label = dialect.quote_identifier("__presentation");
    let sql = format!(
        "SELECT {qualified_id} AS {reference_label}, {expression} AS {presentation_label} FROM {relation} AS {} WHERE {qualified_id} IN ({values})",
        dialect.quote_identifier(alias),
    );
    Ok(CompiledQuery {
        sql,
        columns: vec![
            CompiledColumn::new(
                "__reference".to_owned(),
                ColumnKind::Reference {
                    targets: vec![plan.object],
                    runtime_typed: false,
                },
            ),
            CompiledColumn::new(
                "__presentation".to_owned(),
                ColumnKind::String { length: None },
            ),
        ],
        deferred_presentations: Vec::new(),
    })
}

pub(crate) fn compile_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    options: &CompileOptions<'_>,
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let mut manager = TempTablesManager::new();
    compile_batch(source, snapshot, options, dialect, &mut manager)?.ok_or_else(|| {
        QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::TemporaryTable,
            "the batch returns no rows; compile it with a temporary table manager",
        )
    })
}

/// Compiles a batch, updating `manager` with its definitions and drops.
pub(crate) fn compile_batch(
    source: &str,
    snapshot: &MetadataSnapshot,
    options: &CompileOptions<'_>,
    dialect: SqlDialect,
    manager: &mut TempTablesManager,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let parameters = options.bound_parameters();
    check_parameter_binding(&tokens, options.parameter_values(), parameters)?;
    let restrictions = options.access_restrictions();
    check_restriction_uniqueness(snapshot, restrictions)?;
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations =
        PresentationCompilation::strict(options.presentation_plans(), parameters, dialect)
            .with_restrictions(restrictions);
    let compiled = compile_batch_ast(&ast, snapshot, &mut presentations, manager)?;
    if let Some(unused) = (0..restrictions.len())
        .find(|index| !presentations.used_restrictions.contains(index))
        .map(|index| &restrictions[index])
    {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Restriction,
            format!(
                "restriction of {} is supplied but no ALLOWED statement reads it",
                restriction_label(snapshot, &restriction_target(unused))
            ),
        ));
    }
    Ok(compiled)
}

fn restriction_target(restriction: &AccessRestriction) -> RestrictionTarget {
    RestrictionTarget {
        object: restriction.object(),
        table_part: restriction.table_part_name().map(str::to_owned),
    }
}

/// Two restrictions of one target would have to be combined by a rule the
/// application did not state, so they are rejected.
fn check_restriction_uniqueness(
    snapshot: &MetadataSnapshot,
    restrictions: &[AccessRestriction],
) -> Result<(), QueryDiagnostic> {
    for (index, restriction) in restrictions.iter().enumerate() {
        let target = restriction_target(restriction);
        if restrictions[..index]
            .iter()
            .any(|previous| target.matches(previous))
        {
            return Err(QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::Restriction,
                format!(
                    "restriction of {} is supplied more than once",
                    restriction_label(snapshot, &target)
                ),
            ));
        }
    }
    Ok(())
}

/// Every `&Имя` token needs exactly one value and every query value must be
/// referenced; names compare case-insensitively. Session values are
/// consulted for the first check only.
fn check_parameter_binding(
    tokens: &[crate::Token<'_>],
    parameters: &[QueryParameter],
    bound: Parameters<'_>,
) -> Result<(), QueryDiagnostic> {
    for (index, parameter) in parameters.iter().enumerate() {
        if parameters[..index]
            .iter()
            .any(|previous| names_equal(previous.name(), parameter.name()))
        {
            return Err(QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::Parameter,
                format!(
                    "parameter {:?} is supplied more than once",
                    parameter.name()
                ),
            ));
        }
    }
    let mut referenced = vec![false; parameters.len()];
    for token in tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Parameter)
    {
        let name = parameter_name(token);
        match parameters
            .iter()
            .position(|parameter| names_equal(parameter.name(), name))
        {
            Some(index) => referenced[index] = true,
            None if bound.contains(name) => {}
            None => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Parameter,
                    Some(token),
                    format!("parameter {:?} has no value", token.lexeme),
                ));
            }
        }
    }
    if let Some(unused) = referenced
        .iter()
        .position(|used| !used)
        .map(|index| &parameters[index])
    {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Parameter,
            format!(
                "parameter {:?} is supplied but never referenced",
                unused.name()
            ),
        ));
    }
    Ok(())
}
