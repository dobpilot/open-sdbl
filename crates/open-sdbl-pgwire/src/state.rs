//! What every connection of a gateway shares: the metadata snapshot, the
//! users of the base and the rights of their roles.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use open_sdbl::metadata::{Guid, MetadataSnapshot};
use open_sdbl_db::{
    AccessStore, Credentials, DatabaseConnection, DatabaseDialect, DatabaseSession, DbError,
    Limits, MetadataProgress, ensure_rights, ensure_users,
};

/// The metadata, users and rights a gateway compiles against.
///
/// It is read once, before the gateway accepts connections, and read again
/// on an explicit refresh; a connection keeps the state it started a
/// statement with until that statement ends.
pub struct GatewayState {
    catalog_revision: u32,
    catalog_candidates: Arc<[crate::catalog::Candidate]>,
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
        let mut session = DatabaseSession::connect(connection, credentials, limits).await?;
        let result = Self::read(&mut session, progress).await;
        let closed = session.close().await;
        let state = result?;
        closed?;
        Ok(state)
    }

    async fn read(
        session: &mut DatabaseSession,
        progress: &mut dyn MetadataProgress,
    ) -> Result<Self, DbError> {
        let (snapshot, _report) = session.metadata(progress).await?;
        let mut access = AccessStore::new(&snapshot);
        ensure_users(&mut access, session).await?;
        let mut roles: Vec<Guid> = Vec::new();
        for user in access.users().unwrap_or_default() {
            for role in &user.data.roles {
                if !roles.contains(role) {
                    roles.push(role.clone());
                }
            }
        }
        let unreadable = ensure_rights(&mut access, session, &roles).await?;
        let catalog_candidates = crate::catalog::candidates(&snapshot).into();
        Ok(Self {
            catalog_revision: next_catalog_revision(),
            catalog_candidates,
            snapshot,
            access,
            dialect: session.dialect(),
            unreadable_roles: unreadable.len(),
        })
    }

    /// A state built from parts the caller already holds.
    #[must_use]
    pub fn from_parts(
        snapshot: MetadataSnapshot,
        access: AccessStore,
        dialect: DatabaseDialect,
    ) -> Self {
        let catalog_candidates = crate::catalog::candidates(&snapshot).into();
        Self {
            catalog_revision: next_catalog_revision(),
            catalog_candidates,
            snapshot,
            access,
            dialect,
            unreadable_roles: 0,
        }
    }

    /// The snapshot's relation inventory, prepared before accepting clients.
    pub(crate) fn catalog_candidates(&self) -> &[crate::catalog::Candidate] {
        &self.catalog_candidates
    }

    pub(crate) const fn catalog_revision(&self) -> u32 {
        self.catalog_revision
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
