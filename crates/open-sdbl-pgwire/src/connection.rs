//! One client connection: startup, authentication, and the simple and
//! extended query protocols over SDBL.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Instant;

use open_sdbl::metadata::{InfoBaseUser, MetadataSnapshot, ObjectId};
use open_sdbl::password::{
    PasswordVerification, SourceProfile, VerificationOptions, VerificationStatus,
    verify_stored_password,
};
use open_sdbl::query::{
    AccessDecision, CompileOptions, CompiledColumn, CompiledQuery, FieldUsageRequest, MsSqlBackend,
    PostgresBackend, PrepareOptions, Prepared, PresentationPlan, PresentationRequest,
    QueryCompiler, QueryDiagnostic, QueryParameter, RestrictionRequest, RestrictionTarget,
    SessionParameters, TempTablesManager, object_query_name,
};
use open_sdbl::{Keyword, Lexer, TokenKind};
use open_sdbl_db::presentation::default_presentation_plan;
use open_sdbl_db::{
    AccessStore, Cell, DatabaseDialect, DbError, RowFlow, check_user_field_access, user_decisions,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream, ReadBuf};
use tokio::sync::watch;

use crate::cancel::{CancelKey, CancelRegistry};
use crate::catalog::{self, Catalog};
use crate::chatter::{self, Chatter};
use crate::codec::{
    BackendWriter, ErrorFields, FieldDescription, FrontendMessage, MAX_STARTUP_PACKET,
    StartupPacket, TargetKind, TransactionStatus, decode_frontend, decode_startup,
};
use crate::engine::{Engine, EngineSession, PasswordEvidence};
use crate::errors::{access_denied, cancelled, database_error, diagnostic_error};
use crate::state::GatewayState;
use crate::values::{
    BindError, WireType, decode_parameter, describe_column, encode_cell, format_of, oid,
};
use crate::{GatewayEvent, Shared};

/// A value that never matches, verified for unknown users so that they
/// cost what a known user costs.
const UNKNOWN_USER_VALUE: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAA=,AAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// The plain or TLS stream of a client.
enum Transport<S> {
    Plain(S),
    Tls(Box<tokio_rustls::server::TlsStream<S>>),
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for Transport<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_read(context, buffer),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for Transport<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(context, buffer),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_write(context, buffer),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(context),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Tls(stream) => Pin::new(stream.as_mut()).poll_shutdown(context),
        }
    }
}

/// Why reading the next message failed.
enum ReadError {
    Closed,
    Oversized(usize),
    Malformed,
    Idle,
}

async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<(u8, Vec<u8>), ReadError> {
    let mut header = [0_u8; 5];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|_| ReadError::Closed)?;
    let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
    let body = usize::try_from(length)
        .ok()
        .and_then(|length| length.checked_sub(4))
        .ok_or(ReadError::Malformed)?;
    if body > max {
        return Err(ReadError::Oversized(max));
    }
    let mut bytes = vec![0_u8; body];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ReadError::Closed)?;
    Ok((header[0], bytes))
}

async fn read_startup<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, ReadError> {
    let mut length = [0_u8; 4];
    reader
        .read_exact(&mut length)
        .await
        .map_err(|_| ReadError::Closed)?;
    let length = usize::try_from(u32::from_be_bytes(length)).map_err(|_| ReadError::Malformed)?;
    if !(8..=MAX_STARTUP_PACKET).contains(&length) {
        return Err(ReadError::Malformed);
    }
    let mut body = vec![0_u8; length - 4];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| ReadError::Closed)?;
    Ok(body)
}

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, out: &mut BackendWriter) -> io::Result<()> {
    let bytes = out.take();
    writer.write_all(&bytes).await?;
    writer.flush().await
}

async fn send_fatal<W: AsyncWrite + Unpin>(writer: &mut W, fields: ErrorFields) {
    let mut out = BackendWriter::new();
    out.error(&fields);
    let _ = send(writer, &mut out).await;
}

/// Refuses a connection over the connection limit.
pub(crate) async fn refuse_busy<E: Engine, S: AsyncRead + AsyncWrite + Unpin>(
    shared: &Shared<E>,
    mut stream: S,
    peer: Option<SocketAddr>,
) {
    shared.events.event(GatewayEvent::Refused {
        peer,
        reason: "too many connections",
    });
    send_fatal(
        &mut stream,
        ErrorFields::fatal("53300", "sorry, too many clients already"),
    )
    .await;
}

/// Serves one client from its first byte to its last.
pub(crate) async fn run<E: Engine, S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    shared: Arc<Shared<E>>,
    stream: S,
    peer: Option<SocketAddr>,
    stopped: watch::Receiver<bool>,
) {
    shared.events.event(GatewayEvent::Connected { peer });
    let deadline = shared.limits.startup_timeout;
    match tokio::time::timeout(deadline, startup(&shared, stream, peer)).await {
        Ok(Some(client)) => client.serve(stopped).await,
        Ok(None) => {}
        Err(_) => shared.events.event(GatewayEvent::Refused {
            peer,
            reason: "startup and authentication timed out",
        }),
    }
    shared.events.event(GatewayEvent::Disconnected { peer });
}

/// The reported run-time parameters, in the order they are announced.
fn startup_settings(user: &str, application_name: &str) -> Vec<(String, String)> {
    [
        (
            "server_version",
            format!("13.0 (open-sdbl {})", env!("CARGO_PKG_VERSION")),
        ),
        ("server_encoding", "UTF8".to_owned()),
        ("client_encoding", "UTF8".to_owned()),
        ("DateStyle", "ISO, MDY".to_owned()),
        ("IntervalStyle", "postgres".to_owned()),
        ("integer_datetimes", "on".to_owned()),
        ("standard_conforming_strings", "on".to_owned()),
        ("TimeZone", "UTC".to_owned()),
        ("default_transaction_read_only", "on".to_owned()),
        ("is_superuser", "off".to_owned()),
        ("session_authorization", user.to_owned()),
        ("application_name", application_name.to_owned()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect()
}

async fn startup<E: Engine, S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    shared: &Arc<Shared<E>>,
    stream: S,
    peer: Option<SocketAddr>,
) -> Option<Client<E, S>> {
    let mut transport = Transport::Plain(stream);
    let mut out = BackendWriter::new();
    let parameters = loop {
        let body = read_startup(&mut transport).await.ok()?;
        match decode_startup(&body) {
            Err(error) => {
                send_fatal(&mut transport, ErrorFields::fatal("08P01", error.message())).await;
                return None;
            }
            Ok(StartupPacket::SslRequest) => match (transport, &shared.tls) {
                (Transport::Plain(mut stream), Some(acceptor)) => {
                    stream.write_all(b"S").await.ok()?;
                    stream.flush().await.ok()?;
                    match acceptor.accept(stream).await {
                        Ok(tls) => transport = Transport::Tls(Box::new(tls)),
                        Err(error) => {
                            shared.events.event(GatewayEvent::Refused {
                                peer,
                                reason: &format!("TLS handshake failed: {error}"),
                            });
                            return None;
                        }
                    }
                }
                (Transport::Plain(mut stream), None) => {
                    stream.write_all(b"N").await.ok()?;
                    stream.flush().await.ok()?;
                    transport = Transport::Plain(stream);
                }
                (tls @ Transport::Tls(_), _) => {
                    let mut tls = tls;
                    send_fatal(
                        &mut tls,
                        ErrorFields::fatal("08P01", "TLS was negotiated twice"),
                    )
                    .await;
                    return None;
                }
            },
            Ok(StartupPacket::GssEncRequest) => {
                transport.write_all(b"N").await.ok()?;
                transport.flush().await.ok()?;
            }
            Ok(StartupPacket::CancelRequest { process_id, secret }) => {
                shared.registry.cancel(process_id, &secret);
                return None;
            }
            Ok(StartupPacket::Startup {
                major,
                minor,
                parameters,
            }) => {
                if major != 3 {
                    send_fatal(
                        &mut transport,
                        ErrorFields::fatal(
                            "0A000",
                            format!(
                                "unsupported frontend protocol {major}.{minor}: server supports 3.0"
                            ),
                        ),
                    )
                    .await;
                    return None;
                }
                let (options, parameters): (Vec<_>, Vec<_>) = parameters
                    .into_iter()
                    .partition(|(name, _)| name.starts_with("_pq_."));
                if minor > 0 || !options.is_empty() {
                    let names = options
                        .into_iter()
                        .map(|(name, _)| name)
                        .collect::<Vec<_>>();
                    out.negotiate_protocol_version(&names);
                }
                break parameters;
            }
        }
    };
    let parameter = |name: &str| {
        parameters
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let Some(user) = parameter("user").filter(|user| !user.is_empty()) else {
        out.error(&ErrorFields::fatal(
            "28000",
            "no PostgreSQL user name specified in startup packet",
        ));
        let _ = send(&mut transport, &mut out).await;
        return None;
    };
    if let Some(encoding) = parameter("client_encoding")
        && !chatter::is_utf8(&encoding)
    {
        out.error(&ErrorFields::fatal(
            "22023",
            format!("invalid value for parameter \"client_encoding\": \"{encoding}\""),
        ));
        let _ = send(&mut transport, &mut out).await;
        return None;
    }
    let application_name = parameter("application_name").unwrap_or_default();
    let encrypted = matches!(transport, Transport::Tls(_));
    let secure = encrypted
        || shared.allow_insecure_network
        || (shared.allow_insecure_loopback && peer.is_some_and(|peer| peer.ip().is_loopback()));
    if !secure {
        shared.events.event(GatewayEvent::Refused {
            peer,
            reason: "a password is accepted only over TLS",
        });
        out.error(&ErrorFields::fatal(
            "28000",
            "the gateway accepts a password only over TLS; connect with sslmode=require",
        ));
        let _ = send(&mut transport, &mut out).await;
        return None;
    }
    out.authentication_cleartext();
    send(&mut transport, &mut out).await.ok()?;
    let (code, body) = read_message(&mut transport, shared.limits.max_message_bytes)
        .await
        .ok()?;
    let Ok(FrontendMessage::Password(password)) = decode_frontend(code, &body) else {
        send_fatal(
            &mut transport,
            ErrorFields::fatal("08P01", "expected a password response"),
        )
        .await;
        return None;
    };
    drop(body);
    let received = Instant::now();
    let state = shared.state.current();
    let record = match authenticate(shared, &state, &user, &password) {
        Ok(record) => record,
        Err(reason) => {
            if shared.debug_authentication_secrets {
                let evidence = state
                    .access()
                    .user(&user)
                    .map(|user| shared.engine.password_evidence(user));
                let stored = match &evidence {
                    Some(PasswordEvidence::UserData) => state
                        .access()
                        .user(&user)
                        .and_then(|user| user.data.sensitive_stored_password_for_diagnostics()),
                    Some(PasswordEvidence::Stored { value, .. }) => Some(value.as_str()),
                    None => None,
                };
                shared.events.event(GatewayEvent::AuthenticationSecrets {
                    peer,
                    user: &user,
                    password: &password,
                    stored,
                });
            }
            drop(password);
            tokio::time::sleep_until(
                (received + shared.limits.authentication_failure_delay).into(),
            )
            .await;
            shared
                .events
                .event(GatewayEvent::AuthenticationFailed { peer, user: &user });
            if shared.debug_authentication {
                shared.events.event(GatewayEvent::AuthenticationDiagnostic {
                    peer,
                    user: &user,
                    reason,
                });
            }
            send_fatal(
                &mut transport,
                // One message for every failure: it must not tell an unknown
                // user from a wrong password.
                ErrorFields::fatal("28P01", "password authentication failed"),
            )
            .await;
            return None;
        }
    };
    drop(password);
    let mut db = match shared.engine.open().await {
        Ok(db) => db,
        Err(error) => {
            shared.events.event(GatewayEvent::ConnectionError {
                peer,
                message: &format!("cannot open a database session: {error}"),
            });
            let mut fields = ErrorFields::fatal("08006", "the gateway cannot reach the database");
            if shared.verbose_errors {
                fields.detail = Some(error.to_string());
            }
            send_fatal(&mut transport, fields).await;
            return None;
        }
    };
    let derived = shared
        .engine
        .derive_session(&mut db, &state, &record, &shared.session_parameters)
        .await;
    for reason in &derived.unread {
        shared.events.event(GatewayEvent::SessionParameterUnread {
            peer,
            user: &record.name,
            reason,
        });
    }
    let mut access = state.access().clone();
    let user_name = record.name.clone();
    access.set_current_user(Some(record));
    let key = CancelRegistry::register(&shared.registry);
    let settings = startup_settings(&user_name, &application_name);
    out.authentication_ok();
    for (name, value) in &settings {
        out.parameter_status(name, value);
    }
    out.backend_key_data(key.process_id, key.secret);
    out.ready_for_query(TransactionStatus::Idle);
    send(&mut transport, &mut out).await.ok()?;
    shared.events.event(GatewayEvent::Authenticated {
        peer,
        user: &user_name,
        application_name: &application_name,
    });
    Some(Client {
        shared: Arc::clone(shared),
        io: BufStream::new(transport),
        out: BackendWriter::new(),
        peer,
        user_name,
        database_name: shared.database_name.clone(),
        encrypted,
        catalog: None,
        db: Some(db),
        access,
        parameters: derived.parameters,
        state,
        decisions: HashMap::new(),
        temp: TempTablesManager::new(),
        statements: HashMap::new(),
        portals: HashMap::new(),
        transaction: TransactionStatus::Idle,
        skip_to_sync: false,
        startup_settings: settings.clone(),
        settings,
        key,
    })
}

/// The user a password authenticates, or `None`.
fn authenticate<E: Engine>(
    shared: &Shared<E>,
    state: &GatewayState,
    name: &str,
    password: &str,
) -> Result<InfoBaseUser, &'static str> {
    // Secondary diagnostics are never used for authentication.
    let options = VerificationOptions::new();
    let Some(user) = state.access().user(name) else {
        let _ = verify_stored_password(password, UNKNOWN_USER_VALUE, &options);
        return Err("USER_NOT_FOUND");
    };
    let (verification, source): (PasswordVerification, SourceProfile) =
        match shared.engine.password_evidence(user) {
            PasswordEvidence::UserData => {
                // The adapter verifies against the user's public
                // StoredPasswordValue when the record carried one; a
                // primary MATCH then comes from a confirmed production
                // source, and a record without a stored value yields an
                // indeterminate result that never reaches `permitted`.
                let verification = user.data.verify_password(password, &options);
                (verification, SourceProfile::PublicStoredValue8327)
            }
            PasswordEvidence::Stored {
                value,
                source,
                state,
            } => (
                verify_stored_password(
                    password,
                    &value,
                    &options.clone().source(source).password_state(state),
                ),
                source,
            ),
        };
    if !user.standard_authentication {
        return Err("STANDARD_AUTHENTICATION_DISABLED");
    }
    if !source.is_confirmed() || source.is_synthetic() {
        return Err("PASSWORD_SOURCE_UNTRUSTED");
    }
    if verification.status() != VerificationStatus::Match {
        return Err(verification.reason().code());
    }
    Ok(user.clone())
}

/// A prepared SDBL text for either backend.
enum AnyPrepared {
    Postgres(Prepared<PostgresBackend>),
    MsSql(Prepared<MsSqlBackend>),
}

impl AnyPrepared {
    fn prepare(
        dialect: DatabaseDialect,
        snapshot: &MetadataSnapshot,
        text: &str,
        manager: &TempTablesManager,
    ) -> Result<Self, QueryDiagnostic> {
        let options = PrepareOptions::new().temporary_tables(manager).restricted();
        Ok(match dialect {
            DatabaseDialect::Postgres => Self::Postgres(
                QueryCompiler::new(snapshot, PostgresBackend)
                    .prepare_with_options(text, &options)?,
            ),
            DatabaseDialect::MsSql { backend } => Self::MsSql(
                QueryCompiler::new(snapshot, backend).prepare_with_options(text, &options)?,
            ),
        })
    }

    fn parameter_names(&self) -> &[String] {
        match self {
            Self::Postgres(prepared) => prepared.parameter_names(),
            Self::MsSql(prepared) => prepared.parameter_names(),
        }
    }

    fn restriction_request(&self) -> &RestrictionRequest {
        match self {
            Self::Postgres(prepared) => prepared.restriction_request(),
            Self::MsSql(prepared) => prepared.restriction_request(),
        }
    }

    fn presentation_request(&self) -> &PresentationRequest {
        match self {
            Self::Postgres(prepared) => prepared.presentation_request(),
            Self::MsSql(prepared) => prepared.presentation_request(),
        }
    }

    fn field_usage(&self) -> &FieldUsageRequest {
        match self {
            Self::Postgres(prepared) => prepared.field_usage(),
            Self::MsSql(prepared) => prepared.field_usage(),
        }
    }

    fn compile_batch(
        &self,
        snapshot: &MetadataSnapshot,
        options: &CompileOptions<'_>,
        manager: &mut TempTablesManager,
    ) -> Result<Option<CompiledQuery>, QueryDiagnostic> {
        match self {
            Self::Postgres(prepared) => prepared.compile_batch(snapshot, options, manager),
            Self::MsSql(prepared) => prepared.compile_batch(snapshot, options, manager),
        }
    }

    fn compile_presentation_lookup(
        &self,
        snapshot: &MetadataSnapshot,
        plan: &PresentationPlan,
        references: &[[u8; 16]],
        options: &CompileOptions<'_>,
    ) -> Result<CompiledQuery, QueryDiagnostic> {
        match self {
            Self::Postgres(prepared) => {
                prepared.compile_presentation_lookup(snapshot, plan, references, options)
            }
            Self::MsSql(prepared) => {
                prepared.compile_presentation_lookup(snapshot, plan, references, options)
            }
        }
    }
}

/// What one statement of an SDBL batch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementKind {
    Select,
    Place,
    Drop,
}

impl StatementKind {
    /// The completion tag of a statement that returns no rows.
    const fn tag(self) -> &'static str {
        match self {
            Self::Select | Self::Place => "SELECT 0",
            Self::Drop => "DROP TABLE",
        }
    }
}

/// The statements of a batch, split on top-level semicolons.
fn statement_kinds(text: &str) -> Vec<StatementKind> {
    let mut kinds = Vec::new();
    let mut tokens = Vec::new();
    let flush = |tokens: &mut Vec<TokenKind>, kinds: &mut Vec<StatementKind>| {
        if let Some(first) = tokens.first() {
            kinds.push(if *first == TokenKind::Keyword(Keyword::Drop) {
                StatementKind::Drop
            } else if tokens.contains(&TokenKind::Keyword(Keyword::Into)) {
                StatementKind::Place
            } else {
                StatementKind::Select
            });
        }
        tokens.clear();
    };
    let mut depth = 0_usize;
    for token in Lexer::new(text).map_while(Result::ok) {
        match (token.kind, token.lexeme) {
            (TokenKind::Comment, _) => {}
            (TokenKind::Punctuation, ";") if depth == 0 => flush(&mut tokens, &mut kinds),
            (TokenKind::Punctuation, "(") => {
                depth += 1;
                tokens.push(token.kind);
            }
            (TokenKind::Punctuation, ")") => {
                depth = depth.saturating_sub(1);
                tokens.push(token.kind);
            }
            (kind, _) if depth > 0 && kind == TokenKind::Keyword(Keyword::Into) => {}
            (kind, _) => tokens.push(kind),
        }
    }
    flush(&mut tokens, &mut kinds);
    kinds
}

/// How the visible columns of a result are sent.
#[derive(Debug, Clone)]
struct ResultPlan {
    column_count: usize,
    visible: Vec<usize>,
    wires: Vec<WireType>,
    binary: Vec<bool>,
    fields: Vec<FieldDescription>,
}

impl ResultPlan {
    fn new(columns: &[CompiledColumn], hidden: &[usize], formats: &[i16]) -> Self {
        let visible = (0..columns.len())
            .filter(|index| !hidden.contains(index))
            .collect::<Vec<_>>();
        let mut wires = Vec::with_capacity(visible.len());
        let mut binary = Vec::with_capacity(visible.len());
        let mut fields = Vec::with_capacity(visible.len());
        for (position, index) in visible.iter().enumerate() {
            let format = format_of(formats, position);
            let column = &columns[*index];
            let (field, wire) = describe_column(&column.label, &column.kind, format);
            fields.push(field);
            wires.push(wire);
            binary.push(format == 1);
        }
        Self {
            column_count: columns.len(),
            visible,
            wires,
            binary,
            fields,
        }
    }

    fn of_query(query: &CompiledQuery, formats: &[i16]) -> Self {
        let mut hidden = query.service_columns.clone();
        hidden.extend(query.forbidden_row_column);
        Self::new(&query.columns, &hidden, formats)
    }

    fn encode(&self, cells: &[Cell]) -> Vec<u8> {
        let encoded = self
            .visible
            .iter()
            .enumerate()
            .map(|(position, index)| {
                encode_cell(&cells[*index], self.wires[position], self.binary[position])
            })
            .collect::<Vec<_>>();
        let mut out = BackendWriter::new();
        out.data_row(encoded.iter().map(Option::as_deref));
        out.take()
    }
}

/// The output of a chatter command.
struct ChatterOutput {
    rows: Option<(Vec<FieldDescription>, Vec<Vec<u8>>)>,
    tag: String,
}

fn text_field(name: &str, format: i16) -> FieldDescription {
    FieldDescription {
        name: name.to_owned(),
        type_oid: oid::TEXT,
        type_length: -1,
        type_modifier: -1,
        format,
    }
}

fn one_text_row(name: &str, value: &str) -> (Vec<FieldDescription>, Vec<Vec<u8>>) {
    let mut out = BackendWriter::new();
    out.data_row([Some(value.as_bytes())]);
    (vec![text_field(name, 0)], vec![out.take()])
}

/// The rows a chatter command returns, described before it runs.
fn chatter_fields(command: &Chatter) -> Option<Vec<FieldDescription>> {
    match command {
        Chatter::Show(name) => Some(vec![text_field(name, 0)]),
        Chatter::Version => Some(vec![text_field("version", 0)]),
        Chatter::CurrentSchema => Some(vec![text_field("current_schema", 0)]),
        Chatter::SslStatus => Some(vec![catalog::sql::Column::new("ssl", 16).field(0)]),
        Chatter::StartupTime => Some(vec![
            catalog::sql::Column::new("startup_time", 1700).field(0),
        ]),
        Chatter::SelectText { name, .. } => Some(vec![text_field(name, 0)]),
        Chatter::SelectOne => Some(vec![FieldDescription {
            name: "?column?".to_owned(),
            type_oid: oid::INT4,
            type_length: 4,
            type_modifier: -1,
            format: 0,
        }]),
        _ => None,
    }
}

/// A statement prepared by `Parse`.
struct Statement {
    text: String,
    body: StatementBody,
    parameter_types: Vec<u32>,
}

#[derive(Clone)]
enum StatementBody {
    Chatter(Chatter),
    Catalog(Box<catalog::sql::Query>),
    Sdbl { parameter_names: Vec<String> },
}

/// A portal created by `Bind`.
struct Portal {
    body: PortalBody,
}

enum PortalBody {
    Catalog {
        fields: Vec<FieldDescription>,
        rows: VecDeque<Vec<u8>>,
        sent: usize,
    },
    Chatter {
        command: Chatter,
        formats: Vec<i16>,
        done: bool,
    },
    Sdbl(Box<SdblPortal>),
}

struct SdblPortal {
    prepared: AnyPrepared,
    query: Option<CompiledQuery>,
    plan: Option<ResultPlan>,
    kinds: Vec<StatementKind>,
    /// The temporary tables after the batch, committed when it ran.
    manager: Option<TempTablesManager>,
    progress: PortalProgress,
}

enum PortalProgress {
    Ready,
    Suspended(VecDeque<Vec<u8>>),
    Done,
}

/// What a batch compiled into.
struct Compiled {
    prepared: AnyPrepared,
    query: Option<CompiledQuery>,
}

/// The rows of one execution.
struct Execution {
    sent: usize,
    held: VecDeque<Vec<u8>>,
}

/// How the database part of an execution ended.
enum QueryEnd {
    Done(Result<(), DbError>),
    Cancelled,
}

/// An authenticated client.
struct Client<E: Engine, S> {
    shared: Arc<Shared<E>>,
    io: BufStream<Transport<S>>,
    out: BackendWriter,
    peer: Option<SocketAddr>,
    user_name: String,
    database_name: String,
    encrypted: bool,
    catalog: Option<Catalog>,
    db: Option<E::Session>,
    access: AccessStore,
    parameters: SessionParameters,
    state: Arc<GatewayState>,
    decisions: HashMap<RestrictionTarget, AccessDecision>,
    temp: TempTablesManager,
    statements: HashMap<String, Statement>,
    portals: HashMap<String, Portal>,
    transaction: TransactionStatus,
    skip_to_sync: bool,
    settings: Vec<(String, String)>,
    startup_settings: Vec<(String, String)>,
    key: CancelKey,
}

impl<E: Engine, S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Client<E, S> {
    async fn serve(mut self, mut stopped: watch::Receiver<bool>) {
        loop {
            let max = self.shared.limits.max_message_bytes;
            let idle = self.shared.limits.idle_timeout;
            let next = async {
                match idle {
                    Some(limit) => tokio::time::timeout(limit, read_message(&mut self.io, max))
                        .await
                        .unwrap_or(Err(ReadError::Idle)),
                    None => read_message(&mut self.io, max).await,
                }
            };
            let next = tokio::select! {
                biased;
                _ = stopped.changed() => None,
                next = next => Some(next),
            };
            let message = match next {
                None => {
                    self.fatal(ErrorFields::fatal(
                        "57P01",
                        "terminating connection due to administrator command",
                    ))
                    .await;
                    break;
                }
                Some(Err(ReadError::Closed)) => break,
                Some(Err(ReadError::Oversized(max))) => {
                    self.fatal(ErrorFields::fatal(
                        "08P01",
                        format!("message exceeds the limit of {max} bytes"),
                    ))
                    .await;
                    break;
                }
                Some(Err(ReadError::Malformed)) => {
                    self.fatal(ErrorFields::fatal("08P01", "invalid message length"))
                        .await;
                    break;
                }
                Some(Err(ReadError::Idle)) => {
                    self.fatal(ErrorFields::fatal(
                        "57P05",
                        "terminating connection due to idle-session timeout",
                    ))
                    .await;
                    break;
                }
                Some(Ok((code, body))) => match decode_frontend(code, &body) {
                    Ok(message) => message,
                    Err(error) => {
                        self.fatal(ErrorFields::fatal("08P01", error.message()))
                            .await;
                        break;
                    }
                },
            };
            match self.dispatch(message).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    self.shared.events.event(GatewayEvent::ConnectionError {
                        peer: self.peer,
                        message: &error.to_string(),
                    });
                    break;
                }
            }
        }
        if let Some(db) = self.db.take() {
            let _ = db.close().await;
        }
    }

    async fn fatal(&mut self, fields: ErrorFields) {
        self.out.error(&fields);
        let _ = self.flush().await;
    }

    async fn flush(&mut self) -> io::Result<()> {
        let bytes = self.out.take();
        self.io.write_all(&bytes).await?;
        self.io.flush().await
    }

    async fn ready(&mut self) -> io::Result<()> {
        self.out.ready_for_query(self.transaction);
        self.flush().await
    }

    fn fail(&mut self, fields: &ErrorFields) {
        self.out.error(fields);
        if self.transaction == TransactionStatus::Block {
            self.transaction = TransactionStatus::Failed;
        }
    }

    /// Handles one message; `Ok(false)` ends the connection.
    async fn dispatch(&mut self, message: FrontendMessage) -> io::Result<bool> {
        if self.skip_to_sync
            && !matches!(message, FrontendMessage::Sync | FrontendMessage::Terminate)
        {
            return Ok(true);
        }
        match message {
            FrontendMessage::Query(text) => {
                self.simple_query(&text).await?;
                // Portals live until the end of the transaction they ran in.
                if self.transaction == TransactionStatus::Idle {
                    self.portals.clear();
                }
                self.ready().await?;
            }
            FrontendMessage::Flush => self.flush().await?,
            FrontendMessage::Sync => {
                self.skip_to_sync = false;
                if self.transaction == TransactionStatus::Idle {
                    self.portals.clear();
                }
                self.ready().await?;
            }
            FrontendMessage::Terminate => return Ok(false),
            FrontendMessage::Password(_) => {
                self.fatal(ErrorFields::fatal("08P01", "unexpected password message"))
                    .await;
                return Ok(false);
            }
            FrontendMessage::Unsupported(code) => {
                self.fatal(ErrorFields::fatal(
                    "08P01",
                    format!("invalid frontend message type {:?}", char::from(code)),
                ))
                .await;
                return Ok(false);
            }
            extended => {
                let result = match extended {
                    FrontendMessage::Parse {
                        name,
                        query,
                        parameter_types,
                    } => self.parse(name, query, parameter_types),
                    FrontendMessage::Bind {
                        portal,
                        statement,
                        parameter_formats,
                        parameters,
                        result_formats,
                    } => self.bind(
                        portal,
                        &statement,
                        &parameter_formats,
                        &parameters,
                        result_formats,
                    ),
                    FrontendMessage::Describe { kind, name } => self.describe(kind, &name),
                    FrontendMessage::Execute { portal, max_rows } => {
                        self.execute_portal(&portal, max_rows).await?
                    }
                    FrontendMessage::Close { kind, name } => {
                        match kind {
                            TargetKind::Statement => self.statements.remove(&name).map(drop),
                            TargetKind::Portal => self.portals.remove(&name).map(drop),
                        };
                        self.out.close_complete();
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(fields) = result {
                    self.fail(&fields);
                    self.skip_to_sync = true;
                }
                if self.out.len() > 64 * 1024 {
                    self.flush().await?;
                }
            }
        }
        Ok(true)
    }

    /// Follows a refreshed gateway state: decisions and temporary tables
    /// were made against the old metadata.
    fn follow_state(&mut self) {
        let current = self.shared.state.current();
        if Arc::ptr_eq(&current, &self.state) {
            return;
        }
        let user = self
            .access
            .current_user()
            .and_then(|user| current.access().user(&user.name).cloned());
        let mut access = current.access().clone();
        access.set_current_user(user);
        self.access = access;
        self.state = current;
        self.catalog = None;
        self.decisions.clear();
        if !self.temp.is_empty() {
            self.temp.clear();
            self.out.notice(&ErrorFields {
                severity: "NOTICE",
                ..ErrorFields::error(
                    "00000",
                    "the metadata was refreshed; temporary tables were cleared",
                )
            });
        }
    }

    async fn simple_query(&mut self, text: &str) -> io::Result<()> {
        self.follow_state();
        if let Some(commands) = chatter::classify(text) {
            for command in commands {
                match self.chatter(&command, &[]).await {
                    Ok(output) => {
                        if command == Chatter::Empty {
                            self.out.empty_query();
                            continue;
                        }
                        if let Some((fields, rows)) = output.rows {
                            self.out.row_description(&fields);
                            for row in rows {
                                self.out.raw(&row);
                            }
                        }
                        self.out.command_complete(&output.tag);
                    }
                    Err(fields) => {
                        self.fail(&fields);
                        break;
                    }
                }
            }
            return Ok(());
        }
        if self.transaction == TransactionStatus::Failed {
            self.fail(&aborted());
            return Ok(());
        }
        if let Some(query) = catalog::sql::route(text) {
            let result = query.and_then(|query| {
                let allocation_bytes = self.shared.limits.catalog_allocation_bytes;
                let model = self.catalog();
                let table = catalog::sql::evaluate_with_allocation_limit(
                    &query,
                    model,
                    &[],
                    &[],
                    allocation_bytes,
                )?;
                catalog::encode(&table, &[])
            });
            match result {
                Ok((fields, rows)) => {
                    self.out.row_description(&fields);
                    let count = rows.len();
                    for row in rows {
                        self.out.raw(&row);
                    }
                    self.out.command_complete(&format!("SELECT {count}"));
                }
                Err(fields) => self.fail(&fields),
            }
            return Ok(());
        }
        match self.simple_sdbl(text).await {
            Ok(()) => Ok(()),
            Err(Failure::Client(fields)) => {
                self.fail(&fields);
                Ok(())
            }
            Err(Failure::Io(error)) => Err(error),
        }
    }

    async fn simple_sdbl(&mut self, text: &str) -> Result<(), Failure> {
        let state = Arc::clone(&self.state);
        let mut manager = self.temp.clone();
        let compiled = self.compile(&state, text, &[], &mut manager)?;
        let kinds = statement_kinds(text);
        for kind in kinds.iter().take(kinds.len().saturating_sub(1)) {
            self.out.command_complete(kind.tag());
        }
        match &compiled.query {
            Some(query) => {
                let plan = ResultPlan::of_query(query, &[]);
                self.out.row_description(&plan.fields);
                let execution = self
                    .execute(&state, &compiled.prepared, query, &plan, 0)
                    .await?;
                self.out
                    .command_complete(&format!("SELECT {}", execution.sent));
                for nested in &query.nested {
                    let plan = ResultPlan::new(&nested.columns, &[], &[]);
                    self.out.row_description(&plan.fields);
                    let execution = self.stream(&nested.sql, &plan, 0).await?;
                    self.out
                        .command_complete(&format!("SELECT {}", execution.sent));
                }
            }
            None => match kinds.last() {
                Some(kind) => self.out.command_complete(kind.tag()),
                None => self.out.empty_query(),
            },
        }
        self.temp = manager;
        Ok(())
    }

    /// Builds the visible model without opening or reading a database session.
    fn catalog(&mut self) -> &mut Catalog {
        if self.catalog.is_none() {
            let state = Arc::clone(&self.state);
            let snapshot = state.snapshot();
            let mut visible = Vec::new();
            let mut build_failures = 0;
            for mut candidate in state.catalog_candidates().iter().cloned() {
                let mut manager = TempTablesManager::new();
                let Ok(prepared) =
                    AnyPrepared::prepare(state.dialect(), snapshot, &candidate.query, &manager)
                else {
                    build_failures += 1;
                    continue;
                };
                let Ok(decisions) = self.decisions(&state, prepared.restriction_request(), true)
                else {
                    continue;
                };
                // A section also needs a proven child Read mapping; object
                // Read alone concerns its owner. Preserve the existing boundary
                // for empty sections while listing readable owner relations.
                let retain_empty = prepared
                    .restriction_request()
                    .targets
                    .iter()
                    .all(|target| target.table_part.is_none());
                let options = CompileOptions::new()
                    .session(&self.parameters)
                    .decisions(&decisions);
                let Ok(Some(query)) = prepared.compile_batch(snapshot, &options, &mut manager)
                else {
                    build_failures += 1;
                    // Object Read was proven above. Missing execution context
                    // must not erase a readable relation from metadata.
                    if retain_empty {
                        visible.push(candidate);
                    }
                    continue;
                };
                for (index, column) in query.columns.iter().enumerate() {
                    if query.service_columns.contains(&index)
                        || query.forbidden_row_column == Some(index)
                    {
                        continue;
                    }
                    let usage = match &column.origin {
                        Some(origin) => FieldUsageRequest {
                            fields: vec![open_sdbl::query::FieldUse {
                                object: origin.object,
                                table_part: origin.table_part.clone(),
                                field: origin.field,
                                usage: open_sdbl::query::FieldUsage::Projection,
                            }],
                        },
                        None => prepared.field_usage().clone(),
                    };
                    if check_user_field_access(&self.access, snapshot, &usage).is_err() {
                        continue;
                    }
                    let comment = column.origin.as_ref().and_then(|origin| {
                        if let open_sdbl::metadata::FieldId::Metadata(id) = origin.field {
                            snapshot
                                .attribute_by_id(id)
                                .ok()
                                .and_then(|f| catalog::synonym(&f.synonyms))
                        } else {
                            None
                        }
                    });
                    let (field, _) = describe_column(&column.label, &column.kind, 0);
                    candidate.columns.push((index as i64 + 1, field, comment));
                }
                // Column authorization is independent of owner visibility.
                if retain_empty || !candidate.columns.is_empty() {
                    visible.push(candidate);
                }
            }
            let mut model = catalog::finish(
                visible,
                &self.user_name,
                &self.database_name,
                state.catalog_revision(),
            );
            model._build_failures = build_failures;
            self.catalog = Some(model);
        }
        let catalog = self.catalog.as_mut().expect("the catalog was built");
        catalog.settings(&self.settings);
        catalog
    }

    /// Prepares, authorizes and compiles one SDBL batch.
    fn compile(
        &mut self,
        state: &GatewayState,
        text: &str,
        values: &[QueryParameter],
        manager: &mut TempTablesManager,
    ) -> Result<Compiled, ErrorFields> {
        let snapshot = state.snapshot();
        let prepared = AnyPrepared::prepare(state.dialect(), snapshot, text, manager)
            .map_err(|diagnostic| diagnostic_error(text, &diagnostic))?;
        let decisions = self.decisions(state, prepared.restriction_request(), true)?;
        check_user_field_access(&self.access, snapshot, prepared.field_usage())
            .map_err(|error| access_denied(format!("permission denied: {error}")))?;
        let plans = prepared
            .presentation_request()
            .targets
            .iter()
            .map(|target| default_presentation_plan(snapshot, target.object))
            .collect::<Vec<_>>();
        let options = CompileOptions::new()
            .presentations(&plans)
            .parameters(values)
            .session(&self.parameters)
            .decisions(&decisions);
        let query = prepared
            .compile_batch(snapshot, &options, manager)
            .map_err(|diagnostic| diagnostic_error(text, &diagnostic))?;
        Ok(Compiled { prepared, query })
    }

    /// The decisions of the user's roles for the targets of a request.
    fn decisions(
        &mut self,
        state: &GatewayState,
        request: &RestrictionRequest,
        refuse_denied: bool,
    ) -> Result<Vec<AccessDecision>, ErrorFields> {
        let missing = request
            .targets
            .iter()
            .filter(|target| !self.decisions.contains_key(*target))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let request = RestrictionRequest { targets: missing };
            let decided = user_decisions(
                &self.access,
                state.snapshot(),
                &request,
                &[],
                &self.parameters,
            )
            .map_err(|error| access_denied(format!("permission denied: {error}")))?;
            for (target, decision) in request.targets.into_iter().zip(decided) {
                self.decisions.insert(target, decision);
            }
        }
        let mut decisions = Vec::with_capacity(request.targets.len());
        for target in &request.targets {
            let decision = self
                .decisions
                .get(target)
                .cloned()
                .ok_or_else(|| access_denied("permission denied: a target has no decision"))?;
            if refuse_denied && decision.is_denied() {
                let name = state
                    .snapshot()
                    .object_by_id(target.object)
                    .and_then(|object| object_query_name(state.snapshot(), object))
                    .unwrap_or_else(|| "a table".to_owned());
                let name = match &target.table_part {
                    Some(section) => format!("{name}.{section}"),
                    None => name,
                };
                return Err(access_denied(format!(
                    "permission denied: user \"{}\" may not read {name}",
                    self.user_name
                )));
            }
            decisions.push(decision);
        }
        Ok(decisions)
    }

    /// A usable database session, reopened when the last one died outside
    /// a transaction block.
    async fn session(&mut self) -> Result<(), ErrorFields> {
        if self.db.as_ref().is_some_and(|db| !db.is_dead()) {
            return Ok(());
        }
        if self.transaction != TransactionStatus::Idle {
            self.db = None;
            return Err(ErrorFields::error(
                "08006",
                "the database session of the transaction block is lost",
            ));
        }
        if let Some(db) = self.db.take() {
            let _ = db.close().await;
        }
        match self.shared.engine.open().await {
            Ok(db) => {
                self.db = Some(db);
                Ok(())
            }
            Err(error) => Err(database_error(&error, self.shared.verbose_errors)),
        }
    }

    /// Runs a compiled query, sending at most `limit` rows (0: all) and
    /// holding back the rest.
    async fn execute(
        &mut self,
        state: &GatewayState,
        prepared: &AnyPrepared,
        query: &CompiledQuery,
        plan: &ResultPlan,
        limit: usize,
    ) -> Result<Execution, Failure> {
        if query.forbidden_row_column.is_none() && query.deferred_presentations.is_empty() {
            return self.stream(&query.sql, plan, limit).await;
        }
        // The forbidden-row check and the presentations need every row
        // before the first is sent.
        let mut rows = self.collect(&query.sql, query.columns.len()).await?;
        if let Some(column) = query.forbidden_row_column {
            for row in &rows {
                match row.get(column) {
                    Some(Cell::Bool(false)) => {}
                    Some(Cell::Number(value)) if value.trim() == "0" => {}
                    Some(Cell::Bool(true) | Cell::Number(_)) => {
                        return Err(Failure::Client(access_denied(
                            "permission denied: the statement reads rows the user may not read",
                        )));
                    }
                    _ => {
                        return Err(Failure::Client(ErrorFields::error(
                            "XX000",
                            "the forbidden-row column is missing or not boolean",
                        )));
                    }
                }
            }
        }
        if !query.deferred_presentations.is_empty() {
            self.present(state, prepared, query, &mut rows).await?;
        }
        let mut held = rows
            .iter()
            .map(|row| plan.encode(row))
            .collect::<VecDeque<_>>();
        let count = if limit == 0 {
            held.len()
        } else {
            limit.min(held.len())
        };
        for frame in held.drain(..count) {
            self.out.raw(&frame);
            if self.out.len() > 64 * 1024 {
                self.flush().await.map_err(Failure::Io)?;
            }
        }
        Ok(Execution { sent: count, held })
    }

    /// Reads a whole result, within the buffer ceiling.
    async fn collect(&mut self, sql: &str, columns: usize) -> Result<Vec<Vec<Cell>>, Failure> {
        self.session().await?;
        let cap = self.shared.portal_buffer_rows;
        let verbose = self.shared.verbose_errors;
        let db = self.db.as_mut().expect("the session is open");
        let cancellation = db.cancellation();
        let mut signal = self.key.signal.clone();
        signal.mark_unchanged();
        let mut rows = Vec::new();
        let mut overflow = false;
        let end = {
            let on_row = |cells: Vec<Cell>| {
                if cells.len() != columns {
                    return Err(DbError::Data(format!(
                        "database returned {} columns, expected {columns}",
                        cells.len()
                    )));
                }
                if rows.len() >= cap {
                    overflow = true;
                    return Ok(RowFlow::Stop);
                }
                rows.push(cells);
                Ok(RowFlow::Continue)
            };
            tokio::select! {
                result = db.query_each(sql, columns, on_row) => QueryEnd::Done(result),
                _ = signal.changed() => QueryEnd::Cancelled,
            }
        };
        match end {
            QueryEnd::Cancelled => {
                let _ = db.cancel(cancellation).await;
                Err(Failure::Client(cancelled()))
            }
            QueryEnd::Done(Err(error)) => Err(Failure::Client(database_error(&error, verbose))),
            QueryEnd::Done(Ok(())) if overflow => Err(Failure::Client(too_many_rows(cap))),
            QueryEnd::Done(Ok(())) => Ok(rows),
        }
    }

    /// Streams a result to the client while it is read, sending at most
    /// `limit` rows (0: all) and holding back the rest.
    async fn stream(
        &mut self,
        sql: &str,
        plan: &ResultPlan,
        limit: usize,
    ) -> Result<Execution, Failure> {
        self.session().await?;
        self.flush().await.map_err(Failure::Io)?;
        let cap = self.shared.portal_buffer_rows;
        let verbose = self.shared.verbose_errors;
        let send_limit = if limit == 0 { usize::MAX } else { limit };
        let columns = plan.column_count;
        let db = self.db.as_mut().expect("the session is open");
        let io = &mut self.io;
        let cancellation = db.cancellation();
        let mut signal = self.key.signal.clone();
        signal.mark_unchanged();
        let written = Arc::new(AtomicUsize::new(0));
        let overflow = Arc::new(AtomicBool::new(false));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let encoder = plan.clone();
        let produced_written = Arc::clone(&written);
        let produced_overflow = Arc::clone(&overflow);
        let mut produced = 0_usize;
        let on_row = move |cells: Vec<Cell>| {
            if cells.len() != columns {
                return Err(DbError::Data(format!(
                    "database returned {} columns, expected {columns}",
                    cells.len()
                )));
            }
            // Rows read but not yet written — held for a later Execute or
            // waiting for a slow client — stay under the ceiling.
            let unwritten = produced - produced_written.load(Ordering::Acquire);
            if unwritten >= cap {
                produced_overflow.store(true, Ordering::Release);
                return Ok(RowFlow::Stop);
            }
            produced += 1;
            let _ = sender.send(encoder.encode(&cells));
            Ok(RowFlow::Continue)
        };
        let query = async {
            tokio::select! {
                result = db.query_each(sql, columns, on_row) => QueryEnd::Done(result),
                _ = signal.changed() => QueryEnd::Cancelled,
            }
        };
        let writer = async {
            let mut sent = 0_usize;
            let mut held = VecDeque::new();
            while let Some(frame) = receiver.recv().await {
                if sent < send_limit {
                    io.write_all(&frame).await?;
                    sent += 1;
                    written.store(sent, Ordering::Release);
                } else {
                    held.push_back(frame);
                }
            }
            Ok::<_, io::Error>((sent, held))
        };
        let (end, written) = tokio::join!(query, writer);
        let (sent, held) = written.map_err(Failure::Io)?;
        match end {
            QueryEnd::Cancelled => {
                let _ = db.cancel(cancellation).await;
                Err(Failure::Client(cancelled()))
            }
            QueryEnd::Done(_) if overflow.load(Ordering::Acquire) => {
                Err(Failure::Client(too_many_rows(cap)))
            }
            QueryEnd::Done(Err(error)) => Err(Failure::Client(database_error(&error, verbose))),
            QueryEnd::Done(Ok(())) => Ok(Execution { sent, held }),
        }
    }

    /// Replaces deferred presentation payloads with presentations, read
    /// under the user's decisions for the referenced objects.
    async fn present(
        &mut self,
        state: &GatewayState,
        prepared: &AnyPrepared,
        query: &CompiledQuery,
        rows: &mut [Vec<Cell>],
    ) -> Result<(), Failure> {
        let snapshot = state.snapshot();
        let mut cells = Vec::new();
        let mut references = BTreeMap::<ObjectId, BTreeSet<[u8; 16]>>::new();
        for (row_index, row) in rows.iter_mut().enumerate() {
            for &column in &query.deferred_presentations {
                let Some(cell) = row.get_mut(column) else {
                    continue;
                };
                if cell.is_null() {
                    continue;
                }
                let payload = cell.as_bytes().unwrap_or_default();
                let (Some(table), Some(reference)) = (
                    payload
                        .get(..4)
                        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok()),
                    payload
                        .get(4..)
                        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok()),
                ) else {
                    *cell = Cell::Text(String::new());
                    continue;
                };
                if reference == [0; 16] {
                    *cell = Cell::Text(String::new());
                    continue;
                }
                let Ok(object) = snapshot.object_id_by_database_type(u32::from_be_bytes(table))
                else {
                    *cell = Cell::Text(String::new());
                    continue;
                };
                references.entry(object).or_default().insert(reference);
                cells.push((row_index, column, object, reference));
            }
        }
        let mut presentations = HashMap::<(ObjectId, [u8; 16]), String>::new();
        for (object, object_references) in references {
            let plan = default_presentation_plan(snapshot, object);
            let request = RestrictionRequest {
                targets: vec![RestrictionTarget {
                    object,
                    table_part: None,
                }],
            };
            // A target the user may not read presents nothing, as a
            // deleted object does.
            let decisions = self.decisions(state, &request, false)?;
            let object_references = object_references.into_iter().collect::<Vec<_>>();
            for chunk in object_references.chunks(512) {
                let lookup = prepared
                    .compile_presentation_lookup(
                        snapshot,
                        &plan,
                        chunk,
                        &CompileOptions::new()
                            .session(&self.parameters)
                            .decisions(&decisions),
                    )
                    .map_err(|diagnostic| {
                        Failure::Client(ErrorFields::error(
                            "XX000",
                            format!("cannot compile a presentation lookup: {diagnostic}"),
                        ))
                    })?;
                self.session().await?;
                let verbose = self.shared.verbose_errors;
                let db = self.db.as_mut().expect("the session is open");
                let found = db
                    .query(&lookup.sql, lookup.columns.len())
                    .await
                    .map_err(|error| Failure::Client(database_error(&error, verbose)))?;
                for row in found {
                    let Some(Ok(reference)) = row
                        .first()
                        .and_then(Cell::as_bytes)
                        .map(<[u8; 16]>::try_from)
                    else {
                        continue;
                    };
                    let text = row
                        .get(1)
                        .and_then(Cell::as_text)
                        .unwrap_or_default()
                        .to_owned();
                    presentations.insert((object, reference), text);
                }
            }
        }
        for (row_index, column, object, reference) in cells {
            rows[row_index][column] = Cell::Text(
                presentations
                    .get(&(object, reference))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        Ok(())
    }

    async fn chatter(
        &mut self,
        command: &Chatter,
        formats: &[i16],
    ) -> Result<ChatterOutput, ErrorFields> {
        let control = matches!(command, Chatter::Commit | Chatter::Rollback);
        if self.transaction == TransactionStatus::Failed && !control && *command != Chatter::Empty {
            return Err(aborted());
        }
        let tag = |tag: &str| ChatterOutput {
            rows: None,
            tag: tag.to_owned(),
        };
        Ok(match command {
            Chatter::Empty => tag(""),
            Chatter::Set { name, value } => {
                if name == "client_encoding"
                    && let Some(value) = value
                    && !chatter::is_utf8(value)
                {
                    return Err(ErrorFields::error(
                        "22023",
                        format!("invalid value for parameter \"client_encoding\": \"{value}\""),
                    ));
                }
                let initial = self
                    .startup_settings
                    .iter()
                    .find(|(known, _)| known.eq_ignore_ascii_case(name))
                    .map(|(known, value)| (known.clone(), value.clone()));
                if let Some((known, initial)) = initial
                    && known != "client_encoding"
                {
                    let value = value.clone().unwrap_or(initial);
                    self.set_setting(&known, value);
                }
                tag("SET")
            }
            Chatter::SetReadWrite => {
                return Err(ErrorFields::error(
                    "25006",
                    "the gateway is read-only: a read-write transaction is not available",
                ));
            }
            Chatter::Reset(name) => {
                let names = match name {
                    Some(name) => vec![name.clone()],
                    None => vec!["application_name".to_owned()],
                };
                for name in names {
                    let initial = self
                        .startup_settings
                        .iter()
                        .find(|(known, _)| known.eq_ignore_ascii_case(&name))
                        .cloned();
                    if let Some((known, value)) = initial {
                        self.set_setting(&known, value);
                    }
                }
                tag("RESET")
            }
            Chatter::Show(name) => {
                let value = match name.as_str() {
                    "search_path" => Some(self.catalog().search_path()),
                    "transaction_isolation" => Some("read committed".to_owned()),
                    "transaction_read_only" => Some("on".to_owned()),
                    _ => self
                        .settings
                        .iter()
                        .find(|(known, _)| known.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.clone()),
                };
                let Some(value) = value else {
                    return Err(ErrorFields::error(
                        "42704",
                        format!("unrecognized configuration parameter \"{name}\""),
                    ));
                };
                ChatterOutput {
                    rows: Some(one_text_row(name, &value)),
                    tag: "SHOW".to_owned(),
                }
            }
            Chatter::Begin {
                read_write,
                tag: name,
            } => {
                if *read_write {
                    return Err(ErrorFields::error(
                        "25006",
                        "the gateway is read-only: a read-write transaction is not available",
                    ));
                }
                if self.transaction == TransactionStatus::Idle {
                    self.session().await?;
                    let verbose = self.shared.verbose_errors;
                    let db = self.db.as_mut().expect("the session is open");
                    db.begin()
                        .await
                        .map_err(|error| database_error(&error, verbose))?;
                    self.transaction = TransactionStatus::Block;
                } else {
                    self.out.notice(&warning(
                        "25001",
                        "there is already a transaction in progress",
                    ));
                }
                tag(name)
            }
            Chatter::Commit | Chatter::Rollback => {
                let committing = *command == Chatter::Commit;
                match self.transaction {
                    TransactionStatus::Idle => {
                        self.out
                            .notice(&warning("25P01", "there is no transaction in progress"));
                        tag(if committing { "COMMIT" } else { "ROLLBACK" })
                    }
                    status => {
                        self.transaction = TransactionStatus::Idle;
                        self.portals.clear();
                        let verbose = self.shared.verbose_errors;
                        let finished = match self.db.as_mut() {
                            Some(db) if !db.is_dead() => {
                                if committing && status == TransactionStatus::Block {
                                    db.commit().await
                                } else if db.transaction_active().await.unwrap_or(false) {
                                    db.rollback().await
                                } else {
                                    Ok(())
                                }
                            }
                            _ => Ok(()),
                        };
                        finished.map_err(|error| database_error(&error, verbose))?;
                        tag(if committing && status == TransactionStatus::Block {
                            "COMMIT"
                        } else {
                            "ROLLBACK"
                        })
                    }
                }
            }
            Chatter::DiscardAll => {
                if self.transaction != TransactionStatus::Idle {
                    return Err(ErrorFields::error(
                        "25001",
                        "DISCARD ALL cannot run inside a transaction block",
                    ));
                }
                self.statements.clear();
                self.portals.clear();
                self.temp.clear();
                let settings = self.startup_settings.clone();
                for (name, value) in settings {
                    self.set_setting(&name, value);
                }
                tag("DISCARD ALL")
            }
            Chatter::Version => ChatterOutput {
                rows: Some(one_text_row(
                    "version",
                    &format!(
                        "PostgreSQL 13.0 (open-sdbl {}): SDBL gateway for 1C bases",
                        env!("CARGO_PKG_VERSION")
                    ),
                )),
                tag: "SELECT 1".to_owned(),
            },
            Chatter::SslStatus | Chatter::StartupTime => {
                let (name, oid, value) = if *command == Chatter::SslStatus {
                    ("ssl", 16, catalog::sql::Value::Bool(self.encrypted))
                } else {
                    (
                        "startup_time",
                        1700,
                        catalog::sql::Value::Int(self.shared.startup_time),
                    )
                };
                let table = catalog::sql::Table {
                    columns: vec![catalog::sql::Column::new(name, oid)],
                    rows: vec![vec![value]],
                };
                ChatterOutput {
                    rows: Some(catalog::encode(&table, formats)?),
                    tag: "SELECT 1".into(),
                }
            }
            Chatter::CurrentSchema => ChatterOutput {
                rows: Some(one_text_row("current_schema", "public")),
                tag: "SELECT 1".to_owned(),
            },
            Chatter::SelectText { value, name } => ChatterOutput {
                rows: Some(one_text_row(name, value)),
                tag: "SELECT 1".to_owned(),
            },
            Chatter::SelectOne => {
                let mut out = BackendWriter::new();
                out.data_row([Some(b"1".as_slice())]);
                ChatterOutput {
                    rows: Some((
                        chatter_fields(&Chatter::SelectOne).unwrap_or_default(),
                        vec![out.take()],
                    )),
                    tag: "SELECT 1".to_owned(),
                }
            }
        })
    }

    fn set_setting(&mut self, name: &str, value: String) {
        if let Some((_, current)) = self.settings.iter_mut().find(|(known, _)| known == name)
            && *current != value
        {
            current.clone_from(&value);
            if name == "application_name" {
                self.out.parameter_status(name, &value);
            }
        }
    }

    fn parse(
        &mut self,
        name: String,
        query: String,
        parameter_types: Vec<u32>,
    ) -> Result<(), ErrorFields> {
        self.follow_state();
        if !name.is_empty() && self.statements.contains_key(&name) {
            return Err(ErrorFields::error(
                "42P05",
                format!("prepared statement \"{name}\" already exists"),
            ));
        }
        let body = match chatter::classify(&query) {
            Some(mut commands) => {
                if commands.len() > 1 {
                    return Err(ErrorFields::error(
                        "42601",
                        "cannot insert multiple commands into a prepared statement",
                    ));
                }
                StatementBody::Chatter(commands.remove(0))
            }
            None => {
                if let Some(catalog) = catalog::sql::route(&query) {
                    StatementBody::Catalog(Box::new(catalog?))
                } else {
                    let state = Arc::clone(&self.state);
                    let prepared =
                        AnyPrepared::prepare(state.dialect(), state.snapshot(), &query, &self.temp)
                            .map_err(|diagnostic| diagnostic_error(&query, &diagnostic))?;
                    StatementBody::Sdbl {
                        parameter_names: prepared.parameter_names().to_vec(),
                    }
                }
            }
        };
        let expected = match &body {
            StatementBody::Chatter(_) => 0,
            StatementBody::Catalog(query) => {
                catalog::sql::describe(query, self.catalog(), &parameter_types)?
                    .1
                    .len()
            }
            StatementBody::Sdbl { parameter_names } => parameter_names.len(),
        };
        if parameter_types.len() > expected {
            return Err(ErrorFields::error(
                "08P01",
                format!(
                    "the statement declares {} parameter types for {expected} parameters",
                    parameter_types.len()
                ),
            ));
        }
        let mut parameter_types = parameter_types;
        parameter_types.resize(expected, oid::UNSPECIFIED);
        if let StatementBody::Catalog(query) = &body {
            parameter_types = catalog::sql::describe(query, self.catalog(), &parameter_types)?.1;
        }
        self.statements.insert(
            name,
            Statement {
                text: query,
                body,
                parameter_types,
            },
        );
        self.out.parse_complete();
        Ok(())
    }

    fn describe(&mut self, kind: TargetKind, name: &str) -> Result<(), ErrorFields> {
        match kind {
            TargetKind::Statement => {
                let (text, body, types) = self
                    .statements
                    .get(name)
                    .map(|statement| {
                        (
                            statement.text.clone(),
                            statement.body.clone(),
                            statement.parameter_types.clone(),
                        )
                    })
                    .ok_or_else(|| {
                        ErrorFields::error(
                            "26000",
                            format!("prepared statement \"{name}\" does not exist"),
                        )
                    })?;
                let described = types
                    .iter()
                    .map(|oid| {
                        if *oid == oid::UNSPECIFIED {
                            oid::TEXT
                        } else {
                            *oid
                        }
                    })
                    .collect::<Vec<_>>();
                let fields = match &body {
                    StatementBody::Chatter(command) => chatter_fields(command),
                    StatementBody::Catalog(query) => Some(
                        catalog::sql::describe(query, self.catalog(), &types)?
                            .0
                            .iter()
                            .map(|c| c.field(0))
                            .collect(),
                    ),
                    StatementBody::Sdbl { parameter_names } => {
                        self.describe_sdbl(&text, parameter_names)
                    }
                };
                self.out.parameter_description(&described);
                match fields {
                    Some(fields) => self.out.row_description(&fields),
                    None => self.out.no_data(),
                }
            }
            TargetKind::Portal => {
                let portal = self.portals.get(name).ok_or_else(|| {
                    ErrorFields::error("34000", format!("portal \"{name}\" does not exist"))
                })?;
                let fields = match &portal.body {
                    PortalBody::Catalog { fields, .. } => Some(fields.clone()),
                    PortalBody::Chatter {
                        command, formats, ..
                    } => chatter_fields(command).map(|mut fields| {
                        for (index, field) in fields.iter_mut().enumerate() {
                            field.format = format_of(formats, index);
                        }
                        fields
                    }),
                    PortalBody::Sdbl(portal) => {
                        portal.plan.as_ref().map(|plan| plan.fields.clone())
                    }
                };
                match fields {
                    Some(fields) => self.out.row_description(&fields),
                    None => self.out.no_data(),
                }
            }
        }
        Ok(())
    }

    /// The columns of a statement compiled with every parameter `NULL`;
    /// `None` when that compilation fails, whose error the client then
    /// meets at `Bind`.
    fn describe_sdbl(&mut self, text: &str, names: &[String]) -> Option<Vec<FieldDescription>> {
        let state = Arc::clone(&self.state);
        let values = names
            .iter()
            .map(|name| QueryParameter::new(name, open_sdbl::query::ParameterValue::Null))
            .collect::<Vec<_>>();
        let mut manager = self.temp.clone();
        let compiled = self.compile(&state, text, &values, &mut manager).ok()?;
        let query = compiled.query?;
        if !query.nested.is_empty() {
            return None;
        }
        Some(ResultPlan::of_query(&query, &[]).fields)
    }

    fn bind(
        &mut self,
        portal: String,
        statement: &str,
        parameter_formats: &[i16],
        parameters: &[Option<Vec<u8>>],
        result_formats: Vec<i16>,
    ) -> Result<(), ErrorFields> {
        self.follow_state();
        if self.transaction == TransactionStatus::Failed {
            return Err(aborted());
        }
        let (text, statement_body, parameter_types) = self
            .statements
            .get(statement)
            .map(|prepared| {
                (
                    prepared.text.clone(),
                    prepared.body.clone(),
                    prepared.parameter_types.clone(),
                )
            })
            .ok_or_else(|| {
                ErrorFields::error(
                    "26000",
                    format!("prepared statement \"{statement}\" does not exist"),
                )
            })?;
        if !portal.is_empty() && self.portals.contains_key(&portal) {
            return Err(ErrorFields::error(
                "42P03",
                format!("cursor \"{portal}\" already exists"),
            ));
        }
        let expected = parameter_types.len();
        if parameters.len() != expected {
            return Err(ErrorFields::error(
                "08P01",
                format!(
                    "bind message supplies {} parameters, but prepared statement \"{statement}\" requires {expected}",
                    parameters.len()
                ),
            ));
        }
        if !(parameter_formats.len() <= 1 || parameter_formats.len() == expected) {
            return Err(ErrorFields::error(
                "08P01",
                "bind message has a wrong number of parameter formats",
            ));
        }
        if result_formats
            .iter()
            .chain(parameter_formats)
            .any(|format| !matches!(format, 0 | 1))
        {
            return Err(ErrorFields::error("22023", "unsupported format code"));
        }
        let body = match &statement_body {
            StatementBody::Catalog(query) => {
                let values = parameters
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        catalog::decode(
                            parameter_types[i],
                            format_of(parameter_formats, i) == 1,
                            value.as_deref(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let allocation_bytes = self.shared.limits.catalog_allocation_bytes;
                let table = catalog::sql::evaluate_with_allocation_limit(
                    query,
                    self.catalog(),
                    &values,
                    &parameter_types,
                    allocation_bytes,
                )?;
                let (fields, rows) = catalog::encode(&table, &result_formats)?;
                PortalBody::Catalog {
                    fields,
                    rows: rows.into(),
                    sent: 0,
                }
            }
            StatementBody::Chatter(command) => PortalBody::Chatter {
                command: command.clone(),
                formats: result_formats,
                done: false,
            },
            StatementBody::Sdbl { parameter_names } => {
                let mut values = Vec::with_capacity(expected);
                for (index, (name, value)) in parameter_names.iter().zip(parameters).enumerate() {
                    let type_oid = parameter_types[index];
                    let binary = format_of(parameter_formats, index) == 1;
                    let value = decode_parameter(type_oid, binary, value.as_deref()).map_err(
                        |error| match error {
                            BindError::UnsupportedType(oid) => ErrorFields::error(
                                "42804",
                                format!(
                                    "parameter ${} (&{name}) has type OID {oid}, which the gateway cannot bind",
                                    index + 1
                                ),
                            ),
                            BindError::Invalid(message) => ErrorFields::error(
                                "22P02",
                                format!("parameter ${} (&{name}): {message}", index + 1),
                            ),
                        },
                    )?;
                    values.push(QueryParameter::new(name, value));
                }
                let state = Arc::clone(&self.state);
                let mut manager = self.temp.clone();
                let compiled = self.compile(&state, &text, &values, &mut manager)?;
                let plan = match &compiled.query {
                    Some(query) => {
                        if !query.nested.is_empty() {
                            return Err(ErrorFields::error(
                                "0A000",
                                "a projected tabular section needs the simple query protocol",
                            ));
                        }
                        let visible = ResultPlan::of_query(query, &[]).fields.len();
                        if result_formats.len() > 1 && result_formats.len() != visible {
                            return Err(ErrorFields::error(
                                "08P01",
                                "bind message has a wrong number of result formats",
                            ));
                        }
                        Some(ResultPlan::of_query(query, &result_formats))
                    }
                    None => None,
                };
                PortalBody::Sdbl(Box::new(SdblPortal {
                    kinds: statement_kinds(&text),
                    prepared: compiled.prepared,
                    query: compiled.query,
                    plan,
                    manager: Some(manager),
                    progress: PortalProgress::Ready,
                }))
            }
        };
        self.portals.insert(portal, Portal { body });
        self.out.bind_complete();
        Ok(())
    }

    async fn execute_portal(
        &mut self,
        name: &str,
        max_rows: u32,
    ) -> io::Result<Result<(), ErrorFields>> {
        if self.transaction == TransactionStatus::Failed {
            return Ok(Err(aborted()));
        }
        let Some(mut portal) = self.portals.remove(name) else {
            return Ok(Err(ErrorFields::error(
                "34000",
                format!("portal \"{name}\" does not exist"),
            )));
        };
        let limit = usize::try_from(max_rows).unwrap_or(usize::MAX);
        let result = match &mut portal.body {
            PortalBody::Catalog { rows, sent, .. } => {
                let count = if limit == 0 {
                    rows.len()
                } else {
                    limit.min(rows.len())
                };
                for frame in rows.drain(..count) {
                    self.out.raw(&frame);
                    if self.out.len() > 64 * 1024 {
                        self.flush().await?;
                    }
                }
                *sent += count;
                if rows.is_empty() {
                    self.out.command_complete(&format!("SELECT {sent}"));
                    *sent = 0;
                } else {
                    self.out.portal_suspended();
                }
                Ok(())
            }
            PortalBody::Chatter {
                command,
                formats,
                done,
            } => {
                if *done {
                    self.out.command_complete("SELECT 0");
                    Ok(())
                } else {
                    *done = true;
                    let command = command.clone();
                    match self.chatter(&command, formats).await {
                        Ok(output) => {
                            if command == Chatter::Empty {
                                self.out.empty_query();
                            } else {
                                if let Some((_, rows)) = output.rows {
                                    let binary_int =
                                        command == Chatter::SelectOne && format_of(formats, 0) == 1;
                                    for row in rows {
                                        if binary_int {
                                            let mut out = BackendWriter::new();
                                            out.data_row([Some(1_i32.to_be_bytes().as_slice())]);
                                            self.out.raw(&out.take());
                                        } else {
                                            self.out.raw(&row);
                                        }
                                    }
                                }
                                self.out.command_complete(&output.tag);
                            }
                            Ok(())
                        }
                        Err(fields) => Err(fields),
                    }
                }
            }
            PortalBody::Sdbl(sdbl) => match self.execute_sdbl(sdbl, limit).await {
                Ok(()) => Ok(()),
                Err(Failure::Client(fields)) => Err(fields),
                Err(Failure::Io(error)) => return Err(error),
            },
        };
        self.portals.insert(name.to_owned(), portal);
        Ok(result)
    }

    async fn execute_sdbl(&mut self, portal: &mut SdblPortal, limit: usize) -> Result<(), Failure> {
        let mut progress = std::mem::replace(&mut portal.progress, PortalProgress::Done);
        if let PortalProgress::Ready = progress {
            let Some(query) = portal.query.as_ref() else {
                let tag = portal.kinds.last().map_or("SELECT 0", |kind| kind.tag());
                self.out.command_complete(tag);
                if let Some(manager) = portal.manager.take() {
                    self.temp = manager;
                }
                return Ok(());
            };
            let plan = portal.plan.clone().expect("a query has a plan");
            let state = Arc::clone(&self.state);
            let execution = self
                .execute(&state, &portal.prepared, query, &plan, limit)
                .await?;
            if let Some(manager) = portal.manager.take() {
                self.temp = manager;
            }
            if execution.held.is_empty() {
                self.out
                    .command_complete(&format!("SELECT {}", execution.sent));
                portal.progress = PortalProgress::Done;
            } else {
                self.out.portal_suspended();
                portal.progress = PortalProgress::Suspended(execution.held);
            }
            return Ok(());
        }
        if let PortalProgress::Suspended(held) = &mut progress {
            let count = if limit == 0 {
                held.len()
            } else {
                limit.min(held.len())
            };
            for frame in held.drain(..count) {
                self.out.raw(&frame);
                if self.out.len() > 64 * 1024 {
                    self.flush().await.map_err(Failure::Io)?;
                }
            }
            if held.is_empty() {
                self.out.command_complete(&format!("SELECT {count}"));
                portal.progress = PortalProgress::Done;
            } else {
                self.out.portal_suspended();
                portal.progress = progress;
            }
            return Ok(());
        }
        self.out.command_complete("SELECT 0");
        Ok(())
    }
}

/// A failure while serving a statement: one the client is told about, or
/// a broken connection.
enum Failure {
    Client(ErrorFields),
    Io(io::Error),
}

impl From<ErrorFields> for Failure {
    fn from(fields: ErrorFields) -> Self {
        Self::Client(fields)
    }
}

fn aborted() -> ErrorFields {
    ErrorFields::error(
        "25P02",
        "current transaction is aborted, commands ignored until end of transaction block",
    )
}

fn warning(code: &'static str, message: &str) -> ErrorFields {
    ErrorFields {
        severity: "WARNING",
        ..ErrorFields::error(code, message)
    }
}

fn too_many_rows(cap: usize) -> ErrorFields {
    ErrorFields::error(
        "54000",
        format!("the result holds back more than {cap} rows the client has not read"),
    )
}

#[cfg(test)]
#[path = "tests/statements.rs"]
mod tests;
