//! Прямые имена метаданных из существующего ограниченного дерева запроса.

use super::ast::*;
use super::names::folded_name;
use super::parser::Parser;
use super::resolve::kind_from_query_name;
use super::{QueryDiagnostic, QueryDiagnosticKind};
use crate::metadata::MetadataKind;
use crate::{Token, TokenKind, tokenize};
use std::collections::HashSet;

/// Прямая ссылка запроса на именованный объект метаданных.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryMetadataReference {
    /// Класс метаданных с учётом существующих русских и английских имён.
    pub kind: MetadataKind,
    /// Первое встреченное написание имени объекта.
    pub name: String,
}

/// Нужные запросу поля единой таблицы констант.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum QueryConstantReferences {
    /// Поля констант не упомянуты.
    #[default]
    None,
    /// Однозначно определённые имена констант без повторов.
    Named(Vec<String>),
    /// Звёздочка или неоднозначное разрешение требуют полного состава.
    All,
}

/// Прямые зависимости; ссылочные типы и применённые RLS требуют продолжения.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryMetadataReferences {
    /// Объекты в порядке первого появления.
    pub objects: Vec<QueryMetadataReference>,
    /// Поля таблицы констант.
    pub constants: QueryConstantReferences,
}

/// Находит прямые имена через тот же AST, которым компилируются запросы.
/// Не требует снимка и не строит SQL. Не обещает транзитивную полноту:
/// вызывающая сторона отдельно приобретает ссылочные типы и анализирует
/// реально применённые ограничения доступа. Лимиты и синтаксис — у парсера.
///
/// # Errors
/// Возвращает обычную позиционную диагностику лексера, парсера или неизвестного
/// класса метаданных; динамические имена не угадываются.
pub fn query_metadata_references(source: &str) -> Result<QueryMetadataReferences, QueryDiagnostic> {
    let tokens = tokenize(source)?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let ast = Parser::new(&tokens, source).parse()?;
    let mut visitor = Discovery::default();
    for statement in &ast.statements {
        if let StatementAst::Query(query) = statement {
            visitor.query(query)?;
        }
    }
    Ok(visitor.result)
}

#[derive(Default)]
struct Discovery {
    result: QueryMetadataReferences,
    objects: HashSet<(MetadataKind, String)>,
    constants: HashSet<String>,
}
#[derive(Default)]
struct Scope {
    constants: Vec<String>,
    others: usize,
    aliases: HashSet<String>,
    labels: HashSet<String>,
}
impl Scope {
    fn source(&mut self, source: &SourceAst<'_, '_>) {
        let alias = folded_name(source.alias.unwrap_or(source.object).lexeme);
        if source.constants {
            self.constants.push(alias);
        } else {
            self.others += 1;
            self.aliases.insert(alias);
        }
    }
}
impl Discovery {
    fn object(&mut self, kind: &Token<'_>, object: &Token<'_>) -> Result<(), QueryDiagnostic> {
        let kind = kind_from_query_name(kind.lexeme).ok_or_else(|| {
            QueryDiagnostic::at(
                QueryDiagnosticKind::UnknownObject,
                Some(kind),
                "unknown metadata kind",
            )
        })?;
        if self.objects.insert((kind, folded_name(object.lexeme))) {
            self.result.objects.push(QueryMetadataReference {
                kind,
                name: object.lexeme.into(),
            });
        }
        Ok(())
    }
    fn constant(&mut self, name: &str) {
        if matches!(self.result.constants, QueryConstantReferences::All) {
            return;
        }
        if self.constants.insert(folded_name(name)) {
            match &mut self.result.constants {
                QueryConstantReferences::None => {
                    self.result.constants = QueryConstantReferences::Named(vec![name.into()])
                }
                QueryConstantReferences::Named(names) => names.push(name.into()),
                QueryConstantReferences::All => {}
            }
        }
    }
    fn field(&mut self, field: &FieldReference<'_, '_>, scope: &Scope, label: bool) {
        if scope.constants.is_empty() {
            return;
        }
        let first = folded_name(field.segments[0].lexeme);
        if field.segments.len() > 1 {
            if scope.constants.contains(&first) && scope.aliases.contains(&first) {
                self.result.constants = QueryConstantReferences::All;
            } else if scope.constants.contains(&first) {
                self.constant(field.segments[1].lexeme);
            } else if !scope.aliases.contains(&first) {
                self.result.constants = QueryConstantReferences::All;
            }
        } else if !(label && scope.labels.contains(&first)) {
            if scope.others == 0 && scope.constants.len() == 1 {
                self.constant(field.segments[0].lexeme);
            } else {
                self.result.constants = QueryConstantReferences::All;
            }
        }
    }
    fn query(&mut self, query: &QueryAst<'_, '_>) -> Result<(), QueryDiagnostic> {
        let mut first_scope = None;
        for branch in &query.branches {
            let mut scope = Scope::default();
            if let Some(source) = &branch.source {
                scope.source(source);
            }
            for join in &branch.joins {
                scope.source(&join.source);
            }
            for item in &branch.projection {
                if let Some(alias) = item.alias {
                    scope.labels.insert(folded_name(alias.lexeme));
                }
            }
            if let Some(source) = &branch.source {
                self.source(source, &scope)?;
            }
            for join in &branch.joins {
                self.source(&join.source, &scope)?;
                if let Some(expression) = &join.condition {
                    self.expression(expression, &scope)?;
                }
            }
            for item in &branch.projection {
                self.projection(&item.expression, &scope)?;
            }
            for expression in [&branch.filter, &branch.having].into_iter().flatten() {
                self.expression(expression, &scope)?;
            }
            for key in &branch.group {
                self.expression(&key.expression, &scope)?;
            }
            if first_scope.is_none() {
                first_scope = Some(scope);
            }
        }
        if let Some(scope) = first_scope {
            for order in &query.order {
                match &order.key {
                    OrderKeyAst::Field(field) => self.field(field, &scope, true),
                    OrderKeyAst::Expression(expression) => self.expression(expression, &scope)?,
                }
            }
            if let Some(totals) = &query.totals {
                for field in &totals.fields {
                    self.expression(&field.expression, &scope)?;
                }
                for point in &totals.points {
                    self.field(&point.field, &scope, true);
                }
            }
        }
        Ok(())
    }
    fn source(&mut self, source: &SourceAst<'_, '_>, scope: &Scope) -> Result<(), QueryDiagnostic> {
        if let Some(nested) = &source.nested {
            self.query(nested)?;
        } else if !source.temporary && !source.parameter && !source.constants {
            self.object(source.kind, source.object)?;
        }
        if let Some(slice) = &source.slice {
            for expression in [&slice.period, &slice.condition].into_iter().flatten() {
                self.expression(expression, scope)?;
            }
        }
        if let Some(accumulation) = &source.accumulation {
            for expression in accumulation.arguments.iter().flatten() {
                self.expression(expression, scope)?;
            }
        }
        if let Some(expression) = &source.criterion {
            self.expression(expression, scope)?;
        }
        Ok(())
    }
    fn argument(
        &mut self,
        argument: &AggregateArgument<'_, '_>,
        scope: &Scope,
    ) -> Result<(), QueryDiagnostic> {
        match argument {
            AggregateArgument::All => {
                if !scope.constants.is_empty() {
                    self.result.constants = QueryConstantReferences::All;
                }
            }
            AggregateArgument::Expression(expression) => self.expression(expression, scope)?,
        }
        Ok(())
    }
    fn projection(
        &mut self,
        projection: &Projection<'_, '_>,
        scope: &Scope,
    ) -> Result<(), QueryDiagnostic> {
        match projection {
            Projection::All => {
                if !scope.constants.is_empty() {
                    self.result.constants = QueryConstantReferences::All;
                }
            }
            Projection::Field(field) => self.field(field, scope, false),
            Projection::Scalar(expression) => self.expression(expression, scope)?,
            Projection::Aggregate { argument, .. } => self.argument(argument, scope)?,
            Projection::Presentation { argument, .. } => match argument {
                PresentationArgument::Field(field) => self.field(field, scope, false),
                PresentationArgument::Literal(_) => {}
                PresentationArgument::Expression(expression) => {
                    self.expression(expression, scope)?
                }
            },
            Projection::TabularSection { path, columns } => {
                if columns.is_empty()
                    && scope
                        .constants
                        .contains(&folded_name(path.segments[0].lexeme))
                {
                    self.result.constants = QueryConstantReferences::All;
                } else {
                    self.field(path, scope, false);
                }
            }
        }
        Ok(())
    }
    fn expression(
        &mut self,
        expression: &Expression<'_, '_>,
        scope: &Scope,
    ) -> Result<(), QueryDiagnostic> {
        use Expression::*;
        match expression {
            Field(field)
            | Uuid {
                argument: field, ..
            } => self.field(field, scope, false),
            Literal(_) | DateTime { .. } | SystemValue { .. } | Parameter(_) => {}
            BeginOfPeriod { value, .. }
            | EndOfPeriod { value, .. }
            | DatePart { value, .. }
            | Unary { value, .. }
            | IsNull { value, .. }
            | ValueType {
                argument: value, ..
            } => self.expression(value, scope)?,
            Refs {
                value,
                kind,
                object,
                ..
            } => {
                self.object(kind, object)?;
                self.expression(value, scope)?;
            }
            MetadataValue { kind, object, .. } => self.object(kind, object)?,
            TypeLiteral { name, .. } => {
                if let TypeName::Object { kind, object } = name {
                    self.object(kind, object)?;
                }
            }
            Cast {
                argument, target, ..
            } => {
                self.expression(argument, scope)?;
                if let CastTarget::Reference { kind, object } = target {
                    self.object(kind, object)?;
                }
            }
            DateAdd {
                value: left,
                count: right,
                ..
            }
            | DateDiff {
                from: left,
                to: right,
                ..
            }
            | IsNullFunction {
                value: left,
                fallback: right,
                ..
            } => {
                self.expression(left, scope)?;
                self.expression(right, scope)?;
            }
            Binary { .. } => {
                let (first, terms) = left_binary_spine(expression);
                self.expression(first, scope)?;
                for (_, value) in terms {
                    self.expression(value, scope)?;
                }
            }
            Between {
                value, low, high, ..
            } => {
                for expression in [value, low, high] {
                    self.expression(expression, scope)?;
                }
            }
            Tuple { items, .. }
            | ScalarFunction {
                arguments: items, ..
            } => {
                for expression in items {
                    self.expression(expression, scope)?;
                }
            }
            InList { value, items, .. } => {
                self.expression(value, scope)?;
                for expression in items {
                    self.expression(expression, scope)?;
                }
            }
            InQuery { value, query, .. } => {
                self.expression(value, scope)?;
                self.query(query)?;
            }
            Case {
                subject,
                branches,
                otherwise,
                ..
            } => {
                for expression in [subject, otherwise].into_iter().flatten() {
                    self.expression(expression, scope)?;
                }
                for branch in branches {
                    self.expression(&branch.when, scope)?;
                    self.expression(&branch.then, scope)?;
                }
            }
            Like {
                value,
                pattern,
                escape,
                ..
            } => {
                self.expression(value, scope)?;
                self.expression(pattern, scope)?;
                if let Some(expression) = escape {
                    self.expression(expression, scope)?;
                }
            }
            Aggregate { argument, .. } => self.argument(argument, scope)?,
        }
        Ok(())
    }
}
