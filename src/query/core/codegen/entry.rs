use std::collections::BTreeSet;

use super::expression::single_column_at;
use super::orchestrate::{PresentationCompilation, compile};
use super::sources::compile_live_relation;
use super::virtual_tables::compile_presentation_plan;
use crate::metadata::MetadataSnapshot;
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::parser::Parser;
use crate::query::core::resolve::{
    CompilationCatalog, CompiledQuery, PresentationPlan, PresentationRequest, PresentationTarget,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{TokenKind, tokenize};

pub(crate) fn prepare_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    dialect: SqlDialect,
) -> Result<PresentationRequest, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations = PresentationCompilation::collect(dialect);
    let _ = compile(ast, snapshot, &mut presentations)?;
    Ok(PresentationRequest {
        targets: presentations
            .requested
            .into_iter()
            .map(|object| PresentationTarget { object })
            .collect(),
    })
}

pub(crate) fn compile_presentation_lookup(
    snapshot: &MetadataSnapshot,
    plan: &PresentationPlan,
    references: &[[u8; 16]],
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let catalog = CompilationCatalog::new(snapshot);
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
        .and_then(|physical| {
            snapshot
                .live_tables()
                .iter()
                .find(|table| names_equal(&table.name, physical))
        })
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
        "SELECT {} AS {reference_label}, {expression} AS {presentation_label} FROM {relation} AS {} WHERE {qualified_id} IN ({values})",
        dialect.binary_hex_text(&qualified_id),
        dialect.quote_identifier(alias),
    );
    Ok(CompiledQuery {
        sql,
        columns: vec!["__reference".to_owned(), "__presentation".to_owned()],
        deferred_presentations: Vec::new(),
    })
}

pub(crate) fn compile_query(
    source: &str,
    snapshot: &MetadataSnapshot,
    plans: &[PresentationPlan],
    dialect: SqlDialect,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens, source).parse()?;
    let mut presentations = PresentationCompilation::strict(plans, dialect);
    compile(ast, snapshot, &mut presentations)
}
