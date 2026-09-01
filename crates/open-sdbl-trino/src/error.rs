use std::fmt;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Stable error category shared with the Java connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ObjectNotFound,
    ColumnNotFound,
    UnsupportedType,
    UnsupportedPredicate,
    InvalidMetadata,
    PostgresConnection,
    PostgresQuery,
    Compilation,
    Timeout,
    ResultLimit,
    Protocol,
    Internal,
}

/// An error safe to return across the connector protocol.
#[derive(Debug)]
pub struct ServiceError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ServiceError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    #[must_use]
    pub const fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ServiceError {}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: ErrorCode,
    message: &'a str,
    retryable: bool,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let status = match self.code {
            ErrorCode::ObjectNotFound | ErrorCode::ColumnNotFound => StatusCode::NOT_FOUND,
            ErrorCode::UnsupportedType
            | ErrorCode::UnsupportedPredicate
            | ErrorCode::Compilation
            | ErrorCode::ResultLimit
            | ErrorCode::Protocol => StatusCode::BAD_REQUEST,
            ErrorCode::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::PostgresConnection | ErrorCode::PostgresQuery => StatusCode::BAD_GATEWAY,
            ErrorCode::InvalidMetadata | ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: &self.message,
                    retryable: self.retryable,
                },
            }),
        )
            .into_response()
    }
}
