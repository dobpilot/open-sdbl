//! Lexical analysis for the 1C query language (SDBL).
//!
//! The crate intentionally implements a bounded lexical layer. It preserves
//! original source spelling and byte spans and makes no semantic decisions.

use std::fmt;

/// A zero-based byte range with a one-based source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive byte offset in the UTF-8 source.
    pub start: usize,
    /// Exclusive byte offset in the UTF-8 source.
    pub end: usize,
    /// One-based line of the first character.
    pub line: usize,
    /// One-based Unicode-scalar column of the first character.
    pub column: usize,
}

/// A keyword understood by the initial SDBL lexical subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    /// `ВЫБРАТЬ` or `SELECT`.
    Select,
    /// `ИЗ` or `FROM`.
    From,
    /// `ГДЕ` or `WHERE`.
    Where,
    /// `КАК` or `AS`.
    As,
    /// `И` or `AND`.
    And,
    /// `ИЛИ` or `OR`.
    Or,
    /// `НЕ` or `NOT`.
    Not,
    /// `В` or `IN`.
    In,
    /// `ЕСТЬ` or `IS`.
    Is,
    /// `NULL`.
    Null,
    /// `ИСТИНА` or `TRUE`.
    True,
    /// `ЛОЖЬ` or `FALSE`.
    False,
    /// `РАЗЛИЧНЫЕ` or `DISTINCT`.
    Distinct,
    /// `ПЕРВЫЕ` or `TOP`.
    Top,
    /// `УПОРЯДОЧИТЬ` or `ORDER`.
    Order,
    /// `ПО` or `BY`.
    By,
    /// `СГРУППИРОВАТЬ` or `GROUP`.
    Group,
    /// `ИМЕЮЩИЕ` or `HAVING`.
    Having,
    /// `ОБЪЕДИНИТЬ` or `UNION`.
    Union,
    /// `ВСЕ` or `ALL`.
    All,
    /// `ПОМЕСТИТЬ` or `INTO`.
    Into,
    /// `СОЕДИНЕНИЕ` or `JOIN`.
    Join,
    /// `ЛЕВОЕ` or `LEFT`.
    Left,
    /// `ПРАВОЕ` or `RIGHT`.
    Right,
    /// `ПОЛНОЕ` or `FULL`.
    Full,
    /// `ВНУТРЕННЕЕ` or `INNER`.
    Inner,
    /// `ВНЕШНЕЕ` or `OUTER`.
    Outer,
    /// `ON`.
    On,
    /// `ВЫБОР` or `CASE`.
    Case,
    /// `КОГДА` or `WHEN`.
    When,
    /// `ТОГДА` or `THEN`.
    Then,
    /// `ИНАЧЕ` or `ELSE`.
    Else,
    /// `КОНЕЦ` or `END`.
    End,
}

impl Keyword {
    /// Returns the stable English display name of this keyword.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Select => "SELECT",
            Self::From => "FROM",
            Self::Where => "WHERE",
            Self::As => "AS",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Not => "NOT",
            Self::In => "IN",
            Self::Is => "IS",
            Self::Null => "NULL",
            Self::True => "TRUE",
            Self::False => "FALSE",
            Self::Distinct => "DISTINCT",
            Self::Top => "TOP",
            Self::Order => "ORDER",
            Self::By => "BY",
            Self::Group => "GROUP",
            Self::Having => "HAVING",
            Self::Union => "UNION",
            Self::All => "ALL",
            Self::Into => "INTO",
            Self::Join => "JOIN",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::Full => "FULL",
            Self::Inner => "INNER",
            Self::Outer => "OUTER",
            Self::On => "ON",
            Self::Case => "CASE",
            Self::When => "WHEN",
            Self::Then => "THEN",
            Self::Else => "ELSE",
            Self::End => "END",
        }
    }
}

/// The lexical class of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// A recognized keyword.
    Keyword(Keyword),
    /// A Unicode identifier.
    Identifier,
    /// An ampersand-prefixed query parameter.
    Parameter,
    /// A double-quoted string literal.
    String,
    /// An integer or decimal numeric literal.
    Number,
    /// An operator.
    Operator,
    /// Punctuation such as parentheses or a comma.
    Punctuation,
    /// A `//` line comment.
    Comment,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyword(keyword) => write!(formatter, "KEYWORD({})", keyword.as_str()),
            Self::Identifier => formatter.write_str("IDENTIFIER"),
            Self::Parameter => formatter.write_str("PARAMETER"),
            Self::String => formatter.write_str("STRING"),
            Self::Number => formatter.write_str("NUMBER"),
            Self::Operator => formatter.write_str("OPERATOR"),
            Self::Punctuation => formatter.write_str("PUNCTUATION"),
            Self::Comment => formatter.write_str("COMMENT"),
        }
    }
}

/// A token that borrows its exact spelling from the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'source> {
    /// Lexical class.
    pub kind: TokenKind,
    /// Exact source spelling.
    pub lexeme: &'source str,
    /// Source extent and starting position.
    pub span: Span,
}

/// Machine-readable category of a lexical diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// The source ended before a string's closing quote.
    UnterminatedString,
    /// An ampersand was not followed by an identifier.
    ExpectedParameterName,
    /// The character does not belong to the supported lexical subset.
    UnexpectedCharacter(char),
}

/// A lexical failure with a one-based source position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Diagnostic category.
    pub kind: DiagnosticKind,
    /// Zero-based UTF-8 byte offset.
    pub offset: usize,
    /// One-based line.
    pub line: usize,
    /// One-based Unicode-scalar column.
    pub column: usize,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}: ", self.line, self.column)?;
        match self.kind {
            DiagnosticKind::UnterminatedString => {
                formatter.write_str("unterminated string literal")
            }
            DiagnosticKind::ExpectedParameterName => {
                formatter.write_str("expected a parameter name after '&'")
            }
            DiagnosticKind::UnexpectedCharacter(character) => {
                write!(formatter, "unexpected character {character:?}")
            }
        }
    }
}

impl std::error::Error for Diagnostic {}

/// A streaming lexer over borrowed SDBL source text.
#[derive(Debug, Clone)]
pub struct Lexer<'source> {
    source: &'source str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'source> Lexer<'source> {
    /// Creates a lexer positioned at the beginning of `source`.
    #[must_use]
    pub const fn new(source: &'source str) -> Self {
        Self {
            source,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the next non-whitespace token.
    ///
    /// # Errors
    ///
    /// Returns a [`Diagnostic`] for malformed or unsupported input.
    pub fn next_token(&mut self) -> Result<Option<Token<'source>>, Diagnostic> {
        self.skip_whitespace();
        let Some(character) = self.current() else {
            return Ok(None);
        };

        let start = self.mark();
        if is_identifier_start(character) {
            self.advance();
            while self.current().is_some_and(is_identifier_continue) {
                self.advance();
            }
            let lexeme = &self.source[start.offset..self.offset];
            let kind = keyword(lexeme).map_or(TokenKind::Identifier, TokenKind::Keyword);
            return Ok(Some(self.token(start, kind)));
        }

        if character.is_ascii_digit() {
            self.consume_number();
            return Ok(Some(self.token(start, TokenKind::Number)));
        }

        match character {
            '&' => self.consume_parameter(start).map(Some),
            '"' => self.consume_string(start).map(Some),
            '/' if self.followed_by('/') => Ok(Some(self.consume_line_comment(start))),
            '=' | '<' | '>' | '+' | '-' | '*' | '/' => {
                self.consume_operator();
                Ok(Some(self.token(start, TokenKind::Operator)))
            }
            '(' | ')' | '[' | ']' | ',' | '.' | ';' => {
                self.advance();
                Ok(Some(self.token(start, TokenKind::Punctuation)))
            }
            unexpected => {
                Err(self.diagnostic(start, DiagnosticKind::UnexpectedCharacter(unexpected)))
            }
        }
    }

    fn consume_parameter(&mut self, start: Mark) -> Result<Token<'source>, Diagnostic> {
        self.advance();
        if !self.current().is_some_and(is_identifier_start) {
            return Err(self.diagnostic(start, DiagnosticKind::ExpectedParameterName));
        }
        self.advance();
        while self.current().is_some_and(is_identifier_continue) {
            self.advance();
        }
        Ok(self.token(start, TokenKind::Parameter))
    }

    fn consume_string(&mut self, start: Mark) -> Result<Token<'source>, Diagnostic> {
        self.advance();
        while let Some(character) = self.current() {
            self.advance();
            if character == '"' {
                if self.current() == Some('"') {
                    self.advance();
                } else {
                    return Ok(self.token(start, TokenKind::String));
                }
            }
        }
        Err(self.diagnostic(start, DiagnosticKind::UnterminatedString))
    }

    fn consume_line_comment(&mut self, start: Mark) -> Token<'source> {
        self.advance();
        self.advance();
        while self.current().is_some_and(|character| character != '\n') {
            self.advance();
        }
        self.token(start, TokenKind::Comment)
    }

    fn consume_number(&mut self) {
        while self
            .current()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.advance();
        }
        if self.current() == Some('.')
            && self
                .next_character()
                .is_some_and(|character| character.is_ascii_digit())
        {
            self.advance();
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit())
            {
                self.advance();
            }
        }
    }

    fn consume_operator(&mut self) {
        let first = self.current();
        self.advance();
        let paired = matches!(
            (first, self.current()),
            (Some('<' | '>'), Some('=')) | (Some('<'), Some('>'))
        );
        if paired {
            self.advance();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }

    fn current(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn next_character(&self) -> Option<char> {
        self.source[self.offset..].chars().nth(1)
    }

    fn followed_by(&self, expected: char) -> bool {
        self.next_character() == Some(expected)
    }

    fn advance(&mut self) {
        let Some(character) = self.current() else {
            return;
        };
        self.offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }

    const fn mark(&self) -> Mark {
        Mark {
            offset: self.offset,
            line: self.line,
            column: self.column,
        }
    }

    fn token(&self, start: Mark, kind: TokenKind) -> Token<'source> {
        Token {
            kind,
            lexeme: &self.source[start.offset..self.offset],
            span: Span {
                start: start.offset,
                end: self.offset,
                line: start.line,
                column: start.column,
            },
        }
    }

    const fn diagnostic(&self, start: Mark, kind: DiagnosticKind) -> Diagnostic {
        Diagnostic {
            kind,
            offset: start.offset,
            line: start.line,
            column: start.column,
        }
    }
}

/// Tokenizes all non-whitespace input.
///
/// # Errors
///
/// Returns the first lexical [`Diagnostic`] encountered.
pub fn tokenize(source: &str) -> Result<Vec<Token<'_>>, Diagnostic> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token()? {
        tokens.push(token);
    }
    Ok(tokens)
}

#[derive(Debug, Clone, Copy)]
struct Mark {
    offset: usize,
    line: usize,
    column: usize,
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn keyword(text: &str) -> Option<Keyword> {
    match text.to_uppercase().as_str() {
        "ВЫБРАТЬ" | "SELECT" => Some(Keyword::Select),
        "ИЗ" | "FROM" => Some(Keyword::From),
        "ГДЕ" | "WHERE" => Some(Keyword::Where),
        "КАК" | "AS" => Some(Keyword::As),
        "И" | "AND" => Some(Keyword::And),
        "ИЛИ" | "OR" => Some(Keyword::Or),
        "НЕ" | "NOT" => Some(Keyword::Not),
        "В" | "IN" => Some(Keyword::In),
        "ЕСТЬ" | "IS" => Some(Keyword::Is),
        "NULL" => Some(Keyword::Null),
        "ИСТИНА" | "TRUE" => Some(Keyword::True),
        "ЛОЖЬ" | "FALSE" => Some(Keyword::False),
        "РАЗЛИЧНЫЕ" | "DISTINCT" => Some(Keyword::Distinct),
        "ПЕРВЫЕ" | "TOP" => Some(Keyword::Top),
        "УПОРЯДОЧИТЬ" | "ORDER" => Some(Keyword::Order),
        "ПО" | "BY" => Some(Keyword::By),
        "СГРУППИРОВАТЬ" | "GROUP" => Some(Keyword::Group),
        "ИМЕЮЩИЕ" | "HAVING" => Some(Keyword::Having),
        "ОБЪЕДИНИТЬ" | "UNION" => Some(Keyword::Union),
        "ВСЕ" | "ALL" => Some(Keyword::All),
        "ПОМЕСТИТЬ" | "INTO" => Some(Keyword::Into),
        "СОЕДИНЕНИЕ" | "JOIN" => Some(Keyword::Join),
        "ЛЕВОЕ" | "LEFT" => Some(Keyword::Left),
        "ПРАВОЕ" | "RIGHT" => Some(Keyword::Right),
        "ПОЛНОЕ" | "FULL" => Some(Keyword::Full),
        "ВНУТРЕННЕЕ" | "INNER" => Some(Keyword::Inner),
        "ВНЕШНЕЕ" | "OUTER" => Some(Keyword::Outer),
        "ON" => Some(Keyword::On),
        "ВЫБОР" | "CASE" => Some(Keyword::Case),
        "КОГДА" | "WHEN" => Some(Keyword::When),
        "ТОГДА" | "THEN" => Some(Keyword::Then),
        "ИНАЧЕ" | "ELSE" => Some(Keyword::Else),
        "КОНЕЦ" | "END" => Some(Keyword::End),
        _ => None,
    }
}
