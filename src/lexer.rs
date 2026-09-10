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
#[non_exhaustive]
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
    /// `ПРЕДСТАВЛЕНИЕССЫЛКИ` or `REFPRESENTATION`.
    RefPresentation,
    /// `ПРЕДСТАВЛЕНИЕ` or `PRESENTATION`.
    Presentation,
    /// `КОЛИЧЕСТВО` or `COUNT`.
    Count,
    /// `СУММА` or `SUM`.
    Sum,
    /// `МИНИМУМ` or `MIN`.
    Min,
    /// `МАКСИМУМ` or `MAX`.
    Max,
    /// `СРЕЗПОСЛЕДНИХ` or `SLICELAST`.
    SliceLast,
    /// `СРЕЗПЕРВЫХ` or `SLICEFIRST`.
    SliceFirst,
    /// `ОСТАТКИ` or `BALANCE`.
    Balance,
    /// `ОБОРОТЫ` or `TURNOVERS`.
    Turnovers,
    /// `ДАТАВРЕМЯ` or `DATETIME`.
    DateTime,
    /// `НАЧАЛОПЕРИОДА` or `BEGINOFPERIOD`.
    BeginOfPeriod,
    /// `ЗНАЧЕНИЕ` or `VALUE`.
    Value,
    /// `УНИКАЛЬНЫЙИДЕНТИФИКАТОР` or `UUID`.
    Uuid,
    /// `ВЫРАЗИТЬ` or `CAST`.
    Cast,
    /// `ЕСТЬNULL` or `ISNULL`.
    IsNullFunction,
    /// `ПОДОБНО` or `LIKE`.
    Like,
    /// `СПЕЦСИМВОЛ` or `ESCAPE`.
    Escape,
    /// `ДОБАВИТЬ` or `ADD`.
    Add,
    /// `УНИЧТОЖИТЬ` or `DROP`.
    Drop,
    /// `ИНДЕКСИРОВАТЬ` or `INDEX`.
    Index,
    /// `НАБОРАМ` or `SETS`.
    Sets,
    /// `УНИКАЛЬНО` or `UNIQUE`.
    Unique,
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
            Self::RefPresentation => "REFPRESENTATION",
            Self::Presentation => "PRESENTATION",
            Self::Count => "COUNT",
            Self::Sum => "SUM",
            Self::Min => "MIN",
            Self::Max => "MAX",
            Self::SliceLast => "SLICELAST",
            Self::SliceFirst => "SLICEFIRST",
            Self::Balance => "BALANCE",
            Self::Turnovers => "TURNOVERS",
            Self::DateTime => "DATETIME",
            Self::BeginOfPeriod => "BEGINOFPERIOD",
            Self::Value => "VALUE",
            Self::Uuid => "UUID",
            Self::Cast => "CAST",
            Self::IsNullFunction => "ISNULL",
            Self::Like => "LIKE",
            Self::Escape => "ESCAPE",
            Self::Add => "ADD",
            Self::Drop => "DROP",
            Self::Index => "INDEX",
            Self::Sets => "SETS",
            Self::Unique => "UNIQUE",
        }
    }
}

/// The lexical class of a token.
#[non_exhaustive]
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
    /// A `0x`-prefixed hexadecimal binary literal.
    Binary,
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
            Self::Binary => formatter.write_str("BINARY"),
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
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// The source ended before a string's closing quote.
    UnterminatedString,
    /// An ampersand was not followed by an identifier.
    ExpectedParameterName,
    /// A `0x` literal was empty, odd-length, or contained a non-hex character.
    InvalidBinaryLiteral,
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
            DiagnosticKind::InvalidBinaryLiteral => formatter.write_str(
                "binary literal must use 0x followed by an even number of hexadecimal digits",
            ),
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
    finished: bool,
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
            finished: false,
        }
    }

    /// Returns the next non-whitespace token.
    ///
    /// # Errors
    ///
    /// Returns a [`Diagnostic`] for malformed or unsupported input.
    pub fn next_token(&mut self) -> Result<Option<Token<'source>>, Diagnostic> {
        self.next().transpose()
    }

    fn scan_token(&mut self) -> Result<Option<Token<'source>>, Diagnostic> {
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
            if character == '0' && matches!(self.next_character(), Some('x' | 'X')) {
                return self.consume_binary(start).map(Some);
            }
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

    fn consume_binary(&mut self, start: Mark) -> Result<Token<'source>, Diagnostic> {
        self.advance();
        self.advance();
        let digits_start = self.offset;
        while self
            .current()
            .is_some_and(|character| character.is_ascii_hexdigit())
        {
            self.advance();
        }
        let digit_count = self.offset - digits_start;
        if digit_count == 0
            || digit_count % 2 != 0
            || self.current().is_some_and(is_identifier_continue)
        {
            return Err(self.diagnostic(start, DiagnosticKind::InvalidBinaryLiteral));
        }
        Ok(self.token(start, TokenKind::Binary))
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

impl<'source> Iterator for Lexer<'source> {
    type Item = Result<Token<'source>, Diagnostic>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.scan_token() {
            Ok(Some(token)) => Some(Ok(token)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl std::iter::FusedIterator for Lexer<'_> {}

/// Tokenizes all non-whitespace input.
///
/// # Errors
///
/// Returns the first lexical [`Diagnostic`] encountered.
pub fn tokenize(source: &str) -> Result<Vec<Token<'_>>, Diagnostic> {
    Lexer::new(source).collect()
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

const KEYWORDS: [(Keyword, &str, &str); 56] = [
    (Keyword::Select, "ВЫБРАТЬ", "SELECT"),
    (Keyword::From, "ИЗ", "FROM"),
    (Keyword::Where, "ГДЕ", "WHERE"),
    (Keyword::As, "КАК", "AS"),
    (Keyword::And, "И", "AND"),
    (Keyword::Or, "ИЛИ", "OR"),
    (Keyword::Not, "НЕ", "NOT"),
    (Keyword::In, "В", "IN"),
    (Keyword::Is, "ЕСТЬ", "IS"),
    (Keyword::Null, "NULL", "NULL"),
    (Keyword::True, "ИСТИНА", "TRUE"),
    (Keyword::False, "ЛОЖЬ", "FALSE"),
    (Keyword::Distinct, "РАЗЛИЧНЫЕ", "DISTINCT"),
    (Keyword::Top, "ПЕРВЫЕ", "TOP"),
    (Keyword::Order, "УПОРЯДОЧИТЬ", "ORDER"),
    (Keyword::By, "ПО", "BY"),
    (Keyword::Group, "СГРУППИРОВАТЬ", "GROUP"),
    (Keyword::Having, "ИМЕЮЩИЕ", "HAVING"),
    (Keyword::Union, "ОБЪЕДИНИТЬ", "UNION"),
    (Keyword::All, "ВСЕ", "ALL"),
    (Keyword::Into, "ПОМЕСТИТЬ", "INTO"),
    (Keyword::Join, "СОЕДИНЕНИЕ", "JOIN"),
    (Keyword::Left, "ЛЕВОЕ", "LEFT"),
    (Keyword::Right, "ПРАВОЕ", "RIGHT"),
    (Keyword::Full, "ПОЛНОЕ", "FULL"),
    (Keyword::Inner, "ВНУТРЕННЕЕ", "INNER"),
    (Keyword::Outer, "ВНЕШНЕЕ", "OUTER"),
    (Keyword::On, "ON", "ON"),
    (Keyword::Case, "ВЫБОР", "CASE"),
    (Keyword::When, "КОГДА", "WHEN"),
    (Keyword::Then, "ТОГДА", "THEN"),
    (Keyword::Else, "ИНАЧЕ", "ELSE"),
    (Keyword::End, "КОНЕЦ", "END"),
    (
        Keyword::RefPresentation,
        "ПРЕДСТАВЛЕНИЕССЫЛКИ",
        "REFPRESENTATION",
    ),
    (Keyword::Presentation, "ПРЕДСТАВЛЕНИЕ", "PRESENTATION"),
    (Keyword::Count, "КОЛИЧЕСТВО", "COUNT"),
    (Keyword::Sum, "СУММА", "SUM"),
    (Keyword::Min, "МИНИМУМ", "MIN"),
    (Keyword::Max, "МАКСИМУМ", "MAX"),
    (Keyword::SliceLast, "СРЕЗПОСЛЕДНИХ", "SLICELAST"),
    (Keyword::SliceFirst, "СРЕЗПЕРВЫХ", "SLICEFIRST"),
    (Keyword::Balance, "ОСТАТКИ", "BALANCE"),
    (Keyword::Turnovers, "ОБОРОТЫ", "TURNOVERS"),
    (Keyword::DateTime, "ДАТАВРЕМЯ", "DATETIME"),
    (Keyword::BeginOfPeriod, "НАЧАЛОПЕРИОДА", "BEGINOFPERIOD"),
    (Keyword::Value, "ЗНАЧЕНИЕ", "VALUE"),
    (Keyword::Uuid, "УНИКАЛЬНЫЙИДЕНТИФИКАТОР", "UUID"),
    (Keyword::Cast, "ВЫРАЗИТЬ", "CAST"),
    (Keyword::IsNullFunction, "ЕСТЬNULL", "ISNULL"),
    (Keyword::Like, "ПОДОБНО", "LIKE"),
    (Keyword::Escape, "СПЕЦСИМВОЛ", "ESCAPE"),
    (Keyword::Add, "ДОБАВИТЬ", "ADD"),
    (Keyword::Drop, "УНИЧТОЖИТЬ", "DROP"),
    (Keyword::Index, "ИНДЕКСИРОВАТЬ", "INDEX"),
    (Keyword::Sets, "НАБОРАМ", "SETS"),
    (Keyword::Unique, "УНИКАЛЬНО", "UNIQUE"),
];

fn keyword(text: &str) -> Option<Keyword> {
    KEYWORDS.iter().find_map(|&(keyword, russian, english)| {
        (keyword_spelling_eq(text, russian) || keyword_spelling_eq(text, english))
            .then_some(keyword)
    })
}

fn keyword_spelling_eq(text: &str, spelling: &str) -> bool {
    text.len() == spelling.len()
        && text
            .chars()
            .flat_map(char::to_uppercase)
            .eq(spelling.chars().flat_map(char::to_uppercase))
}
