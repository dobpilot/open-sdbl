//! Bounded recursive-descent parser.

use crate::query::core::ast::{
    AccumulationAst, AccumulationKind, AggregateArgument, AggregateKind, CaseBranch, CastTarget,
    Expression, FieldReference, JoinAst, JoinKind, OrderTerm, PeriodKind, PresentationArgument,
    PresentationOperation, Projection, ProjectionItem, QueryAst, SelectAst, SliceAst, SliceKind,
    SourceAst, UnionLink, parse_datetime_value,
};
use crate::query::core::diag::SourcePosition;
use crate::query::core::names::names_equal;
use crate::query::core::resolve::kind_from_query_name;
use crate::query::core::{QueryDiagnostic, QueryDiagnosticKind};
use crate::{Keyword, Token, TokenKind};

pub(super) struct Parser<'tokens, 'source> {
    tokens: &'tokens [Token<'source>],
    offset: usize,
    depth: usize,
    binary_operators: usize,
    eof: SourcePosition,
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
                    | Keyword::DateTime
                    | Keyword::BeginOfPeriod
                    | Keyword::Value
                    | Keyword::Uuid
                    | Keyword::Cast
                    | Keyword::IsNullFunction
            )
        )
}

fn is_ascending_order(token: &Token<'_>) -> bool {
    names_equal(token.lexeme, "ASC") || names_equal(token.lexeme, "ВОЗР")
}

impl<'tokens, 'source> Parser<'tokens, 'source> {
    const MAX_DEPTH: usize = 128;
    const MAX_BINARY_OPERATORS: usize = 4_096;

    pub(super) fn new(tokens: &'tokens [Token<'source>], source: &str) -> Self {
        let mut line = 1;
        let mut column = 1;
        for character in source.chars() {
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Self {
            tokens,
            offset: 0,
            depth: 0,
            binary_operators: 0,
            eof: SourcePosition {
                offset: source.len(),
                line,
                column,
            },
        }
    }

    pub(super) fn parse(mut self) -> Result<QueryAst<'tokens, 'source>, QueryDiagnostic> {
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
                QueryDiagnosticKind::UnsupportedFeature,
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
                QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(token),
                    "TOP row count must be an integer",
                )
            })?;
            if value == 0 {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
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
            let expression = if self.consume_lexeme("*") {
                Projection::All
            } else {
                self.parse_projection()?
            };
            let alias = if self.consume_keyword(Keyword::As) {
                if matches!(expression, Projection::All) {
                    return Err(self.diagnostic(
                        QueryDiagnosticKind::UnsupportedFeature,
                        self.peek(),
                        "wildcard projection cannot have an alias",
                    ));
                }
                Some(self.expect_identifier("expected projection alias after AS")?)
            } else {
                None
            };
            projection.push(ProjectionItem { expression, alias });
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
                    if matches!(
                        value.kind,
                        TokenKind::String | TokenKind::Number | TokenKind::Binary
                    ) || matches!(
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
            Expression::Aggregate {
                token,
                kind,
                distinct,
                argument,
            } => Ok(Projection::Aggregate {
                token,
                kind,
                distinct,
                argument,
            }),
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
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
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
        let (table_part, slice, accumulation) = if self.consume_lexeme(".") {
            if self.next_lexeme_is("(")
                && let Some(token) = self.consume_keyword_token(Keyword::SliceLast)
            {
                (None, Some(self.parse_slice(token, SliceKind::Last)?), None)
            } else if self.next_lexeme_is("(")
                && let Some(token) = self.consume_keyword_token(Keyword::SliceFirst)
            {
                (None, Some(self.parse_slice(token, SliceKind::First)?), None)
            } else if self.next_lexeme_is("(")
                && let Some(token) = self.consume_keyword_token(Keyword::Balance)
            {
                (
                    None,
                    None,
                    Some(AccumulationAst {
                        token,
                        kind: AccumulationKind::Balance,
                        arguments: self.parse_virtual_arguments(2, "Balance")?,
                    }),
                )
            } else if self.next_lexeme_is("(")
                && let Some(token) = self.consume_keyword_token(Keyword::Turnovers)
            {
                (
                    None,
                    None,
                    Some(AccumulationAst {
                        token,
                        kind: AccumulationKind::Turnovers,
                        arguments: self.parse_virtual_arguments(4, "Turnovers")?,
                    }),
                )
            } else {
                (
                    Some(self.expect_identifier("expected tabular-section name")?),
                    None,
                    None,
                )
            }
        } else {
            (None, None, None)
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
            table_part,
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
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
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
            self.record_binary_operator(operator)?;
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
            self.record_binary_operator(operator)?;
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
        if self.peek().is_some_and(|token| {
            matches!(token.kind, TokenKind::Keyword(Keyword::Like | Keyword::Not))
        }) && let Some(like) = self.parse_like_tail(&mut expression)?
        {
            return Ok(like);
        }
        if let Some(is) = self.consume_keyword_token(Keyword::Is) {
            let negated = self.consume_keyword(Keyword::Not);
            if !self.consume_keyword(Keyword::Null) {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek().or(Some(is)),
                    "IS only supports NULL in this query subset",
                ));
            }
            return Ok(Expression::IsNull {
                value: Box::new(expression),
                negated,
            });
        }
        if let Some(operator) = self.consume_keyword_token(Keyword::In) {
            self.expect_lexeme("(")?;
            if self.consume_lexeme(")") {
                return Err(QueryDiagnostic::at_kind(
                    QueryDiagnosticKind::Syntax,
                    Some(operator),
                    "IN list must contain at least one expression",
                ));
            }
            let mut items = Vec::new();
            loop {
                items.push(self.parse_additive()?);
                if !self.consume_lexeme(",") {
                    break;
                }
                if self.peek().is_some_and(|token| token.lexeme == ")") {
                    return Err(self.diagnostic(
                        QueryDiagnosticKind::Syntax,
                        self.peek(),
                        "expected expression after ',' in IN list",
                    ));
                }
            }
            self.expect_lexeme(")")?;
            return Ok(Expression::InList {
                value: Box::new(expression),
                items,
            });
        }
        if self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Operator && is_comparison(token.lexeme))
        {
            let operator = self.next().expect("peeked token");
            self.record_binary_operator(operator)?;
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
            self.record_binary_operator(operator)?;
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
            self.record_binary_operator(operator)?;
            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(self.parse_unary()?),
            };
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        let mut operators = Vec::new();
        loop {
            let operator = self.consume_keyword_token(Keyword::Not).or_else(|| {
                self.peek()
                    .is_some_and(|token| matches!(token.lexeme, "+" | "-"))
                    .then(|| self.next().expect("peeked token"))
            });
            let Some(operator) = operator else {
                break;
            };
            if self.depth + operators.len() >= Self::MAX_DEPTH {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::TooDeep,
                    Some(operator),
                    format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
                ));
            }
            operators.push(operator);
        }
        let mut expression = self.parse_primary()?;
        for operator in operators.into_iter().rev() {
            expression = Expression::Unary {
                operator,
                value: Box::new(expression),
            };
        }
        Ok(expression)
    }

    fn parse_primary(&mut self) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if let Some(token) = self.consume_keyword_token(Keyword::Case) {
            return self.parse_case(token);
        }
        if self.peek().is_some_and(|token| token.lexeme == "(") {
            let opening = self.next().expect("peeked token");
            if self.depth >= Self::MAX_DEPTH {
                return Err(QueryDiagnostic::at_kind(
                    QueryDiagnosticKind::TooDeep,
                    Some(opening),
                    format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
                ));
            }
            self.depth += 1;
            let result = (|| {
                let expression = self.parse_or()?;
                self.expect_lexeme(")")?;
                Ok(expression)
            })();
            self.depth -= 1;
            return result;
        }
        if self.next_lexeme_is("(") {
            if let Some(token) = self.consume_keyword_token(Keyword::DateTime) {
                return self.parse_datetime(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::BeginOfPeriod) {
                return self.parse_begin_of_period(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Value) {
                return self.parse_metadata_value(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Uuid) {
                return self.parse_uuid(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::Cast) {
                return self.parse_cast(token);
            }
            if let Some(token) = self.consume_keyword_token(Keyword::IsNullFunction) {
                return self.parse_is_null_function(token);
            }
            if let Some((token, kind)) = self.consume_aggregate_keyword() {
                return self.parse_aggregate(token, kind);
            }
        }
        let Some(token) = self.peek() else {
            return Err(self.diagnostic(QueryDiagnosticKind::Syntax, None, "expected expression"));
        };
        if token.kind == TokenKind::Parameter {
            return Ok(Expression::Parameter(self.next().expect("peeked token")));
        }
        if matches!(
            token.kind,
            TokenKind::String | TokenKind::Number | TokenKind::Binary
        ) || matches!(
            token.kind,
            TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::Null)
        ) {
            return Ok(Expression::Literal(self.next().expect("peeked token")));
        }
        Ok(Expression::Field(self.parse_field_reference()?))
    }

    /// Parses `[НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ <escape>]` after the left
    /// operand when the next tokens spell it; otherwise leaves the input
    /// untouched. Kept out of `parse_comparison` so deep expression nesting
    /// does not grow that frame.
    #[inline(never)]
    fn parse_like_tail(
        &mut self,
        value: &mut Expression<'tokens, 'source>,
    ) -> Result<Option<Expression<'tokens, 'source>>, QueryDiagnostic> {
        let negated = self
            .peek()
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Not));
        let like_offset = if negated {
            self.offset + 1
        } else {
            self.offset
        };
        if !self
            .tokens
            .get(like_offset)
            .is_some_and(|token| token.kind == TokenKind::Keyword(Keyword::Like))
        {
            return Ok(None);
        }
        self.offset = like_offset;
        let token = self.next().expect("checked LIKE keyword");
        self.record_binary_operator(token)?;
        let pattern = self.parse_additive()?;
        let escape = if self.consume_keyword(Keyword::Escape) {
            Some(Box::new(self.parse_additive()?))
        } else {
            None
        };
        let value = std::mem::replace(value, Expression::Literal(token));
        Ok(Some(Expression::Like {
            token,
            value: Box::new(value),
            pattern: Box::new(pattern),
            escape,
            negated,
        }))
    }

    fn consume_aggregate_keyword(&mut self) -> Option<(&'tokens Token<'source>, AggregateKind)> {
        let kind = match self.peek()?.kind {
            TokenKind::Keyword(Keyword::Count) => AggregateKind::Count,
            TokenKind::Keyword(Keyword::Sum) => AggregateKind::Sum,
            TokenKind::Keyword(Keyword::Min) => AggregateKind::Min,
            TokenKind::Keyword(Keyword::Max) => AggregateKind::Max,
            _ => return None,
        };
        self.next().map(|token| (token, kind))
    }

    /// Parses the argument list of an aggregate after its keyword.
    fn parse_aggregate(
        &mut self,
        token: &'tokens Token<'source>,
        kind: AggregateKind,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let distinct = self.consume_keyword(Keyword::Distinct);
        if distinct && kind != AggregateKind::Count {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::UnsupportedFeature,
                Some(token),
                "DISTINCT aggregate argument is currently supported only by COUNT",
            ));
        }
        let argument = if self.consume_lexeme("*") {
            if kind != AggregateKind::Count {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "wildcard aggregate argument is supported only by COUNT",
                ));
            }
            if distinct {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(token),
                    "COUNT(DISTINCT *) is not supported",
                ));
            }
            AggregateArgument::All
        } else {
            AggregateArgument::Expression(Box::new(self.parse_or()?))
        };
        self.expect_lexeme(")")?;
        Ok(Expression::Aggregate {
            token,
            kind,
            distinct,
            argument,
        })
    }

    /// Parses `ВЫБОР КОГДА <predicate> ТОГДА <value> … [ИНАЧЕ <value>] КОНЕЦ`
    /// after the `ВЫБОР` keyword. Every alternative counts against the
    /// binary-operator budget and the whole expression against the depth
    /// limit.
    fn parse_case(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::TooDeep,
                Some(token),
                format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
            ));
        }
        self.depth += 1;
        let result = (|| {
            let mut branches = Vec::new();
            while let Some(when_token) = self.consume_keyword_token(Keyword::When) {
                self.record_binary_operator(when_token)?;
                let when = self.parse_or()?;
                self.expect_keyword(Keyword::Then)?;
                let then = self.parse_or()?;
                branches.push(CaseBranch {
                    token: when_token,
                    when,
                    then,
                });
            }
            if branches.is_empty() {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek().or(Some(token)),
                    "CASE requires at least one WHEN alternative",
                ));
            }
            let otherwise = if self.consume_keyword(Keyword::Else) {
                Some(Box::new(self.parse_or()?))
            } else {
                None
            };
            self.expect_keyword(Keyword::End)?;
            Ok(Expression::Case {
                token,
                branches,
                otherwise,
            })
        })();
        self.depth -= 1;
        result
    }

    /// Parses `ЕСТЬNULL(<value>, <fallback>)` after the keyword.
    fn parse_is_null_function(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.record_binary_operator(token)?;
        self.expect_lexeme("(")?;
        let value = self.parse_or()?;
        if !self.consume_lexeme(",") {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                self.peek(),
                "ISNULL requires two arguments",
            ));
        }
        let fallback = self.parse_or()?;
        self.expect_lexeme(")")?;
        Ok(Expression::IsNullFunction {
            token,
            value: Box::new(value),
            fallback: Box::new(fallback),
        })
    }

    fn parse_datetime(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let mut arguments = Vec::with_capacity(6);
        loop {
            let argument = self.expect_kind(
                TokenKind::Number,
                "DATETIME components must be integer literals",
            )?;
            if argument.lexeme.contains('.') {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(argument),
                    "DATETIME components must be integer literals",
                ));
            }
            arguments.push(argument);
            if !self.consume_lexeme(",") {
                break;
            }
            if arguments.len() == 6 {
                return Err(self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    self.peek(),
                    "DATETIME accepts at most 6 components",
                ));
            }
        }
        self.expect_lexeme(")")?;
        let value = parse_datetime_value(token, &arguments)?;
        Ok(Expression::DateTime { token, value })
    }

    fn parse_begin_of_period(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::TooDeep,
                Some(token),
                format!("query nesting depth exceeds limit of {}", Self::MAX_DEPTH),
            ));
        }
        self.depth += 1;
        let result = (|| {
            self.expect_lexeme("(")?;
            let value = self.parse_or()?;
            self.expect_lexeme(",")?;
            let period = self.expect_identifier("expected period kind after ','")?;
            let period = PeriodKind::from_name(period.lexeme).ok_or_else(|| {
                QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(period),
                    format!("unsupported BEGINOFPERIOD period {:?}", period.lexeme),
                )
            })?;
            self.expect_lexeme(")")?;
            Ok(Expression::BeginOfPeriod {
                token,
                value: Box::new(value),
                period,
            })
        })();
        self.depth -= 1;
        result
    }

    fn parse_metadata_value(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let kind = self.expect_identifier("VALUE expects a metadata kind")?;
        self.expect_lexeme(".")?;
        let object = self.expect_identifier("VALUE expects a metadata object")?;
        self.expect_lexeme(".")?;
        let value = self.expect_identifier("VALUE expects a predefined value")?;
        self.expect_lexeme(")")?;
        Ok(Expression::MetadataValue {
            token,
            kind,
            object,
            value,
        })
    }

    fn parse_uuid(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let argument = self.parse_field_reference()?;
        if self.peek().is_some_and(|next| next.lexeme == ",") {
            return Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
                Some(token),
                "UUID expects exactly one reference field",
            ));
        }
        self.expect_lexeme(")")?;
        Ok(Expression::Uuid { token, argument })
    }

    fn parse_cast(
        &mut self,
        token: &'tokens Token<'source>,
    ) -> Result<Expression<'tokens, 'source>, QueryDiagnostic> {
        self.expect_lexeme("(")?;
        let argument = self.parse_or()?;
        self.expect_keyword(Keyword::As)?;
        let target = self.parse_cast_target()?;
        self.expect_lexeme(")")?;
        let path = if matches!(target, CastTarget::Reference { .. }) && self.consume_lexeme(".") {
            let field = self.expect_identifier("expected field name after '.'")?;
            if let Some(next) = self.peek()
                && next.lexeme == "."
            {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::UnsupportedFeature,
                    Some(next),
                    "reference paths deeper than one hop are not supported after CAST",
                ));
            }
            Some(field)
        } else {
            None
        };
        Ok(Expression::Cast {
            token,
            argument: Box::new(argument),
            target,
            path,
        })
    }

    fn parse_cast_target(&mut self) -> Result<CastTarget<'tokens, 'source>, QueryDiagnostic> {
        let name = self.expect_identifier("CAST expects a target type")?;
        let is = |candidates: [&str; 2]| {
            candidates
                .iter()
                .any(|candidate| names_equal(name.lexeme, candidate))
        };
        if is(["СТРОКА", "STRING"]) {
            let parameters = self.parse_cast_parameters(name, 1)?;
            return Ok(CastTarget::String {
                length: parameters.first().copied(),
            });
        }
        if is(["ЧИСЛО", "NUMBER"]) {
            let parameters = self.parse_cast_parameters(name, 2)?;
            let narrow = |value: Option<&u32>| value.and_then(|value| u8::try_from(*value).ok());
            return Ok(CastTarget::Number {
                precision: narrow(parameters.first()),
                scale: narrow(parameters.get(1)),
            });
        }
        if is(["БУЛЕВО", "BOOLEAN"]) {
            return Ok(CastTarget::Boolean);
        }
        if is(["ДАТА", "DATE"]) {
            return Ok(CastTarget::Date);
        }
        if kind_from_query_name(name.lexeme).is_some() && self.consume_lexeme(".") {
            let object = self.expect_identifier("CAST expects a metadata object")?;
            return Ok(CastTarget::Reference { kind: name, object });
        }
        Err(QueryDiagnostic::at(
            QueryDiagnosticKind::UnsupportedFeature,
            Some(name),
            format!(
                "unsupported CAST target {:?}; expected STRING(n), NUMBER(p, s), BOOLEAN, DATE, or <Kind>.<Object>",
                name.lexeme
            ),
        ))
    }

    /// Parses an optional `(n[, m])` parameter list of a scalar cast target.
    fn parse_cast_parameters(
        &mut self,
        target: &'tokens Token<'source>,
        limit: usize,
    ) -> Result<Vec<u32>, QueryDiagnostic> {
        let mut parameters = Vec::new();
        if !self.consume_lexeme("(") {
            return Ok(parameters);
        }
        loop {
            let number = self.peek().ok_or_else(|| {
                self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    None,
                    "CAST target expects a numeric parameter",
                )
            })?;
            let value = (number.kind == TokenKind::Number)
                .then(|| number.lexeme.parse::<u32>().ok())
                .flatten()
                .ok_or_else(|| {
                    QueryDiagnostic::at(
                        QueryDiagnosticKind::Syntax,
                        Some(number),
                        "CAST target expects a non-negative integer parameter",
                    )
                })?;
            self.next();
            parameters.push(value);
            if parameters.len() > limit {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
                    Some(target),
                    format!(
                        "CAST target {:?} accepts at most {limit} parameters",
                        target.lexeme
                    ),
                ));
            }
            if !self.consume_lexeme(",") {
                break;
            }
        }
        self.expect_lexeme(")")?;
        Ok(parameters)
    }

    fn parse_field_reference(
        &mut self,
    ) -> Result<FieldReference<'tokens, 'source>, QueryDiagnostic> {
        let mut segments = vec![self.expect_identifier("expected field name")?];
        while self.consume_lexeme(".") {
            let token = self.peek().ok_or_else(|| {
                self.diagnostic(
                    QueryDiagnosticKind::Syntax,
                    None,
                    "expected field name after '.'",
                )
            })?;
            if !is_contextual_identifier(token.kind) {
                return Err(QueryDiagnostic::at(
                    QueryDiagnosticKind::Syntax,
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
            Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
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
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        if !is_contextual_identifier(token.kind) {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
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
            .ok_or_else(|| self.diagnostic(QueryDiagnosticKind::Syntax, None, message))?;
        if token.kind != kind {
            return Err(QueryDiagnostic::at(
                QueryDiagnosticKind::Syntax,
                Some(token),
                message,
            ));
        }
        Ok(self.next().expect("peeked token"))
    }

    fn expect_lexeme(&mut self, lexeme: &str) -> Result<(), QueryDiagnostic> {
        if self.consume_lexeme(lexeme) {
            Ok(())
        } else {
            Err(self.diagnostic(
                QueryDiagnosticKind::Syntax,
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

    fn record_binary_operator(
        &mut self,
        operator: &'tokens Token<'source>,
    ) -> Result<(), QueryDiagnostic> {
        if self.binary_operators >= Self::MAX_BINARY_OPERATORS {
            return Err(QueryDiagnostic::at_kind(
                QueryDiagnosticKind::TooDeep,
                Some(operator),
                format!(
                    "query nesting depth exceeds limit of {} binary operators",
                    Self::MAX_BINARY_OPERATORS
                ),
            ));
        }
        self.binary_operators += 1;
        Ok(())
    }

    fn diagnostic(
        &self,
        kind: QueryDiagnosticKind,
        token: Option<&Token<'_>>,
        message: impl Into<String>,
    ) -> QueryDiagnostic {
        match token {
            Some(token) => QueryDiagnostic::at(kind, Some(token), message),
            None => QueryDiagnostic::at_position(kind, self.eof, message),
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
