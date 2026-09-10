//! Query diagnostics and source-error conversion.

use std::fmt;

use crate::metadata::LookupError;
use crate::{Diagnostic, DiagnosticKind, Token};

/// A positional query parsing or metadata-resolution failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDiagnostic {
    kind: QueryDiagnosticKind,
    message: String,
    offset: usize,
    line: usize,
    column: usize,
    source: Option<QueryDiagnosticSource>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SourcePosition {
    pub(super) offset: usize,
    pub(super) line: usize,
    pub(super) column: usize,
}

/// Machine-readable category of a query compilation diagnostic.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryDiagnosticKind {
    /// The SDBL lexer rejected malformed source text.
    Lex,
    /// The parser rejected syntactically invalid source text.
    Syntax,
    /// A parser or expression-work budget was exceeded.
    TooDeep,
    /// No metadata object matched the requested name or identity.
    UnknownObject,
    /// More than one metadata object matched the requested name.
    AmbiguousObject,
    /// No field matched the requested name or identity.
    UnknownField,
    /// More than one field matched the requested name.
    AmbiguousField,
    /// No predefined catalog or enumeration value matched the requested name.
    UnknownValue,
    /// More than one predefined value matched the requested name.
    AmbiguousValue,
    /// A resolved metadata table or column is absent from the live catalog.
    NotLive,
    /// A valid SDBL construct is outside the compiler's supported subset.
    UnsupportedFeature,
    /// A presentation plan is invalid or cannot be resolved.
    PresentationPlan,
    /// A presentation lookup batch violates its size contract.
    PresentationBatch,
    /// Metadata is inconsistent or incomplete.
    Metadata,
    /// A prepared query was compiled against a different metadata snapshot.
    SnapshotMismatch,
    /// The total compilation work budget was exhausted.
    WorkBudgetExceeded,
    /// A named parameter is missing, unused, or used where a list is not
    /// allowed.
    Parameter,
    /// A temporary table is unknown, already defined, structurally
    /// incompatible, or unusable with the supplied manager.
    TemporaryTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryDiagnosticSource {
    Lex(Diagnostic),
    Lookup(LookupError),
}

impl QueryDiagnostic {
    pub(crate) fn snapshot_mismatch() -> Self {
        Self::unpositioned(
            QueryDiagnosticKind::SnapshotMismatch,
            "prepared query belongs to a different metadata snapshot",
        )
    }

    pub(super) fn at(
        kind: QueryDiagnosticKind,
        token: Option<&Token<'_>>,
        message: impl Into<String>,
    ) -> Self {
        Self::at_kind(kind, token, message)
    }

    pub(super) fn at_kind(
        kind: QueryDiagnosticKind,
        token: Option<&Token<'_>>,
        message: impl Into<String>,
    ) -> Self {
        let (offset, line, column) = token.map_or((0, 0, 0), |token| {
            (token.span.start, token.span.line, token.span.column)
        });
        Self {
            kind,
            message: message.into(),
            offset,
            line,
            column,
            source: None,
        }
    }

    pub(super) fn at_position(
        kind: QueryDiagnosticKind,
        position: SourcePosition,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: position.offset,
            line: position.line,
            column: position.column,
            source: None,
        }
    }

    pub(super) fn unpositioned(kind: QueryDiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: 0,
            line: 0,
            column: 0,
            source: None,
        }
    }

    pub(super) fn at_or_unpositioned(
        kind: QueryDiagnosticKind,
        token: Option<&Token<'_>>,
        message: impl Into<String>,
    ) -> Self {
        match token {
            Some(token) => Self::at_kind(kind, Some(token), message),
            None => Self::unpositioned(kind, message),
        }
    }

    pub(super) fn lookup(
        token: &Token<'_>,
        error: LookupError,
        message: impl Into<String>,
    ) -> Self {
        Self::lookup_at(Some(token), error, message)
    }

    pub(super) fn lookup_at(
        token: Option<&Token<'_>>,
        error: LookupError,
        message: impl Into<String>,
    ) -> Self {
        let kind = match error {
            LookupError::ObjectNotFound | LookupError::OwnerNotFound => {
                QueryDiagnosticKind::UnknownObject
            }
            LookupError::AmbiguousObject => QueryDiagnosticKind::AmbiguousObject,
            LookupError::FieldNotFound | LookupError::StandardFieldHasNoMetadataGuid(_) => {
                QueryDiagnosticKind::UnknownField
            }
            LookupError::AmbiguousField => QueryDiagnosticKind::AmbiguousField,
            LookupError::ValueNotFound => QueryDiagnosticKind::UnknownValue,
            LookupError::AmbiguousValue => QueryDiagnosticKind::AmbiguousValue,
        };
        let mut diagnostic = Self::at_or_unpositioned(kind, token, message);
        diagnostic.source = Some(QueryDiagnosticSource::Lookup(error));
        diagnostic
    }

    /// Returns the machine-readable diagnostic category.
    #[must_use]
    pub const fn kind(&self) -> QueryDiagnosticKind {
        self.kind
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
        if self.line == 0 {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "{}:{}: {}", self.line, self.column, self.message)
        }
    }
}

impl std::error::Error for QueryDiagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(QueryDiagnosticSource::Lex(error)) => Some(error),
            Some(QueryDiagnosticSource::Lookup(error)) => Some(error),
            None => None,
        }
    }
}

impl From<Diagnostic> for QueryDiagnostic {
    fn from(error: Diagnostic) -> Self {
        let message = match error.kind {
            DiagnosticKind::UnterminatedString => "unterminated string literal".to_owned(),
            DiagnosticKind::ExpectedParameterName => {
                "expected a parameter name after '&'".to_owned()
            }
            DiagnosticKind::InvalidBinaryLiteral => {
                "binary literal must use 0x followed by an even number of hexadecimal digits"
                    .to_owned()
            }
            DiagnosticKind::UnexpectedCharacter(character) => {
                format!("unexpected character {character:?}")
            }
        };
        Self {
            kind: QueryDiagnosticKind::Lex,
            message,
            offset: error.offset,
            line: error.line,
            column: error.column,
            source: Some(QueryDiagnosticSource::Lex(error)),
        }
    }
}
