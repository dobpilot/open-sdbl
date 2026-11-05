//! The gateway against an in-memory engine: raw protocol exchanges and a
//! real `tokio-postgres` client.

#[path = "tests/fixture.rs"]
mod fixture;

#[path = "../../../tests/support/mod.rs"]
pub(crate) mod support;

#[path = "../../../tests/support/section_reference.rs"]
pub(crate) mod section_reference;

#[path = "tests/protocol.rs"]
mod protocol;

#[path = "tests/client.rs"]
mod client;

#[path = "tests/catalog.rs"]
mod catalog;

#[path = "tests/corpus.rs"]
mod corpus;

#[path = "tests/psql.rs"]
mod psql;

#[path = "tests/password_record.rs"]
mod password_record;

#[path = "tests/jdbc.rs"]
mod jdbc;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use open_sdbl::metadata::InfoBaseUser;
use open_sdbl::password::{PasswordState, SourceProfile};
use open_sdbl::query::{ParameterValue, QueryParameter, SessionParameters};
use open_sdbl_db::{AccessStore, Cell, DatabaseDialect, DbError, QueryRows, RowFlow};
use tokio::io::DuplexStream;
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::cancel::CancelRegistry;
use crate::engine::{DerivedSession, Engine, EngineSession, PasswordEvidence};
use crate::state::{GatewayState, StateHandle};
use crate::{GatewayEvent, GatewayEvents, GatewayLimits, Shared};

/// The SHA-1 stored value of `PaSs123!` from the supplied demo.
pub(crate) const DEMO: &str = "IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=";
pub(crate) const USER: &str = "Абдулов (директор)";
pub(crate) const PASSWORD: &str = "PaSs123!";

type Responder = dyn Fn(&str, usize) -> Result<QueryRows, DbError> + Send + Sync;

/// What the fake database saw.
#[derive(Default)]
pub(crate) struct Journal {
    pub(crate) dead: bool,
    pub(crate) statements: Vec<String>,
    pub(crate) events: Vec<&'static str>,
}

pub(crate) struct FakeEngine {
    pub(crate) journal: Arc<Mutex<Journal>>,
    pub(crate) responder: Arc<Responder>,
    pub(crate) passwords: HashMap<String, (String, SourceProfile, PasswordState)>,
    pub(crate) delay: Option<Duration>,
    pub(crate) derived: Vec<(String, ParameterValue)>,
}

pub(crate) struct FakeSession {
    journal: Arc<Mutex<Journal>>,
    responder: Arc<Responder>,
    delay: Option<Duration>,
    in_transaction: bool,
    dead: bool,
}

impl Engine for FakeEngine {
    type Session = FakeSession;

    async fn open(&self) -> Result<Self::Session, DbError> {
        self.journal.lock().unwrap().events.push("open");
        Ok(FakeSession {
            journal: Arc::clone(&self.journal),
            responder: Arc::clone(&self.responder),
            delay: self.delay,
            in_transaction: false,
            dead: false,
        })
    }

    fn password_evidence(&self, user: &InfoBaseUser) -> PasswordEvidence {
        match self.passwords.get(&user.name.to_lowercase()) {
            Some((value, source, state)) => PasswordEvidence::Stored {
                value: Zeroizing::new(value.clone()),
                source: *source,
                state: *state,
            },
            None => PasswordEvidence::UserData,
        }
    }

    async fn derive_session(
        &self,
        _session: &mut Self::Session,
        _state: &GatewayState,
        _user: &InfoBaseUser,
        operator: &SessionParameters,
    ) -> DerivedSession {
        let mut parameters = operator.clone();
        for (name, value) in &self.derived {
            if parameters.get(name).is_none() {
                parameters.set(QueryParameter::new(name, value.clone()));
            }
        }
        DerivedSession {
            parameters,
            unread: vec!["ПараметрыОграниченияДоступа: absent in the fake".to_owned()],
        }
    }
}

impl EngineSession for FakeSession {
    type Cancel = ();

    async fn query_each<F>(
        &mut self,
        sql: &str,
        column_count: usize,
        mut on_row: F,
    ) -> Result<(), DbError>
    where
        F: FnMut(Vec<Cell>) -> Result<RowFlow, DbError> + Send,
    {
        self.journal.lock().unwrap().statements.push(sql.to_owned());
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        let rows = (self.responder)(sql, column_count)?;
        for row in rows {
            // Yield now and then, as a real driver does between packets.
            tokio::task::yield_now().await;
            if on_row(row)? == RowFlow::Stop {
                break;
            }
        }
        Ok(())
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        self.journal.lock().unwrap().statements.push(sql.to_owned());
        (self.responder)(sql, column_count)
    }

    async fn begin(&mut self) -> Result<(), DbError> {
        self.journal.lock().unwrap().events.push("begin");
        self.in_transaction = true;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DbError> {
        self.journal.lock().unwrap().events.push("commit");
        self.in_transaction = false;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DbError> {
        self.journal.lock().unwrap().events.push("rollback");
        self.in_transaction = false;
        Ok(())
    }

    async fn transaction_active(&mut self) -> Result<bool, DbError> {
        Ok(self.in_transaction)
    }

    fn cancellation(&self) -> Self::Cancel {}

    async fn cancel(&mut self, (): Self::Cancel) -> Result<(), DbError> {
        self.journal.lock().unwrap().events.push("cancel");
        Ok(())
    }

    fn is_dead(&self) -> bool {
        self.dead || self.journal.lock().unwrap().dead
    }

    async fn close(self) -> Result<(), DbError> {
        Ok(())
    }
}

/// Events, as text.
#[derive(Default)]
pub(crate) struct Recorder {
    pub(crate) lines: Mutex<Vec<String>>,
}

impl GatewayEvents for Recorder {
    fn event(&self, event: GatewayEvent<'_>) {
        self.lines.lock().unwrap().push(format!("{event:?}"));
    }
}

/// Rows the fake answers for the register: `count` rows of
/// (Категория, Количество) wherever the statement reads the register.
pub(crate) fn register_rows(count: usize) -> Arc<Responder> {
    Arc::new(move |sql: &str, columns: usize| {
        if !sql.contains("_accumrg1") {
            return Ok(Vec::new());
        }
        Ok((0..count)
            .map(|index| {
                (0..columns)
                    .map(|column| match column % 2 {
                        0 => Cell::Text(format!("К{index}")),
                        _ => Cell::Number(format!("{index}.50")),
                    })
                    .collect()
            })
            .collect())
    })
}

#[derive(Clone)]
pub(crate) struct Harness {
    pub(crate) shared: Arc<Shared<FakeEngine>>,
    pub(crate) journal: Arc<Mutex<Journal>>,
    pub(crate) events: Arc<Recorder>,
    pub(crate) state: Arc<StateHandle>,
    stop: watch::Sender<bool>,
}

pub(crate) struct Options {
    pub(crate) snapshot: Option<open_sdbl::metadata::MetadataSnapshot>,
    pub(crate) read: bool,
    pub(crate) responder: Arc<Responder>,
    pub(crate) source: SourceProfile,
    pub(crate) state: PasswordState,
    pub(crate) stored: &'static str,
    pub(crate) tls: Option<Arc<rustls::ServerConfig>>,
    pub(crate) insecure_loopback: bool,
    pub(crate) insecure_network: bool,
    pub(crate) debug_authentication: bool,
    pub(crate) debug_authentication_secrets: bool,
    pub(crate) limits: GatewayLimits,
    pub(crate) portal_buffer_rows: usize,
    pub(crate) delay: Option<Duration>,
    pub(crate) operator: SessionParameters,
    pub(crate) access: Option<AccessStore>,
    pub(crate) dialect: DatabaseDialect,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            snapshot: None,
            read: true,
            responder: register_rows(3),
            source: SourceProfile::PublicStoredValue8327,
            state: PasswordState::Unknown,
            stored: DEMO,
            tls: None,
            insecure_loopback: true,
            insecure_network: false,
            debug_authentication: false,
            debug_authentication_secrets: false,
            limits: GatewayLimits {
                authentication_failure_delay: Duration::from_millis(20),
                ..GatewayLimits::default()
            },
            portal_buffer_rows: 1000,
            delay: None,
            operator: SessionParameters::new(),
            access: None,
            dialect: DatabaseDialect::Postgres,
        }
    }
}

impl Harness {
    pub(crate) fn new(options: Options) -> Self {
        let snapshot = options.snapshot.unwrap_or_else(fixture::snapshot);
        let access = options
            .access
            .unwrap_or_else(|| fixture::store(&snapshot, options.read));
        let state = Arc::new(StateHandle::new(GatewayState::from_parts(
            snapshot,
            access,
            options.dialect,
        )));
        let journal = Arc::new(Mutex::new(Journal::default()));
        let events = Arc::new(Recorder::default());
        let mut passwords = HashMap::new();
        passwords.insert(
            USER.to_lowercase(),
            (options.stored.to_owned(), options.source, options.state),
        );
        passwords.insert(
            "edievents".to_owned(),
            (
                DEMO.to_owned(),
                SourceProfile::SyntheticPublicStoredValueLegacy,
                PasswordState::Unknown,
            ),
        );
        let shared = Arc::new(Shared {
            database_name: "unf".into(),
            startup_time: 1_700_000_000,
            engine: FakeEngine {
                journal: Arc::clone(&journal),
                responder: options.responder,
                passwords,
                delay: options.delay,
                derived: vec![(
                    "ТекущийПользователь".to_owned(),
                    ParameterValue::String("derived".to_owned()),
                )],
            },
            state: Arc::clone(&state),
            tls: options.tls.map(tokio_rustls::TlsAcceptor::from),
            session_parameters: options.operator,
            allow_insecure_loopback: options.insecure_loopback,
            allow_insecure_network: options.insecure_network,
            verbose_errors: false,
            debug_authentication: options.debug_authentication,
            debug_authentication_secrets: options.debug_authentication_secrets,
            limits: options.limits,
            portal_buffer_rows: options.portal_buffer_rows,
            registry: Arc::new(CancelRegistry::default()),
            events: Arc::clone(&events) as Arc<dyn GatewayEvents>,
        });
        let (stop, _) = watch::channel(false);
        Self {
            shared,
            journal,
            events,
            state,
            stop,
        }
    }

    /// A connection from `peer`, served on its own task.
    pub(crate) fn connect_from(&self, peer: SocketAddr) -> DuplexStream {
        let (client, server) = tokio::io::duplex(1 << 16);
        let shared = Arc::clone(&self.shared);
        let stopped = self.stop.subscribe();
        tokio::spawn(crate::connection::run(shared, server, Some(peer), stopped));
        client
    }

    pub(crate) fn connect(&self) -> DuplexStream {
        self.connect_from("127.0.0.1:40000".parse().unwrap())
    }

    pub(crate) fn stop(&self) {
        let _ = self.stop.send(true);
    }

    pub(crate) fn statements(&self) -> Vec<String> {
        self.journal.lock().unwrap().statements.clone()
    }

    pub(crate) fn db_events(&self) -> Vec<&'static str> {
        self.journal.lock().unwrap().events.clone()
    }

    pub(crate) fn event_lines(&self) -> Vec<String> {
        self.events.lines.lock().unwrap().clone()
    }
}
