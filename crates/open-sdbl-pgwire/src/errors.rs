//! SDBL diagnostics and database errors as PostgreSQL `ErrorResponse`s.

use open_sdbl::query::{QueryDiagnostic, QueryDiagnosticKind};
use open_sdbl_db::DbError;

use crate::codec::ErrorFields;

/// The SQLSTATE of a compilation diagnostic.
pub(crate) fn diagnostic_code(diagnostic: &QueryDiagnostic) -> &'static str {
    if diagnostic.is_missing_session_parameter() {
        // The client cannot set session parameters: a restriction that
        // needs an absent one cannot be proven to allow the read.
        return "42501";
    }
    match diagnostic.kind() {
        QueryDiagnosticKind::Lex | QueryDiagnosticKind::Syntax => "42601",
        QueryDiagnosticKind::TooDeep | QueryDiagnosticKind::WorkBudgetExceeded => "54001",
        QueryDiagnosticKind::UnknownObject | QueryDiagnosticKind::NotLive => "42P01",
        QueryDiagnosticKind::UnknownField => "42703",
        QueryDiagnosticKind::AmbiguousObject
        | QueryDiagnosticKind::AmbiguousField
        | QueryDiagnosticKind::AmbiguousValue => "42702",
        QueryDiagnosticKind::UnknownValue => "42704",
        QueryDiagnosticKind::UnsupportedFeature => "0A000",
        QueryDiagnosticKind::Parameter if diagnostic.missing_parameter_name().is_some() => "42P02",
        QueryDiagnosticKind::Parameter => "22023",
        QueryDiagnosticKind::TemporaryTable => "42P01",
        QueryDiagnosticKind::Restriction => "42501",
        _ => "XX000",
    }
}

/// The 1-based character position of a byte offset of `text`.
pub(crate) fn character_position(text: &str, offset: usize) -> Option<usize> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    Some(text[..offset].chars().count() + 1)
}

/// The `ErrorResponse` of a diagnostic about `text`.
pub(crate) fn diagnostic_error(text: &str, diagnostic: &QueryDiagnostic) -> ErrorFields {
    let mut fields = ErrorFields::error(diagnostic_code(diagnostic), diagnostic.message());
    // A restriction's position lies inside the restriction text, and an
    // unpositioned diagnostic reports line zero.
    if diagnostic.kind() != QueryDiagnosticKind::Restriction && diagnostic.line() > 0 {
        fields.position = character_position(text, diagnostic.offset());
    }
    if let Some(name) = diagnostic.missing_parameter_name()
        && !diagnostic.is_missing_session_parameter()
    {
        fields.hint = Some(format!(
            "bind a value to &{name} with the extended query protocol"
        ));
    }
    fields
}

/// The `ErrorResponse` of a database failure. The database's own message
/// may name physical tables, so it is a detail, sent only when the
/// operator asked for verbose errors.
pub(crate) fn database_error(error: &DbError, verbose: bool) -> ErrorFields {
    let (code, message) = if error.is_session_closed() {
        ("08006", "the database session is lost")
    } else if error.is_database_timeout() {
        ("57014", "canceling statement due to statement timeout")
    } else {
        ("XX000", "the database could not run the statement")
    };
    let mut fields = ErrorFields::error(code, message);
    if verbose {
        fields.detail = Some(error.to_string());
    }
    fields
}

/// The `ErrorResponse` of a cancelled statement.
pub(crate) fn cancelled() -> ErrorFields {
    ErrorFields::error("57014", "canceling statement due to user request")
}

/// The `ErrorResponse` of an access refusal.
pub(crate) fn access_denied(message: impl Into<String>) -> ErrorFields {
    ErrorFields::error("42501", message)
}

#[cfg(test)]
#[path = "tests/errors.rs"]
mod tests;
