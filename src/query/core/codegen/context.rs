use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use super::constants::ConstantsSource;
use super::expression::{
    matching_fields, reference_column, reference_type_column, resolve_named_field, single_column,
    type_constant, value_type_sql,
};
use super::orchestrate::PresentationCompilation;
use super::select::{derived_data_type, derived_owner};
use super::sources::{
    ReferencePresentationTargets, compile_deferred_reference_presentation, compile_live_relation,
    presentation_targets, wrap_reference_presentation,
};
use super::virtual_tables::AggregateSource;
use super::virtual_tables::compile_presentation_plan;
use crate::Token;
use crate::metadata::{MetadataKind, MetadataSnapshot, ObjectId};
use crate::query::core::ast::{FieldReference, PresentationArgument, PresentationOperation};
use crate::query::core::dialect::{SqlDialect, compile_literal};
use crate::query::core::names::names_equal;
use crate::query::core::resolve::{
    ColumnKind, CompilationCatalog, CompiledColumn, QueryableColumn, QueryableField,
    is_standard_field_name,
};
use crate::query::core::types::TypeValue;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};

pub(super) struct CompiledBranch {
    pub(super) sql: String,
    pub(super) columns: Vec<CompiledColumn>,
    pub(super) deferred_presentations: Vec<usize>,
    pub(super) logical_width: usize,
    pub(super) order: Vec<OrderKey>,
}

/// One rendered `ORDER BY` key: a positional output column of a union or
/// joined branch, or a source expression that a totals wrapper projects as
/// a hidden `__order_<n>` column.
#[derive(Debug, Clone)]
pub(super) struct OrderKey {
    /// The column expression, or the hidden column label when the branch
    /// projected the expression for a totals wrapper.
    pub(super) sql: String,
    /// The one-based output position when the key addresses a projection.
    pub(super) position: Option<usize>,
    pub(super) descending: bool,
}

impl OrderKey {
    pub(super) fn render(&self) -> String {
        format!(
            "{}{}",
            self.position
                .map_or_else(|| self.sql.clone(), |position| position.to_string()),
            if self.descending { " DESC" } else { " ASC" }
        )
    }
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
    /// Rendered key expressions when the source members are not plain
    /// columns, as for the payload column of a derived source.
    pub(super) source_value_sql: Option<String>,
    pub(super) source_type_sql: Option<String>,
    /// Data-separator predicates of the target, qualified with `alias`,
    /// conjoined to the join condition.
    pub(super) target_predicates: Vec<String>,
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
    /// Data-separator predicates of the relation, qualified with
    /// `sql_alias`, placed by the branch renderer.
    pub(super) separator_predicates: Vec<String>,
    /// Set for the `Константы` source, whose relation is rendered from the
    /// fields the statement resolves against it.
    pub(super) constants: Option<ConstantsSource>,
    /// Set for an aggregating register table, whose unused dimensions are
    /// summed away once the statement is known.
    pub(super) aggregate: Option<AggregateSource>,
    /// Indexes into `fields` the statement resolved, in any role.
    pub(super) used_fields: RefCell<BTreeSet<usize>>,
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
    /// The comma element of every scope, in scope order: a join condition
    /// may reference only scopes of its own element.
    pub(super) source_elements: Vec<usize>,
    /// How many leading scopes belong to this statement. The scopes after
    /// them are the sources of the enclosing statement, visible to a
    /// correlated subquery by their qualifier and never rendered here.
    pub(super) local_sources: usize,
}

/// One source of an enclosing statement, carried into a correlated
/// subquery so that its qualifier resolves there.
#[derive(Clone)]
pub(super) struct OuterScope {
    object: ObjectId,
    fields: Arc<[QueryableField]>,
    sql_alias: String,
    object_name: String,
    source_alias: Option<String>,
    identity_is_base: bool,
}

impl OuterScope {
    /// The read-only scope a correlated subquery resolves against: it
    /// carries no relation, because the enclosing statement renders it.
    fn into_scope(self) -> SourceScope {
        SourceScope {
            object: self.object,
            fields: self.fields,
            relation: String::new(),
            sql_alias: self.sql_alias,
            object_name: self.object_name,
            source_alias: self.source_alias,
            identity_is_base: self.identity_is_base,
            reference_joins: Vec::new(),
            separator_predicates: Vec::new(),
            constants: None,
            aggregate: None,
            used_fields: RefCell::new(BTreeSet::new()),
        }
    }
}

/// Appends the sources of an enclosing statement to a freshly built
/// context, where they are visible only by qualifier.
pub(super) fn attach_outer_scopes(context: &mut CompilationContext<'_, '_>, outer: &[OuterScope]) {
    context.local_sources = context.sources.len();
    context
        .sources
        .extend(outer.iter().cloned().map(OuterScope::into_scope));
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
    /// Rendered value replacing `alias.column`, used by a dereference that
    /// selects across several reference targets.
    pub(super) expression: Option<String>,
}

impl ResolvedPath {
    pub(super) fn from_source(
        scope: ScopeId,
        source: &SourceScope,
        field: (usize, &QueryableField),
    ) -> Self {
        let (field_index, _) = field;
        source.used_fields.borrow_mut().insert(field_index);
        Self {
            scope,
            owner: source.object,
            identity_is_base: source.identity_is_base,
            fields: Arc::clone(&source.fields),
            field_index,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
            expression: None,
        }
    }

    pub(super) fn field(&self) -> &QueryableField {
        &self.fields[self.field_index]
    }

    /// The same resolution pointing at another field of the same source.
    fn with_field_index(&self, field_index: usize) -> Self {
        Self {
            field_index,
            path_label: None,
            expression: None,
            fields: Arc::clone(&self.fields),
            sql_alias: self.sql_alias.clone(),
            ..*self
        }
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
    /// Reference targets one dereference may reach before the query is
    /// asked to narrow the field with `ВЫРАЗИТЬ`.
    const MAX_DEREFERENCE_TARGETS: usize = 32;

    pub(super) fn source(&self, scope: ScopeId) -> &SourceScope {
        &self.sources[scope.0]
    }

    fn source_mut(&mut self, scope: ScopeId) -> &mut SourceScope {
        &mut self.sources[scope.0]
    }

    /// The sources of this statement, as an enclosing statement hands them
    /// to a correlated subquery.
    pub(super) fn outer_scopes(&self) -> Vec<OuterScope> {
        self.sources[..self.local_sources]
            .iter()
            .map(|source| OuterScope {
                object: source.object,
                fields: Arc::clone(&source.fields),
                sql_alias: source.sql_alias.clone(),
                object_name: source.object_name.clone(),
                source_alias: source.source_alias.clone(),
                identity_is_base: source.identity_is_base,
            })
            .collect()
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
        let (field_index, _) = match resolve_named_field(&source.fields, field) {
            Ok(resolved) => resolved,
            Err(error) => {
                return self.computed_scope_field(scope, field).ok_or(error);
            }
        };
        source.used_fields.borrow_mut().insert(field_index);
        Ok(ResolvedPath {
            scope,
            owner: source.object,
            identity_is_base: source.identity_is_base,
            fields: Arc::clone(&source.fields),
            field_index,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
            expression: None,
        })
    }

    /// The standard fields the platform computes instead of storing:
    /// `ЭтоГруппа` is the negation of the `Folder` column, which holds
    /// true for an item, `Предопределенный` says the `PredefinedID`
    /// column is not the empty reference, `ИмяПредопределенныхДанных`
    /// names the predefined item that column identifies, and `Тип` of a
    /// document journal is the type of its reference. All are measured
    /// against the platform.
    fn computed_standard_field(
        &self,
        owner: ObjectId,
        fields: &[QueryableField],
        alias: &str,
        token: &Token<'_>,
    ) -> Option<(QueryableField, String)> {
        if names_equal(token.lexeme, "ИмяПредопределенныхДанных")
            || names_equal(token.lexeme, "PredefinedDataName")
        {
            return self.predefined_data_name(owner, fields, alias, token);
        }
        if names_equal(token.lexeme, "Тип") || names_equal(token.lexeme, "Type") {
            return self.journal_reference_type(fields, alias, token);
        }
        let (schema_name, negated) = if names_equal(token.lexeme, "ЭтоГруппа")
            || names_equal(token.lexeme, "IsFolder")
        {
            ("Folder", true)
        } else if names_equal(token.lexeme, "Предопределенный")
            || names_equal(token.lexeme, "Предопределённый")
        {
            ("PredefinedID", false)
        } else {
            return None;
        };
        let source = fields
            .iter()
            .find(|field| names_equal(&field.schema_name, schema_name))?;
        let column = source.columns.first()?;
        let qualified = self
            .dialect
            .qualified_column(Some(alias), &column.physical_name);
        let predicate = if negated {
            format!("{qualified} = {}", self.dialect.boolean_literal(false))
        } else {
            format!("{qualified} <> {}", self.dialect.binary_literal(&[0; 16]))
        };
        let name = token.lexeme.to_owned();
        let field = QueryableField {
            name: name.clone(),
            schema_name: schema_name.to_owned(),
            aliases: vec![name.clone()],
            columns: vec![QueryableColumn {
                physical_name: column.physical_name.clone(),
                data_type: "boolean".to_owned(),
                output_label: name,
                kind: ColumnKind::Boolean,
            }],
            reference_target: None,
            reference_targets: Vec::new(),
        };
        Some((field, self.dialect.boolean_value(&predicate)))
    }

    /// `Тип` of a document journal answers the type of the registered
    /// document: the journal stores one reference column when it registers
    /// a single document kind and an `RTRef ‖ RRRef` pair otherwise, so
    /// the type value is read from the same members `ТИПЗНАЧЕНИЯ` uses.
    fn journal_reference_type(
        &self,
        fields: &[QueryableField],
        alias: &str,
        token: &Token<'_>,
    ) -> Option<(QueryableField, String)> {
        let source = fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "Document"))?;
        let type_member = source
            .columns
            .iter()
            .find(|column| column.is_reference_type_member());
        let value_member = source
            .columns
            .iter()
            .find(|column| column.is_reference_value_member())?;
        let expression = match type_member {
            Some(type_member) => format!(
                "COALESCE({}, {})",
                self.dialect.reference_payload(
                    &self.dialect.binary_literal(&[TypeValue::TAG_REFERENCE]),
                    &self
                        .dialect
                        .qualified_column(Some(alias), &type_member.physical_name),
                ),
                type_constant(TypeValue::Null, self.dialect),
            ),
            None => value_type_sql(
                &self
                    .dialect
                    .qualified_column(Some(alias), &value_member.physical_name),
                &value_member.kind,
                true,
                self.snapshot,
                self.dialect,
                token,
            )
            .ok()?,
        };
        let name = token.lexeme.to_owned();
        let field = QueryableField {
            name: name.clone(),
            schema_name: "Type".to_owned(),
            aliases: vec![name.clone()],
            columns: vec![QueryableColumn {
                physical_name: value_member.physical_name.clone(),
                data_type: String::new(),
                output_label: name,
                kind: ColumnKind::Type,
            }],
            reference_target: None,
            reference_targets: Vec::new(),
        };
        Some((field, expression))
    }

    /// `ИмяПредопределенныхДанных` names the predefined item a row is. The
    /// platform stores only its GUID in `PredefinedID` and maps it back to
    /// the name declared in the configuration, answering an empty string
    /// for a row that is not predefined; that mapping is spelled here as a
    /// conditional over the stored identifier.
    fn predefined_data_name(
        &self,
        owner: ObjectId,
        fields: &[QueryableField],
        alias: &str,
        token: &Token<'_>,
    ) -> Option<(QueryableField, String)> {
        let source = fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "PredefinedID"))?;
        let column = source.columns.first()?;
        let qualified = self
            .dialect
            .qualified_column(Some(alias), &column.physical_name);
        let empty = self.dialect.string_literal("");
        // A row the outer join missed keeps NULL, as it does on the
        // platform, which reads the stored identifier itself.
        let mut branches = format!(
            "WHEN {qualified} IS NULL THEN {} ",
            self.dialect.null_text()
        );
        for value in self
            .snapshot
            .values()
            .iter()
            .filter(|value| value.owner == owner)
        {
            branches.push_str(&format!(
                "WHEN {qualified} = {} THEN {} ",
                self.dialect.binary_literal(&value.guid.to_1c_bytes()),
                self.dialect.string_literal(&value.name),
            ));
        }
        let expression = format!("CASE {branches}ELSE {empty} END");
        let name = token.lexeme.to_owned();
        let kind = ColumnKind::String { length: None };
        let field = QueryableField {
            name: name.clone(),
            schema_name: "PredefinedDataName".to_owned(),
            aliases: vec![name.clone()],
            columns: vec![QueryableColumn {
                physical_name: column.physical_name.clone(),
                data_type: derived_data_type(&kind, self.dialect),
                output_label: name,
                kind,
            }],
            reference_target: None,
            reference_targets: Vec::new(),
        };
        Some((field, expression))
    }

    /// Resolves a computed standard field of one source scope.
    fn computed_scope_field(&self, scope: ScopeId, token: &Token<'_>) -> Option<ResolvedPath> {
        let source = self.source(scope);
        let (field, expression) =
            self.computed_standard_field(source.object, &source.fields, &source.sql_alias, token)?;
        Some(ResolvedPath {
            scope,
            owner: source.object,
            identity_is_base: false,
            fields: Arc::from(vec![field]),
            field_index: 0,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
            expression: Some(expression),
        })
    }

    /// The field of the same source named `<name><suffix>`, which is how a
    /// derived source exposes the extra physical members of a composite
    /// field next to its reference payload.
    pub(super) fn companion_field(
        &self,
        resolved: &ResolvedPath,
        suffix: &str,
    ) -> Option<ResolvedPath> {
        if resolved.expression.is_some() {
            return None;
        }
        let wanted = format!("{}{suffix}", resolved.field().name);
        let source = self.source(resolved.scope);
        let index = source
            .fields
            .iter()
            .position(|field| names_equal(&field.name, &wanted))?;
        source.used_fields.borrow_mut().insert(index);
        Some(resolved.with_field_index(index))
    }

    pub(super) fn resolve_direct(
        &self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [field] => {
                let matches = self.sources[..self.local_sources]
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
                    [] => (0..self.local_sources)
                        .find_map(|index| self.computed_scope_field(ScopeId(index), field))
                        .ok_or_else(|| {
                            QueryDiagnostic::at(
                                QueryDiagnosticKind::UnknownField,
                                Some(field),
                                format!(
                                    "field {:?} was not found in {}",
                                    field.lexeme,
                                    self.scope_description()
                                ),
                            )
                        }),
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
            [qualifier, rest @ ..] => {
                let Some(scope) = self.qualifier_scope(qualifier)? else {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnknownObject,
                        Some(qualifier),
                        format!("unknown source qualifier {:?}", qualifier.lexeme),
                    ));
                };
                match rest {
                    [reference_field, target_field] => {
                        self.resolve_dereference(scope, reference_field, target_field)
                    }
                    hops => self.resolve_deep_dereference(scope, hops),
                }
            }
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
        let single_target = {
            let (field_index, reference_field) =
                resolve_named_field(&self.source(scope).fields, reference_token)?;
            self.source(scope)
                .used_fields
                .borrow_mut()
                .insert(field_index);
            reference_field.reference_target.clone()
        };
        let Some(target_table) = single_target else {
            return self.resolve_composite_dereference(scope, reference_token, target_token);
        };
        let (source_field, source_column) = {
            let (_, reference_field) =
                resolve_named_field(&self.source(scope).fields, reference_token)?;
            (
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
        let computed = resolve_named_field(&target_fields, target_token).is_err();
        let (target_field_index, _) = if computed {
            (0, &target_fields[0])
        } else {
            resolve_named_field(&target_fields, target_token)?
        };
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
            let target = compile_live_relation(
                self.snapshot,
                self.catalog,
                target_live_table,
                &target_fields,
                &alias,
                Some(reference_token),
                self.dialect,
            )?;
            self.source_mut(scope).reference_joins.push(JoinPlan {
                source_alias,
                source_field,
                source_column,
                source_type_column: None,
                database_type: None,
                target_object: target_object_id,
                target_relation: target.sql,
                target_id_column,
                alias: alias.clone(),
                source_value_sql: None,
                source_type_sql: None,
                target_predicates: target.separators,
            });
            alias
        };
        if self.compiling_join_condition {
            self.dereference_in_join = true;
        }
        // The joined table may expose a computed standard field too.
        let computed = computed.then(|| {
            self.computed_standard_field(target_object_id, &target_fields, &alias, target_token)
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::UnknownField,
                        Some(target_token),
                        format!("field {:?} was not found", target_token.lexeme),
                    )
                })
        });
        let (fields, field_index, expression) = match computed.transpose()? {
            Some((field, expression)) => (Arc::from(vec![field]), 0, Some(expression)),
            None => (target_fields, target_field_index, None),
        };
        Ok(ResolvedPath {
            scope,
            owner: target_object_id,
            identity_is_base: true,
            fields,
            field_index,
            sql_alias: alias,
            path_label: Some(format!(
                "{}.{}",
                reference_token.lexeme, target_token.lexeme
            )),
            expression,
        })
    }

    /// Walks a reference path of more than one hop: every hop but the
    /// last joins its target to the alias the previous hop produced, and
    /// the last segment is read from the table the walk ended on. Only
    /// single-target references can be walked through; a composite one
    /// selects its value by type and has no single table to continue
    /// from.
    fn resolve_deep_dereference(
        &mut self,
        scope: ScopeId,
        hops: &[&Token<'_>],
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        let [first, second, rest @ ..] = hops else {
            unreachable!("a deep path has at least three segments");
        };
        let mut resolved = self.resolve_dereference(scope, first, second)?;
        let mut label = format!("{}.{}", first.lexeme, second.lexeme);
        for segment in rest {
            let field = resolved.field();
            let Some(target_table) = field.reference_target.clone() else {
                let composite = field
                    .columns
                    .iter()
                    .any(QueryableColumn::is_reference_value_member)
                    || !field.reference_targets.is_empty();
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(segment),
                    format!(
                        "reference path cannot continue through {:?}, which {}",
                        field.name,
                        if composite {
                            "references more than one table"
                        } else {
                            "is not a reference"
                        }
                    ),
                ));
            };
            let column = reference_column(field, segment)?.physical_name.clone();
            let source_field = field.schema_name.clone();
            let alias = resolved.sql_alias.clone();
            resolved = self.join_reference_target(
                scope,
                &alias,
                source_field,
                column,
                &target_table,
                segment,
                segment,
            )?;
            label.push('.');
            label.push_str(segment.lexeme);
        }
        resolved.path_label = Some(label);
        Ok(resolved)
    }

    /// Joins the table a reference column points at and resolves
    /// `target_token` in it, reusing an identical join of the same scope.
    #[allow(clippy::too_many_arguments)]
    fn join_reference_target(
        &mut self,
        scope: ScopeId,
        source_alias: &str,
        source_field: String,
        source_column: String,
        target_table: &str,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        let target_physical = format!(
            "_{}",
            target_table.strip_prefix('_').unwrap_or(target_table)
        );
        let target_object = self.reference_target_object(&target_physical, reference_token)?;
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
        let target_id_column = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(reference_token),
                    format!("reference target {target_physical:?} has no ID field"),
                )
            })
            .and_then(|field| Ok(single_column(field, reference_token)?.physical_name.clone()))?;
        let existing = self
            .source(scope)
            .reference_joins
            .iter()
            .find(|join| {
                join.matches(JoinKey {
                    source_alias,
                    source_field: &source_field,
                    target_object: target_object_id,
                    database_type: None,
                })
            })
            .map(|join| join.alias.clone());
        let alias = if let Some(alias) = existing {
            alias
        } else {
            let alias = self.next_reference_alias(scope);
            let target = compile_live_relation(
                self.snapshot,
                self.catalog,
                target_live_table,
                &target_fields,
                &alias,
                Some(reference_token),
                self.dialect,
            )?;
            self.source_mut(scope).reference_joins.push(JoinPlan {
                source_alias: source_alias.to_owned(),
                source_field,
                source_column,
                source_type_column: None,
                database_type: None,
                target_object: target_object_id,
                target_relation: target.sql,
                target_id_column,
                alias: alias.clone(),
                source_value_sql: None,
                source_type_sql: None,
                target_predicates: target.separators,
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
            path_label: None,
            expression: None,
        })
    }

    /// The metadata object a reference column points at.
    fn reference_target_object(
        &self,
        target_physical: &str,
        token: &Token<'_>,
    ) -> Result<&crate::metadata::MetadataObject, QueryDiagnostic> {
        let objects = self
            .snapshot
            .objects()
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| names_equal(table, target_physical))
            })
            .collect::<Vec<_>>();
        match objects.as_slice() {
            [object] => Ok(object),
            [] => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnknownObject,
                Some(token),
                format!("reference target {target_physical:?} was not resolved"),
            )),
            _ => Err(QueryDiagnostic::at(
                QueryDiagnosticKind::AmbiguousObject,
                Some(token),
                format!("reference target {target_physical:?} is ambiguous"),
            )),
        }
    }

    /// Dereferences a composite reference: every candidate target is
    /// joined under its own type guard and the value is selected by the
    /// reference type, exactly as the platform resolves `ЛюбаяСсылка`.
    fn resolve_composite_dereference(
        &mut self,
        scope: ScopeId,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        let source = self.composite_source(scope, reference_token)?;
        let candidates = self.dereference_candidates(&source, target_token)?;
        let mut branches = Vec::new();
        for candidate in candidates {
            if let Some(branch) =
                self.dereference_branch(scope, &source, candidate, reference_token, target_token)?
            {
                branches.push(branch);
            }
        }
        if branches.is_empty() {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnknownField,
                Some(target_token),
                format!(
                    "field {:?} was not found in any target of {:?}",
                    target_token.lexeme, reference_token.lexeme
                ),
            ));
        }
        let kind = unify_dereference_kinds(&branches, target_token)?;
        let widen = matches!(kind, ColumnKind::Reference { .. })
            && branches.iter().any(|branch| branch.kind != kind);
        let mut arms = Vec::with_capacity(branches.len());
        for branch in &branches {
            let value = self.branch_value(branch, widen, target_token)?;
            arms.push((branch.database_type, value));
        }
        let expression = if let [(_, value)] = arms.as_slice() {
            value.clone()
        } else {
            let mut sql = String::from("CASE");
            for (database_type, value) in &arms {
                sql.push_str(&format!(
                    " WHEN {} = {} THEN {value}",
                    source.type_sql,
                    self.dialect.binary_u32(*database_type)
                ));
            }
            sql.push_str(" END");
            sql
        };
        let first = branches.first().expect("a branch was compiled");
        let field = QueryableField {
            name: first.field_name.clone(),
            schema_name: first.schema_name.clone(),
            aliases: vec![first.field_name.clone()],
            columns: vec![QueryableColumn {
                physical_name: first.physical_name.clone(),
                data_type: first.data_type.clone(),
                output_label: target_token.lexeme.to_owned(),
                kind,
            }],
            reference_target: None,
            reference_targets: Vec::new(),
        };
        if self.compiling_join_condition {
            self.dereference_in_join = true;
        }
        Ok(ResolvedPath {
            scope,
            owner: derived_owner(),
            identity_is_base: false,
            fields: Arc::from(vec![field]),
            field_index: 0,
            sql_alias: self.source(scope).sql_alias.clone(),
            path_label: Some(format!(
                "{}.{}",
                reference_token.lexeme, target_token.lexeme
            )),
            expression: Some(expression),
        })
    }

    /// The type and identifier expressions of a composite reference field
    /// or of the payload column a derived source projects for one.
    fn composite_source(
        &self,
        scope: ScopeId,
        reference_token: &Token<'_>,
    ) -> Result<CompositeSource, QueryDiagnostic> {
        let alias = self.source(scope).sql_alias.clone();
        let (_, field) = resolve_named_field(&self.source(scope).fields, reference_token)?;
        let missing_target = || {
            QueryDiagnostic::at(
                QueryDiagnosticKind::Metadata,
                Some(reference_token),
                format!(
                    "field {:?} has no unique SchemaStorage reference target",
                    reference_token.lexeme
                ),
            )
        };
        let declared = field
            .reference_targets
            .iter()
            .filter(|target| !target.is_empty())
            .cloned()
            .collect::<Vec<_>>();
        if let [column] = field.columns.as_slice() {
            let ColumnKind::Reference {
                targets,
                runtime_typed: true,
            } = &column.kind
            else {
                return Err(missing_target());
            };
            let payload = self
                .dialect
                .qualified_column(Some(&alias), &column.physical_name);
            return Ok(CompositeSource {
                schema_name: field.schema_name.clone(),
                value_sql: self.dialect.payload_reference(&payload),
                type_sql: self.dialect.payload_type(&payload),
                value_column: None,
                type_column: None,
                known_targets: targets.clone(),
                declared,
            });
        }
        let value_column =
            reference_column(field, reference_token).map_err(|_| missing_target())?;
        let type_column =
            reference_type_column(field, reference_token).map_err(|_| missing_target())?;
        // The value member knows the objects the reference can hold, so
        // the dereference reaches them directly instead of scanning the
        // snapshot for an attribute of that name, which never finds a
        // standard field.
        let known_targets = match &value_column.kind {
            ColumnKind::Reference { targets, .. } => targets.clone(),
            _ => Vec::new(),
        };
        Ok(CompositeSource {
            schema_name: field.schema_name.clone(),
            value_sql: self
                .dialect
                .qualified_column(Some(&alias), &value_column.physical_name),
            type_sql: self
                .dialect
                .qualified_column(Some(&alias), &type_column.physical_name),
            value_column: Some(value_column.physical_name.clone()),
            type_column: Some(type_column.physical_name.clone()),
            known_targets,
            declared,
        })
    }

    /// The objects a composite dereference may reach: the column's known
    /// targets, the field's declared targets, or the objects that define an
    /// attribute of this name.
    fn dereference_candidates(
        &self,
        source: &CompositeSource,
        target_token: &Token<'_>,
    ) -> Result<Vec<ObjectId>, QueryDiagnostic> {
        let mut candidates = Vec::new();
        if !source.known_targets.is_empty() {
            candidates.extend(source.known_targets.iter().copied());
        } else if !source.declared.is_empty() {
            for target in &source.declared {
                let physical = format!("_{}", target.strip_prefix('_').unwrap_or(target));
                let object = self
                    .snapshot
                    .objects()
                    .iter()
                    .find(|object| {
                        object
                            .physical_table
                            .as_deref()
                            .is_some_and(|table| names_equal(table, &physical))
                    })
                    .ok_or_else(|| {
                        QueryDiagnostic::at(
                            QueryDiagnosticKind::UnknownObject,
                            Some(target_token),
                            format!("reference target {physical:?} was not resolved"),
                        )
                    })?;
                candidates.push(ObjectId::from(&object.guid));
            }
        } else {
            candidates = self.scan_dereference_candidates(target_token)?;
        }
        candidates.dedup();
        if candidates.len() > Self::MAX_DEREFERENCE_TARGETS {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(target_token),
                format!(
                    "field {:?} is defined by more than {} reference targets; narrow the reference with ВЫРАЗИТЬ",
                    target_token.lexeme,
                    Self::MAX_DEREFERENCE_TARGETS
                ),
            ));
        }
        Ok(candidates)
    }

    /// Objects that may define the attribute: owners of a configuration
    /// attribute of that name and, for a standard field name, every
    /// reference-kind object.
    fn scan_dereference_candidates(
        &self,
        target_token: &Token<'_>,
    ) -> Result<Vec<ObjectId>, QueryDiagnostic> {
        let standard = is_standard_field_name(target_token.lexeme);
        let mut tables = Vec::new();
        for field in self.snapshot.fields() {
            if field
                .name
                .as_deref()
                .is_some_and(|name| names_equal(name, target_token.lexeme))
            {
                tables.extend(field.owner_tables.iter().cloned());
            }
        }
        let mut candidates = Vec::new();
        for object in self.snapshot.objects() {
            if !object.kind.is_some_and(is_reference_kind) {
                continue;
            }
            let Some(physical) = object.physical_table.as_deref() else {
                continue;
            };
            let owns = tables.iter().any(|table| names_equal(table, physical));
            if !owns && !standard {
                continue;
            }
            self.catalog.charge(1, Some(target_token))?;
            candidates.push(ObjectId::from(&object.guid));
            if candidates.len() > Self::MAX_DEREFERENCE_TARGETS {
                break;
            }
        }
        Ok(candidates)
    }

    /// Plans the guarded join of one candidate and describes its attribute,
    /// or `None` when the candidate does not define it.
    fn dereference_branch(
        &mut self,
        scope: ScopeId,
        source: &CompositeSource,
        candidate: ObjectId,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<Option<DereferenceBranch>, QueryDiagnostic> {
        let Some(object) = self.snapshot.object_by_id(candidate) else {
            return Ok(None);
        };
        let (Some(physical), Some(database_type)) = (object.physical_table.clone(), object.number)
        else {
            return Ok(None);
        };
        let Some(live_table) = self.snapshot.live_table(&physical) else {
            return Ok(None);
        };
        let fields = self.catalog.fields(object, Some(target_token))?;
        let matches = matching_fields(&fields, target_token);
        let (field_index, field) = match matches.as_slice() {
            [candidate] => *candidate,
            [] => return Ok(None),
            _ => {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::AmbiguousField,
                    Some(target_token),
                    format!(
                        "field {:?} is ambiguous in reference target {:?}",
                        target_token.lexeme, physical
                    ),
                ));
            }
        };
        let column = match field.columns.as_slice() {
            [column] => DereferenceMember::Single(column.clone()),
            _ => {
                let value = reference_column(field, target_token)?.clone();
                let member_type = reference_type_column(field, target_token)?.clone();
                DereferenceMember::Reference(member_type, value)
            }
        };
        let id_column = fields
            .iter()
            .find(|candidate| names_equal(&candidate.schema_name, "ID"))
            .and_then(|id| id.columns.first())
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Metadata,
                    Some(reference_token),
                    format!("reference target {physical:?} has no ID field"),
                )
            })?
            .physical_name
            .clone();
        let source_alias = self.source(scope).sql_alias.clone();
        let join_key = JoinKey {
            source_alias: &source_alias,
            source_field: &source.schema_name,
            target_object: candidate,
            database_type: Some(database_type),
        };
        let alias = if let Some(join) = self
            .source(scope)
            .reference_joins
            .iter()
            .find(|join| join.matches(join_key))
        {
            join.alias.clone()
        } else {
            let alias = self.next_reference_alias(scope);
            let target = compile_live_relation(
                self.snapshot,
                self.catalog,
                live_table,
                &fields,
                &alias,
                Some(reference_token),
                self.dialect,
            )?;
            self.source_mut(scope).reference_joins.push(JoinPlan {
                source_alias,
                source_field: source.schema_name.clone(),
                source_column: source.value_column.clone().unwrap_or_default(),
                source_type_column: source.type_column.clone(),
                database_type: Some(database_type),
                target_object: candidate,
                target_relation: target.sql,
                target_id_column: id_column,
                alias: alias.clone(),
                source_value_sql: source
                    .value_column
                    .is_none()
                    .then(|| source.value_sql.clone()),
                source_type_sql: source
                    .type_column
                    .is_none()
                    .then(|| source.type_sql.clone()),
                target_predicates: target.separators,
            });
            alias
        };
        let _ = field_index;
        Ok(Some(DereferenceBranch {
            database_type,
            alias,
            kind: column.kind(),
            member: column,
            field_name: field.name.clone(),
            schema_name: field.schema_name.clone(),
            physical_name: field
                .columns
                .first()
                .map(|column| column.physical_name.clone())
                .unwrap_or_default(),
            data_type: field
                .columns
                .first()
                .map(|column| column.data_type.clone())
                .unwrap_or_default(),
            reference_target: field.reference_target.clone(),
        }))
    }

    /// The value one branch contributes, widened to a payload when the
    /// branches disagree on the reference target.
    fn branch_value(
        &self,
        branch: &DereferenceBranch,
        widen: bool,
        target_token: &Token<'_>,
    ) -> Result<String, QueryDiagnostic> {
        match &branch.member {
            DereferenceMember::Single(column) => {
                let sql = self
                    .dialect
                    .qualified_column(Some(&branch.alias), &column.physical_name);
                if !widen {
                    return Ok(sql);
                }
                let target = branch.reference_target.as_deref().ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Metadata,
                        Some(target_token),
                        format!(
                            "reference field {:?} has no unique target to widen",
                            branch.field_name
                        ),
                    )
                })?;
                let physical = format!("_{}", target.strip_prefix('_').unwrap_or(target));
                let number = self
                    .snapshot
                    .objects()
                    .iter()
                    .find(|object| {
                        object
                            .physical_table
                            .as_deref()
                            .is_some_and(|table| names_equal(table, &physical))
                    })
                    .and_then(|object| object.number)
                    .ok_or_else(|| {
                        QueryDiagnostic::at(
                            QueryDiagnosticKind::Metadata,
                            Some(target_token),
                            format!("reference target {physical:?} has no database type"),
                        )
                    })?;
                Ok(self
                    .dialect
                    .reference_payload(&self.dialect.binary_u32(number), &sql))
            }
            DereferenceMember::Reference(type_column, value_column) => {
                let type_sql = self
                    .dialect
                    .qualified_column(Some(&branch.alias), &type_column.physical_name);
                let value_sql = self
                    .dialect
                    .qualified_column(Some(&branch.alias), &value_column.physical_name);
                Ok(self.dialect.reference_payload(&type_sql, &value_sql))
            }
        }
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
        if let Some(expression) = &resolved.expression {
            return expression.clone();
        }
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
        let target_relation = compile_live_relation(
            self.snapshot,
            self.catalog,
            target_table,
            &target_fields,
            &alias,
            Some(token),
            self.dialect,
        )?;
        self.source_mut(scope).reference_joins.push(JoinPlan {
            source_alias: source_alias.to_owned(),
            source_field: reference.schema_name.clone(),
            source_column,
            source_type_column,
            database_type,
            target_object: target,
            target_relation: target_relation.sql,
            target_id_column,
            alias: alias.clone(),
            source_value_sql: None,
            source_type_sql: None,
            target_predicates: target_relation.separators,
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
    if resolved.expression.is_some() {
        return Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(token),
            "a value dereferenced across reference targets cannot be presented; narrow the reference with ВЫРАЗИТЬ",
        ));
    }
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

/// The reference side of a composite dereference.
struct CompositeSource {
    schema_name: String,
    value_sql: String,
    type_sql: String,
    value_column: Option<String>,
    type_column: Option<String>,
    known_targets: Vec<ObjectId>,
    declared: Vec<String>,
}

/// The attribute members one candidate contributes.
enum DereferenceMember {
    Single(QueryableColumn),
    Reference(QueryableColumn, QueryableColumn),
}

impl DereferenceMember {
    fn kind(&self) -> ColumnKind {
        match self {
            Self::Single(column) => column.kind.clone(),
            Self::Reference(_, value) => match &value.kind {
                ColumnKind::Reference { targets, .. } => ColumnKind::Reference {
                    targets: targets.clone(),
                    runtime_typed: true,
                },
                other => other.clone(),
            },
        }
    }
}

struct DereferenceBranch {
    database_type: u32,
    alias: String,
    member: DereferenceMember,
    kind: ColumnKind,
    field_name: String,
    schema_name: String,
    physical_name: String,
    data_type: String,
    reference_target: Option<String>,
}

fn is_reference_kind(kind: MetadataKind) -> bool {
    matches!(
        kind,
        MetadataKind::Catalog
            | MetadataKind::Document
            | MetadataKind::Enumeration
            | MetadataKind::ChartOfCharacteristicTypes
            | MetadataKind::ChartOfCalculationTypes
            | MetadataKind::ChartOfAccounts
            | MetadataKind::ExchangePlan
            | MetadataKind::BusinessProcess
            | MetadataKind::Task
    )
}

/// The kind every branch of a composite dereference must agree on:
/// the same variant, the widest string, and references widened to one
/// runtime-typed payload when the targets differ.
fn unify_dereference_kinds(
    branches: &[DereferenceBranch],
    token: &Token<'_>,
) -> Result<ColumnKind, QueryDiagnostic> {
    let mut common: Option<ColumnKind> = None;
    for branch in branches {
        let kind = branch.kind.clone();
        common = Some(match common {
            None => kind,
            Some(current) if current.is_wildcard() => kind,
            Some(current) if kind.is_wildcard() => current,
            Some(current) => match (&current, &kind) {
                (ColumnKind::String { length: left }, ColumnKind::String { length: right }) => {
                    ColumnKind::String {
                        length: match (left, right) {
                            (Some(left), Some(right)) => Some(*left.max(right)),
                            _ => None,
                        },
                    }
                }
                (ColumnKind::Number { .. }, ColumnKind::Number { .. }) => ColumnKind::Number {
                    precision: None,
                    scale: None,
                },
                (ColumnKind::Binary { length: left }, ColumnKind::Binary { length: right }) => {
                    ColumnKind::Binary {
                        length: match (left, right) {
                            (Some(left), Some(right)) => Some(*left.max(right)),
                            _ => None,
                        },
                    }
                }
                (
                    ColumnKind::Reference {
                        targets: left,
                        runtime_typed: left_runtime,
                    },
                    ColumnKind::Reference {
                        targets: right,
                        runtime_typed: right_runtime,
                    },
                ) => {
                    if left == right && left_runtime == right_runtime {
                        current.clone()
                    } else {
                        let mut targets = left.clone();
                        for target in right {
                            if !targets.contains(target) {
                                targets.push(*target);
                            }
                        }
                        ColumnKind::Reference {
                            targets,
                            runtime_typed: true,
                        }
                    }
                }
                (left, right) if left == right => current.clone(),
                (left, right) => {
                    return Err(QueryDiagnostic::at(
                        QueryDiagnosticKind::UnsupportedFeature,
                        Some(token),
                        format!(
                            "field {:?} is {left:?} in one reference target and {right:?} in another",
                            token.lexeme
                        ),
                    ));
                }
            },
        });
    }
    common.ok_or_else(|| {
        QueryDiagnostic::at(
            QueryDiagnosticKind::UnknownField,
            Some(token),
            format!(
                "field {:?} was not found in any reference target",
                token.lexeme
            ),
        )
    })
}
