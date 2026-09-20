use std::collections::BTreeSet;

use super::batch::compile_batch_ast;
use super::expression::single_column_at;
use super::orchestrate::PresentationCompilation;
use super::sources::{
    SourceRestriction, compile_live_relation, compile_restriction_predicate,
    wrap_restricted_relation,
};
use super::virtual_tables::compile_presentation_plan;
use crate::metadata::MetadataSnapshot;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::params::{CompileOptions, Parameters, QueryParameter, parameter_name};
use crate::query::core::parser::Parser;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, CompiledQuery, PresentationPlan,
    PresentationRequest, PresentationTarget, RestrictionState, restriction_label,
};
use crate::query::core::restrict::{
    AccessDecision, AccessRestriction, RestrictionMode, RestrictionRequest, RestrictionTarget,
};
use crate::query::core::temp_tables::TempTablesManager;
use crate::query::core::usage::FieldUsageRequest;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{TokenKind, tokenize};

/// The application callback requests collected by preparation.
pub(crate) struct PreparedRequests {
    pub(crate) presentations: PresentationRequest,
    pub(crate) restrictions: RestrictionRequest,
    pub(crate) field_usage: FieldUsageRequest,
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
    mode: RestrictionMode,
) -> Result<PreparedRequests, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations = PresentationCompilation::collect(dialect).with_mode(mode);
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
        field_usage: FieldUsageRequest {
            fields: presentations.field_usage,
        },
    })
}

pub(crate) fn compile_presentation_lookup(
    snapshot: &MetadataSnapshot,
    plan: &PresentationPlan,
    references: &[[u8; 16]],
    dialect: SqlDialect,
    options: &CompileOptions<'_>,
    mode: RestrictionMode,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let mut catalog = CompilationCatalog::new(snapshot, options.bound_parameters());
    // The lookup reads one table, and the question about it is the one a
    // statement asks about its own sources, so it is armed the same way.
    catalog.set_restrictions(RestrictionState {
        restrictions: options.access_restrictions(),
        decisions: options.access_decisions(),
        mode,
        keyword: false,
        collecting: false,
    });
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
    let target = RestrictionTarget {
        object: plan.object,
        table_part: None,
    };
    let decision = catalog.decide(target.clone(), None, || {
        restriction_label(snapshot, &target)
    })?;
    let relation = match decision.condition() {
        // A reference the decision excludes matches no row, so the
        // application presents nothing for it — the same answer a deleted
        // object already gives.
        Some(condition) => {
            let restricted = compile_live_relation(
                snapshot,
                &catalog,
                target_table,
                &fields,
                "__restricted",
                None,
                dialect,
            )?;
            let restriction = SourceRestriction {
                condition,
                label: restriction_label(snapshot, &target),
                identity_is_base: true,
            };
            let predicate = compile_restriction_predicate(
                &restriction,
                snapshot,
                &catalog,
                object,
                &restriction.label,
                &fields,
                "__restricted",
                dialect,
            )?;
            wrap_restricted_relation(
                &restricted.sql,
                &fields,
                &predicate,
                &restricted.separators,
                dialect,
            )
        }
        None => {
            compile_live_relation(
                snapshot,
                &catalog,
                target_table,
                &fields,
                alias,
                None,
                dialect,
            )?
            .sql
        }
    };
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
        nested: Vec::new(),
        service_columns: Vec::new(),
    })
}

pub(crate) fn compile_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    options: &CompileOptions<'_>,
    dialect: SqlDialect,
    mode: RestrictionMode,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let mut manager = TempTablesManager::new();
    compile_batch(source, snapshot, options, dialect, &mut manager, mode)?.ok_or_else(|| {
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
    mode: RestrictionMode,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let parameters = options.bound_parameters();
    check_parameter_binding(&tokens, options.parameter_values(), parameters)?;
    let restrictions = options.access_restrictions();
    let decisions = options.access_decisions();
    check_decision_uniqueness(snapshot, restrictions, decisions)?;
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations =
        PresentationCompilation::strict(options.presentation_plans(), parameters, dialect)
            .with_restrictions(restrictions, decisions)
            .with_mode(mode)
            .with_totals_level(options.totals_level_enabled());
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
    if let Some(unused) = (0..decisions.len())
        .find(|index| !presentations.used_decisions.contains(index))
        .map(|index| &decisions[index])
    {
        return Err(QueryDiagnostic::unpositioned(
            QueryDiagnosticKind::Restriction,
            format!(
                "access decision for {} is supplied but no statement reads it",
                restriction_label(snapshot, &decision_target(unused))
            ),
        ));
    }
    Ok(compiled)
}

fn decision_target(decision: &AccessDecision) -> RestrictionTarget {
    RestrictionTarget {
        object: decision.object(),
        table_part: decision.table_part_name().map(str::to_owned),
    }
}

fn restriction_target(restriction: &AccessRestriction) -> RestrictionTarget {
    RestrictionTarget {
        object: restriction.object(),
        table_part: restriction.table_part_name().map(str::to_owned),
    }
}

/// Two answers about one target would have to be combined by a rule the
/// application did not state, so they are rejected — whether both are
/// conditions, both decisions, or one of each.
fn check_decision_uniqueness(
    snapshot: &MetadataSnapshot,
    restrictions: &[AccessRestriction],
    decisions: &[AccessDecision],
) -> Result<(), QueryDiagnostic> {
    // Section names compare case-insensitively, like every 1C identifier,
    // so two spellings of one section are still two answers about it.
    let same = |left: &RestrictionTarget, right: &RestrictionTarget| {
        left.object == right.object
            && match (&left.table_part, &right.table_part) {
                (None, None) => true,
                (Some(left), Some(right)) => names_equal(left, right),
                _ => false,
            }
    };
    let mut seen: Vec<RestrictionTarget> = Vec::new();
    let mut check = |target: RestrictionTarget| -> Result<(), QueryDiagnostic> {
        if seen.iter().any(|previous| same(previous, &target)) {
            return Err(QueryDiagnostic::unpositioned(
                QueryDiagnosticKind::Restriction,
                format!(
                    "restriction of {} is supplied more than once",
                    restriction_label(snapshot, &target)
                ),
            ));
        }
        seen.push(target);
        Ok(())
    };
    for restriction in restrictions {
        check(restriction_target(restriction))?;
    }
    for decision in decisions {
        check(decision_target(decision))?;
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
