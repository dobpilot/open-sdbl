//! One database session driven from synchronous code.
//!
//! [`BlockingDatabaseSession`] owns a single-threaded runtime and one
//! [`DatabaseSession`], and answers each operation by driving the
//! asynchronous operation of the same name to completion. It adds no
//! statement and no decoding of its own: limits, timeouts, diagnostics
//! and the rule that a broken SQL Server session is not reused are the
//! asynchronous session's.

use std::fmt;

use open_sdbl::metadata::{Guid, MetadataSnapshot, ResolutionReport};

use crate::access::{AccessStore, ensure_rights, ensure_users};
use tokio::runtime::{Builder, Handle, Runtime};

use crate::cells::{Cell, QueryRows, RowFlow};
use crate::connection::DatabaseConnection;
use crate::credentials::Credentials;
use crate::error::DbError;
use crate::limits::Limits;
use crate::pipeline::{AcquiredConfiguration, ConfigSelector};
use crate::progress::MetadataProgress;
use crate::session::{DatabaseDialect, DatabaseSession};

#[cfg(test)]
#[path = "tests/blocking.rs"]
mod tests;

/// The operations of one session, as the blocking driver drives them.
///
/// [`DatabaseSession`] is the one implementation outside the tests, and it
/// implements each method by calling its own method of the same name. The
/// trait lets the driver be exercised over an artificial provider; it is
/// not a way to add one.
// The trait is private to the crate and awaited on the driver's own
// runtime; the auto-trait bounds `async fn` cannot express are not needed.
#[allow(async_fn_in_trait)]
pub(crate) trait SessionOperations: Sized {
    /// What a session is opened against.
    type Target: ?Sized;

    async fn connect(
        target: &Self::Target,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError>;

    async fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError>;

    async fn ensure_users(&mut self, store: &mut AccessStore) -> Result<(), DbError>;

    async fn ensure_rights(
        &mut self,
        store: &mut AccessStore,
        roles: &[Guid],
    ) -> Result<Vec<Guid>, DbError>;

    async fn configuration_metadata_inputs(
        &mut self,
    ) -> Result<crate::ConfigurationMetadataInputs, DbError> {
        Err(DbError::Data(
            "test session does not support physical discovery".into(),
        ))
    }

    async fn configuration_resource_names(&mut self) -> Result<Vec<String>, DbError> {
        Err(DbError::Data(
            "test session does not support physical discovery".into(),
        ))
    }

    async fn configuration_extension_metadata(
        &mut self,
    ) -> Result<Vec<open_sdbl::metadata::ExtensionMetadata>, DbError> {
        Err(DbError::Data(
            "test session does not support physical discovery".into(),
        ))
    }

    async fn configuration_bootstrap(
        &mut self,
        _selector: &mut dyn ConfigSelector,
    ) -> Result<crate::ConfigurationBootstrap, DbError> {
        Err(DbError::Data(
            "test session does not support indexed reads".into(),
        ))
    }

    async fn configuration_resources(
        &mut self,
        _names: &[&str],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        Err(DbError::Data(
            "test session does not support indexed reads".into(),
        ))
    }

    async fn extension_resources(
        &mut self,
        _keys: &[open_sdbl::metadata::ContentKey],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        Err(DbError::Data(
            "test session does not support indexed reads".into(),
        ))
    }

    async fn configuration_selected(
        &mut self,
        progress: &mut dyn MetadataProgress,
        selector: &mut dyn ConfigSelector,
    ) -> Result<AcquiredConfiguration, DbError>;

    async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError>;

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError>;

    async fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError>;

    async fn close(self) -> Result<(), DbError>;
}

impl SessionOperations for DatabaseSession {
    type Target = DatabaseConnection;

    async fn connect(
        target: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        Self::connect(target, credentials, limits).await
    }

    async fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        Self::configuration(self, progress).await
    }

    async fn ensure_users(&mut self, store: &mut AccessStore) -> Result<(), DbError> {
        ensure_users(store, self).await
    }

    async fn ensure_rights(
        &mut self,
        store: &mut AccessStore,
        roles: &[Guid],
    ) -> Result<Vec<Guid>, DbError> {
        ensure_rights(store, self, roles).await
    }

    async fn configuration_metadata_inputs(
        &mut self,
    ) -> Result<crate::ConfigurationMetadataInputs, DbError> {
        Self::configuration_metadata_inputs(self).await
    }

    async fn configuration_resource_names(&mut self) -> Result<Vec<String>, DbError> {
        Self::configuration_resource_names(self).await
    }

    async fn configuration_extension_metadata(
        &mut self,
    ) -> Result<Vec<open_sdbl::metadata::ExtensionMetadata>, DbError> {
        Self::configuration_extension_metadata(self).await
    }

    async fn configuration_bootstrap(
        &mut self,
        selector: &mut dyn ConfigSelector,
    ) -> Result<crate::ConfigurationBootstrap, DbError> {
        Self::configuration_bootstrap(self, selector).await
    }

    async fn configuration_resources(
        &mut self,
        names: &[&str],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        Self::configuration_resources(self, names).await
    }

    async fn extension_resources(
        &mut self,
        keys: &[open_sdbl::metadata::ContentKey],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        Self::extension_resources(self, keys).await
    }

    async fn configuration_selected(
        &mut self,
        progress: &mut dyn MetadataProgress,
        selector: &mut dyn ConfigSelector,
    ) -> Result<AcquiredConfiguration, DbError> {
        Self::configuration_selected(self, progress, selector).await
    }

    async fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        Self::metadata(self, progress).await
    }

    async fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        Self::query(self, sql, column_count).await
    }

    async fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        Self::query_each(self, sql, column_count, on_row).await
    }

    async fn close(self) -> Result<(), DbError> {
        Self::close(self).await
    }
}

/// A runtime of its own and the session it drives, until it is closed.
pub(crate) struct BlockingDriver<S> {
    /// Taken only when the driver is dropped.
    runtime: Option<Runtime>,
    /// `None` once the session was closed.
    session: Option<S>,
}

impl<S: SessionOperations> BlockingDriver<S> {
    /// Starts a runtime and opens a session on it.
    pub(crate) fn connect(
        target: &S::Target,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        refuse_async_context()?;
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| DbError::Io("starting the session runtime".to_owned(), error))?;
        let session = runtime.block_on(S::connect(target, credentials, limits))?;
        Ok(Self {
            runtime: Some(runtime),
            session: Some(session),
        })
    }

    /// The session, when it is still open.
    pub(crate) const fn session(&self) -> Option<&S> {
        self.session.as_ref()
    }

    /// The runtime and the open session a call is driven on, or the error
    /// saying why there is none to drive it on.
    ///
    /// A closed session answers [`DbError::SessionClosed`] wherever it is
    /// called from: there is nothing left to block on, so the thread it is
    /// called on does not matter. Only an open session refuses an
    /// asynchronous context.
    fn open(&mut self) -> Result<(&Runtime, &mut S), DbError> {
        let (Some(runtime), Some(session)) = (&self.runtime, &mut self.session) else {
            return Err(DbError::SessionClosed);
        };
        refuse_async_context()?;
        Ok((runtime, session))
    }

    pub(crate) fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration(progress))
    }

    pub(crate) fn ensure_users(&mut self, store: &mut AccessStore) -> Result<(), DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.ensure_users(store))
    }

    pub(crate) fn ensure_rights(
        &mut self,
        store: &mut AccessStore,
        roles: &[Guid],
    ) -> Result<Vec<Guid>, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.ensure_rights(store, roles))
    }

    pub(crate) fn configuration_metadata_inputs(
        &mut self,
    ) -> Result<crate::ConfigurationMetadataInputs, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_metadata_inputs())
    }

    pub(crate) fn configuration_resource_names(&mut self) -> Result<Vec<String>, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_resource_names())
    }

    pub(crate) fn configuration_extension_metadata(
        &mut self,
    ) -> Result<Vec<open_sdbl::metadata::ExtensionMetadata>, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_extension_metadata())
    }

    pub(crate) fn configuration_bootstrap(
        &mut self,
        selector: &mut dyn ConfigSelector,
    ) -> Result<crate::ConfigurationBootstrap, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_bootstrap(selector))
    }

    pub(crate) fn configuration_resources(
        &mut self,
        names: &[&str],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_resources(names))
    }

    pub(crate) fn extension_resources(
        &mut self,
        keys: &[open_sdbl::metadata::ContentKey],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.extension_resources(keys))
    }

    pub(crate) fn configuration_selected(
        &mut self,
        progress: &mut dyn MetadataProgress,
        selector: &mut dyn ConfigSelector,
    ) -> Result<AcquiredConfiguration, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.configuration_selected(progress, selector))
    }

    pub(crate) fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.metadata(progress))
    }

    pub(crate) fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.query(sql, column_count))
    }

    pub(crate) fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        let (runtime, session) = self.open()?;
        runtime.block_on(session.query_each(sql, column_count, on_row))
    }

    /// Closes the session. It is gone afterwards whatever the close
    /// answered, so every later call answers [`DbError::SessionClosed`].
    pub(crate) fn close(&mut self) -> Result<(), DbError> {
        self.open()?;
        let (Some(runtime), Some(session)) = (&self.runtime, self.session.take()) else {
            return Err(DbError::SessionClosed);
        };
        runtime.block_on(session.close())
    }
}

impl<S> Drop for BlockingDriver<S> {
    /// Drops an unclosed session inside its runtime, then the runtime.
    ///
    /// Dropping a runtime blocks until its worker threads stop, which a
    /// thread driving another runtime must not do; there the runtime is
    /// shut down in the background instead of panicking.
    fn drop(&mut self) {
        let Some(runtime) = self.runtime.take() else {
            return;
        };
        {
            let _context = runtime.enter();
            drop(self.session.take());
        }
        if Handle::try_current().is_ok() {
            runtime.shutdown_background();
        }
    }
}

/// Refuses to block a thread that is driving an asynchronous runtime.
fn refuse_async_context() -> Result<(), DbError> {
    if Handle::try_current().is_ok() {
        Err(DbError::AsyncContext)
    } else {
        Ok(())
    }
}

/// One session over a database provider, driven from synchronous code.
///
/// The value owns a single-threaded runtime and one [`DatabaseSession`]
/// on it, so it is one connection: two values are two connections that
/// share nothing, and closing one leaves the other usable. Each method
/// blocks the calling thread until the asynchronous method of the same
/// name completes, under the [`Limits`] the session was opened with.
///
/// After [`BlockingDatabaseSession::close`] every method answers
/// [`DbError::SessionClosed`]. A method called on a thread that is
/// already driving an asynchronous runtime answers
/// [`DbError::AsyncContext`]; such a caller uses [`DatabaseSession`]
/// directly.
///
/// ```no_run
/// # fn example(connection: &open_sdbl_db::DatabaseConnection, sql: &str)
/// # -> Result<(), open_sdbl_db::DbError> {
/// use open_sdbl_db::{BlockingDatabaseSession, Credentials, Limits, NoProgress};
///
/// let credentials = Credentials::new(Some("password".to_owned()), None, None);
/// let mut session =
///     BlockingDatabaseSession::connect(connection, &credentials, Limits::default())?;
/// let configuration = session.configuration(&mut NoProgress)?;
/// let rows = session.query(sql, 1)?;
/// session.close()?;
/// # let _ = (configuration, rows);
/// # Ok(())
/// # }
/// ```
pub struct BlockingDatabaseSession {
    driver: BlockingDriver<DatabaseSession>,
}

impl BlockingDatabaseSession {
    /// Opens a session against the described base under `limits`.
    ///
    /// # Errors
    ///
    /// Answers what [`DatabaseSession::connect`] answers, an I/O error
    /// when the runtime cannot be started, or [`DbError::AsyncContext`].
    pub fn connect(
        connection: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
    ) -> Result<Self, DbError> {
        BlockingDriver::connect(connection, credentials, limits).map(|driver| Self { driver })
    }

    /// Выполняет [`DatabaseSession::begin_read_only_transaction`] на собственном соединении и runtime.
    ///
    /// # Errors
    /// Возвращает ошибку асинхронной операции, [`DbError::SessionClosed`] или [`DbError::AsyncContext`].
    pub fn begin_read_only_transaction(&mut self) -> Result<(), DbError> {
        let (runtime, session) = self.driver.open()?;
        runtime.block_on(session.begin_read_only_transaction())
    }

    /// Выполняет [`DatabaseSession::commit_transaction`] на собственном соединении и runtime.
    ///
    /// # Errors
    /// Возвращает ошибку асинхронной операции, [`DbError::SessionClosed`] или [`DbError::AsyncContext`].
    pub fn commit_transaction(&mut self) -> Result<(), DbError> {
        let (runtime, session) = self.driver.open()?;
        runtime.block_on(session.commit_transaction())
    }

    /// Выполняет [`DatabaseSession::rollback_transaction`] на собственном соединении и runtime.
    ///
    /// # Errors
    /// Возвращает ошибку асинхронной операции, [`DbError::SessionClosed`] или [`DbError::AsyncContext`].
    pub fn rollback_transaction(&mut self) -> Result<(), DbError> {
        let (runtime, session) = self.driver.open()?;
        runtime.block_on(session.rollback_transaction())
    }

    /// Выполняет [`DatabaseSession::transaction_active`] на собственном соединении и runtime.
    ///
    /// # Errors
    /// Возвращает ошибку асинхронной операции, [`DbError::SessionClosed`] или [`DbError::AsyncContext`].
    pub fn transaction_active(&mut self) -> Result<bool, DbError> {
        let (runtime, session) = self.driver.open()?;
        runtime.block_on(session.transaction_active())
    }

    /// Reads the whole configuration of the base, as
    /// [`DatabaseSession::configuration`] does.
    ///
    /// # Errors
    ///
    /// Answers what the asynchronous operation answers,
    /// [`DbError::SessionClosed`] or [`DbError::AsyncContext`].
    pub fn configuration(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        self.driver.configuration(progress)
    }

    /// Reads the users into `store` through the owned database session.
    ///
    /// # Errors
    ///
    /// Answers what [`ensure_users`] answers, [`DbError::SessionClosed`],
    /// or [`DbError::AsyncContext`].
    pub fn ensure_users(&mut self, store: &mut AccessStore) -> Result<(), DbError> {
        self.driver.ensure_users(store)
    }

    /// Reads missing role rights into `store` through the owned session.
    ///
    /// # Errors
    ///
    /// Answers what [`ensure_rights`] answers, [`DbError::SessionClosed`],
    /// or [`DbError::AsyncContext`].
    pub fn ensure_rights(
        &mut self,
        store: &mut AccessStore,
        roles: &[Guid],
    ) -> Result<Vec<Guid>, DbError> {
        self.driver.ensure_rights(store, roles)
    }

    /// Reads selected Config resources through the owned asynchronous session.
    ///
    /// # Errors
    ///
    /// Синхронно выполняет [`DatabaseSession::configuration_metadata_inputs`] на принадлежащем соединении.
    ///
    /// # Errors
    /// Возвращает `SessionClosed` после закрытия, `AsyncContext` внутри runtime
    /// либо ошибку соответствующего асинхронного обнаружения.
    pub fn configuration_metadata_inputs(
        &mut self,
    ) -> Result<crate::ConfigurationMetadataInputs, DbError> {
        self.driver.configuration_metadata_inputs()
    }

    /// Синхронно выполняет [`DatabaseSession::configuration_resource_names`] на принадлежащем соединении.
    ///
    /// # Errors
    /// Возвращает `SessionClosed` после закрытия, `AsyncContext` внутри runtime
    /// либо ошибку соответствующего асинхронного обнаружения.
    pub fn configuration_resource_names(&mut self) -> Result<Vec<String>, DbError> {
        self.driver.configuration_resource_names()
    }

    /// Читает физические структуры расширений без `Config` и `ConfigCAS`.
    /// Результат ограничен существующими бюджетами хранения и распаковки.
    ///
    /// # Errors
    /// Возвращает ошибку сеанса, запроса, лимита или декодирования структуры.
    pub fn configuration_extension_metadata(
        &mut self,
    ) -> Result<Vec<open_sdbl::metadata::ExtensionMetadata>, DbError> {
        self.driver.configuration_extension_metadata()
    }

    /// Синхронно выполняет [`DatabaseSession::configuration_bootstrap`] на принадлежащем соединении.
    ///
    /// # Errors
    /// Возвращает `SessionClosed` после закрытия, `AsyncContext` внутри runtime
    /// или ошибку соответствующего асинхронного чтения.
    pub fn configuration_bootstrap(
        &mut self,
        selector: &mut dyn ConfigSelector,
    ) -> Result<crate::ConfigurationBootstrap, DbError> {
        self.driver.configuration_bootstrap(selector)
    }

    /// Синхронно выполняет [`DatabaseSession::configuration_resources`] на принадлежащем соединении.
    ///
    /// # Errors
    /// Возвращает `SessionClosed` после закрытия, `AsyncContext` внутри runtime
    /// или ошибку соответствующего асинхронного чтения.
    pub fn configuration_resources(
        &mut self,
        names: &[&str],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        self.driver.configuration_resources(names)
    }

    /// Синхронно выполняет [`DatabaseSession::extension_resources`] на принадлежащем соединении.
    ///
    /// # Errors
    /// Возвращает `SessionClosed` после закрытия, `AsyncContext` внутри runtime
    /// или ошибку соответствующего асинхронного чтения.
    pub fn extension_resources(
        &mut self,
        keys: &[open_sdbl::metadata::ContentKey],
    ) -> Result<Vec<crate::pipeline::ConfigResource>, DbError> {
        self.driver.extension_resources(keys)
    }

    /// Answers what [`DatabaseSession::configuration_selected`] answers,
    /// [`DbError::SessionClosed`] or [`DbError::AsyncContext`].
    pub fn configuration_selected(
        &mut self,
        progress: &mut dyn MetadataProgress,
        selector: &mut dyn ConfigSelector,
    ) -> Result<AcquiredConfiguration, DbError> {
        self.driver.configuration_selected(progress, selector)
    }

    /// Reads and resolves the metadata of the base, as
    /// [`DatabaseSession::metadata`] does.
    ///
    /// # Errors
    ///
    /// Answers what the asynchronous operation answers,
    /// [`DbError::SessionClosed`] or [`DbError::AsyncContext`].
    pub fn metadata(
        &mut self,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        self.driver.metadata(progress)
    }

    /// Runs one read-only statement and decodes `column_count` columns, as
    /// [`DatabaseSession::query`] does.
    ///
    /// # Errors
    ///
    /// Answers what the asynchronous operation answers,
    /// [`DbError::SessionClosed`] or [`DbError::AsyncContext`].
    pub fn query(&mut self, sql: &str, column_count: usize) -> Result<QueryRows, DbError> {
        self.driver.query(sql, column_count)
    }

    /// Reads one read-only statement row by row, as
    /// [`DatabaseSession::query_each`] does — stopping early included, and
    /// what stopping early costs a SQL Server session.
    ///
    /// # Errors
    ///
    /// Answers what the asynchronous operation answers,
    /// [`DbError::SessionClosed`] or [`DbError::AsyncContext`].
    pub fn query_each(
        &mut self,
        sql: &str,
        column_count: usize,
        on_row: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        self.driver.query_each(sql, column_count, on_row)
    }

    /// Closes the session.
    ///
    /// The session is gone afterwards whatever the close answered.
    ///
    /// # Errors
    ///
    /// Answers what [`DatabaseSession::close`] answers,
    /// [`DbError::SessionClosed`] when the session was already closed, or
    /// [`DbError::AsyncContext`], in which case the session stays open.
    pub fn close(&mut self) -> Result<(), DbError> {
        self.driver.close()
    }

    /// The provider the session speaks to, or `None` once it is closed.
    pub fn dialect(&self) -> Option<DatabaseDialect> {
        self.driver.session().map(DatabaseSession::dialect)
    }

    /// Whether the session was closed.
    pub const fn is_closed(&self) -> bool {
        self.driver.session().is_none()
    }

    /// Whether the session can no longer be used: it was closed, or the
    /// provider reports it dead, as [`DatabaseSession::is_dead`] does.
    pub fn is_dead(&self) -> bool {
        self.driver.session().is_none_or(DatabaseSession::is_dead)
    }
}

impl fmt::Debug for BlockingDatabaseSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BlockingDatabaseSession")
            .field("dialect", &self.dialect())
            .field("closed", &self.is_closed())
            .finish()
    }
}

#[cfg(test)]
#[path = "tests/indexed_blocking.rs"]
mod indexed_tests;

#[cfg(test)]
#[path = "tests/metadata_inputs_blocking.rs"]
mod metadata_inputs_tests;
