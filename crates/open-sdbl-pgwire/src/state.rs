//! What every connection of a gateway shares: the metadata snapshot, the
//! users of the base and the rights of their roles.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use open_sdbl::metadata::{Guid, MetadataSnapshot};
use open_sdbl_db::{
    AccessStore, Credentials, DatabaseConnection, DatabaseDialect, DatabaseSession, DbError,
    Limits, MetadataProgress, ensure_rights, ensure_users,
};

/// The metadata, users, rights and complete virtual catalog a gateway serves.
///
/// They are acquired once before the gateway accepts connections and again on
/// an explicit refresh. Relation catalogs are compiled concurrently during
/// acquisition. A connection keeps the state it started a statement with until
/// that statement ends.
pub struct GatewayState {
    catalog: Arc<crate::catalog::Catalog>,
    snapshot: MetadataSnapshot,
    access: AccessStore,
    dialect: DatabaseDialect,
    unreadable_roles: usize,
}

// A virtual snapshot token, never a transaction ID read from the backing DB.
// Reserve PostgreSQL's special xid values 0..=2, including on wraparound.
fn next_catalog_revision() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(3);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.checked_add(1).unwrap_or(3))
    })
    .expect("the update always supplies a revision")
}

impl GatewayState {
    /// Reads the metadata of the base, its users and the rights of every
    /// role a user holds, through one read-only session that is closed
    /// afterwards.
    ///
    /// # Errors
    ///
    /// Returns what the database or the decoders reported.
    pub async fn acquire(
        connection: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
        progress: &mut dyn MetadataProgress,
    ) -> Result<Self, DbError> {
        Self::acquire_with_access_mode(connection, credentials, limits, progress, false).await
    }

    /// Reads metadata and users without loading role rights for unrestricted access.
    ///
    /// # Errors
    ///
    /// Returns what the database or the decoders reported.
    pub async fn acquire_unrestricted(
        connection: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
        progress: &mut dyn MetadataProgress,
    ) -> Result<Self, DbError> {
        Self::acquire_with_access_mode(connection, credentials, limits, progress, true).await
    }

    async fn acquire_with_access_mode(
        connection: &DatabaseConnection,
        credentials: &Credentials,
        limits: Limits,
        progress: &mut dyn MetadataProgress,
        unrestricted: bool,
    ) -> Result<Self, DbError> {
        let database = match connection {
            DatabaseConnection::Postgres(connection) => connection.options.database.clone(),
            DatabaseConnection::MsSql(connection) => connection.options.database.clone(),
        };
        let mut session = DatabaseSession::connect(connection, credentials, limits).await?;
        let result = Self::read(&mut session, progress, unrestricted).await;
        let closed = session.close().await;
        let (snapshot, access, dialect, unreadable_roles) = result?;
        closed?;
        tokio::task::spawn_blocking(move || {
            let revision = next_catalog_revision();
            let catalog = crate::catalog::build(&snapshot, dialect, &database, revision);
            Self {
                catalog: Arc::new(catalog),
                snapshot,
                access,
                dialect,
                unreadable_roles,
            }
        })
        .await
        .map_err(|error| DbError::Data(format!("catalog construction failed: {error}")))
    }

    async fn read(
        session: &mut DatabaseSession,
        progress: &mut dyn MetadataProgress,
        unrestricted: bool,
    ) -> Result<(MetadataSnapshot, AccessStore, DatabaseDialect, usize), DbError> {
        let (snapshot, _report) = session.metadata(progress).await?;
        let mut access = AccessStore::new(&snapshot);
        ensure_users(&mut access, session).await?;
        let unreadable = if unrestricted {
            Vec::new()
        } else {
            let mut roles: Vec<Guid> = Vec::new();
            for user in access.users().unwrap_or_default() {
                for role in &user.data.roles {
                    if !roles.contains(role) {
                        roles.push(role.clone());
                    }
                }
            }
            ensure_rights(&mut access, session, &roles).await?
        };
        Ok((snapshot, access, session.dialect(), unreadable.len()))
    }

    /// A state built from parts the caller already holds.
    #[must_use]
    pub fn from_parts(
        snapshot: MetadataSnapshot,
        access: AccessStore,
        dialect: DatabaseDialect,
    ) -> Self {
        let revision = next_catalog_revision();
        let catalog = crate::catalog::build(&snapshot, dialect, "unf", revision);
        Self {
            catalog: Arc::new(catalog),
            snapshot,
            access,
            dialect,
            unreadable_roles: 0,
        }
    }

    pub(crate) fn catalog(&self) -> &crate::catalog::Catalog {
        &self.catalog
    }

    #[cfg(test)]
    pub(crate) fn catalog_revision(&self) -> u32 {
        self.catalog.revision
    }

    /// The metadata snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &MetadataSnapshot {
        &self.snapshot
    }

    /// The users and role rights read.
    #[must_use]
    pub const fn access(&self) -> &AccessStore {
        &self.access
    }

    /// How generated SQL is spelled.
    #[must_use]
    pub const fn dialect(&self) -> DatabaseDialect {
        self.dialect
    }

    /// How many users the base declares.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.access.users().map_or(0, <[_]>::len)
    }

    /// How many roles held by users carry no rights resource in the base;
    /// their holders fail every restricted statement closed.
    #[must_use]
    pub const fn unreadable_role_count(&self) -> usize {
        self.unreadable_roles
    }
}

/// The current [`GatewayState`], replaced atomically on refresh.
pub struct StateHandle {
    current: RwLock<Arc<GatewayState>>,
}

impl StateHandle {
    /// A handle holding `state`.
    #[must_use]
    pub fn new(state: GatewayState) -> Self {
        Self {
            current: RwLock::new(Arc::new(state)),
        }
    }

    /// The state new statements compile against.
    #[must_use]
    pub fn current(&self) -> Arc<GatewayState> {
        Arc::clone(
            &self
                .current
                .read()
                .unwrap_or_else(|poison| poison.into_inner()),
        )
    }

    /// Replaces the state; statements already running keep the old one.
    pub fn replace(&self, state: GatewayState) {
        *self
            .current
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = Arc::new(state);
    }
}
