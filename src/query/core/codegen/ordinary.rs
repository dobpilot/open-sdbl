//! Проверка запрещённых текущих остатков в одном снимке SQL-запроса.

use super::batch::compile_batch_ast;
use super::orchestrate::PresentationCompilation;
use crate::metadata::{ConfigFieldPurpose, MetadataKind, MetadataSnapshot, ObjectId};
use crate::query::core::ast::{
    AccumulationKind, BatchAst, Expression, FieldReference, Projection, StatementAst,
};
use crate::query::core::dialect::SqlDialect;
use crate::query::core::names::names_equal;
use crate::query::core::params::CompileOptions;
use crate::query::core::parser::Parser;
use crate::query::core::resolve::{
    ColumnKind, CompiledColumn, CompiledQuery, find_metadata_object, queryable_fields,
};
use crate::query::core::restrict::{AccessDecision, AccessRestriction, RestrictionMode};
use crate::query::core::temp_tables::TempTablesManager;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{TokenKind, tokenize};

fn unsupported() -> QueryDiagnostic {
    QueryDiagnostic::unpositioned(
        QueryDiagnosticKind::Restriction,
        "ordinary row checks support only a single PostgreSQL current accumulation Balance SELECT with direct fields and dimension equality conditions",
    )
}

/// Собирает данные и свидетельство отказа одним оператором: отдельная
/// предварительная проверка под READ COMMITTED оставляла бы окно гонки.
pub(super) fn compile_checked(
    ast: &BatchAst<'_, '_>,
    snapshot: &MetadataSnapshot,
    options: &CompileOptions<'_>,
    dialect: SqlDialect,
    manager: &TempTablesManager,
    mode: RestrictionMode,
) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
    let ordinary = ast.statements.iter().any(
        |statement| matches!(statement, StatementAst::Query(query) if query.allowed.is_none()),
    );
    if !ordinary {
        return Ok(None);
    }
    if mode != RestrictionMode::Restricted {
        return Err(unsupported());
    }
    let restricted = !options.access_restrictions().is_empty()
        || options
            .access_decisions()
            .iter()
            .any(|decision| !matches!(decision, AccessDecision::Unrestricted(_)));
    if !restricted {
        return Ok(None);
    }
    let [StatementAst::Query(query)] = ast.statements.as_slice() else {
        return Err(unsupported());
    };
    let [branch] = query.branches.as_slice() else {
        return Err(unsupported());
    };
    if dialect != SqlDialect::Postgres
        || query.into.is_some() || query.totals.is_some() || !query.order.is_empty()
        || query.index.is_some() || !query.unions.is_empty()
        || branch.distinct || branch.top.is_some() || !branch.joins.is_empty()
        || branch.filter.is_some() || !branch.group.is_empty() || branch.having.is_some()
        || branch.projection.iter().any(|item| !matches!(&item.expression, Projection::Field(field) if field.segments.len() <= 2))
    {
        return Err(unsupported());
    }
    let source = branch.source.as_ref().ok_or_else(unsupported)?;
    if branch.projection.iter().any(|item| {
        matches!(&item.expression, Projection::Field(field) if field.segments.len() == 2
            && !source.alias.is_some_and(|alias| names_equal(alias.lexeme, field.segments[0].lexeme)))
    }) {
        return Err(unsupported());
    }
    let accumulation = source.accumulation.as_ref().ok_or_else(unsupported)?;
    if accumulation.kind != AccumulationKind::Balance
        || accumulation.arguments.first().is_some_and(Option::is_some)
        || source.table_part.is_some()
        || source.nested.is_some()
    {
        return Err(unsupported());
    }
    let object = find_metadata_object(
        snapshot,
        &format!("{}.{}", source.kind.lexeme, source.object.lexeme),
    )?;
    if object.kind != Some(MetadataKind::AccumulationRegister) {
        return Err(unsupported());
    }
    let target = ObjectId::from(&object.guid);
    let fields = queryable_fields(snapshot, object)?;
    let dimension = |field: &FieldReference<'_, '_>| {
        field.segments.len() == 1
            && fields.iter().any(|candidate| {
                (names_equal(&candidate.name, field.last().lexeme)
                    || names_equal(&candidate.schema_name, field.last().lexeme)
                    || candidate
                        .aliases
                        .iter()
                        .any(|alias| names_equal(alias, field.last().lexeme)))
                    && candidate.columns.len() == 1
                    && matches!(candidate.columns[0].kind, ColumnKind::String { .. })
                    && candidate.field.is_some_and(|id| {
                        snapshot.fields().iter().any(|metadata| {
                            crate::metadata::FieldId::Metadata(crate::metadata::AttributeId::from(
                                &metadata.guid,
                            )) == id
                                && metadata.purpose
                                    == Some(ConfigFieldPurpose::AccumulationRegisterDimension)
                        })
                    })
            })
    };
    if let Some(Some(condition)) = accumulation.arguments.get(1) {
        equality(condition, &dimension)?;
    }
    let invert = |restriction: &AccessRestriction| -> Result<AccessRestriction, QueryDiagnostic> {
        if restriction.object() != target || restriction.section().is_some() {
            return Err(unsupported());
        }
        let text = restriction.condition();
        let tokens = tokenize(text)?
            .into_iter()
            .filter(|token| token.kind != TokenKind::Comment)
            .collect::<Vec<_>>();
        let parsed = Parser::new(&tokens, text).parse_restriction()?;
        if parsed.alias.is_some() || !parsed.joins.is_empty() {
            return Err(unsupported());
        }
        let (field, value) = equality(&parsed.condition, &dimension)?;
        Ok(AccessRestriction::new(
            target,
            format!("НЕ ЕСТЬNULL(({} = {}), ЛОЖЬ)", field, value),
        ))
    };
    let restrictions = options
        .access_restrictions()
        .iter()
        .map(invert)
        .collect::<Result<Vec<_>, _>>()?;
    let decisions = options
        .access_decisions()
        .iter()
        .map(|decision| {
            if decision.object() != target || decision.table_part_name().is_some() {
                return Err(unsupported());
            }
            Ok(match decision {
                AccessDecision::Unrestricted(target) => AccessDecision::denied(target.clone()),
                AccessDecision::Denied(target) => AccessDecision::unrestricted(target.clone()),
                AccessDecision::Restricted(restriction) => {
                    AccessDecision::restricted(invert(restriction)?)
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut data_state = PresentationCompilation::strict(
        options.presentation_plans(),
        options.bound_parameters(),
        dialect,
    )
    .with_restrictions(options.access_restrictions(), options.access_decisions())
    .with_mode(mode);
    let mut probe_state = PresentationCompilation::strict(
        options.presentation_plans(),
        options.bound_parameters(),
        dialect,
    )
    .with_restrictions(&restrictions, &decisions)
    .with_mode(mode);
    probe_state.forbidden_balance_probe = true;
    let mut data_manager = manager.clone();
    let mut probe_manager = manager.clone();
    let mut data = compile_batch_ast(ast, snapshot, &mut data_state, &mut data_manager)?
        .ok_or_else(unsupported)?;
    let probe = compile_batch_ast(ast, snapshot, &mut probe_state, &mut probe_manager)?
        .ok_or_else(unsupported)?;
    if !data.nested.is_empty()
        || !data.deferred_presentations.is_empty()
        || !data.service_columns.is_empty()
    {
        return Err(unsupported());
    }
    let index = data.columns.len();
    let nulls = vec!["NULL"; index].join(", ");
    let marker = dialect.quote_identifier("__forbidden_row");
    data.sql = format!(
        "WITH \"__ordinary_forbidden\" AS (SELECT 1 WHERE EXISTS ({})) SELECT \"__ordinary_data\".*, FALSE AS {marker} FROM ({}) AS \"__ordinary_data\" WHERE NOT EXISTS (SELECT 1 FROM \"__ordinary_forbidden\") UNION ALL SELECT {nulls}, TRUE AS {marker} FROM \"__ordinary_forbidden\"",
        probe.sql, data.sql,
    );
    data.columns.push(CompiledColumn::new(
        "__forbidden_row".to_owned(),
        ColumnKind::Boolean,
    ));
    data.service_columns.push(index);
    data.forbidden_row_column = Some(index);
    Ok(Some(data))
}

/// Принимается только измеренное равенство собственного измерения значению.
fn equality<'a>(
    expression: &'a Expression<'_, '_>,
    dimension: &impl Fn(&FieldReference<'_, '_>) -> bool,
) -> Result<(&'a str, &'a str), QueryDiagnostic> {
    let Expression::Binary {
        left,
        operator,
        right,
    } = expression
    else {
        return Err(unsupported());
    };
    let Expression::Field(field) = left.as_ref() else {
        return Err(unsupported());
    };
    let value = match right.as_ref() {
        Expression::Literal(token) if token.kind == TokenKind::String => token.lexeme,
        Expression::Parameter(token) => token.lexeme,
        _ => return Err(unsupported()),
    };
    if operator.lexeme != "=" || !dimension(field) {
        return Err(unsupported());
    }
    Ok((field.last().lexeme, value))
}
