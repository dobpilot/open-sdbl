//! PostgreSQL frontend/backend protocol 3.0: the messages a client sends
//! and the messages the gateway answers with.
//!
//! Decoding is split from framing so it can be fuzzed: [`decode_startup`]
//! reads the body of a length-prefixed startup packet, [`decode_frontend`]
//! the body of a typed message. Neither allocates more than the body it is
//! given, and both reject trailing bytes, negative counts and strings
//! that are not UTF-8.

use std::fmt;

/// The request codes a startup packet carries instead of a version.
const SSL_REQUEST: u32 = 80_877_103;
const GSSENC_REQUEST: u32 = 80_877_104;
const CANCEL_REQUEST: u32 = 80_877_102;

/// The largest startup packet accepted, as PostgreSQL bounds it.
pub const MAX_STARTUP_PACKET: usize = 10_000;

/// A malformed frontend message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError {
    message: String,
}

impl ProtocolError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// What was wrong with the message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProtocolError {}

/// The packet a client opens a connection with.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StartupPacket {
    /// The client asks for TLS before the startup message.
    SslRequest,
    /// The client asks for GSSAPI encryption.
    GssEncRequest,
    /// The client asks to cancel the query of another connection.
    CancelRequest {
        /// The process identifier of `BackendKeyData`.
        process_id: u32,
        /// The secret key of `BackendKeyData`.
        secret: Vec<u8>,
    },
    /// The startup message proper.
    Startup {
        /// The major protocol version.
        major: u16,
        /// The minor protocol version.
        minor: u16,
        /// The parameters, in the order the client sent them.
        parameters: Vec<(String, String)>,
    },
}

/// One message of the regular protocol phase, or the password.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrontendMessage {
    /// `Query`: a simple query.
    Query(String),
    /// `Parse`: prepares a statement.
    Parse {
        /// The statement name; empty for the unnamed statement.
        name: String,
        /// The query text.
        query: String,
        /// The declared parameter types; zero means unspecified.
        parameter_types: Vec<u32>,
    },
    /// `Bind`: binds parameter values into a portal.
    Bind {
        /// The portal name; empty for the unnamed portal.
        portal: String,
        /// The statement name.
        statement: String,
        /// The parameter format codes: none, one for all, or one each.
        parameter_formats: Vec<i16>,
        /// The parameter values; `None` is SQL `NULL`.
        parameters: Vec<Option<Vec<u8>>>,
        /// The result format codes: none, one for all, or one each.
        result_formats: Vec<i16>,
    },
    /// `Describe` of a statement (`S`) or a portal (`P`).
    Describe {
        /// `S` or `P`.
        kind: TargetKind,
        /// The name.
        name: String,
    },
    /// `Execute`: runs a portal.
    Execute {
        /// The portal name.
        portal: String,
        /// The row limit; zero means no limit.
        max_rows: u32,
    },
    /// `Close` of a statement or a portal.
    Close {
        /// `S` or `P`.
        kind: TargetKind,
        /// The name.
        name: String,
    },
    /// `Flush`.
    Flush,
    /// `Sync`.
    Sync,
    /// `Terminate`.
    Terminate,
    /// `PasswordMessage`: the cleartext password.
    Password(zeroize::Zeroizing<String>),
    /// A message type the gateway does not serve: COPY, function calls,
    /// SASL.
    Unsupported(u8),
}

impl fmt::Debug for FrontendMessage {
    /// Shows every message but the password.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(text) => formatter.debug_tuple("Query").field(text).finish(),
            Self::Parse {
                name,
                query,
                parameter_types,
            } => formatter
                .debug_struct("Parse")
                .field("name", name)
                .field("query", query)
                .field("parameter_types", parameter_types)
                .finish(),
            Self::Bind {
                portal,
                statement,
                parameter_formats,
                parameters,
                result_formats,
            } => formatter
                .debug_struct("Bind")
                .field("portal", portal)
                .field("statement", statement)
                .field("parameter_formats", parameter_formats)
                .field("parameters", &parameters.len())
                .field("result_formats", result_formats)
                .finish(),
            Self::Describe { kind, name } => formatter
                .debug_struct("Describe")
                .field("kind", kind)
                .field("name", name)
                .finish(),
            Self::Execute { portal, max_rows } => formatter
                .debug_struct("Execute")
                .field("portal", portal)
                .field("max_rows", max_rows)
                .finish(),
            Self::Close { kind, name } => formatter
                .debug_struct("Close")
                .field("kind", kind)
                .field("name", name)
                .finish(),
            Self::Flush => formatter.write_str("Flush"),
            Self::Sync => formatter.write_str("Sync"),
            Self::Terminate => formatter.write_str("Terminate"),
            Self::Password(_) => formatter.write_str("Password(<redacted>)"),
            Self::Unsupported(code) => formatter.debug_tuple("Unsupported").field(code).finish(),
        }
    }
}

/// Whether `Describe` or `Close` addresses a statement or a portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// A prepared statement.
    Statement,
    /// A portal.
    Portal,
}

/// A cursor over a message body.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        if self.bytes.len() < count {
            return Err(ProtocolError::new("message is truncated"));
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn i16(&mut self) -> Result<i16, ProtocolError> {
        let bytes = self.take(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn i32(&mut self) -> Result<i32, ProtocolError> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn count(&mut self) -> Result<usize, ProtocolError> {
        usize::try_from(self.i16()?).map_err(|_| ProtocolError::new("negative count"))
    }

    fn string(&mut self) -> Result<String, ProtocolError> {
        let end = self
            .bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| ProtocolError::new("string is not terminated"))?;
        let text = std::str::from_utf8(&self.bytes[..end])
            .map_err(|_| ProtocolError::new("string is not valid UTF-8"))?
            .to_owned();
        self.bytes = &self.bytes[end + 1..];
        Ok(text)
    }

    fn finish(&self) -> Result<(), ProtocolError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::new("message has trailing bytes"))
        }
    }
}

/// Decodes the body of a startup packet: everything after its length.
///
/// # Errors
///
/// Returns [`ProtocolError`] for a truncated, oversized or malformed body.
pub fn decode_startup(body: &[u8]) -> Result<StartupPacket, ProtocolError> {
    if body.len() + 4 > MAX_STARTUP_PACKET {
        return Err(ProtocolError::new("startup packet is too long"));
    }
    let mut reader = Reader { bytes: body };
    let code = reader.u32()?;
    let packet = match code {
        SSL_REQUEST => StartupPacket::SslRequest,
        GSSENC_REQUEST => StartupPacket::GssEncRequest,
        CANCEL_REQUEST => {
            let process_id = reader.u32()?;
            let secret = reader.bytes.to_vec();
            reader.bytes = &[];
            if secret.is_empty() || secret.len() > 256 {
                return Err(ProtocolError::new("cancel request key has a wrong length"));
            }
            StartupPacket::CancelRequest { process_id, secret }
        }
        version => {
            let major = u16::try_from(version >> 16).expect("16 bits");
            let minor = u16::try_from(version & 0xffff).expect("16 bits");
            let mut parameters = Vec::new();
            loop {
                let name = reader.string()?;
                if name.is_empty() {
                    break;
                }
                let value = reader.string()?;
                parameters.push((name, value));
            }
            StartupPacket::Startup {
                major,
                minor,
                parameters,
            }
        }
    };
    reader.finish()?;
    Ok(packet)
}

fn target(byte: u8) -> Result<TargetKind, ProtocolError> {
    match byte {
        b'S' => Ok(TargetKind::Statement),
        b'P' => Ok(TargetKind::Portal),
        _ => Err(ProtocolError::new("describe or close target is not S or P")),
    }
}

/// Decodes the body of a typed frontend message: everything after its
/// type byte and length.
///
/// # Errors
///
/// Returns [`ProtocolError`] for a truncated or malformed body.
pub fn decode_frontend(code: u8, body: &[u8]) -> Result<FrontendMessage, ProtocolError> {
    let mut reader = Reader { bytes: body };
    let message = match code {
        b'Q' => FrontendMessage::Query(reader.string()?),
        b'P' => {
            let name = reader.string()?;
            let query = reader.string()?;
            let count = reader.count()?;
            let mut parameter_types = Vec::with_capacity(count.min(reader.bytes.len() / 4));
            for _ in 0..count {
                parameter_types.push(reader.u32()?);
            }
            FrontendMessage::Parse {
                name,
                query,
                parameter_types,
            }
        }
        b'B' => {
            let portal = reader.string()?;
            let statement = reader.string()?;
            let count = reader.count()?;
            let mut parameter_formats = Vec::with_capacity(count.min(reader.bytes.len() / 2));
            for _ in 0..count {
                parameter_formats.push(reader.i16()?);
            }
            let count = reader.count()?;
            let mut parameters = Vec::with_capacity(count.min(reader.bytes.len() / 4));
            for _ in 0..count {
                let length = reader.i32()?;
                parameters.push(match length {
                    -1 => None,
                    length => {
                        let length = usize::try_from(length)
                            .map_err(|_| ProtocolError::new("negative parameter length"))?;
                        Some(reader.take(length)?.to_vec())
                    }
                });
            }
            let count = reader.count()?;
            let mut result_formats = Vec::with_capacity(count.min(reader.bytes.len() / 2));
            for _ in 0..count {
                result_formats.push(reader.i16()?);
            }
            FrontendMessage::Bind {
                portal,
                statement,
                parameter_formats,
                parameters,
                result_formats,
            }
        }
        b'D' => {
            let kind = target(reader.u8()?)?;
            FrontendMessage::Describe {
                kind,
                name: reader.string()?,
            }
        }
        b'E' => {
            let portal = reader.string()?;
            let max_rows = reader.i32()?;
            FrontendMessage::Execute {
                portal,
                // A negative limit is no limit, as PostgreSQL reads it.
                max_rows: u32::try_from(max_rows).unwrap_or(0),
            }
        }
        b'C' => {
            let kind = target(reader.u8()?)?;
            FrontendMessage::Close {
                kind,
                name: reader.string()?,
            }
        }
        b'H' => FrontendMessage::Flush,
        b'S' => FrontendMessage::Sync,
        b'X' => FrontendMessage::Terminate,
        b'p' => {
            let password = zeroize::Zeroizing::new(reader.string()?);
            FrontendMessage::Password(password)
        }
        other => {
            reader.bytes = &[];
            FrontendMessage::Unsupported(other)
        }
    };
    reader.finish()?;
    Ok(message)
}

/// The transaction status `ReadyForQuery` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    /// Not in a transaction block.
    Idle,
    /// In a transaction block.
    Block,
    /// In a failed transaction block.
    Failed,
}

/// One column of a `RowDescription`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDescription {
    /// The column label.
    pub name: String,
    /// The type OID.
    pub type_oid: u32,
    /// The type length: positive for a fixed width, `-1` for varlena.
    pub type_length: i16,
    /// The type modifier; `-1` when none.
    pub type_modifier: i32,
    /// The format code: 0 text, 1 binary.
    pub format: i16,
}

/// The fields of an `ErrorResponse` or `NoticeResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorFields {
    /// `ERROR`, `FATAL` or `WARNING`.
    pub severity: &'static str,
    /// The SQLSTATE.
    pub code: &'static str,
    /// The primary message.
    pub message: String,
    /// The detail, when there is one.
    pub detail: Option<String>,
    /// The hint, when there is one.
    pub hint: Option<String>,
    /// The 1-based character position in the query text.
    pub position: Option<usize>,
}

impl ErrorFields {
    /// An `ERROR` with a code and a message.
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: "ERROR",
            code,
            message: message.into(),
            detail: None,
            hint: None,
            position: None,
        }
    }

    /// A `FATAL` with a code and a message.
    #[must_use]
    pub fn fatal(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: "FATAL",
            ..Self::error(code, message)
        }
    }
}

/// Appends backend messages to a buffer.
#[derive(Debug, Default)]
pub struct BackendWriter {
    buffer: Vec<u8>,
}

impl BackendWriter {
    /// An empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// The encoded bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Takes the encoded bytes, leaving the buffer empty.
    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buffer)
    }

    /// Bytes waiting to be sent.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether nothing waits to be sent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Appends raw, already framed bytes.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn message(&mut self, code: u8, body: impl FnOnce(&mut Vec<u8>)) {
        self.buffer.push(code);
        let start = self.buffer.len();
        self.buffer.extend_from_slice(&[0; 4]);
        body(&mut self.buffer);
        let length = u32::try_from(self.buffer.len() - start).expect("message fits u32");
        self.buffer[start..start + 4].copy_from_slice(&length.to_be_bytes());
    }

    /// `AuthenticationOk`.
    pub fn authentication_ok(&mut self) {
        self.message(b'R', |body| body.extend_from_slice(&0_i32.to_be_bytes()));
    }

    /// `AuthenticationCleartextPassword`.
    pub fn authentication_cleartext(&mut self) {
        self.message(b'R', |body| body.extend_from_slice(&3_i32.to_be_bytes()));
    }

    /// `ParameterStatus`.
    pub fn parameter_status(&mut self, name: &str, value: &str) {
        self.message(b'S', |body| {
            put_string(body, name);
            put_string(body, value);
        });
    }

    /// `BackendKeyData` with a 4-byte secret.
    pub fn backend_key_data(&mut self, process_id: u32, secret: u32) {
        self.message(b'K', |body| {
            body.extend_from_slice(&process_id.to_be_bytes());
            body.extend_from_slice(&secret.to_be_bytes());
        });
    }

    /// `NegotiateProtocolVersion` offering minor version 0.
    pub fn negotiate_protocol_version(&mut self, unrecognized: &[String]) {
        self.message(b'v', |body| {
            body.extend_from_slice(&0_i32.to_be_bytes());
            let count = i32::try_from(unrecognized.len()).unwrap_or(i32::MAX);
            body.extend_from_slice(&count.to_be_bytes());
            for option in unrecognized {
                put_string(body, option);
            }
        });
    }

    /// `ReadyForQuery`.
    pub fn ready_for_query(&mut self, status: TransactionStatus) {
        self.message(b'Z', |body| {
            body.push(match status {
                TransactionStatus::Idle => b'I',
                TransactionStatus::Block => b'T',
                TransactionStatus::Failed => b'E',
            });
        });
    }

    /// `RowDescription`.
    pub fn row_description(&mut self, fields: &[FieldDescription]) {
        self.message(b'T', |body| {
            let count = i16::try_from(fields.len()).unwrap_or(i16::MAX);
            body.extend_from_slice(&count.to_be_bytes());
            for field in fields {
                put_string(body, &field.name);
                body.extend_from_slice(&0_u32.to_be_bytes());
                body.extend_from_slice(&0_i16.to_be_bytes());
                body.extend_from_slice(&field.type_oid.to_be_bytes());
                body.extend_from_slice(&field.type_length.to_be_bytes());
                body.extend_from_slice(&field.type_modifier.to_be_bytes());
                body.extend_from_slice(&field.format.to_be_bytes());
            }
        });
    }

    /// `DataRow`; `None` is SQL `NULL`.
    pub fn data_row<'a>(&mut self, values: impl IntoIterator<Item = Option<&'a [u8]>>) {
        self.message(b'D', |body| {
            let count_at = body.len();
            body.extend_from_slice(&[0; 2]);
            let mut count = 0_i16;
            for value in values {
                count += 1;
                match value {
                    None => body.extend_from_slice(&(-1_i32).to_be_bytes()),
                    Some(bytes) => {
                        let length = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
                        body.extend_from_slice(&length.to_be_bytes());
                        body.extend_from_slice(bytes);
                    }
                }
            }
            body[count_at..count_at + 2].copy_from_slice(&count.to_be_bytes());
        });
    }

    /// `CommandComplete`.
    pub fn command_complete(&mut self, tag: &str) {
        self.message(b'C', |body| put_string(body, tag));
    }

    /// `EmptyQueryResponse`.
    pub fn empty_query(&mut self) {
        self.message(b'I', |_| {});
    }

    /// `ErrorResponse`.
    pub fn error(&mut self, fields: &ErrorFields) {
        self.fields(b'E', fields);
    }

    /// `NoticeResponse`.
    pub fn notice(&mut self, fields: &ErrorFields) {
        self.fields(b'N', fields);
    }

    fn fields(&mut self, code: u8, fields: &ErrorFields) {
        self.message(code, |body| {
            let mut field = |code: u8, value: &str| {
                body.push(code);
                put_string(body, value);
            };
            field(b'S', fields.severity);
            field(b'V', fields.severity);
            field(b'C', fields.code);
            field(b'M', &fields.message);
            if let Some(detail) = &fields.detail {
                field(b'D', detail);
            }
            if let Some(hint) = &fields.hint {
                field(b'H', hint);
            }
            if let Some(position) = fields.position {
                field(b'P', &position.to_string());
            }
            body.push(0);
        });
    }

    /// `ParseComplete`.
    pub fn parse_complete(&mut self) {
        self.message(b'1', |_| {});
    }

    /// `BindComplete`.
    pub fn bind_complete(&mut self) {
        self.message(b'2', |_| {});
    }

    /// `CloseComplete`.
    pub fn close_complete(&mut self) {
        self.message(b'3', |_| {});
    }

    /// `NoData`.
    pub fn no_data(&mut self) {
        self.message(b'n', |_| {});
    }

    /// `PortalSuspended`.
    pub fn portal_suspended(&mut self) {
        self.message(b's', |_| {});
    }

    /// `ParameterDescription`.
    pub fn parameter_description(&mut self, types: &[u32]) {
        self.message(b't', |body| {
            let count = i16::try_from(types.len()).unwrap_or(i16::MAX);
            body.extend_from_slice(&count.to_be_bytes());
            for oid in types {
                body.extend_from_slice(&oid.to_be_bytes());
            }
        });
    }
}

fn put_string(body: &mut Vec<u8>, value: &str) {
    // A NUL inside would end the string early on the client.
    body.extend(
        value
            .bytes()
            .map(|byte| if byte == 0 { b'?' } else { byte }),
    );
    body.push(0);
}

#[cfg(test)]
#[path = "tests/codec.rs"]
mod tests;
