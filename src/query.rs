//! Bounded compilation of 1C SELECT queries through resolved metadata.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::metadata::{
    ConfigFieldPurpose, FieldId, LiveColumn, LiveTable, MetadataKind, MetadataObject,
    MetadataSnapshot, ObjectId, SchemaColumn, recase_postgres_identifier,
};
use crate::{Diagnostic, Keyword, Token, TokenKind, tokenize};

/// One physical PostgreSQL member of a queryable logical field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryableColumn {
    /// Exact catalog identifier used for SQL generation.
    pub physical_name: String,
    /// PostgreSQL catalog type name.
    pub data_type: String,
    /// Stable output label used when projecting this member.
    pub output_label: String,
}

/// One logical 1C field and all physical members implementing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryableField {
    /// Human-facing field name, preferring Config metadata names.
    pub name: String,
    /// Canonical schema field name such as `Code`, `ID`, or `Fld54`.
    pub schema_name: String,
    /// Names accepted by the bounded compiler.
    pub aliases: Vec<String>,
    /// Exact live physical members.
    pub columns: Vec<QueryableColumn>,
    /// Unique canonical SchemaStorage target table for an `R` field.
    pub reference_target: Option<String>,
    /// Every canonical SchemaStorage target table for a pure reference field.
    pub reference_targets: Vec<String>,
}

/// One safe node in an application-defined reference-presentation template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationExpression {
    /// A field owned by the target metadata object.
    Field(FieldId),
    /// Literal text. The core performs PostgreSQL quoting.
    Literal(String),
    /// Concatenation evaluated by PostgreSQL.
    Concat(Vec<Self>),
}

/// Application policy for presenting one reference target type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationPlan {
    /// Target object whose rows the plan can present.
    pub object: ObjectId,
    /// Fields explicitly authorized for this plan.
    pub fields: Vec<FieldId>,
    /// Safe structured template; raw SQL is intentionally impossible.
    pub expression: PresentationExpression,
}

/// One target requested by the core before SQL generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PresentationTarget {
    /// Real metadata GUID of the possible reference target.
    pub object: ObjectId,
}

/// Deduplicated batch callback request for application presentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRequest {
    /// Possible reference targets in stable GUID order.
    pub targets: Vec<PresentationTarget>,
}

/// Parsed and metadata-resolved query waiting for application presentation plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPostgresQuery {
    source: String,
    request: PresentationRequest,
}

impl PreparedPostgresQuery {
    /// Returns the batch callback request. It is empty when the query uses no
    /// reference presentation.
    #[must_use]
    pub fn presentation_request(&self) -> &PresentationRequest {
        &self.request
    }

    /// Finishes SQL generation with application-provided presentation plans.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic for a missing, duplicate, foreign-field, or
    /// structurally invalid plan.
    pub fn compile(
        &self,
        snapshot: &MetadataSnapshot,
        plans: &[PresentationPlan],
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        compile_postgres_query_with_presentations(&self.source, snapshot, plans)
    }
}

/// PostgreSQL text generated from one bounded 1C query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledQuery {
    /// SELECT-only PostgreSQL statement.
    pub sql: String,
    /// Output labels in statement order.
    pub columns: Vec<String>,
}

/// A positional query parsing or metadata-resolution failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDiagnostic {
    message: String,
    offset: usize,
    line: usize,
    column: usize,
}

impl QueryDiagnostic {
    fn at(token: Option<&Token<'_>>, message: impl Into<String>) -> Self {
        let (offset, line, column) = token.map_or((0, 1, 1), |token| {
            (token.span.start, token.span.line, token.span.column)
        });
        Self {
            message: message.into(),
            offset,
            line,
            column,
        }
    }

    fn metadata(message: impl Into<String>) -> Self {
        Self::at(None, message)
    }

    /// Returns the diagnostic text without its source position.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the zero-based byte offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the one-based source line.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the one-based source column.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }
}

impl fmt::Display for QueryDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for QueryDiagnostic {}

impl From<Diagnostic> for QueryDiagnostic {
    fn from(error: Diagnostic) -> Self {
        let prefix = format!("{}:{}: ", error.line, error.column);
        let rendered = error.to_string();
        Self {
            message: rendered
                .strip_prefix(&prefix)
                .unwrap_or(&rendered)
                .to_owned(),
            offset: error.offset,
            line: error.line,
            column: error.column,
        }
    }
}

/// Finds a tabular metadata object by qualified name, unique bare name, or
/// canonical physical table name.
///
/// # Errors
///
/// Returns a diagnostic when the name is missing, ambiguous, non-tabular, or
/// has an unknown kind qualifier.
pub fn find_metadata_object<'snapshot>(
    snapshot: &'snapshot MetadataSnapshot,
    name: &str,
) -> Result<&'snapshot MetadataObject, QueryDiagnostic> {
    let name = name.trim();
    if name.is_empty() {
        return Err(QueryDiagnostic::metadata("metadata name is empty"));
    }

    let (kind, object_name) = if let Some((kind, object_name)) = name.split_once('.') {
        let kind = kind_from_query_name(kind)
            .ok_or_else(|| QueryDiagnostic::metadata(format!("unknown metadata kind {kind:?}")))?;
        (Some(kind), object_name)
    } else {
        (None, name)
    };

    let matches: Vec<&MetadataObject> = snapshot
        .objects
        .iter()
        .filter(|object| object.kind.is_some() && object.physical_table.is_some())
        .filter(|object| kind.is_none_or(|kind| object.kind == Some(kind)))
        .filter(|object| {
            object
                .name
                .as_deref()
                .is_some_and(|candidate| names_equal(candidate, object_name))
                || (kind.is_none()
                    && object
                        .physical_table
                        .as_deref()
                        .is_some_and(|candidate| names_equal(candidate, object_name)))
        })
        .collect();

    match matches.as_slice() {
        [object] => Ok(*object),
        [] => Err(QueryDiagnostic::metadata(format!(
            "metadata object {name:?} was not found"
        ))),
        _ => Err(QueryDiagnostic::metadata(format!(
            "metadata object name {name:?} is ambiguous; use <kind>.<name>"
        ))),
    }
}

/// Builds queryable logical fields for one resolved live object.
///
/// # Errors
///
/// Returns a diagnostic when the object has no live physical table.
pub fn queryable_fields(
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
) -> Result<Vec<QueryableField>, QueryDiagnostic> {
    let physical_table = object
        .physical_table
        .as_deref()
        .ok_or_else(|| QueryDiagnostic::metadata("metadata object has no physical table"))?;
    let table = snapshot
        .live_tables
        .iter()
        .find(|table| table.name.eq_ignore_ascii_case(physical_table))
        .ok_or_else(|| {
            QueryDiagnostic::metadata(format!("physical table {physical_table} is not live"))
        })?;
    let schema_table = snapshot.schema.table(physical_table);

    Ok(project_queryable_fields(
        physical_table,
        table,
        schema_table,
        &CustomFieldNames::Scan(snapshot),
    ))
}

/// Builds queryable fields for every currently live object in one indexed
/// pass over the snapshot's current public vectors.
///
/// Objects without a live physical table are omitted. Rebuilding the catalog
/// after changing the snapshot reflects those changes.
#[must_use]
pub fn queryable_field_catalog(
    snapshot: &MetadataSnapshot,
) -> HashMap<ObjectId, Vec<QueryableField>> {
    let custom_names = index_custom_field_names(snapshot);
    let mut live_tables = HashMap::with_capacity(snapshot.live_tables.len());
    for table in &snapshot.live_tables {
        live_tables
            .entry(table.name.to_ascii_lowercase())
            .or_insert(table);
    }
    let mut schema_tables = HashMap::with_capacity(snapshot.schema.tables.len());
    for table in &snapshot.schema.tables {
        schema_tables.entry(table.name.as_str()).or_insert(table);
    }

    let mut catalog = HashMap::new();
    for object in &snapshot.objects {
        let Some(physical_table) = object.physical_table.as_deref() else {
            continue;
        };
        let Some(table) = live_tables.get(&physical_table.to_ascii_lowercase()) else {
            continue;
        };
        let schema_table = schema_tables
            .get(physical_table.strip_prefix('_').unwrap_or(physical_table))
            .copied();
        catalog
            .entry(ObjectId::from(&object.guid))
            .or_insert_with(|| {
                project_queryable_fields(
                    physical_table,
                    table,
                    schema_table,
                    &CustomFieldNames::Indexed(&custom_names),
                )
            });
    }
    catalog
}

type CustomFieldNameIndex = HashMap<(String, u32), Option<String>>;

enum CustomFieldNames<'snapshot> {
    Scan(&'snapshot MetadataSnapshot),
    Indexed(&'snapshot CustomFieldNameIndex),
}

fn index_custom_field_names(snapshot: &MetadataSnapshot) -> CustomFieldNameIndex {
    let mut names = HashMap::new();
    for field in &snapshot.fields {
        for owner in &field.owner_tables {
            names
                .entry((owner.to_lowercase(), field.number))
                .or_insert_with(|| field.name.clone());
        }
    }
    names
}

fn project_queryable_fields(
    physical_table: &str,
    table: &LiveTable,
    schema_table: Option<&crate::metadata::SchemaTable>,
    custom_names: &CustomFieldNames<'_>,
) -> Vec<QueryableField> {
    let mut schema_columns = BTreeMap::new();
    if let Some(table) = schema_table {
        for column in &table.columns {
            let canonical = logical_column_name(&column.physical_name());
            schema_columns
                .entry(canonical.to_lowercase())
                .or_insert((canonical, column));
        }
    }

    let mut order = Vec::<String>::new();
    let mut groups = BTreeMap::<String, Vec<&LiveColumn>>::new();
    for column in &table.columns {
        let observed_name = logical_column_name(&column.name);
        let schema_name = schema_columns
            .get(&observed_name.to_lowercase())
            .map_or(observed_name, |(canonical, _)| canonical.clone());
        if !groups.contains_key(&schema_name) {
            order.push(schema_name.clone());
        }
        groups.entry(schema_name).or_default().push(column);
    }

    order
        .into_iter()
        .map(|schema_name| {
            let columns = groups.remove(&schema_name).unwrap_or_default();
            let reference_targets = schema_columns
                .get(&schema_name.to_lowercase())
                .map(|(_, column)| *column)
                .map(reference_targets)
                .unwrap_or_default();
            let reference_target =
                (reference_targets.len() == 1).then(|| reference_targets[0].clone());
            let query_schema_name = query_schema_name(&schema_name);
            let custom_name = match custom_names {
                CustomFieldNames::Scan(snapshot) => {
                    custom_field_name(snapshot, physical_table, &schema_name)
                }
                CustomFieldNames::Indexed(names) => {
                    indexed_custom_field_name(names, physical_table, &schema_name)
                }
            };
            let name = custom_name.unwrap_or_else(|| query_schema_name.clone());
            let mut aliases = standard_field_aliases(&query_schema_name)
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect::<Vec<_>>();
            push_unique_name(&mut aliases, query_schema_name.clone());
            push_unique_name(&mut aliases, schema_name.clone());
            push_unique_name(&mut aliases, name.clone());
            let compound = columns.len() > 1;
            let columns = columns
                .into_iter()
                .map(|column| QueryableColumn {
                    output_label: if compound {
                        compound_label(&name, &schema_name, &column.name)
                    } else {
                        name.clone()
                    },
                    physical_name: column.name.clone(),
                    data_type: column.data_type.clone(),
                })
                .collect();
            QueryableField {
                name,
                schema_name: query_schema_name,
                aliases,
                columns,
                reference_target,
                reference_targets,
            }
        })
        .collect()
}

/// Compiles one bounded 1C SELECT query to PostgreSQL.
///
/// # Errors
///
/// Returns a positional diagnostic for lexical, syntactic, unsupported, or
/// metadata-resolution failures. No partial SQL is returned.
pub fn compile_postgres_query(
    source: &str,
    snapshot: &MetadataSnapshot,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens).parse()?;
    let mut presentations = PresentationCompilation::strict(&[]);
    compile(ast, snapshot, &mut presentations)
}

/// Parses and resolves a query and returns its compile-time presentation
/// callback request.
///
/// # Errors
///
/// Returns a positional diagnostic when the query cannot be safely resolved.
pub fn prepare_postgres_query(
    source: &str,
    snapshot: &MetadataSnapshot,
) -> Result<PreparedPostgresQuery, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens).parse()?;
    let mut presentations = PresentationCompilation::collect();
    let _ = compile(ast, snapshot, &mut presentations)?;
    Ok(PreparedPostgresQuery {
        source: source.to_owned(),
        request: PresentationRequest {
            targets: presentations
                .requested
                .into_iter()
                .map(|object| PresentationTarget { object })
                .collect(),
        },
    })
}

/// Compiles a query with plans returned by the application's compile-time
/// presentation callback.
///
/// # Errors
///
/// Returns a positional diagnostic when syntax, metadata, or any plan is
/// invalid. No partial SQL is returned.
pub fn compile_postgres_query_with_presentations(
    source: &str,
    snapshot: &MetadataSnapshot,
    plans: &[PresentationPlan],
) -> Result<CompiledQuery, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens).parse()?;
    let mut presentations = PresentationCompilation::strict(plans);
    compile(ast, snapshot, &mut presentations)
}

#[derive(Debug)]
struct QueryAst<'tokens, 'source> {
    branches: Vec<SelectAst<'tokens, 'source>>,
    unions: Vec<UnionLink<'tokens, 'source>>,
    order: Vec<OrderTerm<'tokens, 'source>>,
}

#[derive(Debug)]
struct UnionLink<'tokens, 'source> {
    token: &'tokens Token<'source>,
    all: bool,
}

#[derive(Debug)]
struct SelectAst<'tokens, 'source> {
    distinct: bool,
    top: Option<u32>,
    projection: Vec<Projection<'tokens, 'source>>,
    source: Option<SourceAst<'tokens, 'source>>,
    join: Option<JoinAst<'tokens, 'source>>,
    filter: Option<Expression<'tokens, 'source>>,
}

#[derive(Debug)]
struct SourceAst<'tokens, 'source> {
    kind: &'tokens Token<'source>,
    object: &'tokens Token<'source>,
    slice: Option<SliceAst<'tokens, 'source>>,
    accumulation: Option<AccumulationAst<'tokens, 'source>>,
    alias: Option<&'tokens Token<'source>>,
}

#[derive(Debug)]
struct SliceAst<'tokens, 'source> {
    token: &'tokens Token<'source>,
    kind: SliceKind,
    period: Option<Expression<'tokens, 'source>>,
    condition: Option<Expression<'tokens, 'source>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliceKind {
    First,
    Last,
}

impl SliceKind {
    const fn name(self) -> &'static str {
        match self {
            Self::First => "SliceFirst",
            Self::Last => "SliceLast",
        }
    }

    const fn period_operator(self) -> &'static str {
        match self {
            Self::First => ">=",
            Self::Last => "<=",
        }
    }

    const fn order(self) -> &'static str {
        match self {
            Self::First => "ASC",
            Self::Last => "DESC",
        }
    }
}

#[derive(Debug)]
struct AccumulationAst<'tokens, 'source> {
    token: &'tokens Token<'source>,
    kind: AccumulationKind,
    arguments: Vec<Option<Expression<'tokens, 'source>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccumulationKind {
    Balance,
    Turnovers,
}

impl AccumulationKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Balance => "Balance",
            Self::Turnovers => "Turnovers",
        }
    }

    const fn resource_suffix(self) -> (&'static str, &'static str) {
        match self {
            Self::Balance => ("Остаток", "Balance"),
            Self::Turnovers => ("Оборот", "Turnover"),
        }
    }
}

#[derive(Debug)]
struct JoinAst<'tokens, 'source> {
    token: &'tokens Token<'source>,
    kind: JoinKind,
    source: SourceAst<'tokens, 'source>,
    condition: Expression<'tokens, 'source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug)]
enum Projection<'tokens, 'source> {
    All,
    Field(FieldReference<'tokens, 'source>),
    Scalar(Expression<'tokens, 'source>),
    Aggregate {
        token: &'tokens Token<'source>,
        kind: AggregateKind,
        distinct: bool,
        argument: AggregateArgument<'tokens, 'source>,
    },
    Presentation {
        token: &'tokens Token<'source>,
        operation: PresentationOperation,
        argument: PresentationArgument<'tokens, 'source>,
    },
}

#[derive(Debug)]
enum AggregateArgument<'tokens, 'source> {
    All,
    Field(FieldReference<'tokens, 'source>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateKind {
    Count,
    Sum,
    Min,
    Max,
}

impl AggregateKind {
    const fn sql_name(self) -> &'static str {
        match self {
            Self::Count => "COUNT",
            Self::Sum => "SUM",
            Self::Min => "MIN",
            Self::Max => "MAX",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PresentationOperation {
    Reference,
    String,
    Property,
}

#[derive(Debug)]
enum PresentationArgument<'tokens, 'source> {
    Field(FieldReference<'tokens, 'source>),
    Literal(&'tokens Token<'source>),
}

#[derive(Debug, Clone)]
struct FieldReference<'tokens, 'source> {
    segments: Vec<&'tokens Token<'source>>,
}

impl<'tokens, 'source> FieldReference<'tokens, 'source> {
    fn last(&self) -> &'tokens Token<'source> {
        self.segments
            .last()
            .copied()
            .expect("field path is non-empty")
    }
}

#[derive(Debug)]
struct OrderTerm<'tokens, 'source> {
    field: FieldReference<'tokens, 'source>,
    descending: bool,
}

#[derive(Debug)]
enum Expression<'tokens, 'source> {
    Field(FieldReference<'tokens, 'source>),
    Literal(&'tokens Token<'source>),
    Unary {
        operator: &'tokens Token<'source>,
        value: Box<Self>,
    },
    Binary {
        left: Box<Self>,
        operator: &'tokens Token<'source>,
        right: Box<Self>,
    },
    IsNull {
        value: Box<Self>,
        negated: bool,
    },
}

struct Parser<'tokens, 'source> {
    tokens: &'tokens [Token<'source>],
    offset: usize,
}

impl<'tokens, 'source> Parser<'tokens, 'source> {
    const fn new(tokens: &'tokens [Token<'source>]) -> Self {
        Self { tokens, offset: 0 }
    }

    fn parse(mut self) -> Result<QueryAst<'tokens, 'source>, QueryDiagnostic> {
        let mut branches = vec![self.parse_select()?];
        let mut unions = Vec::new();
        while let Some(token) = self.consume_keyword_token(Keyword::Union) {
            unions.push(UnionLink {
                token,
                all: self.consume_keyword(Keyword::All),
            });
            branches.push(self.parse_select()?);
        }
        let order = self.parse_order()?;
        while self.consume_lexeme(";") {}
        if let Some(token) = self.peek() {
            return Err(QueryDiagnostic::at(
                Some(token),
                format!("unsupported query syntax starting at {:?}", token.lexeme),
            ));
        }
        Ok(QueryAst {
            branches,
            unions,
            order,
        })
    }

    fn parse_select(&mut self) -> Result<SelectAst<'tokens, 'source>, QueryDiagnostic> {
        self.expect_keyword(Keyword::Select)?;
        let distinct = self.consume_keyword(Keyword::Distinct);
        let top = if self.consume_keyword(Keyword::Top) {
            let token = self.expect_kind(TokenKind::Number, "expected TOP row count")?;
            let value = token.lexeme.parse::<u32>().map_err(|_| {
                QueryDiagnostic::at(Some(token), "TOP row count must be an integer")
            })?;
            if value == 0 {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "TOP row count must be greater than zero",
                ));
            }
            Some(value)
        } else {
            None
        };

        let mut projection = Vec::new();
        loop {
            if self.consume_lexeme("*") {
                projection.push(Projection::All);
            } else {
                projection.push(self.parse_projection()?);
            }
            if !self.consume_lexeme(",") {
                break;
            }
        }
        let source = if self.consume_keyword(Keyword::From) {
            Some(self.parse_source()?)
        } else {
            None
        };
        let join = if source.is_some() {
            self.parse_join()?
        } else {
            None
        };
        let filter = if self.consume_keyword(Keyword::Where) {
            Some(self.parse_or()?)
        } else {
            None
        };

        Ok(SelectAst {
            distinct,
            top,
            projection,
            source,
            join,
            filter,
        })
    }

    fn parse_projection(&mut self) -> Result<Projection<'tokens, 'source>, QueryDiagnostic> {
        let aggregate = if self.next_lexeme_is("(") {
            self.consume_keyword_token(Keyword::Count)
                .map(|token| (token, AggregateKind::Count))
                .or_else(|| {
                    self.consume_keyword_token(Keyword::Sum)
                        .map(|token| (token, AggregateKind::Sum))
                })
                .or_else(|| {
                    self.consume_keyword_token(Keyword::Min)
                        .map(|token| (token, AggregateKind::Min))
                })
                .or_else(|| {
                    self.consume_keyword_token(Keyword::Max)
                        .map(|token| (token, AggregateKind::Max))
                })
        } else {
            None
        };
        if let Some((token, kind)) = aggregate {
            self.expect_lexeme("(")?;
            let distinct = self.consume_keyword(Keyword::Distinct);
            if distinct && kind != AggregateKind::Count {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "DISTINCT aggregate argument is currently supported only by COUNT",
                ));
            }
            let argument = if self.consume_lexeme("*") {
                if kind != AggregateKind::Count {
                    return Err(QueryDiagnostic::at(
                        Some(token),
                        "wildcard aggregate argument is supported only by COUNT",
                    ));
                }
                if distinct {
                    return Err(QueryDiagnostic::at(
                        Some(token),
                        "COUNT(DISTINCT *) is not supported",
                    ));
                }
                AggregateArgument::All
            } else {
                AggregateArgument::Field(self.parse_field_reference()?)
            };
            self.expect_lexeme(")")?;
            return Ok(Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            });
        }
        let function = if self.next_lexeme_is("(") {
            self.consume_keyword_token(Keyword::RefPresentation)
                .map(|token| (token, PresentationOperation::Reference))
                .or_else(|| {
                    self.consume_keyword_token(Keyword::Presentation)
                        .map(|token| (token, PresentationOperation::String))
                })
        } else {
            None
        };
        if let Some((token, operation)) = function {
            self.expect_lexeme("(")?;
            let argument = match self.peek() {
                Some(value)
                    if matches!(value.kind, TokenKind::String | TokenKind::Number)
                        || matches!(
                            value.kind,
                            TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::Null)
                        ) =>
                {
                    PresentationArgument::Literal(self.next().expect("peeked token"))
                }
                _ => PresentationArgument::Field(self.parse_field_reference()?),
            };
            self.expect_lexeme(")")?;
            return Ok(Projection::Presentation {
                token,
                operation,
                argument,
            });
        }

        let expression = self.parse_or()?;
        match expression {
            Expression::Field(mut reference) => {
                if reference.segments.len() > 1
                    && reference.segments.last().is_some_and(|token| {
                        token.kind == TokenKind::Keyword(Keyword::Presentation)
                    })
                {
                    let token = reference.segments.pop().expect("checked last segment");
                    return Ok(Projection::Presentation {
                        token,
                        operation: PresentationOperation::Property,
                        argument: PresentationArgument::Field(reference),
                    });
                }
                Ok(Projection::Field(reference))
            }
            expression => Ok(Projection::Scalar(expression)),
        }
    }

    fn parse_join(&mut self) -> Result<Option<JoinAst<'tokens, 'source>>, QueryDiagnostic> {
        let (token, kind) = if let Some(token) = self.consume_keyword_token(Keyword::Inner) {
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Inner)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Left) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Left)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Right) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Right)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Full) {
            self.consume_keyword(Keyword::Outer);
            self.expect_keyword(Keyword::Join)?;
            (token, JoinKind::Full)
        } else if let Some(token) = self.consume_keyword_token(Keyword::Join) {
            (token, JoinKind::Inner)
        } else {
            return Ok(None);
        };
        let source = self.parse_source()?;
        if !self.consume_keyword(Keyword::On) && !self.consume_keyword(Keyword::By) {
            return Err(QueryDiagnostic::at(
                self.peek(),
                "expected ON or ПО after JOIN source",
            ));
        }
        Ok(Some(JoinAst {
            token,
            kind,
            source,
            condition: self.parse_or()?,
        }))
    }

    fn parse_source(&mut self) -> Result<SourceAst<'tokens, 'source>, QueryDiagnostic> {
        let kind = self.expect_identifier("expected metadata kind after FROM")?;
        self.expect_lexeme(".")?;
        let object = self.expect_identifier("expected metadata object name")?;
        let (slice, accumulation) = if self.consume_lexeme(".") {
            if let Some(token) = self.consume_keyword_token(Keyword::SliceLast) {
                (Some(self.parse_slice(token, SliceKind::Last)?), None)
            } else if let Some(token) = self.consume_keyword_token(Keyword::SliceFirst) {
                (Some(self.parse_slice(token, SliceKind::First)?), None)
            } else if let Some(token) = self.consume_keyword_token(Keyword::Balance) {
                (
                    None,
                    Some(AccumulationAst {
                        token,
                        kind: AccumulationKind::Balance,
                        arguments: self.parse_virtual_arguments(2, "Balance")?,
                    }),
                )
            } else if let Some(token) = self.consume_keyword_token(Keyword::Turnovers) {
                (
                    None,
                    Some(AccumulationAst {
                        token,
                        kind: AccumulationKind::Turnovers,
                        arguments: self.parse_virtual_arguments(4, "Turnovers")?,
                    }),
                )
            } else {
                return Err(QueryDiagnostic::at(
                    self.peek(),
                    "expected a supported virtual table after metadata object",
                ));
            }
        } else {
            (None, None)
        };
        let alias = if self.consume_keyword(Keyword::As) {
            Some(self.expect_identifier("expected source alias after AS")?)
        } else if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            self.next()
        } else {
            None
        };
        Ok(SourceAst {
            kind,
            object,
            slice,
            accumulation,
            alias,
        })
    }

    fn parse_slice(
        &mut self,
        token: &'tokens Token<'source>,
        kind: SliceKind,
    ) -> Result<SliceAst<'tokens, 'source>, QueryDiagnostic> {
        let mut arguments = self.parse_virtual_arguments(2, kind.name())?.into_iter();
        Ok(SliceAst {
            token,
            kind,
            period: arguments.next().flatten(),
            condition: arguments.next().flatten(),
        })
    }

    fn parse_virtual_arguments(
        &mut self,
        maximum: usize,
        name: &str,
    ) -> Result<Vec<Option<Expression<'tokens, 'source>>>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let mut arguments = Vec::new();
        if self.consume_lexeme(")") {
            return Ok(arguments);
        }
        loop {
            if arguments.len() == maximum {
                return Err(QueryDiagnostic::at(
                    self.peek(),
                    format!("{name} accepts at most {maximum} arguments"),
                ));
            }
            let argument = if self
                .peek()
                .is_some_and(|token| matches!(token.lexeme, "," | ")"))
            {
                None
            } else {
                Some(self.parse_or()?)
            };
            arguments.push(argument);
            if self.consume_lexeme(",") {
                continue;
            }
            self.expect_lexeme(")")?;
            break;
        }
        Ok(arguments)
    }

    fn parse_order(&mut self) -> Result<Vec<OrderTerm<'tokens, 'source>>, QueryDiagnostic> {
        let mut order = Vec::new();
        if self.consume_keyword(Keyword::Order) {
            self.expect_keyword(Keyword::By)?;
            loop {
                let field = self.parse_field_reference()?;
                let descending = self.peek().is_some_and(|token| {
                    names_equal(token.lexeme, "DESC") || names_equal(token.lexeme, "УБЫВ")
                });
                if descending || self.peek().is_some_and(is_ascending_order) {
                    self.offset += 1;
                }
                order.push(OrderTerm { field, descending });
                if !self.consume_lexeme(",") {
                    break;
                }
            }
        }
        Ok(order)
    }

    fn parse_or(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_and()?;
        while let Some(operator) = self.consume_keyword_token(Keyword::Or) {
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_and()?),
            };
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_comparison()?;
        while let Some(operator) = self.consume_keyword_token(Keyword::And) {
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_comparison()?),
            };
        }
        Ok(expression)
    }

    fn parse_comparison(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_additive()?;
        if let Some(is) = self.consume_keyword_token(Keyword::Is) {
            let negated = self.consume_keyword(Keyword::Not);
            if !self.consume_keyword(Keyword::Null) {
                return Err(QueryDiagnostic::at(
                    self.peek().or(Some(is)),
                    "IS only supports NULL in this query subset",
                ));
            }
            return Ok(Expression::IsNull {
                value: Box::new(expression),
                negated,
            });
        }
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Operator && is_comparison(token.lexeme))
        {
            let operator = self.next().expect("peeked token");
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_additive()?),
            };
        }
        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_multiplicative()?;
        while self
            .peek()
            .is_some_and(|token| matches!(token.lexeme, "+" | "-"))
        {
            let operator = self.next().expect("peeked token");
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_multiplicative()?),
            };
        }
        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut expression = self.parse_unary()?;
        while self
            .peek()
            .is_some_and(|token| matches!(token.lexeme, "*" | "/"))
        {
            let operator = self.next().expect("peeked token");
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_unary()?),
            };
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if let Some(operator) = self.consume_keyword_token(Keyword::Not) {
            return Ok(Expression::Unary {
                operator,
                value: Box::new(self.parse_unary()?),
            });
        }
        if self
            .peek()
            .is_some_and(|token| matches!(token.lexeme, "+" | "-"))
        {
            let operator = self.next().expect("peeked token");
            return Ok(Expression::Unary {
                operator,
                value: Box::new(self.parse_unary()?),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if self.consume_lexeme("(") {
            let expression = self.parse_or()?;
            self.expect_lexeme(")")?;
            return Ok(expression);
        }
        let Some(token) = self.peek() else {
            return Err(QueryDiagnostic::at(None, "expected expression"));
        };
        if token.kind == TokenKind::Parameter {
            return Err(QueryDiagnostic::at(
                Some(token),
                "query parameters are not supported by this REPL",
            ));
        }
        if matches!(token.kind, TokenKind::String | TokenKind::Number)
            || matches!(
                token.kind,
                TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::Null)
            )
        {
            return Ok(Expression::Literal(self.next().expect("peeked token")));
        }
        Ok(Expression::Field(self.parse_field_reference()?))
    }

    fn parse_field_reference(
        &mut self,
    ) -> Result<FieldReference<'tokens, 'source>, QueryDiagnostic> {
        let mut segments = vec![self.expect_identifier("expected field name")?];
        while self.consume_lexeme(".") {
            let token = self
                .peek()
                .ok_or_else(|| QueryDiagnostic::at(None, "expected field name after '.'"))?;
            if !is_contextual_identifier(token.kind) {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "expected field name after '.'",
                ));
            }
            segments.push(self.next().expect("peeked token"));
        }
        Ok(FieldReference { segments })
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), QueryDiagnostic> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(QueryDiagnostic::at(
                self.peek(),
                format!("expected {}", keyword.as_str()),
            ))
        }
    }

    fn expect_identifier(
        &mut self,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| QueryDiagnostic::at(None, message))?;
        if !is_contextual_identifier(token.kind) {
            return Err(QueryDiagnostic::at(Some(token), message));
        }
        Ok(self.next().expect("peeked token"))
    }

    fn expect_kind(
        &mut self,
        kind: TokenKind,
        message: &'static str,
    ) -> Result<&'tokens Token<'source>, QueryDiagnostic> {
        let token = self
            .peek()
            .ok_or_else(|| QueryDiagnostic::at(None, message))?;
        if token.kind != kind {
            return Err(QueryDiagnostic::at(Some(token), message));
        }
        Ok(self.next().expect("peeked token"))
    }

    fn expect_lexeme(&mut self, lexeme: &str) -> Result<(), QueryDiagnostic> {
        if self.consume_lexeme(lexeme) {
            Ok(())
        } else {
            Err(QueryDiagnostic::at(
                self.peek(),
                format!("expected {lexeme:?}"),
            ))
        }
    }

    fn consume_keyword(&mut self, keyword: Keyword) -> bool {
        self.consume_keyword_token(keyword).is_some()
    }

    fn consume_keyword_token(&mut self, keyword: Keyword) -> Option<&'tokens Token<'source>> {
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(keyword))
        {
            self.next()
        } else {
            None
        }
    }

    fn consume_lexeme(&mut self, lexeme: &str) -> bool {
        if self.peek().is_some_and(|token| token.lexeme == lexeme) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<&'tokens Token<'source>> {
        self.tokens.get(self.offset)
    }

    fn next_lexeme_is(&self, lexeme: &str) -> bool {
        self.tokens
            .get(self.offset + 1)
            .is_some_and(|token| token.lexeme == lexeme)
    }

    fn next(&mut self) -> Option<&'tokens Token<'source>> {
        let token = self.tokens.get(self.offset)?;
        self.offset += 1;
        Some(token)
    }
}

struct PresentationCompilation<'plans> {
    plans: &'plans [PresentationPlan],
    requested: BTreeSet<ObjectId>,
    collect_only: bool,
}

impl<'plans> PresentationCompilation<'plans> {
    fn strict(plans: &'plans [PresentationPlan]) -> Self {
        Self {
            plans,
            requested: BTreeSet::new(),
            collect_only: false,
        }
    }

    fn collect() -> Self {
        Self {
            plans: &[],
            requested: BTreeSet::new(),
            collect_only: true,
        }
    }

    fn plan(
        &mut self,
        object: ObjectId,
        token: &Token<'_>,
    ) -> Result<Option<&'plans PresentationPlan>, QueryDiagnostic> {
        self.requested.insert(object);
        let mut matches = self.plans.iter().filter(|plan| plan.object == object);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(QueryDiagnostic::at(
                Some(token),
                format!("duplicate presentation plans for object {object}"),
            ));
        }
        if first.is_none() && !self.collect_only {
            return Err(QueryDiagnostic::at(
                Some(token),
                format!("missing presentation plan for object {object}"),
            ));
        }
        Ok(first)
    }
}

fn compile(
    ast: QueryAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompiledQuery, QueryDiagnostic> {
    let unioned = !ast.unions.is_empty();
    let mut branches = Vec::with_capacity(ast.branches.len());
    for (index, branch) in ast.branches.iter().enumerate() {
        let order: &[OrderTerm<'_, '_>] = if index == 0 { &ast.order } else { &[] };
        branches.push(compile_branch(
            branch,
            snapshot,
            order,
            unioned && index == 0,
            presentations,
        )?);
    }

    let first = branches.first().expect("a query has at least one branch");
    for (index, branch) in branches.iter().enumerate().skip(1) {
        if branch.logical_width != first.logical_width
            || branch.columns.len() != first.columns.len()
        {
            return Err(QueryDiagnostic::at(
                Some(ast.unions[index - 1].token),
                format!(
                    "UNION branch {} projects {} logical fields and {} SQL columns; expected {} logical fields and {} SQL columns",
                    index + 1,
                    branch.logical_width,
                    branch.columns.len(),
                    first.logical_width,
                    first.columns.len(),
                ),
            ));
        }
    }

    if !unioned {
        let branch = branches.pop().expect("a query has one branch");
        return Ok(CompiledQuery {
            sql: branch.sql,
            columns: branch.columns,
        });
    }

    let mut sql = format!("({})", first.sql);
    for (link, branch) in ast.unions.iter().zip(branches.iter().skip(1)) {
        sql.push_str(if link.all { " UNION ALL (" } else { " UNION (" });
        sql.push_str(&branch.sql);
        sql.push(')');
    }
    if !first.order.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(&first.order.join(", "));
    }
    Ok(CompiledQuery {
        sql,
        columns: first.columns.clone(),
    })
}

struct CompiledBranch {
    sql: String,
    columns: Vec<String>,
    logical_width: usize,
    order: Vec<String>,
}

enum SelectedProjection {
    Field(ResolvedPath),
    Generated { sql: String, label: String },
}

enum JoinedProjection {
    Field(JoinedPath),
    Generated { sql: String, label: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinedSide {
    Left,
    Right,
}

struct JoinedSource {
    object: ObjectId,
    fields: Vec<QueryableField>,
    relation: String,
    sql_alias: String,
    object_name: String,
    source_alias: Option<String>,
    reference_joins: Vec<JoinPlan>,
}

impl JoinedSource {
    fn is_qualifier(&self, name: &str) -> bool {
        self.source_alias
            .as_deref()
            .is_some_and(|alias| names_equal(alias, name))
            || names_equal(&self.object_name, name)
    }
}

struct JoinedContext<'snapshot> {
    snapshot: &'snapshot MetadataSnapshot,
    left: JoinedSource,
    right: JoinedSource,
}

#[derive(Debug, Clone)]
struct JoinedPath {
    side: JoinedSide,
    field: QueryableField,
    sql_alias: String,
    path_label: Option<String>,
}

impl JoinedPath {
    fn output_label(&self, column: &QueryableColumn) -> String {
        let Some(path_label) = &self.path_label else {
            return column.output_label.clone();
        };
        if self.field.columns.len() == 1 {
            return path_label.clone();
        }
        column
            .output_label
            .strip_prefix(&self.field.name)
            .map_or_else(
                || format!("{path_label}_{}", column.output_label),
                |suffix| format!("{path_label}{suffix}"),
            )
    }
}

impl JoinedContext<'_> {
    fn source(&self, side: JoinedSide) -> &JoinedSource {
        match side {
            JoinedSide::Left => &self.left,
            JoinedSide::Right => &self.right,
        }
    }

    fn source_mut(&mut self, side: JoinedSide) -> &mut JoinedSource {
        match side {
            JoinedSide::Left => &mut self.left,
            JoinedSide::Right => &mut self.right,
        }
    }

    fn qualifier_side(&self, qualifier: &Token<'_>) -> Result<Option<JoinedSide>, QueryDiagnostic> {
        match (
            self.left.is_qualifier(qualifier.lexeme),
            self.right.is_qualifier(qualifier.lexeme),
        ) {
            (true, false) => Ok(Some(JoinedSide::Left)),
            (false, true) => Ok(Some(JoinedSide::Right)),
            (false, false) => Ok(None),
            (true, true) => Err(QueryDiagnostic::at(
                Some(qualifier),
                format!("JOIN source qualifier {:?} is ambiguous", qualifier.lexeme),
            )),
        }
    }

    fn direct_field(
        &self,
        side: JoinedSide,
        field: &Token<'_>,
    ) -> Result<JoinedPath, QueryDiagnostic> {
        let source = self.source(side);
        Ok(JoinedPath {
            side,
            field: resolve_named_field(&source.fields, field)?,
            sql_alias: source.sql_alias.clone(),
            path_label: None,
        })
    }

    fn resolve_direct(
        &self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<JoinedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [field] => {
                let left = matching_fields(&self.left.fields, field);
                let right = matching_fields(&self.right.fields, field);
                match (left.as_slice(), right.as_slice()) {
                    ([field], []) => Ok(JoinedPath {
                        side: JoinedSide::Left,
                        field: (*field).clone(),
                        sql_alias: self.left.sql_alias.clone(),
                        path_label: None,
                    }),
                    ([], [field]) => Ok(JoinedPath {
                        side: JoinedSide::Right,
                        field: (*field).clone(),
                        sql_alias: self.right.sql_alias.clone(),
                        path_label: None,
                    }),
                    ([], []) => Err(QueryDiagnostic::at(
                        Some(field),
                        format!("field {:?} was not found in JOIN sources", field.lexeme),
                    )),
                    _ => Err(QueryDiagnostic::at(
                        Some(field),
                        format!("field {:?} is ambiguous in JOIN sources", field.lexeme),
                    )),
                }
            }
            [qualifier, field] => match self.qualifier_side(qualifier)? {
                Some(side) => self.direct_field(side, field),
                None => Err(QueryDiagnostic::at(
                    Some(qualifier),
                    format!("unknown JOIN source qualifier {:?}", qualifier.lexeme),
                )),
            },
            [_, _, unsupported, ..] => Err(QueryDiagnostic::at(
                Some(unsupported),
                "JOIN condition supports direct fields only",
            )),
            [] => unreachable!("field path is non-empty"),
        }
    }

    fn resolve(
        &mut self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<JoinedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [_] => self.resolve_direct(reference),
            [first, second] => match self.qualifier_side(first)? {
                Some(side) => self.direct_field(side, second),
                None => {
                    let candidates = self.reference_sides(first);
                    match candidates.as_slice() {
                        [side] => self.resolve_dereference(*side, first, second),
                        [] => Err(QueryDiagnostic::at(
                            Some(first),
                            format!("field {:?} was not found in JOIN sources", first.lexeme),
                        )),
                        _ => Err(QueryDiagnostic::at(
                            Some(first),
                            format!(
                                "reference field {:?} is ambiguous in JOIN sources",
                                first.lexeme
                            ),
                        )),
                    }
                }
            },
            [qualifier, reference_field, target_field] => {
                let Some(side) = self.qualifier_side(qualifier)? else {
                    return Err(QueryDiagnostic::at(
                        Some(qualifier),
                        format!("unknown JOIN source qualifier {:?}", qualifier.lexeme),
                    ));
                };
                self.resolve_dereference(side, reference_field, target_field)
            }
            [_, _, _, unsupported, ..] => Err(QueryDiagnostic::at(
                Some(unsupported),
                "reference paths deeper than one hop are not supported",
            )),
            [] => unreachable!("field path is non-empty"),
        }
    }

    fn reference_sides(&self, token: &Token<'_>) -> Vec<JoinedSide> {
        [JoinedSide::Left, JoinedSide::Right]
            .into_iter()
            .filter(|side| {
                matching_fields(&self.source(*side).fields, token)
                    .iter()
                    .any(|field| field.reference_target.is_some())
            })
            .collect()
    }

    fn resolve_dereference(
        &mut self,
        side: JoinedSide,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<JoinedPath, QueryDiagnostic> {
        let reference_field = resolve_named_field(&self.source(side).fields, reference_token)?;
        let target_table = reference_field.reference_target.as_deref().ok_or_else(|| {
            QueryDiagnostic::at(
                Some(reference_token),
                format!(
                    "field {:?} has no unique SchemaStorage reference target",
                    reference_token.lexeme
                ),
            )
        })?;
        let source_column = reference_column(&reference_field, reference_token)?
            .physical_name
            .clone();
        let target_physical = format!("_{}", target_table.trim_start_matches('_'));
        let target_objects = self
            .snapshot
            .objects
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| table.eq_ignore_ascii_case(&target_physical))
            })
            .collect::<Vec<_>>();
        let target_object = match target_objects.as_slice() {
            [object] => *object,
            [] => {
                return Err(QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} was not resolved"),
                ));
            }
            _ => {
                return Err(QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} is ambiguous"),
                ));
            }
        };
        let target_live_table = self
            .snapshot
            .live_tables
            .iter()
            .find(|table| table.name.eq_ignore_ascii_case(&target_physical))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target table {target_physical:?} is not live"),
                )
            })?;
        let target_fields = queryable_fields(self.snapshot, target_object)?;
        let target_field = resolve_named_field(&target_fields, target_token)?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} has no ID field"),
                )
            })?;
        let target_id_column = single_column(target_id, reference_token)?
            .physical_name
            .clone();

        let existing = self
            .source(side)
            .reference_joins
            .iter()
            .find(|join| names_equal(&join.source_field, &reference_field.schema_name))
            .map(|join| join.alias.clone());
        let alias = if let Some(alias) = existing {
            alias
        } else {
            let alias = self.next_reference_alias(side);
            self.source_mut(side).reference_joins.push(JoinPlan {
                source_field: reference_field.schema_name,
                source_column,
                source_type_column: None,
                database_type: None,
                target_table: target_live_table.name.clone(),
                target_id_column,
                alias: alias.clone(),
            });
            alias
        };
        Ok(JoinedPath {
            side,
            field: target_field,
            sql_alias: alias,
            path_label: Some(format!(
                "{}.{}",
                reference_token.lexeme, target_token.lexeme
            )),
        })
    }

    fn next_reference_alias(&self, side: JoinedSide) -> String {
        let prefix = match side {
            JoinedSide::Left => "__left_ref",
            JoinedSide::Right => "__right_ref",
        };
        let mut number = self.source(side).reference_joins.len() + 1;
        loop {
            let candidate = format!("{prefix}{number}");
            if !names_equal(&self.left.sql_alias, &candidate)
                && !names_equal(&self.right.sql_alias, &candidate)
                && self
                    .left
                    .reference_joins
                    .iter()
                    .chain(&self.right.reference_joins)
                    .all(|join| !names_equal(&join.alias, &candidate))
            {
                return candidate;
            }
            number += 1;
        }
    }

    fn sql_column(&self, resolved: &JoinedPath, column: &QueryableColumn) -> String {
        qualified_column(Some(&resolved.sql_alias), &column.physical_name)
    }

    fn ensure_presentation_join(
        &mut self,
        side: JoinedSide,
        reference: &QueryableField,
        target: ObjectId,
        multiple: bool,
        token: &Token<'_>,
    ) -> Result<String, QueryDiagnostic> {
        let source_column = reference_column(reference, token)?.physical_name.clone();
        let source_type_column = multiple
            .then(|| reference_type_column(reference, token))
            .transpose()?
            .map(|column| column.physical_name.clone());
        let target_object = self.snapshot.object_by_id(target).ok_or_else(|| {
            QueryDiagnostic::at(Some(token), "presentation target was not resolved")
        })?;
        let database_type = multiple.then_some(target_object.number).flatten();
        let target_table = target_object
            .physical_table
            .as_deref()
            .and_then(|physical| {
                self.snapshot
                    .live_tables
                    .iter()
                    .find(|table| table.name.eq_ignore_ascii_case(physical))
            })
            .ok_or_else(|| {
                QueryDiagnostic::at(Some(token), "presentation target table is not live")
            })?;
        let target_fields = queryable_fields(self.snapshot, target_object)?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| QueryDiagnostic::at(Some(token), "presentation target has no ID"))?;
        let target_id_column = single_column(target_id, token)?.physical_name.clone();
        if let Some(join) = self.source(side).reference_joins.iter().find(|join| {
            names_equal(&join.source_field, &reference.schema_name)
                && names_equal(&join.target_table, &target_table.name)
                && join.database_type == database_type
        }) {
            return Ok(join.alias.clone());
        }
        let alias = self.next_reference_alias(side);
        self.source_mut(side).reference_joins.push(JoinPlan {
            source_field: reference.schema_name.clone(),
            source_column,
            source_type_column,
            database_type,
            target_table: target_table.name.clone(),
            target_id_column,
            alias: alias.clone(),
        });
        Ok(alias)
    }
}

fn compile_single_presentation(
    context: &mut CompilationContext<'_>,
    token: &Token<'_>,
    operation: PresentationOperation,
    argument: &PresentationArgument<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<(String, String), QueryDiagnostic> {
    let label = token.lexeme.to_owned();
    let PresentationArgument::Field(reference) = argument else {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Presentation property requires a reference field",
            ));
        }
        let PresentationArgument::Literal(literal) = argument else {
            unreachable!()
        };
        let value = compile_literal(literal)?;
        return Ok((format!("({value})::text"), label));
    };

    let resolved = context.resolve_path(reference)?;
    if !matches!(resolved.source, FieldSource::Base) {
        return Err(QueryDiagnostic::at(
            Some(token),
            "presentation of an already dereferenced path is not supported",
        ));
    }
    let targets = presentation_targets(
        context.snapshot,
        context.object,
        &resolved.field,
        reference.last(),
    )?;
    if targets.is_empty() {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Presentation property is available only for reference fields",
            ));
        }
        let column = single_column(&resolved.field, reference.last())?;
        let value = qualified_column(Some(context.base_alias()), &column.physical_name);
        return Ok((format!("({value})::text"), label));
    }

    let multiple = targets.len() > 1;
    let source_reference = reference_column(&resolved.field, reference.last())?
        .physical_name
        .clone();
    let source_type = multiple
        .then(|| reference_type_column(&resolved.field, reference.last()))
        .transpose()?
        .map(|column| column.physical_name.clone());
    let mut variants = Vec::new();
    for (target, number) in targets {
        let plan = presentations.plan(target, token)?;
        let alias = if target == context.object && names_equal(&resolved.field.schema_name, "ID") {
            context.base_alias().to_owned()
        } else {
            context.ensure_presentation_join(&resolved.field, target, multiple, token)?
        };
        let expression = plan.map_or_else(
            || Ok("NULL::text".to_owned()),
            |plan| compile_presentation_plan(context.snapshot, target, &alias, plan, token),
        )?;
        variants.push((number, expression));
    }
    Ok((
        wrap_reference_presentation(
            context.base_alias(),
            &source_reference,
            source_type.as_deref(),
            &variants,
        ),
        label,
    ))
}

fn compile_joined_presentation(
    context: &mut JoinedContext<'_>,
    token: &Token<'_>,
    operation: PresentationOperation,
    argument: &PresentationArgument<'_, '_>,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<(String, String), QueryDiagnostic> {
    let label = token.lexeme.to_owned();
    let PresentationArgument::Field(reference) = argument else {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Presentation property requires a reference field",
            ));
        }
        let PresentationArgument::Literal(literal) = argument else {
            unreachable!()
        };
        let value = compile_literal(literal)?;
        return Ok((format!("({value})::text"), label));
    };
    let resolved = context.resolve(reference)?;
    let source = context.source(resolved.side);
    if !names_equal(&resolved.sql_alias, &source.sql_alias) {
        return Err(QueryDiagnostic::at(
            Some(token),
            "presentation of an already dereferenced JOIN path is not supported",
        ));
    }
    let owner = source.object;
    let source_alias = source.sql_alias.clone();
    let targets = presentation_targets(context.snapshot, owner, &resolved.field, reference.last())?;
    if targets.is_empty() {
        if operation == PresentationOperation::Property {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Presentation property is available only for reference fields",
            ));
        }
        let column = single_column(&resolved.field, reference.last())?;
        let value = qualified_column(Some(&source_alias), &column.physical_name);
        return Ok((format!("({value})::text"), label));
    }
    let multiple = targets.len() > 1;
    let source_reference = reference_column(&resolved.field, reference.last())?
        .physical_name
        .clone();
    let source_type = multiple
        .then(|| reference_type_column(&resolved.field, reference.last()))
        .transpose()?
        .map(|column| column.physical_name.clone());
    let mut variants = Vec::new();
    for (target, number) in targets {
        let plan = presentations.plan(target, token)?;
        let alias = if target == owner && names_equal(&resolved.field.schema_name, "ID") {
            source_alias.clone()
        } else {
            context.ensure_presentation_join(
                resolved.side,
                &resolved.field,
                target,
                multiple,
                token,
            )?
        };
        let expression = plan.map_or_else(
            || Ok("NULL::text".to_owned()),
            |plan| compile_presentation_plan(context.snapshot, target, &alias, plan, token),
        )?;
        variants.push((number, expression));
    }
    Ok((
        wrap_reference_presentation(
            &source_alias,
            &source_reference,
            source_type.as_deref(),
            &variants,
        ),
        label,
    ))
}

fn compile_source_free_branch(
    ast: &SelectAst<'_, '_>,
    order_terms: &[OrderTerm<'_, '_>],
) -> Result<CompiledBranch, QueryDiagnostic> {
    if ast.join.is_some() {
        return Err(QueryDiagnostic::metadata("JOIN requires FROM"));
    }
    if ast.filter.is_some() {
        return Err(QueryDiagnostic::metadata("WHERE requires FROM"));
    }
    if !order_terms.is_empty() {
        return Err(QueryDiagnostic::at(
            Some(order_terms[0].field.last()),
            "ORDER BY requires FROM for a source-free SELECT",
        ));
    }

    let mut projections = Vec::with_capacity(ast.projection.len());
    let mut columns = Vec::with_capacity(ast.projection.len());
    for (index, projection) in ast.projection.iter().enumerate() {
        let (sql, label) = match projection {
            Projection::Aggregate {
                token,
                kind: AggregateKind::Count,
                distinct: false,
                argument: AggregateArgument::All,
            } => ("COUNT(*)::text".to_owned(), token.lexeme.to_owned()),
            Projection::Aggregate { token, .. } => {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "aggregate field argument requires FROM",
                ));
            }
            Projection::Scalar(expression) => (
                format!("({})::text", compile_source_free_expression(expression)?),
                format!("column{}", index + 1),
            ),
            Projection::Presentation {
                token,
                operation: PresentationOperation::Reference | PresentationOperation::String,
                argument: PresentationArgument::Literal(literal),
            } => (
                format!("({})::text", compile_literal(literal)?),
                token.lexeme.to_owned(),
            ),
            Projection::Presentation { token, .. } => {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "reference presentation field requires FROM",
                ));
            }
            Projection::Field(reference) => {
                return Err(QueryDiagnostic::at(
                    Some(reference.last()),
                    "field projection requires FROM",
                ));
            }
            Projection::All => {
                return Err(QueryDiagnostic::metadata(
                    "wildcard projection requires FROM",
                ));
            }
        };
        projections.push(format!("{sql} AS {}", quote_identifier(&label)));
        columns.push(label);
    }
    let mut sql = String::from("SELECT ");
    if ast.distinct {
        sql.push_str("DISTINCT ");
    }
    sql.push_str(&projections.join(", "));
    if let Some(top) = ast.top {
        sql.push_str(" LIMIT ");
        sql.push_str(&top.to_string());
    }
    Ok(CompiledBranch {
        sql,
        logical_width: columns.len(),
        columns,
        order: Vec::new(),
    })
}

fn compile_source_free_expression(
    expression: &Expression<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => Err(QueryDiagnostic::at(
            Some(reference.last()),
            "field expression requires FROM",
        )),
        Expression::Literal(token) => compile_literal(token),
        Expression::Unary { operator, value } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::Not) => "NOT ",
                _ if operator.lexeme == "+" => "+",
                _ if operator.lexeme == "-" => "-",
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported unary operator",
                    ));
                }
            };
            Ok(format!(
                "({operator}{})",
                compile_source_free_expression(value)?
            ))
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::And) => "AND",
                TokenKind::Keyword(Keyword::Or) => "OR",
                _ if matches!(
                    operator.lexeme,
                    "=" | "<>" | "<" | ">" | "<=" | ">=" | "+" | "-" | "*" | "/"
                ) =>
                {
                    operator.lexeme
                }
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported binary operator",
                    ));
                }
            };
            Ok(format!(
                "({} {operator} {})",
                compile_source_free_expression(left)?,
                compile_source_free_expression(right)?
            ))
        }
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_source_free_expression(value)?,
            if *negated { "NOT " } else { "" }
        )),
    }
}

fn validate_aggregate_projection(ast: &SelectAst<'_, '_>) -> Result<(), QueryDiagnostic> {
    let count = ast
        .projection
        .iter()
        .find_map(|projection| match projection {
            Projection::Aggregate { token, .. } => Some(*token),
            _ => None,
        });
    if let Some(token) = count
        && ast
            .projection
            .iter()
            .any(|projection| !matches!(projection, Projection::Aggregate { .. }))
    {
        return Err(QueryDiagnostic::at(
            Some(token),
            "aggregates cannot be mixed with non-aggregate projections without GROUP BY",
        ));
    }
    Ok(())
}

fn presentation_targets(
    snapshot: &MetadataSnapshot,
    owner: ObjectId,
    field: &QueryableField,
    token: &Token<'_>,
) -> Result<Vec<(ObjectId, u32)>, QueryDiagnostic> {
    if names_equal(&field.schema_name, "ID") {
        let object = snapshot
            .object_by_id(owner)
            .ok_or_else(|| QueryDiagnostic::at(Some(token), "reference owner was not resolved"))?;
        return Ok(vec![(owner, object.number.unwrap_or_default())]);
    }
    let mut targets = Vec::new();
    for target in &field.reference_targets {
        let physical = format!("_{}", target.trim_start_matches('_'));
        let matches = snapshot
            .objects
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| table.eq_ignore_ascii_case(&physical))
            })
            .collect::<Vec<_>>();
        let object = match matches.as_slice() {
            [object] => *object,
            [] => {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    format!("reference target {physical:?} was not resolved"),
                ));
            }
            _ => {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    format!("reference target {physical:?} is ambiguous"),
                ));
            }
        };
        let id = ObjectId::from(&object.guid);
        if !targets.iter().any(|(candidate, _)| *candidate == id) {
            targets.push((id, object.number.unwrap_or_default()));
        }
    }
    Ok(targets)
}

fn wrap_reference_presentation(
    source_alias: &str,
    reference_column: &str,
    type_column: Option<&str>,
    variants: &[(u32, String)],
) -> String {
    let reference = qualified_column(Some(source_alias), reference_column);
    if variants.len() == 1 {
        return format!(
            "CASE WHEN {reference} IS NULL THEN '' ELSE {} END",
            variants[0].1
        );
    }
    let type_column = type_column.expect("multiple reference targets have RTRef");
    let type_value = qualified_column(Some(source_alias), type_column);
    let mut sql = format!("CASE WHEN {reference} IS NULL THEN ''");
    for (number, expression) in variants {
        use std::fmt::Write as _;
        write!(sql, " WHEN {type_value} = '\\x").expect("writing to String cannot fail");
        for byte in number.to_be_bytes() {
            write!(sql, "{byte:02x}").expect("writing to String cannot fail");
        }
        write!(sql, "'::bytea THEN {expression}").expect("writing to String cannot fail");
    }
    sql.push_str(" ELSE '' END");
    sql
}

struct CompiledSourceRelation {
    sql: String,
    fields: Vec<QueryableField>,
}

fn compile_source_relation(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
    live_table: &LiveTable,
    fields: &[QueryableField],
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    if let Some(accumulation) = &source.accumulation {
        return compile_accumulation_relation(
            source,
            accumulation,
            snapshot,
            object,
            live_table,
            fields,
        );
    }
    let Some(slice) = &source.slice else {
        return Ok(CompiledSourceRelation {
            sql: quote_identifier(&live_table.name),
            fields: fields.to_vec(),
        });
    };
    if object.kind != Some(MetadataKind::InformationRegister) {
        return Err(QueryDiagnostic::at(
            Some(slice.token),
            format!(
                "{} is supported only for information registers",
                slice.kind.name()
            ),
        ));
    }
    let period = fields
        .iter()
        .find(|field| names_equal(&field.schema_name, "Period"))
        .ok_or_else(|| {
            QueryDiagnostic::at(
                Some(slice.token),
                format!("{} requires a live Period field", slice.kind.name()),
            )
        })?;
    let period_column = single_column(period, slice.token)?;

    let physical_table = object
        .physical_table
        .as_deref()
        .expect("a live information register has a physical table");
    let owned_fields = snapshot
        .fields
        .iter()
        .filter(|field| {
            field
                .owner_tables
                .iter()
                .any(|owner| names_equal(owner, physical_table))
        })
        .collect::<Vec<_>>();
    if !owned_fields.is_empty() && owned_fields.iter().all(|field| field.purpose.is_none()) {
        return Err(QueryDiagnostic::at(
            Some(slice.token),
            format!(
                "{} dimension roles are unavailable in Config metadata",
                slice.kind.name()
            ),
        ));
    }

    let mut partition_names = BTreeSet::new();
    let mut partition_columns = Vec::new();
    for metadata_field in owned_fields.iter().filter(|field| {
        field.data_separator
            || field.purpose == Some(ConfigFieldPurpose::InformationRegisterDimension)
    }) {
        let schema_name = format!("Fld{}", metadata_field.number);
        let field = fields
            .iter()
            .find(|field| names_equal(&field.schema_name, &schema_name))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(slice.token),
                    format!(
                        "{} dimension {:?} has no live physical representation",
                        slice.kind.name(),
                        metadata_field.name.as_deref().unwrap_or(&schema_name)
                    ),
                )
            })?;
        for column in &field.columns {
            if partition_names.insert(column.physical_name.to_lowercase()) {
                partition_columns.push(qualified_column(
                    Some("__slice_base"),
                    &column.physical_name,
                ));
            }
        }
    }

    let period_bound = slice
        .period
        .as_ref()
        .map(|expression| match expression {
            Expression::Literal(token) => compile_literal(token),
            _ => Err(QueryDiagnostic::at(
                Some(slice.token),
                format!("{} period must be a scalar literal", slice.kind.name()),
            )),
        })
        .transpose()?;
    let virtual_condition = if let Some(condition) = &slice.condition {
        let mut condition_context = CompilationContext {
            snapshot,
            object: ObjectId::from(&object.guid),
            fields: fields.to_vec(),
            source_alias: Some("__slice_base".to_owned()),
            object_name: source.object.lexeme.to_owned(),
            joins: Vec::new(),
        };
        let sql = compile_expression(condition, &mut condition_context)?;
        if !condition_context.joins.is_empty() {
            return Err(QueryDiagnostic::at(
                Some(slice.token),
                format!(
                    "{} condition supports direct fields only",
                    slice.kind.name()
                ),
            ));
        }
        Some(sql)
    } else {
        None
    };

    let qualified_period = qualified_column(Some("__slice_base"), &period_column.physical_name);
    let mut predicates = Vec::new();
    if let Some(period_bound) = period_bound {
        predicates.push(format!(
            "({qualified_period} {} {period_bound})",
            slice.kind.period_operator()
        ));
    }
    if let Some(condition) = virtual_condition {
        predicates.push(condition);
    }
    let partition = if partition_columns.is_empty() {
        String::new()
    } else {
        format!("PARTITION BY {} ", partition_columns.join(", "))
    };
    let mut relation = format!(
        "(SELECT \"__slice_ranked\".* FROM (SELECT \"__slice_base\".*, DENSE_RANK() OVER ({partition}ORDER BY {qualified_period} {}) AS \"__open_sdbl_slice_rank\" FROM {} AS \"__slice_base\"",
        slice.kind.order(),
        quote_identifier(&live_table.name),
    );
    if !predicates.is_empty() {
        relation.push_str(" WHERE ");
        relation.push_str(&predicates.join(" AND "));
    }
    relation.push_str(
        ") AS \"__slice_ranked\" WHERE \"__slice_ranked\".\"__open_sdbl_slice_rank\" = 1)",
    );
    Ok(CompiledSourceRelation {
        sql: relation,
        fields: fields.to_vec(),
    })
}

fn compile_accumulation_relation(
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
    live_table: &LiveTable,
    fields: &[QueryableField],
) -> Result<CompiledSourceRelation, QueryDiagnostic> {
    if object.kind != Some(MetadataKind::AccumulationRegister) {
        return Err(QueryDiagnostic::at(
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
        .fields
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
            Some(virtual_table.token),
            "Balance is unavailable for a turnover-only accumulation register",
        ));
    }

    if virtual_table.kind == AccumulationKind::Balance {
        return compile_accumulation_balance_relation(
            source,
            virtual_table,
            snapshot,
            object,
            &dimension_fields,
            &resource_fields,
            &live_table.name,
            active_column,
            period_column,
            record_kind.expect("a balance register has RecordKind"),
        );
    }

    if virtual_table
        .arguments
        .get(2)
        .and_then(Option::as_ref)
        .is_some()
    {
        return Err(QueryDiagnostic::at(
            Some(virtual_table.token),
            "Turnovers periodicity is not supported yet; omit the third argument",
        ));
    }
    let begin = virtual_table.arguments.first().and_then(Option::as_ref);
    let end = virtual_table.arguments.get(1).and_then(Option::as_ref);
    let condition = virtual_table.arguments.get(3).and_then(Option::as_ref);

    let begin = begin
        .map(|expression| compile_virtual_period_literal(expression, virtual_table, "begin period"))
        .transpose()?;
    let end = end
        .map(|expression| {
            compile_virtual_period_literal(expression, virtual_table, "period boundary")
        })
        .transpose()?;
    let mut predicates = vec![format!(
        "{} = TRUE",
        qualified_column(Some("__aggregate_base"), &active_column.physical_name)
    )];
    let qualified_period = qualified_column(Some("__aggregate_base"), &period_column.physical_name);
    if let Some(begin) = begin {
        predicates.push(format!("({qualified_period} >= {begin})"));
    }
    if let Some(end) = end {
        predicates.push(format!("({qualified_period} < {end})"));
    }
    if let Some(condition) = condition {
        let mut condition_context = CompilationContext {
            snapshot,
            object: ObjectId::from(&object.guid),
            fields: dimension_fields.clone(),
            source_alias: Some("__aggregate_base".to_owned()),
            object_name: source.object.lexeme.to_owned(),
            joins: Vec::new(),
        };
        let sql = compile_expression(condition, &mut condition_context)?;
        if !condition_context.joins.is_empty() {
            return Err(QueryDiagnostic::at(
                Some(virtual_table.token),
                format!(
                    "{} condition supports direct dimensions and separators only",
                    virtual_table.kind.name()
                ),
            ));
        }
        predicates.push(sql);
    }

    let mut projections = Vec::new();
    let mut grouping = Vec::new();
    for field in &dimension_fields {
        for column in &field.columns {
            let sql = qualified_column(Some("__aggregate_base"), &column.physical_name);
            projections.push(format!(
                "{sql} AS {}",
                quote_identifier(&column.physical_name)
            ));
            grouping.push(sql);
        }
    }
    let mut virtual_resources = Vec::new();
    for field in &resource_fields {
        let column = single_column(field, virtual_table.token)?;
        let value = qualified_column(Some("__aggregate_base"), &column.physical_name);
        let value = record_kind.map_or(value.clone(), |record_kind| {
            format!(
                "CASE WHEN {} = 0 THEN {value} ELSE -{value} END",
                qualified_column(Some("__aggregate_base"), &record_kind.physical_name)
            )
        });
        let aggregate = format!("SUM({value})");
        projections.push(format!(
            "{aggregate} AS {}",
            quote_identifier(&column.physical_name)
        ));
        virtual_resources.push(accumulation_resource_field(field, virtual_table.kind));
    }

    let mut relation = format!(
        "(SELECT {} FROM {} AS \"__aggregate_base\" WHERE {}",
        projections.join(", "),
        quote_identifier(&live_table.name),
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
        fields: dimension_fields,
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
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    resource_fields: &[QueryableField],
    movement_table: &str,
    active_column: &QueryableColumn,
    movement_period: &QueryableColumn,
    record_kind: &QueryableColumn,
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
            compile_virtual_period_literal(expression, virtual_table, "period boundary")
        })
        .transpose()?;
    let totals_condition = compile_accumulation_condition(
        condition,
        source,
        virtual_table,
        snapshot,
        object,
        dimension_fields,
        "__totals_base",
    )?;

    let relation = if let Some(boundary) = boundary {
        let movement_condition = compile_accumulation_condition(
            condition,
            source,
            virtual_table,
            snapshot,
            object,
            dimension_fields,
            "__movement_base",
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
        )?
    } else {
        compile_current_balance_sql(
            totals,
            dimension_fields,
            resource_fields,
            totals_condition.as_deref(),
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
        fields,
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
        .db_names
        .entries()
        .iter()
        .filter(|entry| entry.guid == object.guid && entry.alias == "AccumRgT")
        .collect::<Vec<_>>();
    let entry = match entries.as_slice() {
        [entry] => *entry,
        [] => {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Balance requires an AccumRgT entry for the register GUID in DBNames",
            ));
        }
        _ => {
            return Err(QueryDiagnostic::at(
                Some(token),
                "Balance totals mapping is ambiguous in DBNames",
            ));
        }
    };
    let physical_name = format!("_AccumRgT{}", entry.number);
    let schema_table = snapshot.schema.table(&physical_name).ok_or_else(|| {
        QueryDiagnostic::at(
            Some(token),
            format!("Balance totals table {physical_name} is absent from SchemaStorage"),
        )
    })?;
    let live_table = snapshot
        .live_tables
        .iter()
        .find(|table| names_equal(&table.name, &physical_name))
        .ok_or_else(|| {
            QueryDiagnostic::at(
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

fn compile_accumulation_condition(
    condition: Option<&Expression<'_, '_>>,
    source: &SourceAst<'_, '_>,
    virtual_table: &AccumulationAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    object: &MetadataObject,
    dimension_fields: &[QueryableField],
    alias: &str,
) -> Result<Option<String>, QueryDiagnostic> {
    let Some(condition) = condition else {
        return Ok(None);
    };
    let mut context = CompilationContext {
        snapshot,
        object: ObjectId::from(&object.guid),
        fields: dimension_fields.to_vec(),
        source_alias: Some(alias.to_owned()),
        object_name: source.object.lexeme.to_owned(),
        joins: Vec::new(),
    };
    let sql = compile_expression(condition, &mut context)?;
    if !context.joins.is_empty() {
        return Err(QueryDiagnostic::at(
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
) -> Result<String, QueryDiagnostic> {
    let (dimension_projection, grouping) =
        accumulation_dimensions(dimension_fields, "__totals_base");
    let mut projections = dimension_projection;
    let mut aggregates = Vec::new();
    for field in resource_fields {
        let column = single_column_without_token(field)?;
        let aggregate = format!(
            "SUM({})",
            qualified_column(Some("__totals_base"), &column.physical_name)
        );
        projections.push(format!(
            "{aggregate} AS {}",
            quote_identifier(&column.physical_name)
        ));
        aggregates.push(aggregate);
    }
    let period = qualified_column(Some("__totals_base"), &totals.period.name);
    let latest_period = qualified_column(Some("__totals_latest"), &totals.period.name);
    let mut predicates = vec![format!(
        "{period} = (SELECT MAX({latest_period}) FROM {} AS \"__totals_latest\")",
        quote_identifier(&totals.table.name)
    )];
    if let Some(condition) = condition {
        predicates.push(condition.to_owned());
    }
    let mut sql = format!(
        "(SELECT {} FROM {} AS \"__totals_base\" WHERE {}",
        projections.join(", "),
        quote_identifier(&totals.table.name),
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
) -> Result<String, QueryDiagnostic> {
    let totals_period = qualified_column(Some("__anchor_totals"), &totals.period.name);
    let anchor_period = qualified_column(Some("__balance_anchor"), "__period");
    let mut totals_parts = accumulation_part_dimensions(dimension_fields, "__totals_base");
    let mut movement_parts = accumulation_part_dimensions(dimension_fields, "__movement_base");
    let mut outer_projection = accumulation_part_dimensions(dimension_fields, "__balance_parts");
    let grouping = dimension_fields
        .iter()
        .flat_map(|field| field.columns.iter())
        .map(|column| qualified_column(Some("__balance_parts"), &column.physical_name))
        .collect::<Vec<_>>();
    let mut aggregates = Vec::new();
    for field in resource_fields {
        let column = single_column_without_token(field)?;
        totals_parts.push(format!(
            "{} AS {}",
            qualified_column(Some("__totals_base"), &column.physical_name),
            quote_identifier(&column.physical_name)
        ));
        let value = qualified_column(Some("__movement_base"), &column.physical_name);
        let signed = format!(
            "CASE WHEN {} = 0 THEN {value} ELSE -{value} END",
            qualified_column(Some("__movement_base"), &record_kind.physical_name)
        );
        movement_parts.push(format!(
            "CASE WHEN {anchor_period} <= {boundary} THEN {signed} ELSE -({signed}) END AS {}",
            quote_identifier(&column.physical_name)
        ));
        let aggregate = format!(
            "SUM({})",
            qualified_column(Some("__balance_parts"), &column.physical_name)
        );
        outer_projection.push(format!(
            "{aggregate} AS {}",
            quote_identifier(&column.physical_name)
        ));
        aggregates.push(aggregate);
    }

    let totals_base_period = qualified_column(Some("__totals_base"), &totals.period.name);
    let mut totals_predicates = vec![format!("{totals_base_period} = {anchor_period}")];
    if let Some(condition) = totals_condition {
        totals_predicates.push(condition.to_owned());
    }
    let movement_period = qualified_column(Some("__movement_base"), &movement_period.physical_name);
    let mut movement_predicates = vec![format!(
        "{} = TRUE",
        qualified_column(Some("__movement_base"), &active_column.physical_name)
    )];
    movement_predicates.push(format!(
        "(({anchor_period} <= {boundary} AND {movement_period} >= {anchor_period} AND {movement_period} < {boundary}) OR ({anchor_period} > {boundary} AND {movement_period} >= {boundary} AND {movement_period} < {anchor_period}))"
    ));
    if let Some(condition) = movement_condition {
        movement_predicates.push(condition.to_owned());
    }

    let mut sql = format!(
        "(WITH \"__balance_anchor\" AS (SELECT COALESCE(MAX({totals_period}) FILTER (WHERE {totals_period} <= {boundary}), MAX({totals_period})) AS \"__period\" FROM {} AS \"__anchor_totals\"), \"__balance_parts\" AS (SELECT {} FROM {} AS \"__totals_base\" CROSS JOIN \"__balance_anchor\" WHERE {} UNION ALL SELECT {} FROM {} AS \"__movement_base\" CROSS JOIN \"__balance_anchor\" WHERE {}) SELECT {} FROM \"__balance_parts\"",
        quote_identifier(&totals.table.name),
        totals_parts.join(", "),
        quote_identifier(&totals.table.name),
        totals_predicates.join(" AND "),
        movement_parts.join(", "),
        quote_identifier(movement_table),
        movement_predicates.join(" AND "),
        outer_projection.join(", ")
    );
    append_balance_grouping(&mut sql, &grouping, &aggregates);
    sql.push(')');
    Ok(sql)
}

fn accumulation_dimensions(fields: &[QueryableField], alias: &str) -> (Vec<String>, Vec<String>) {
    let mut projection = Vec::new();
    let mut grouping = Vec::new();
    for field in fields {
        for column in &field.columns {
            let value = qualified_column(Some(alias), &column.physical_name);
            projection.push(format!(
                "{value} AS {}",
                quote_identifier(&column.physical_name)
            ));
            grouping.push(value);
        }
    }
    (projection, grouping)
}

fn accumulation_part_dimensions(fields: &[QueryableField], alias: &str) -> Vec<String> {
    accumulation_dimensions(fields, alias).0
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
        _ => Err(QueryDiagnostic::metadata(format!(
            "accumulation resource {:?} must have one physical column",
            field.name
        ))),
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
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Literal(token) => compile_literal(token),
        _ => Err(QueryDiagnostic::at(
            Some(virtual_table.token),
            format!(
                "{} {argument} must be a scalar literal",
                virtual_table.kind.name()
            ),
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

fn compile_presentation_plan(
    snapshot: &MetadataSnapshot,
    target: ObjectId,
    alias: &str,
    plan: &PresentationPlan,
    token: &Token<'_>,
) -> Result<String, QueryDiagnostic> {
    if plan.object != target {
        return Err(QueryDiagnostic::at(
            Some(token),
            "presentation plan target does not match the requested object",
        ));
    }
    let object = snapshot
        .object_by_id(target)
        .ok_or_else(|| QueryDiagnostic::at(Some(token), "presentation target was not resolved"))?;
    let fields = queryable_fields(snapshot, object)?;
    let mut unique = BTreeSet::new();
    for field in &plan.fields {
        if !unique.insert(*field) {
            return Err(QueryDiagnostic::at(
                Some(token),
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
    token: &Token<'_>,
    depth: usize,
    budget: &mut usize,
) -> Result<String, QueryDiagnostic> {
    if depth > 32 || *budget == 0 {
        return Err(QueryDiagnostic::at(
            Some(token),
            "presentation expression is too complex",
        ));
    }
    *budget -= 1;
    match expression {
        PresentationExpression::Field(id) => {
            if !authorized.contains(id) {
                return Err(QueryDiagnostic::at(
                    Some(token),
                    "presentation expression uses a field not listed by its plan",
                ));
            }
            let field = presentation_field(snapshot, object, fields, *id, token)?;
            let column = single_column(field, token)?;
            Ok(format!(
                "COALESCE({}::text, '')",
                qualified_column(Some(alias), &column.physical_name)
            ))
        }
        PresentationExpression::Literal(value) => Ok(format!("'{}'", value.replace('\'', "''"))),
        PresentationExpression::Concat(parts) => {
            if parts.is_empty() {
                return Err(QueryDiagnostic::at(
                    Some(token),
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
    token: &Token<'_>,
) -> Result<&'field QueryableField, QueryDiagnostic> {
    let schema_name = match id {
        FieldId::Standard(standard) => standard.schema_name().to_owned(),
        FieldId::Metadata(attribute) => {
            let field = snapshot.attribute_by_id(attribute).map_err(|error| {
                QueryDiagnostic::at(Some(token), format!("invalid presentation field: {error}"))
            })?;
            let owner = object.physical_table.as_deref().ok_or_else(|| {
                QueryDiagnostic::at(Some(token), "presentation target has no physical table")
            })?;
            if !field
                .owner_tables
                .iter()
                .any(|table| table.eq_ignore_ascii_case(owner))
            {
                return Err(QueryDiagnostic::at(
                    Some(token),
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
            QueryDiagnostic::at(
                Some(token),
                format!("presentation field {schema_name} is not live on its target"),
            )
        })
}

fn compile_branch(
    ast: &SelectAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    order_terms: &[OrderTerm<'_, '_>],
    union_order: bool,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    validate_aggregate_projection(ast)?;
    let Some(source) = ast.source.as_ref() else {
        return compile_source_free_branch(ast, order_terms);
    };
    if let Some(join) = &ast.join {
        return compile_joined_branch(ast, join, snapshot, order_terms, union_order, presentations);
    }
    let qualified_name = format!("{}.{}", source.kind.lexeme, source.object.lexeme);
    let object = find_metadata_object(snapshot, &qualified_name)?;
    let live_table = object
        .physical_table
        .as_deref()
        .and_then(|physical| {
            snapshot
                .live_tables
                .iter()
                .find(|table| table.name.eq_ignore_ascii_case(physical))
        })
        .ok_or_else(|| QueryDiagnostic::at(Some(source.object), "metadata table is not live"))?;
    let fields = queryable_fields(snapshot, object)?;
    let compiled_source = compile_source_relation(source, snapshot, object, live_table, &fields)?;
    let mut context = CompilationContext {
        snapshot,
        object: ObjectId::from(&object.guid),
        fields: compiled_source.fields,
        source_alias: source.alias.map(|token| token.lexeme.to_owned()),
        object_name: source.object.lexeme.to_owned(),
        joins: Vec::new(),
    };

    let selected = if matches!(ast.projection.as_slice(), [Projection::All]) {
        context
            .fields
            .iter()
            .cloned()
            .map(ResolvedPath::base)
            .map(SelectedProjection::Field)
            .collect::<Vec<_>>()
    } else {
        if ast
            .projection
            .iter()
            .any(|projection| matches!(projection, Projection::All))
        {
            return Err(QueryDiagnostic::at(
                Some(source.object),
                "'*' cannot be combined with named fields",
            ));
        }
        let mut selected = Vec::with_capacity(ast.projection.len());
        for projection in &ast.projection {
            match projection {
                Projection::Field(reference) => {
                    selected.push(SelectedProjection::Field(context.resolve_path(reference)?))
                }
                Projection::Presentation {
                    token,
                    operation,
                    argument,
                } => {
                    let (sql, label) = compile_single_presentation(
                        &mut context,
                        token,
                        *operation,
                        argument,
                        presentations,
                    )?;
                    selected.push(SelectedProjection::Generated { sql, label });
                }
                Projection::Scalar(expression) => {
                    let number = selected.len() + 1;
                    let sql = compile_expression(expression, &mut context)?;
                    selected.push(SelectedProjection::Generated {
                        sql: format!("({sql})::text"),
                        label: format!("column{number}"),
                    });
                }
                Projection::Aggregate {
                    token,
                    kind,
                    distinct,
                    argument,
                } => {
                    let sql = compile_aggregate_single(&mut context, *kind, *distinct, argument)?;
                    selected.push(SelectedProjection::Generated {
                        sql,
                        label: token.lexeme.to_owned(),
                    });
                }
                Projection::All => unreachable!(),
            }
        }
        selected
    };

    let mut columns = Vec::new();
    let mut projections = Vec::new();
    for selected in &selected {
        match selected {
            SelectedProjection::Field(resolved) => {
                for column in &resolved.field.columns {
                    let output_label = resolved.output_label(column);
                    projections.push(format!(
                        "{}::text AS {}",
                        qualified_column(
                            Some(resolved.sql_alias(context.base_alias())),
                            &column.physical_name
                        ),
                        quote_identifier(&output_label)
                    ));
                    columns.push(output_label);
                }
            }
            SelectedProjection::Generated { sql, label } => {
                projections.push(format!("{sql} AS {}", quote_identifier(label)));
                columns.push(label.clone());
            }
        }
    }
    if projections.is_empty() {
        return Err(QueryDiagnostic::at(
            Some(source.object),
            "metadata table has no queryable live columns",
        ));
    }

    let filter = ast
        .filter
        .as_ref()
        .map(|filter| compile_expression(filter, &mut context))
        .transpose()?;
    let order = order_terms
        .iter()
        .map(|term| {
            let resolved = context.resolve_path(&term.field)?;
            let column = single_column(&resolved.field, term.field.last())?;
            let expression = if union_order {
                selected_column_position(&selected, &resolved, &column.physical_name)
                    .ok_or_else(|| {
                        QueryDiagnostic::at(
                            Some(term.field.last()),
                            "UNION ORDER BY field must occur in the first branch projection",
                        )
                    })?
                    .to_string()
            } else {
                qualified_column(
                    Some(resolved.sql_alias(context.base_alias())),
                    &column.physical_name,
                )
            };
            Ok(format!(
                "{expression}{}",
                if term.descending { " DESC" } else { " ASC" }
            ))
        })
        .collect::<Result<Vec<_>, QueryDiagnostic>>()?;

    let mut sql = String::from("SELECT ");
    if ast.distinct {
        sql.push_str("DISTINCT ");
    }
    sql.push_str(&projections.join(", "));
    sql.push_str(" FROM ");
    sql.push_str(&compiled_source.sql);
    sql.push_str(" AS ");
    sql.push_str(&quote_identifier(context.base_alias()));
    for join in &context.joins {
        sql.push_str(" LEFT JOIN ");
        sql.push_str(&quote_identifier(&join.target_table));
        sql.push_str(" AS ");
        sql.push_str(&quote_identifier(&join.alias));
        sql.push_str(" ON ");
        sql.push_str(&qualified_column(
            Some(context.base_alias()),
            &join.source_column,
        ));
        sql.push_str(" = ");
        sql.push_str(&qualified_column(Some(&join.alias), &join.target_id_column));
        append_type_guard(&mut sql, context.base_alias(), join);
    }
    if let Some(filter) = filter {
        sql.push_str(" WHERE ");
        sql.push_str(&filter);
    }
    if !order.is_empty() && !union_order {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order.join(", "));
    }
    if let Some(top) = ast.top {
        sql.push_str(" LIMIT ");
        sql.push_str(&top.to_string());
    }
    Ok(CompiledBranch {
        sql,
        columns,
        logical_width: selected.len(),
        order,
    })
}

struct FullJoinCondition {
    sql: String,
    left_marker: String,
}

fn compile_joined_branch(
    ast: &SelectAst<'_, '_>,
    join: &JoinAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    order_terms: &[OrderTerm<'_, '_>],
    union_order: bool,
    presentations: &mut PresentationCompilation<'_>,
) -> Result<CompiledBranch, QueryDiagnostic> {
    if join.kind == JoinKind::Full
        && ast
            .projection
            .iter()
            .any(|projection| matches!(projection, Projection::Aggregate { .. }))
    {
        return Err(QueryDiagnostic::at(
            Some(join.token),
            "aggregates over a transposed FULL JOIN are not supported",
        ));
    }
    if ast
        .projection
        .iter()
        .any(|projection| matches!(projection, Projection::All))
    {
        return Err(QueryDiagnostic::at(
            Some(join.token),
            "wildcard projection in JOIN is not supported",
        ));
    }

    let left_source = ast
        .source
        .as_ref()
        .expect("a joined branch always has a left source");
    let left = resolve_full_join_source(left_source, snapshot, "__left")?;
    let right = resolve_full_join_source(&join.source, snapshot, "__right")?;
    if names_equal(&left.sql_alias, &right.sql_alias) {
        return Err(QueryDiagnostic::at(
            Some(join.token),
            format!(
                "JOIN sources must have distinct aliases; both resolve to {:?}",
                left.sql_alias
            ),
        ));
    }
    let mut context = JoinedContext {
        snapshot,
        left,
        right,
    };
    let mut selected = Vec::with_capacity(ast.projection.len());
    for projection in &ast.projection {
        match projection {
            Projection::Field(reference) => {
                selected.push(JoinedProjection::Field(context.resolve(reference)?));
            }
            Projection::Presentation {
                token,
                operation,
                argument,
            } => {
                let (sql, label) = compile_joined_presentation(
                    &mut context,
                    token,
                    *operation,
                    argument,
                    presentations,
                )?;
                selected.push(JoinedProjection::Generated { sql, label });
            }
            Projection::Scalar(expression) => {
                let number = selected.len() + 1;
                let sql = compile_full_join_expression(expression, &mut context)?;
                selected.push(JoinedProjection::Generated {
                    sql: format!("({sql})::text"),
                    label: format!("column{number}"),
                });
            }
            Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            } => {
                let sql = compile_aggregate_joined(&mut context, *kind, *distinct, argument)?;
                selected.push(JoinedProjection::Generated {
                    sql,
                    label: token.lexeme.to_owned(),
                });
            }
            Projection::All => unreachable!(),
        }
    }

    let mut columns = Vec::new();
    let mut projections = Vec::new();
    for selected in &selected {
        match selected {
            JoinedProjection::Field(resolved) => {
                for column in &resolved.field.columns {
                    let output_label = resolved.output_label(column);
                    projections.push(format!(
                        "{}::text AS {}",
                        context.sql_column(resolved, column),
                        quote_identifier(&output_label)
                    ));
                    columns.push(output_label);
                }
            }
            JoinedProjection::Generated { sql, label } => {
                projections.push(format!("{sql} AS {}", quote_identifier(label)));
                columns.push(label.clone());
            }
        }
    }
    if projections.is_empty() {
        return Err(QueryDiagnostic::at(
            Some(join.token),
            "JOIN has no queryable projected columns",
        ));
    }

    let condition = compile_full_join_condition(&join.condition, &context, join.token)?;
    let filter = ast
        .filter
        .as_ref()
        .map(|filter| compile_full_join_expression(filter, &mut context))
        .transpose()?;
    let order = order_terms
        .iter()
        .map(|term| {
            let resolved = context.resolve(&term.field)?;
            let column = single_column(&resolved.field, term.field.last())?;
            let position = selected_full_join_column_position(
                &selected,
                resolved.side,
                &resolved.sql_alias,
                &column.physical_name,
            )
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(term.field.last()),
                    "JOIN ORDER BY field must occur in the projection",
                )
            })?;
            Ok(format!(
                "{position}{}",
                if term.descending { " DESC" } else { " ASC" }
            ))
        })
        .collect::<Result<Vec<_>, QueryDiagnostic>>()?;

    let mut sql = if join.kind == JoinKind::Full {
        let first = compile_directional_full_join(
            &context,
            &context.left,
            &context.right,
            &projections,
            &condition.sql,
            filter.as_deref(),
            None,
        );
        let second = compile_directional_full_join(
            &context,
            &context.right,
            &context.left,
            &projections,
            &condition.sql,
            filter.as_deref(),
            Some(&condition.left_marker),
        );
        let mut sql = String::from("SELECT ");
        if ast.distinct {
            sql.push_str("DISTINCT ");
        }
        sql.push_str("* FROM ((");
        sql.push_str(&first);
        sql.push_str(") UNION ALL (");
        sql.push_str(&second);
        sql.push_str(")) AS \"__full\"");
        sql
    } else {
        compile_native_join(
            ast,
            join.kind,
            &context,
            &projections,
            &condition.sql,
            filter.as_deref(),
        )
    };
    if !order.is_empty() && !union_order {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order.join(", "));
    }
    if let Some(top) = ast.top {
        sql.push_str(" LIMIT ");
        sql.push_str(&top.to_string());
    }

    Ok(CompiledBranch {
        sql,
        columns,
        logical_width: selected.len(),
        order,
    })
}

fn resolve_full_join_source(
    source: &SourceAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    default_alias: &str,
) -> Result<JoinedSource, QueryDiagnostic> {
    let qualified_name = format!("{}.{}", source.kind.lexeme, source.object.lexeme);
    let object = find_metadata_object(snapshot, &qualified_name)?;
    let live_table = object
        .physical_table
        .as_deref()
        .and_then(|physical| {
            snapshot
                .live_tables
                .iter()
                .find(|table| table.name.eq_ignore_ascii_case(physical))
        })
        .ok_or_else(|| QueryDiagnostic::at(Some(source.object), "metadata table is not live"))?;
    let fields = queryable_fields(snapshot, object)?;
    let compiled_source = compile_source_relation(source, snapshot, object, live_table, &fields)?;
    Ok(JoinedSource {
        object: ObjectId::from(&object.guid),
        fields: compiled_source.fields,
        relation: compiled_source.sql,
        sql_alias: source
            .alias
            .map_or_else(|| default_alias.to_owned(), |token| token.lexeme.to_owned()),
        object_name: source.object.lexeme.to_owned(),
        source_alias: source.alias.map(|token| token.lexeme.to_owned()),
        reference_joins: Vec::new(),
    })
}

fn compile_full_join_condition(
    expression: &Expression<'_, '_>,
    context: &JoinedContext<'_>,
    token: &Token<'_>,
) -> Result<FullJoinCondition, QueryDiagnostic> {
    let mut parts = Vec::new();
    let mut left_marker = None;
    collect_full_join_equalities(expression, context, token, &mut parts, &mut left_marker)?;
    Ok(FullJoinCondition {
        sql: parts.join(" AND "),
        left_marker: left_marker.expect("a valid FULL JOIN condition has an equality"),
    })
}

fn collect_full_join_equalities(
    expression: &Expression<'_, '_>,
    context: &JoinedContext<'_>,
    token: &Token<'_>,
    parts: &mut Vec<String>,
    left_marker: &mut Option<String>,
) -> Result<(), QueryDiagnostic> {
    match expression {
        Expression::Binary {
            left,
            operator,
            right,
        } if operator.kind == TokenKind::Keyword(Keyword::And) => {
            collect_full_join_equalities(left, context, token, parts, left_marker)?;
            collect_full_join_equalities(right, context, token, parts, left_marker)
        }
        Expression::Binary {
            left,
            operator,
            right,
        } if operator.lexeme == "=" => {
            let (Expression::Field(left_reference), Expression::Field(right_reference)) =
                (left.as_ref(), right.as_ref())
            else {
                return Err(QueryDiagnostic::at(
                    Some(operator),
                    "JOIN equality must compare fields from opposing sources",
                ));
            };
            let left_field = context.resolve_direct(left_reference)?;
            let right_field = context.resolve_direct(right_reference)?;
            if left_field.side == right_field.side {
                return Err(QueryDiagnostic::at(
                    Some(operator),
                    "JOIN equality must compare fields from opposing sources",
                ));
            }
            let left_column = single_column(&left_field.field, left_reference.last())?;
            let right_column = single_column(&right_field.field, right_reference.last())?;
            let left_sql = context.sql_column(&left_field, left_column);
            let right_sql = context.sql_column(&right_field, right_column);
            if left_marker.is_none() {
                let (resolved, column) = if left_field.side == JoinedSide::Left {
                    (&left_field, left_column)
                } else {
                    (&right_field, right_column)
                };
                *left_marker = Some(context.sql_column(resolved, column));
            }
            parts.push(format!("{left_sql} = {right_sql}"));
            Ok(())
        }
        _ => Err(QueryDiagnostic::at(
            Some(token),
            "JOIN condition supports only cross-source field equalities combined by AND",
        )),
    }
}

fn compile_full_join_expression(
    expression: &Expression<'_, '_>,
    context: &mut JoinedContext<'_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = single_column(&resolved.field, reference.last())?;
            Ok(context.sql_column(&resolved, column))
        }
        Expression::Literal(token) => compile_literal(token),
        Expression::Unary { operator, value } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::Not) => "NOT ",
                _ if operator.lexeme == "+" => "+",
                _ if operator.lexeme == "-" => "-",
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported unary operator",
                    ));
                }
            };
            Ok(format!(
                "({operator}{})",
                compile_full_join_expression(value, context)?
            ))
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::And) => "AND",
                TokenKind::Keyword(Keyword::Or) => "OR",
                _ if matches!(
                    operator.lexeme,
                    "=" | "<>" | "<" | ">" | "<=" | ">=" | "+" | "-" | "*" | "/"
                ) =>
                {
                    operator.lexeme
                }
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported binary operator",
                    ));
                }
            };
            let left = compile_full_join_expression(left, context)?;
            let right = compile_full_join_expression(right, context)?;
            Ok(format!("({left} {operator} {right})"))
        }
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_full_join_expression(value, context)?,
            if *negated { "NOT " } else { "" }
        )),
    }
}

fn compile_directional_full_join(
    context: &JoinedContext<'_>,
    base: &JoinedSource,
    joined: &JoinedSource,
    projections: &[String],
    condition: &str,
    filter: Option<&str>,
    anti_match: Option<&str>,
) -> String {
    let mut sql = format!(
        "SELECT {} FROM {} AS {} LEFT JOIN {} AS {} ON {}",
        projections.join(", "),
        base.relation,
        quote_identifier(&base.sql_alias),
        joined.relation,
        quote_identifier(&joined.sql_alias),
        condition,
    );
    append_joined_reference_joins(&mut sql, context);
    let mut predicates = Vec::new();
    if let Some(anti_match) = anti_match {
        predicates.push(format!("({anti_match} IS NULL)"));
    }
    if let Some(filter) = filter {
        predicates.push(filter.to_owned());
    }
    if !predicates.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&predicates.join(" AND "));
    }
    sql
}

fn compile_native_join(
    ast: &SelectAst<'_, '_>,
    kind: JoinKind,
    context: &JoinedContext<'_>,
    projections: &[String],
    condition: &str,
    filter: Option<&str>,
) -> String {
    let operator = match kind {
        JoinKind::Inner => "INNER JOIN",
        JoinKind::Left => "LEFT JOIN",
        JoinKind::Right => "RIGHT JOIN",
        JoinKind::Full => unreachable!("FULL JOIN is transposed separately"),
    };
    let mut sql = String::from("SELECT ");
    if ast.distinct {
        sql.push_str("DISTINCT ");
    }
    sql.push_str(&projections.join(", "));
    sql.push_str(" FROM ");
    sql.push_str(&context.left.relation);
    sql.push_str(" AS ");
    sql.push_str(&quote_identifier(&context.left.sql_alias));
    sql.push(' ');
    sql.push_str(operator);
    sql.push(' ');
    sql.push_str(&context.right.relation);
    sql.push_str(" AS ");
    sql.push_str(&quote_identifier(&context.right.sql_alias));
    sql.push_str(" ON ");
    sql.push_str(condition);
    append_joined_reference_joins(&mut sql, context);
    if let Some(filter) = filter {
        sql.push_str(" WHERE ");
        sql.push_str(filter);
    }
    sql
}

fn append_joined_reference_joins(sql: &mut String, context: &JoinedContext<'_>) {
    for source in [&context.left, &context.right] {
        for join in &source.reference_joins {
            sql.push_str(" LEFT JOIN ");
            sql.push_str(&quote_identifier(&join.target_table));
            sql.push_str(" AS ");
            sql.push_str(&quote_identifier(&join.alias));
            sql.push_str(" ON ");
            sql.push_str(&qualified_column(
                Some(&source.sql_alias),
                &join.source_column,
            ));
            sql.push_str(" = ");
            sql.push_str(&qualified_column(Some(&join.alias), &join.target_id_column));
            append_type_guard(sql, &source.sql_alias, join);
        }
    }
}

fn append_type_guard(sql: &mut String, source_alias: &str, join: &JoinPlan) {
    if let (Some(column), Some(number)) = (&join.source_type_column, join.database_type) {
        sql.push_str(" AND ");
        sql.push_str(&qualified_column(Some(source_alias), column));
        sql.push_str(" = '\\x");
        for byte in number.to_be_bytes() {
            use std::fmt::Write as _;
            write!(sql, "{byte:02x}").expect("writing to String cannot fail");
        }
        sql.push_str("'::bytea");
    }
}

fn selected_full_join_column_position(
    selected: &[JoinedProjection],
    ordered_side: JoinedSide,
    ordered_alias: &str,
    ordered_column: &str,
) -> Option<usize> {
    let mut position = 1;
    for selected in selected {
        match selected {
            JoinedProjection::Field(resolved) => {
                for column in &resolved.field.columns {
                    if resolved.side == ordered_side
                        && names_equal(&resolved.sql_alias, ordered_alias)
                        && names_equal(&column.physical_name, ordered_column)
                    {
                        return Some(position);
                    }
                    position += 1;
                }
            }
            JoinedProjection::Generated { .. } => position += 1,
        }
    }
    None
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
                for column in &resolved.field.columns {
                    if field_sources_equal(&resolved.source, &ordered.source)
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

fn field_sources_equal(left: &FieldSource, right: &FieldSource) -> bool {
    match (left, right) {
        (FieldSource::Base, FieldSource::Base) => true,
        (FieldSource::Join(left), FieldSource::Join(right)) => names_equal(left, right),
        _ => false,
    }
}

fn compile_expression(
    expression: &Expression<'_, '_>,
    context: &mut CompilationContext<'_>,
) -> Result<String, QueryDiagnostic> {
    match expression {
        Expression::Field(reference) => {
            let resolved = context.resolve_path(reference)?;
            let column = single_column(&resolved.field, reference.last())?;
            Ok(qualified_column(
                Some(resolved.sql_alias(context.base_alias())),
                &column.physical_name,
            ))
        }
        Expression::Literal(token) => compile_literal(token),
        Expression::Unary { operator, value } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::Not) => "NOT ",
                _ if operator.lexeme == "+" => "+",
                _ if operator.lexeme == "-" => "-",
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported unary operator",
                    ));
                }
            };
            Ok(format!(
                "({operator}{})",
                compile_expression(value, context)?
            ))
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let operator = match operator.kind {
                TokenKind::Keyword(Keyword::And) => "AND",
                TokenKind::Keyword(Keyword::Or) => "OR",
                _ if matches!(
                    operator.lexeme,
                    "=" | "<>" | "<" | ">" | "<=" | ">=" | "+" | "-" | "*" | "/"
                ) =>
                {
                    operator.lexeme
                }
                _ => {
                    return Err(QueryDiagnostic::at(
                        Some(operator),
                        "unsupported binary operator",
                    ));
                }
            };
            let left = compile_expression(left, context)?;
            let right = compile_expression(right, context)?;
            Ok(format!("({left} {operator} {right})"))
        }
        Expression::IsNull { value, negated } => Ok(format!(
            "({} IS {}NULL)",
            compile_expression(value, context)?,
            if *negated { "NOT " } else { "" }
        )),
    }
}

fn compile_literal(token: &Token<'_>) -> Result<String, QueryDiagnostic> {
    match token.kind {
        TokenKind::String => {
            let inner = token
                .lexeme
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| QueryDiagnostic::at(Some(token), "invalid string literal"))?;
            Ok(format!(
                "'{}'",
                inner.replace("\"\"", "\"").replace('\'', "''")
            ))
        }
        TokenKind::Number => Ok(token.lexeme.to_owned()),
        TokenKind::Keyword(Keyword::True) => Ok("TRUE".to_owned()),
        TokenKind::Keyword(Keyword::False) => Ok("FALSE".to_owned()),
        TokenKind::Keyword(Keyword::Null) => Ok("NULL".to_owned()),
        _ => Err(QueryDiagnostic::at(Some(token), "unsupported literal")),
    }
}

#[derive(Debug, Clone)]
enum FieldSource {
    Base,
    Join(String),
}

#[derive(Debug, Clone)]
struct ResolvedPath {
    field: QueryableField,
    source: FieldSource,
    path_label: Option<String>,
}

impl ResolvedPath {
    fn base(field: QueryableField) -> Self {
        Self {
            field,
            source: FieldSource::Base,
            path_label: None,
        }
    }

    fn sql_alias<'alias>(&'alias self, base_alias: &'alias str) -> &'alias str {
        match &self.source {
            FieldSource::Base => base_alias,
            FieldSource::Join(alias) => alias,
        }
    }

    fn output_label(&self, column: &QueryableColumn) -> String {
        let Some(path_label) = &self.path_label else {
            return column.output_label.clone();
        };
        if self.field.columns.len() == 1 {
            return path_label.clone();
        }
        column
            .output_label
            .strip_prefix(&self.field.name)
            .map_or_else(
                || format!("{path_label}_{}", column.output_label),
                |suffix| format!("{path_label}{suffix}"),
            )
    }
}

#[derive(Debug, Clone)]
struct JoinPlan {
    source_field: String,
    source_column: String,
    source_type_column: Option<String>,
    database_type: Option<u32>,
    target_table: String,
    target_id_column: String,
    alias: String,
}

struct CompilationContext<'snapshot> {
    snapshot: &'snapshot MetadataSnapshot,
    object: ObjectId,
    fields: Vec<QueryableField>,
    source_alias: Option<String>,
    object_name: String,
    joins: Vec<JoinPlan>,
}

impl CompilationContext<'_> {
    fn base_alias(&self) -> &str {
        self.source_alias.as_deref().unwrap_or("__src")
    }

    fn is_source_qualifier(&self, name: &str) -> bool {
        self.source_alias
            .as_deref()
            .is_some_and(|alias| names_equal(alias, name))
            || names_equal(&self.object_name, name)
    }

    fn resolve_path(
        &mut self,
        reference: &FieldReference<'_, '_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        match reference.segments.as_slice() {
            [field] => resolve_named_field(&self.fields, field).map(ResolvedPath::base),
            [first, second] if self.is_source_qualifier(first.lexeme) => {
                resolve_named_field(&self.fields, second).map(ResolvedPath::base)
            }
            [reference_field, target_field] => {
                self.resolve_dereference(reference_field, target_field)
            }
            [qualifier, reference_field, target_field] => {
                if !self.is_source_qualifier(qualifier.lexeme) {
                    return Err(QueryDiagnostic::at(
                        Some(qualifier),
                        format!("unknown source qualifier {:?}", qualifier.lexeme),
                    ));
                }
                self.resolve_dereference(reference_field, target_field)
            }
            [_, _, _, unsupported, ..] => Err(QueryDiagnostic::at(
                Some(unsupported),
                "reference paths deeper than one hop are not supported",
            )),
            [] => unreachable!("field path is non-empty"),
        }
    }

    fn resolve_dereference(
        &mut self,
        reference_token: &Token<'_>,
        target_token: &Token<'_>,
    ) -> Result<ResolvedPath, QueryDiagnostic> {
        let reference_field = resolve_named_field(&self.fields, reference_token)?;
        let target_table = reference_field.reference_target.as_deref().ok_or_else(|| {
            QueryDiagnostic::at(
                Some(reference_token),
                format!(
                    "field {:?} has no unique SchemaStorage reference target",
                    reference_token.lexeme
                ),
            )
        })?;
        let source_column = reference_column(&reference_field, reference_token)?
            .physical_name
            .clone();
        let target_physical = format!("_{}", target_table.trim_start_matches('_'));
        let target_objects = self
            .snapshot
            .objects
            .iter()
            .filter(|object| {
                object
                    .physical_table
                    .as_deref()
                    .is_some_and(|table| table.eq_ignore_ascii_case(&target_physical))
            })
            .collect::<Vec<_>>();
        let target_object = match target_objects.as_slice() {
            [object] => *object,
            [] => {
                return Err(QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} was not resolved"),
                ));
            }
            _ => {
                return Err(QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} is ambiguous"),
                ));
            }
        };
        let target_live_table = self
            .snapshot
            .live_tables
            .iter()
            .find(|table| table.name.eq_ignore_ascii_case(&target_physical))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target table {target_physical:?} is not live"),
                )
            })?;
        let target_fields = queryable_fields(self.snapshot, target_object)?;
        let target_field = resolve_named_field(&target_fields, target_token)?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| {
                QueryDiagnostic::at(
                    Some(reference_token),
                    format!("reference target {target_physical:?} has no ID field"),
                )
            })?;
        let target_id_column = single_column(target_id, reference_token)?
            .physical_name
            .clone();

        let alias = if let Some(join) = self
            .joins
            .iter()
            .find(|join| names_equal(&join.source_field, &reference_field.schema_name))
        {
            join.alias.clone()
        } else {
            let alias = self.next_join_alias();
            self.joins.push(JoinPlan {
                source_field: reference_field.schema_name,
                source_column,
                source_type_column: None,
                database_type: None,
                target_table: target_live_table.name.clone(),
                target_id_column,
                alias: alias.clone(),
            });
            alias
        };
        Ok(ResolvedPath {
            field: target_field,
            source: FieldSource::Join(alias),
            path_label: Some(format!(
                "{}.{}",
                reference_token.lexeme, target_token.lexeme
            )),
        })
    }

    fn next_join_alias(&self) -> String {
        let mut number = self.joins.len() + 1;
        loop {
            let candidate = format!("__ref{number}");
            if !names_equal(self.base_alias(), &candidate)
                && self
                    .joins
                    .iter()
                    .all(|join| !names_equal(&join.alias, &candidate))
            {
                return candidate;
            }
            number += 1;
        }
    }

    fn ensure_presentation_join(
        &mut self,
        reference: &QueryableField,
        target: ObjectId,
        multiple: bool,
        token: &Token<'_>,
    ) -> Result<String, QueryDiagnostic> {
        let source_column = reference_column(reference, token)?.physical_name.clone();
        let source_type_column = multiple
            .then(|| reference_type_column(reference, token))
            .transpose()?
            .map(|column| column.physical_name.clone());
        let target_object = self.snapshot.object_by_id(target).ok_or_else(|| {
            QueryDiagnostic::at(Some(token), "presentation target was not resolved")
        })?;
        let database_type = multiple.then_some(target_object.number).flatten();
        let target_table = target_object
            .physical_table
            .as_deref()
            .and_then(|physical| {
                self.snapshot
                    .live_tables
                    .iter()
                    .find(|table| table.name.eq_ignore_ascii_case(physical))
            })
            .ok_or_else(|| {
                QueryDiagnostic::at(Some(token), "presentation target table is not live")
            })?;
        let target_fields = queryable_fields(self.snapshot, target_object)?;
        let target_id = target_fields
            .iter()
            .find(|field| names_equal(&field.schema_name, "ID"))
            .ok_or_else(|| QueryDiagnostic::at(Some(token), "presentation target has no ID"))?;
        let target_id_column = single_column(target_id, token)?.physical_name.clone();
        if let Some(join) = self.joins.iter().find(|join| {
            names_equal(&join.source_field, &reference.schema_name)
                && names_equal(&join.target_table, &target_table.name)
                && join.database_type == database_type
        }) {
            return Ok(join.alias.clone());
        }
        let alias = self.next_join_alias();
        self.joins.push(JoinPlan {
            source_field: reference.schema_name.clone(),
            source_column,
            source_type_column,
            database_type,
            target_table: target_table.name.clone(),
            target_id_column,
            alias: alias.clone(),
        });
        Ok(alias)
    }
}

fn compile_aggregate_single(
    context: &mut CompilationContext<'_>,
    kind: AggregateKind,
    distinct: bool,
    argument: &AggregateArgument<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let argument = match argument {
        AggregateArgument::All => "*".to_owned(),
        AggregateArgument::Field(reference) => {
            let resolved = context.resolve_path(reference)?;
            let column = countable_column(&resolved.field, reference.last())?;
            qualified_column(
                Some(resolved.sql_alias(context.base_alias())),
                &column.physical_name,
            )
        }
    };
    Ok(format!(
        "{}({}{argument})::text",
        kind.sql_name(),
        if distinct { "DISTINCT " } else { "" }
    ))
}

fn compile_aggregate_joined(
    context: &mut JoinedContext<'_>,
    kind: AggregateKind,
    distinct: bool,
    argument: &AggregateArgument<'_, '_>,
) -> Result<String, QueryDiagnostic> {
    let argument = match argument {
        AggregateArgument::All => "*".to_owned(),
        AggregateArgument::Field(reference) => {
            let resolved = context.resolve(reference)?;
            let column = countable_column(&resolved.field, reference.last())?;
            context.sql_column(&resolved, column)
        }
    };
    Ok(format!(
        "{}({}{argument})::text",
        kind.sql_name(),
        if distinct { "DISTINCT " } else { "" }
    ))
}

fn countable_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ if !field.reference_targets.is_empty() => reference_column(field, token),
        _ => Err(QueryDiagnostic::at(
            Some(token),
            format!(
                "compound field {:?} cannot be used as an aggregate argument",
                field.name
            ),
        )),
    }
}

fn reference_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    let candidates = field
        .columns
        .iter()
        .filter(|column| {
            let lower = column.physical_name.to_ascii_lowercase();
            lower.ends_with("rref") && !lower.ends_with("rtref")
        })
        .collect::<Vec<_>>();
    match (field.columns.as_slice(), candidates.as_slice()) {
        ([column], _) => Ok(column),
        (_, [column]) => Ok(*column),
        _ => Err(QueryDiagnostic::at(
            Some(token),
            format!(
                "reference field {:?} has no unique RRef physical member",
                field.name
            ),
        )),
    }
}

fn reference_type_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    let candidates = field
        .columns
        .iter()
        .filter(|column| column.physical_name.to_ascii_lowercase().ends_with("rtref"))
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [column] => Ok(*column),
        _ => Err(QueryDiagnostic::at(
            Some(token),
            format!(
                "multi-target reference field {:?} has no unique RTRef physical member",
                field.name
            ),
        )),
    }
}

fn resolve_named_field(
    fields: &[QueryableField],
    name: &Token<'_>,
) -> Result<QueryableField, QueryDiagnostic> {
    let matches = matching_fields(fields, name);
    match matches.as_slice() {
        [field] => Ok((*field).clone()),
        [] => Err(QueryDiagnostic::at(
            Some(name),
            format!("field {:?} was not found", name.lexeme),
        )),
        _ => Err(QueryDiagnostic::at(
            Some(name),
            format!("field {:?} is ambiguous", name.lexeme),
        )),
    }
}

fn matching_fields<'field>(
    fields: &'field [QueryableField],
    name: &Token<'_>,
) -> Vec<&'field QueryableField> {
    fields
        .iter()
        .filter(|field| {
            field
                .aliases
                .iter()
                .any(|alias| names_equal(alias, name.lexeme))
        })
        .collect()
}

fn single_column<'field>(
    field: &'field QueryableField,
    token: &Token<'_>,
) -> Result<&'field QueryableColumn, QueryDiagnostic> {
    match field.columns.as_slice() {
        [column] => Ok(column),
        _ => Err(QueryDiagnostic::at(
            Some(token),
            format!(
                "compound field {:?} can be projected but not used in expressions",
                field.name
            ),
        )),
    }
}

fn logical_column_name(physical_name: &str) -> String {
    crate::metadata::collapse_logical_fields([physical_name])
        .into_iter()
        .next()
        .map_or_else(
            || physical_name.trim_start_matches('_').to_owned(),
            |field| field.name,
        )
}

fn reference_targets(column: &SchemaColumn) -> Vec<String> {
    column
        .types
        .iter()
        .filter_map(|column_type| column_type.reference_target.as_deref())
        .fold(Vec::<String>::new(), |mut targets, target| {
            if !targets
                .iter()
                .any(|candidate| names_equal(candidate, target))
            {
                targets.push(target.to_owned());
            }
            targets
        })
}

fn custom_field_name(
    snapshot: &MetadataSnapshot,
    physical_table: &str,
    schema_name: &str,
) -> Option<String> {
    let number = schema_name.strip_prefix("Fld")?.parse::<u32>().ok()?;
    snapshot
        .fields
        .iter()
        .find(|field| {
            field.number == number
                && field
                    .owner_tables
                    .iter()
                    .any(|owner| owner.eq_ignore_ascii_case(physical_table))
        })?
        .name
        .clone()
}

fn indexed_custom_field_name(
    names: &CustomFieldNameIndex,
    physical_table: &str,
    schema_name: &str,
) -> Option<String> {
    let number = schema_name.strip_prefix("Fld")?.parse::<u32>().ok()?;
    names
        .get(&(physical_table.to_lowercase(), number))
        .cloned()
        .flatten()
}

fn query_schema_name(schema_name: &str) -> String {
    match schema_name {
        "Date_Time" => "Date".to_owned(),
        _ => schema_name.to_owned(),
    }
}

fn push_unique_name(names: &mut Vec<String>, name: String) {
    if !names.iter().any(|candidate| names_equal(candidate, &name)) {
        names.push(name);
    }
}

fn compound_label(display_name: &str, schema_name: &str, physical_name: &str) -> String {
    let canonical = recase_postgres_identifier(physical_name);
    let base = format!("_{schema_name}");
    canonical.strip_prefix(&base).map_or_else(
        || format!("{display_name}_{}", canonical.trim_start_matches('_')),
        |suffix| format!("{display_name}{suffix}"),
    )
}

fn standard_field_aliases(schema_name: &str) -> &'static [&'static str] {
    match schema_name {
        "ID" => &["ID", "Ссылка"],
        "Code" => &["Code", "Код"],
        "Description" => &["Description", "Наименование"],
        "Marked" => &["Marked", "ПометкаУдаления"],
        "Version" => &["Version", "ВерсияДанных"],
        "Number" => &["Number", "Номер"],
        "Date" => &["Date", "Дата"],
        "Posted" => &["Posted", "Проведен"],
        "Recorder" => &["Recorder", "Регистратор"],
        "LineNo" => &["LineNo", "НомерСтроки"],
        "Period" => &["Period", "Период"],
        "Active" => &["Active", "Активность"],
        _ => &[],
    }
}

fn kind_from_query_name(name: &str) -> Option<MetadataKind> {
    let names = [
        (
            MetadataKind::Catalog,
            ["Catalog", "Reference", "Справочник"],
        ),
        (MetadataKind::Document, ["Document", "Document", "Документ"]),
        (
            MetadataKind::Enumeration,
            ["Enumeration", "Enum", "Перечисление"],
        ),
        (
            MetadataKind::InformationRegister,
            ["InformationRegister", "InfoRg", "РегистрСведений"],
        ),
        (
            MetadataKind::AccumulationRegister,
            ["AccumulationRegister", "AccumRg", "РегистрНакопления"],
        ),
        (
            MetadataKind::AccountingRegister,
            ["AccountingRegister", "AccRg", "РегистрБухгалтерии"],
        ),
        (
            MetadataKind::CalculationRegister,
            ["CalculationRegister", "CRg", "РегистрРасчета"],
        ),
        (
            MetadataKind::ChartOfCharacteristicTypes,
            [
                "ChartOfCharacteristicTypes",
                "Chrc",
                "ПланВидовХарактеристик",
            ],
        ),
        (
            MetadataKind::ChartOfCalculationTypes,
            ["ChartOfCalculationTypes", "CKinds", "ПланВидовРасчета"],
        ),
        (
            MetadataKind::ChartOfAccounts,
            ["ChartOfAccounts", "Acc", "ПланСчетов"],
        ),
        (MetadataKind::Constant, ["Constant", "Const", "Константа"]),
        (
            MetadataKind::ExchangePlan,
            ["ExchangePlan", "Node", "ПланОбмена"],
        ),
        (
            MetadataKind::BusinessProcess,
            ["BusinessProcess", "BPr", "БизнесПроцесс"],
        ),
        (MetadataKind::Task, ["Task", "Task", "Задача"]),
        (
            MetadataKind::Sequence,
            ["Sequence", "Seq", "Последовательность"],
        ),
    ];
    names.into_iter().find_map(|(kind, aliases)| {
        aliases
            .iter()
            .any(|alias| names_equal(alias, name))
            .then_some(kind)
    })
}

fn is_comparison(operator: &str) -> bool {
    matches!(operator, "=" | "<>" | "<" | ">" | "<=" | ">=")
}

fn is_contextual_identifier(kind: TokenKind) -> bool {
    kind == TokenKind::Identifier
        || matches!(
            kind,
            TokenKind::Keyword(
                Keyword::Count
                    | Keyword::Sum
                    | Keyword::Min
                    | Keyword::Max
                    | Keyword::Presentation
                    | Keyword::RefPresentation
                    | Keyword::SliceFirst
                    | Keyword::SliceLast
                    | Keyword::Balance
                    | Keyword::Turnovers
            )
        )
}

fn is_ascending_order(token: &Token<'_>) -> bool {
    names_equal(token.lexeme, "ASC") || names_equal(token.lexeme, "ВОЗР")
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn qualified_column(alias: Option<&str>, column: &str) -> String {
    alias.map_or_else(
        || quote_identifier(column),
        |alias| format!("{}.{}", quote_identifier(alias), quote_identifier(column)),
    )
}

fn names_equal(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}
