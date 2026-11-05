//! The database side of a connection, behind a sealed trait so the state
//! machine can be tested against an in-memory engine.

use std::future::Future;

use open_sdbl::metadata::{InfoBaseUser, ObjectId};
use open_sdbl::password::{PasswordState, SourceProfile};
use open_sdbl::query::{ParameterValue, QueryParameter, SessionParameters, find_metadata_object};
use open_sdbl_db::{
    Cell, Credentials, DatabaseConnection, DatabaseSession, DbError, Limits, QueryCancellation,
    QueryRows, RowFlow, read_current_user, read_template_parameters,
};
use zeroize::Zeroizing;

use crate::state::GatewayState;

/// What a password is checked against.
pub(crate) enum PasswordEvidence {
    /// The user's decoded record through `UserData::verify_password` and
    /// its confirmed legacy or extended record mapping.
    UserData,
    /// A public stored value with an asserted provenance.
    #[cfg_attr(not(test), allow(dead_code))]
    Stored {
        value: Zeroizing<String>,
        source: SourceProfile,
        state: PasswordState,
    },
}

/// The session parameters derived for a user, and what could not be read.
pub(crate) struct DerivedSession {
    pub(crate) parameters: SessionParameters,
    pub(crate) unread: Vec<String>,
}

/// Opens database sessions and answers what authentication needs.
pub(crate) trait Engine: Send + Sync + 'static {
    type Session: EngineSession;

    fn open(&self) -> impl Future<Output = Result<Self::Session, DbError>> + Send;

    fn password_evidence(&self, user: &InfoBaseUser) -> PasswordEvidence;

    /// The parameters `\as` would learn for `user`; the operator's values
    /// in `operator` win.
    fn derive_session(
        &self,
        session: &mut Self::Session,
        state: &GatewayState,
        user: &InfoBaseUser,
        operator: &SessionParameters,
    ) -> impl Future<Output = DerivedSession> + Send;
}

/// One read-only database session.
pub(crate) trait EngineSession: Send + 'static {
    type Cancel: Send + 'static;

    fn query_each<F>(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: F,
    ) -> impl Future<Output = Result<(), DbError>> + Send
    where
        F: FnMut(Vec<Cell>) -> Result<RowFlow, DbError> + Send;

    fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> impl Future<Output = Result<QueryRows, DbError>> + Send;

    fn begin(&mut self) -> impl Future<Output = Result<(), DbError>> + Send;

    fn commit(&mut self) -> impl Future<Output = Result<(), DbError>> + Send;

    fn rollback(&mut self) -> impl Future<Output = Result<(), DbError>> + Send;

    fn transaction_active(&mut self) -> impl Future<Output = Result<bool, DbError>> + Send;

    fn cancellation(&self) -> Self::Cancel;

    fn cancel(&mut self, cancel: Self::Cancel) -> impl Future<Output = Result<(), DbError>> + Send;

    fn is_dead(&self) -> bool;

    fn close(self) -> impl Future<Output = Result<(), DbError>> + Send;
}

/// The engine of a real base.
pub(crate) struct DbEngine {
    pub(crate) connection: DatabaseConnection,
    pub(crate) credentials: Credentials,
    pub(crate) limits: Limits,
}

impl Engine for DbEngine {
    type Session = DatabaseSession;

    fn open(&self) -> impl Future<Output = Result<Self::Session, DbError>> + Send {
        DatabaseSession::connect(&self.connection, &self.credentials, self.limits)
    }

    fn password_evidence(&self, _user: &InfoBaseUser) -> PasswordEvidence {
        PasswordEvidence::UserData
    }

    async fn derive_session(
        &self,
        session: &mut Self::Session,
        state: &GatewayState,
        user: &InfoBaseUser,
        operator: &SessionParameters,
    ) -> DerivedSession {
        let snapshot = state.snapshot();
        let mut parameters = operator.clone();
        let mut unread = Vec::new();
        // A conditionally-separated common attribute reads its separation
        // from a Config constant. Without the constant's value the compiler
        // refuses every separated table, so the gateway serves the shared
        // (empty) data area by default — correct for a base that is not
        // split into data areas, which is the self-hosted case this gateway
        // targets. The operator keeps the last word: a condition already
        // bound on the operator parameters is left as it is.
        for field in snapshot.separators() {
            if let Some(constant) = field
                .separation
                .as_ref()
                .and_then(|separation| separation.conditional_separation.clone())
                && parameters.separation_condition(&constant).is_none()
            {
                parameters.set_separation_condition(constant, false);
            }
        }
        let set_if_absent = |parameters: &mut SessionParameters, name: &str, value| {
            if parameters.get(name).is_none() {
                parameters.set(QueryParameter::new(name, value));
            }
        };
        match read_template_parameters(session, snapshot, &parameters).await {
            Ok(read) => {
                if let Some(reason) = read.unread {
                    unread.push(format!("ПараметрыОграниченияДоступа: {reason}"));
                }
                for (name, value) in read.values {
                    set_if_absent(&mut parameters, &name, ParameterValue::String(value));
                }
            }
            Err(error) => unread.push(format!("ПараметрыОграниченияДоступа: {error}")),
        }
        match read_current_user(session, snapshot, &parameters, &user.data.id).await {
            Ok(Some(current)) => set_if_absent(&mut parameters, "ТекущийПользователь", current),
            Ok(None) => {}
            Err(error) => unread.push(format!("ТекущийПользователь: {error}")),
        }
        // A user of the base is not an external one: the templates compare
        // the parameter with an empty reference.
        if let Ok(object) = find_metadata_object(snapshot, "Справочник.ВнешниеПользователи")
        {
            set_if_absent(
                &mut parameters,
                "ТекущийВнешнийПользователь",
                ParameterValue::Reference {
                    object: ObjectId::from(&object.guid),
                    id: [0; 16],
                },
            );
        }
        DerivedSession { parameters, unread }
    }
}

impl EngineSession for DatabaseSession {
    type Cancel = QueryCancellation;

    fn query_each<F>(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: F,
    ) -> impl Future<Output = Result<(), DbError>> + Send
    where
        F: FnMut(Vec<Cell>) -> Result<RowFlow, DbError> + Send,
    {
        Self::query_each(self, sql, column_count, on_row)
    }

    fn query(
        &mut self,
        sql: &str,
        column_count: usize,
    ) -> impl Future<Output = Result<QueryRows, DbError>> + Send {
        Self::query(self, sql, column_count)
    }

    fn begin(&mut self) -> impl Future<Output = Result<(), DbError>> + Send {
        self.begin_read_only_transaction()
    }

    fn commit(&mut self) -> impl Future<Output = Result<(), DbError>> + Send {
        self.commit_transaction()
    }

    fn rollback(&mut self) -> impl Future<Output = Result<(), DbError>> + Send {
        self.rollback_transaction()
    }

    fn transaction_active(&mut self) -> impl Future<Output = Result<bool, DbError>> + Send {
        Self::transaction_active(self)
    }

    fn cancellation(&self) -> Self::Cancel {
        Self::cancellation(self)
    }

    fn cancel(&mut self, cancel: Self::Cancel) -> impl Future<Output = Result<(), DbError>> + Send {
        self.cancel_query(cancel)
    }

    fn is_dead(&self) -> bool {
        Self::is_dead(self)
    }

    fn close(self) -> impl Future<Output = Result<(), DbError>> + Send {
        Self::close(self)
    }
}
