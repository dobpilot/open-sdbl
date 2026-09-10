use std::sync::Arc;

use super::expression::{
    matching_fields, reference_column, reference_type_column, resolve_named_field, single_column,
};
use super::orchestrate::PresentationCompilation;
use super::sources::{
    ReferencePresentationTargets, compile_deferred_reference_presentation, compile_live_relation,
    presentation_targets, wrap_reference_presentation,
};
use super::virtual_tables::compile_presentation_plan;
use crate::Token;
use crate::metadata::{MetadataSnapshot, ObjectId};
use crate::query::core::ast::{FieldReference, PresentationArgument, PresentationOperation};
use crate::query::core::dialect::{SqlDialect, compile_literal};
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableColumn, QueryableField,
};
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

pub(super) struct CompiledBranch {
    pub(super) sql: String,
    pub(super) columns: Vec<CompiledColumn>,
    pub(super) deferred_presentations: Vec<usize>,
    pub(super) logical_width: usize,
    pub(super) order: Vec<String>,
}

pub(super) enum SelectedProjection {
    Field(ResolvedPath),
    Generated {
        sql: String,
        label: String,
        deferred: bool,
        kind: ColumnKind,
    },
}

/// One SQL column rendered for a projected logical field.
pub(super) enum ProjectedMember<'field> {
    /// A physical member projected as-is.
    Single(&'field QueryableColumn),
    /// The `RTRef`/`RRRef` pair collapsed into one runtime-typed reference.
    Reference {
        type_member: &'field QueryableColumn,
        value_member: &'field QueryableColumn,
    },
}

/// Groups the physical members of a field into rendered SQL columns: a
/// reference pair becomes one column, every other member stays separate.
pub(super) fn projected_members(field: &QueryableField) -> Vec<ProjectedMember<'_>> {
    let type_member = field
        .columns
        .iter()
        .find(|column| column.is_reference_type_member());
    let value_member = field
        .columns
        .iter()
        .find(|column| column.is_reference_value_member());
    let (Some(type_member), Some(value_member)) = (type_member, value_member) else {
        return field.columns.iter().map(ProjectedMember::Single).collect();
    };
    let mut members = Vec::with_capacity(field.columns.len());
    for column in &field.columns {
        if std::ptr::eq(column, type_member) {
            continue;
        }
        if std::ptr::eq(column, value_member) {
            members.push(ProjectedMember::Reference {
                type_member,
                value_member,
            });
        } else {
            members.push(ProjectedMember::Single(column));
        }
    }
    members
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScopeId(pub(super) usize);

#[derive(Debug, Clone)]
pub(super) struct JoinPlan {
    pub(super) source_alias: String,
    pub(super) source_field: String,
    pub(super) source_column: String,
    pub(super) source_type_column: Option<String>,
    pub(super) database_type: Option<u32>,
    pub(super) target_object: ObjectId,
    pub(super) target_relation: String,
    pub(super) target_id_column: String,
    pub(super) alias: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct JoinKey<'value> {
    pub(super) source_alias: &'value str,
    pub(super) source_field: &'value str,
    pub(super) target_object: ObjectId,
    pub(super) database_type: Option<u32>,
}

impl JoinPlan {
    pub(super) fn matches(&self, key: JoinKey<'_>) -> bool {
        names_equal(&self.source_alias, key.source_alias)
            && names_equal(&self.source_field, key.source_field)
            && self.target_object == key.target_object
            && self.database_type == key.database_type
    }
}

pub(super) struct SourceScope {
    pub(super) object: ObjectId,
    pub(super) fields: Arc<[QueryableField]>,
    pub(super) relation: String,
    pub(super) sql_alias: String,
    pub(super) object_name: String,
    pub(super) source_alias: Option<String>,
    pub(super) identity_is_base: bool,
    pub(super) reference_joins: Vec<JoinPlan>,
}

impl SourceScope {
    fn is_qualifier(&self, name: &str) -> bool {
        self.source_alias
            .as_deref()
            .is_some_and(|alias| names_equal(alias, name))
            || names_equal(&self.object_name, name)
    }
}

pub(super) struct CompilationContext<'snapshot, 'catalog> {
    pub(super) snapshot: &'snapshot MetadataSnapshot,
    pub(super) catalog: &'catalog CompilationCatalog<'snapshot>,
    pub(super) sources: Vec<SourceScope>,
    pub(super) dialect: SqlDialect,
    /// Whether aggregate calls may appear in the expression being compiled
    /// (projections and `HAVING` of a grouped branch).
    pub(super) aggregates_allowed: bool,
    /// Set while a join condition is compiled.
    pub(super) compiling_join_condition: bool,
    /// Whether a join condition dereferenced a reference, which forces the
    /// dereference joins to be rendered next to their own source.
    pub(super) dereference_in_join: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedPath {
    pub(super) scope: ScopeId,
    pub(super) owner: ObjectId,
    pub(super) identity_is_base: bool,
    fields: Arc<[QueryableField]>,
    field_index: usize,
    pub(super) sql_alias: String,
    pub(super) path_label: Option<String>,
}

impl ResolvedPath {
    pub(super) fn from_source(
        scope: ScopeId,
        source: &SourceScope,
        field: (usize, &QueryableField),
    ) -> Self {
        let (field_index, _) = field;
        Self {
            scope,
            owner: source.object,
            identity_is_base: source.identity_is_base,
            fields: Arc::clone(&source.fields),
            field_index,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
        }
    }

    pub(super) fn field(&self) -> &QueryableField {
        &self.fields[self.field_index]
    }

    /// Whether two resolutions name the same field of the same source scope.
    pub(super) fn same_path(&self, other: &Self) -> bool {
        self.scope == other.scope
            && self.field_index == other.field_index
            && self.sql_alias == other.sql_alias
    }

    /// Label of the whole logical field: the alias or path label when one
    /// was given, otherwise the field name.
    pub(super) fn field_label(&self) -> String {
        self.path_label
            .clone()
            .unwrap_or_else(|| self.field().name.clone())
    }

    pub(super) fn output_label(&self, column: &QueryableColumn) -> String {
        let Some(path_label) = &self.path_label else {
            return column.output_label.clone();
        };
        if self.field().columns.len() == 1 {
            return path_label.clone();
        }
        column
            .output_label
            .strip_prefix(&self.field().name)
            .map_or_else(
                || format!("{path_label}_{}", column.output_label),
                |suffix| format!("{path_label}{suffix}"),
            )
    }
}

impl CompilationContext<'_, '_> {
    pub(super) fn source(&self, scope: ScopeId) -> &SourceScope {
        &self.sources[scope.0]
    }

    fn source_mut(&mut self, scope: ScopeId) -> &mut SourceScope {
        &mut self.sources[scope.0]
    }

    pub(super) fn base_alias(&self) -> &str {
        &self.sources[0].sql_alias
    }

    fn scope_description(&self) -> &'static str {
        if self.sources.len() > 1 {
            "JOIN sources"
        } else {
            "source"
        }
    }

    fn qualifier_scope(&self, qualifier: &Token<'_>) -> Result<Option<ScopeId>, QueryDiagnostic> {
        let matches = self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.is_qualifier(qualifier.lexeme))
            .map(|(index, _)| ScopeId(index))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [scope] => Ok(Some(*scope)),
            _ => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::AmbiguousObject,
                Some(qualifier),
                format!("source qualifier {:?} is ambiguous", qualifier.lexeme),
            )),
        }
    }

    fn direct_field(
        &self,
        scope: ScopeId,
        field: &Token<'_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        let source = self.source(scope);
        let (field_index, _) = resolve_named_field(&source.fields, field)?;
        Ok(ResolvedPath {
            scope,
            owner: source.object,
            identity_is_base: source.identity_is_base,
            fields: Arc::clone(&source.fields),
            field_index,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
        })
    }

    pub(super) fn resolve_direct(
        &self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [field] => {
                let matches = self
                    .sources
                    .iter()
                    .enumerate()
                    .flat_map(|(index, source)| {
                        matching_fields(&source.fields, field)
                            .into_iter()
                            .map(move |candidate| (ScopeId(index), candidate))
                    })
                    .collect::<Vec<_>>();
                match matches.as_slice() {
                    [(scope, _)] => self.direct_field(*scope, field),
                    [] => Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnknownField,
                        Some(field),
                        format!(
                            "field {:?} was not found in {}",
                            field.lexeme,
                            self.scope_description()
                        ),
                    )),
                    _ => Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::AmbiguousField,
                        Some(field),
                        format!(
                            "field {:?} is ambiguous in {}",
                            field.lexeme,
                            self.scope_description()
                        ),
                    )),
                }
            }
            [qualifier, field] => match self.qualifier_scope(qualifier)? {
                Some(scope) => self.direct_field(scope, field),
                None => Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnknownObject,
                    Some(qualifier),
                    format!("unknown source qualifier {:?}", qualifier.lexeme),
                )),
            },
            [_, _, unsupported, ..] => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(unsupported),
                "JOIN condition supports direct fields only",
            )),
            [] => unreachable!("field path is non-empty"),
        }
    }

    pub(super) fn resolve(
        &mut self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [_] => self.resolve_direct(reference),
            [first, second] => match self.qualifier_scope(first)? {
                Some(scope) => self.direct_field(scope, second),
                None => {
                    let candidates = self.reference_scopes(first);
                    match candidates.as_slice() {
                        [scope] => self.resolve_dereference(*scope, first, second),
                        [] => Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::UnknownField,
                            Some(first),
                            format!(
                                "field {:?} was not found in {}",
                                first.lexeme,
                                self.scope_description()
                            ),
                        )),
                        _ => Err(QueryDiagnostic::at(
                            QueryDiagnosticKind::AmbiguousField,
                            Some(first),
                            format!(
                                "reference field {:?} is ambiguous in {}",
                                first.lexeme,
                                self.scope_description()
                            ),
                        )),
                    }
                }
            },
            [qualifier, reference_field, target_field] => {
                let Some(scope) = self.qualifier_scope(qualifier)? else {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnknownObject,
                        Some(qualifier),
                        format!("unknown source qualifier {:?}", qualifier.lexeme),
                    ));
                };
                self.resolve_dereference(scope, reference_field, target_field)
            }
            [_, _, _, unsupported, ..] => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(unsupported),
                "reference paths deeper than one hop are not supported",
            )),
            [] => unreachable!("field path is non-empty"),
        }
    }

    fn reference_scopes(&self, token: &Token<'_>) -> Vec<ScopeId> {
        let matching = self
            .sources
            .iter()
            .enumerate()
            .map(|(index, _)| ScopeId(index))
            .filter(|scope| !matching_fields(&self.source(*scope).fields, token).is_empty())
            .collect::<Vec<_>>();
        let references = matching
            .iter()
            .copied()
            .filter(|scope| {
                matching_fields(&self.source(*scope).fields, token)
                    .iter()
                    .any(|(_, field)| {
                        field.reference_target.is_some() || !field.reference_targets.is_empty()
                    })
            })
            .collect::<Vec<_>>();
        if references.is_empty() {
            matching
        } else {
            references
        }
    }

    fn resolve_dereference(
        &mut self,
        scope: ScopeId,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        self.catalog.charge(
            self.source(scope).fields.len().saturating_add(1),
            Some(reference_token),
        )?;
        let (target_table, source_field, source_column) = {
            let (_, reference_field) =
                resolve_named_field(&self.source(scope).fields, reference_token)?;
            let target_table = reference_field.reference_target.as_deref().ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(reference_token),
                    format!(
                        "field {:?} has no unique SchemaStorage reference target",
                        reference_token.lexeme
                    ),
                )
            })?;
            (
                target_table.to_owned(),
                reference_field.schema_name.clone(),
                reference_column(reference_field, reference_token)?
                    .physical_name
                    .clone(),
            )
        };
        let target_physical = format!(
            "_{}",
            target_table.strip_prefix('_').unwrap_or(&target_table)
        );
        let target_objects = self
            .snapshot
            .objects()
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| names_equal(table, &target_physical))
            })
            .collect::<Vec<_>>();
        let target_object = match target_objects.as_slice() {
            [object] => *object,
            [] => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnknownObject,
                    Some(reference_token),
                    format!("reference target {target_physical:?} was not resolved"),
                ));
            }
            _ => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::AmbiguousObject,
                    Some(reference_token),
                    format!("reference target {target_physical:?} is ambiguous"),
                ));
            }
        };
        let target_live_table = self.snapshot.live_table(&target_physical).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::NotLive,
                Some(reference_token),
                format!("reference target table {target_physical:?} is not live"),
            )
        })?;
        let target_object_id = ObjectId::from(&target_object.guid);
        let target_fields = self.catalog.fields(target_object, Some(reference_token))?;
        let (target_field_index, _) = resolve_named_field(&target_fields, target_token)?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(reference_token),
                    format!("reference target {target_physical:?} has no ID field"),
                )
            })?;
        let target_id_column = single_column(target_id, reference_token)?
            .physical_name
            .clone();
        let target_relation = compile_live_relation(
            self.snapshot,
            target_live_table,
            &target_fields,
            self.dialect,
        );

        let source_alias = self.source(scope).sql_alias.clone();
        let join_key = JoinKey {
            source_alias: &source_alias,
            source_field: &source_field,
            target_object: target_object_id,
            database_type: None,
        };
        let existing = self
            .source(scope)
            .reference_joins
            .iter()
            .find(|join| join.matches(join_key))
            .map(|join| join.alias.clone());
        let alias = if let Some(alias) = existing {
            alias
        } else {
            let alias = self.next_reference_alias(scope);
            self.source_mut(scope).reference_joins.push(JoinPlan {
                source_alias,
                source_field,
                source_column,
                source_type_column: None,
                database_type: None,
                target_object: target_object_id,
                target_relation,
                target_id_column,
                alias: alias.clone(),
            });
            alias
        };
        if self.compiling_join_condition {
            self.dereference_in_join = true;
        }
        Ok(ResolvedPath {
            scope,
            owner: target_object_id,
            identity_is_base: true,
            fields: target_fields,
            field_index: target_field_index,
            sql_alias: alias,
            path_label: Some(format!(
                "{}.{}",
                reference_token.lexeme, target_token.lexeme
            )),
        })
    }

    fn next_reference_alias(&self, scope: ScopeId) -> String {
        let prefix = match (self.sources.len(), scope.0) {
            (1, _) => "__ref".to_owned(),
            (_, 0) => "__left_ref".to_owned(),
            (_, 1) => "__right_ref".to_owned(),
            (_, index) => format!("__join{}_ref", index + 1),
        };
        let mut number = self.source(scope).reference_joins.len() + 1;
        loop {
            let candidate = format!("{prefix}{number}");
            if self
                .sources
                .iter()
                .all(|source| !names_equal(&source.sql_alias, &candidate))
                && self
                    .sources
                    .iter()
                    .flat_map(|source| &source.reference_joins)
                    .all(|join| !names_equal(&join.alias, &candidate))
            {
                return candidate;
            }
            number += 1;
        }
    }

    pub(super) fn sql_column(&self, resolved: &ResolvedPath, column: &QueryableColumn) -> String {
        self.dialect
            .qualified_column(Some(&resolved.sql_alias), &column.physical_name)
    }

    pub(super) fn ensure_presentation_join(
        &mut self,
        scope: ScopeId,
        source_alias: &str,
        reference: &QueryableField,
        target: ObjectId,
        multiple: bool,
        token: &Token<'_>,
    ) -> Result<String, QueryDiagnostic> {
        self.catalog.charge(
            reference.reference_targets.len().saturating_add(1),
            Some(token),
        )?;
        let source_column = reference_column(reference, token)?.physical_name.clone();
        let source_type_column = multiple
            .then(|| reference_type_column(reference, token))
            .transpose()?
            .map(|column| column.physical_name.clone());
        let target_object = self.snapshot.object_by_id(target).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::PresentationPlan,
                Some(token),
                "presentation target was not resolved",
            )
        })?;
        let database_type = multiple.then_some(target_object.number).flatten();
        let target_table = target_object
            .physical_table
            .as_deref()
            .and_then(|physical| self.snapshot.live_table(physical))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::NotLive,
                    Some(token),
                    "presentation target table is not live",
                )
            })?;
        let target_fields = self.catalog.fields(target_object, Some(token))?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::PresentationPlan,
                    Some(token),
                    "presentation target has no ID",
                )
            })?;
        let target_id_column = single_column(target_id, token)?.physical_name.clone();
        let target_relation =
            compile_live_relation(self.snapshot, target_table, &target_fields, self.dialect);
        let join_key = JoinKey {
            source_alias,
            source_field: &reference.schema_name,
            target_object: target,
            database_type,
        };
        if let Some(join) = self
            .source(scope)
            .reference_joins
            .iter()
            .find(|join| join.matches(join_key))
        {
            return Ok(join.alias.clone());
        }
        let alias = self.next_reference_alias(scope);
        self.source_mut(scope).reference_joins.push(JoinPlan {
            source_alias: source_alias.to_owned(),
            source_field: reference.schema_name.clone(),
            source_column,
            source_type_column,
            database_type,
            target_object: target,
            target_relation,
            target_id_column,
            alias: alias.clone(),
        });
        Ok(alias)
    }
}

pub(super) fn compile_presentation(
    context: &mut CompilationContext<'_, '_>,
    token: &Token<'_>,
    operation: PresentationOperation,
    argument: &PresentationArgument<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<(String, String, bool), QueryDiagnostic> {
    let label = token.lexeme.to_owned();
    let PresentationArgument::Field(reference) = argument else {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "Presentation property requires a reference field",
            ));
        }
        let PresentationArgument::Literal(literal) = argument else {
            unreachable!()
        };
        let value = compile_literal(literal, context.dialect)?;
        return Ok((context.dialect.scalar_text(&value), label, false));
    };
    let resolved = context.resolve(reference)?;
    let owner = resolved.owner;
    let source_identity_is_base = resolved.identity_is_base;
    let source_alias = resolved.sql_alias.clone();
    let targets =
        presentation_targets(context.snapshot, owner, resolved.field(), reference.last())?;
    if targets == ReferencePresentationTargets::Scalar {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "Presentation property is available only for reference fields",
            ));
        }
        let column = single_column(resolved.field(), reference.last())?;
        let value = context
            .dialect
            .qualified_column(Some(&source_alias), &column.physical_name);
        return Ok((context.dialect.scalar_text(&value), label, false));
    }

    if targets == ReferencePresentationTargets::Deferred {
        let payload = compile_deferred_reference_presentation(
            &source_alias,
            resolved.field(),
            reference.last(),
            context.dialect,
        )?;
        return Ok((payload, label, true));
    }
    let ReferencePresentationTargets::Static(targets) = targets else {
        unreachable!("scalar and deferred targets returned above")
    };
    let multiple = targets.len() > 1;
    let source_reference = reference_column(resolved.field(), reference.last())?
        .physical_name
        .clone();
    let source_type = multiple
        .then(|| reference_type_column(resolved.field(), reference.last()))
        .transpose()?
        .map(|column| column.physical_name.clone());
    let mut variants = Vec::new();
    for (target, number) in targets {
        let plan = presentations.plan(target, token)?;
        let alias = if source_identity_is_base
            && target == owner
            && names_equal(&resolved.field().schema_name, "ID")
        {
            source_alias.clone()
        } else {
            context.ensure_presentation_join(
                resolved.scope,
                &source_alias,
                resolved.field(),
                target,
                multiple,
                token,
            )?
        };
        let expression = plan.map_or_else(
            || Ok(context.dialect.null_text().to_owned()),
            |plan| {
                compile_presentation_plan(
                    context.snapshot,
                    context.catalog,
                    target,
                    &alias,
                    plan,
                    Some(token),
                    context.dialect,
                )
            },
        )?;
        variants.push((number, expression));
    }
    Ok((
        wrap_reference_presentation(
            &source_alias,
            &source_reference,
            source_type.as_deref(),
            &variants,
            context.dialect,
        ),
        label,
        false,
    ))
}
