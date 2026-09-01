use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use deadpool_postgres::Pool;
use open_sdbl::metadata::MetadataSnapshot;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

use crate::error::ServiceError;
use crate::model::MetadataCatalog;
use crate::postgres::load_metadata;
use crate::schema::build_catalog;

/// One atomically published metadata generation.
pub struct MetadataGeneration {
    pub snapshot: Arc<MetadataSnapshot>,
    pub catalog: Arc<MetadataCatalog>,
    loaded_at: Instant,
}

impl MetadataGeneration {
    fn fresh(&self, ttl: Duration) -> bool {
        generation_is_fresh(self.loaded_at, ttl)
    }
}

fn generation_is_fresh(loaded_at: Instant, ttl: Duration) -> bool {
    loaded_at.elapsed() < ttl
}

/// Stale-while-refresh cache around expensive 1C metadata reconstruction.
pub struct MetadataCache {
    pool: Pool,
    ttl: Duration,
    batch_size: usize,
    current: RwLock<Option<Arc<MetadataGeneration>>>,
    refresh: Mutex<()>,
    refresh_total: AtomicU64,
    refresh_errors_total: AtomicU64,
}

impl MetadataCache {
    #[must_use]
    pub fn new(pool: Pool, ttl: Duration, batch_size: usize) -> Arc<Self> {
        Arc::new(Self {
            pool,
            ttl,
            batch_size,
            current: RwLock::new(None),
            refresh: Mutex::new(()),
            refresh_total: AtomicU64::new(0),
            refresh_errors_total: AtomicU64::new(0),
        })
    }

    /// Gets a generation. Expired valid data is returned while one background
    /// refresh runs; the first load waits because readiness has no fallback.
    pub async fn get(self: &Arc<Self>) -> Result<Arc<MetadataGeneration>, ServiceError> {
        if let Some(generation) = self.current.read().await.clone() {
            if generation.fresh(self.ttl) {
                return Ok(generation);
            }
            let cache = Arc::clone(self);
            tokio::spawn(async move {
                if let Err(error) = cache.refresh_if_stale().await {
                    error!(error = %error, "metadata background refresh failed");
                }
            });
            return Ok(generation);
        }
        self.refresh_if_stale().await
    }

    /// Forces a synchronous generation replacement.
    pub async fn force_refresh(self: &Arc<Self>) -> Result<Arc<MetadataGeneration>, ServiceError> {
        let _guard = self.refresh.lock().await;
        self.load_and_publish().await
    }

    #[must_use]
    pub async fn ready(&self) -> bool {
        self.current.read().await.is_some()
    }

    #[must_use]
    pub fn refresh_counts(&self) -> (u64, u64) {
        (
            self.refresh_total.load(Ordering::Relaxed),
            self.refresh_errors_total.load(Ordering::Relaxed),
        )
    }

    async fn refresh_if_stale(self: &Arc<Self>) -> Result<Arc<MetadataGeneration>, ServiceError> {
        let Ok(_guard) = self.refresh.try_lock() else {
            if let Some(generation) = self.current.read().await.clone() {
                return Ok(generation);
            }
            let _guard = self.refresh.lock().await;
            if let Some(generation) = self.current.read().await.clone() {
                return Ok(generation);
            }
            return self.load_and_publish().await;
        };
        if let Some(generation) = self.current.read().await.clone()
            && generation.fresh(self.ttl)
        {
            return Ok(generation);
        }
        self.load_and_publish().await
    }

    async fn load_and_publish(&self) -> Result<Arc<MetadataGeneration>, ServiceError> {
        self.refresh_total.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let snapshot = match load_metadata(&self.pool, self.batch_size).await {
            Ok(snapshot) => Arc::new(snapshot),
            Err(error) => {
                self.refresh_errors_total.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        };
        let catalog = Arc::new(build_catalog(&snapshot));
        let generation = Arc::new(MetadataGeneration {
            snapshot,
            catalog,
            loaded_at: Instant::now(),
        });
        info!(
            duration_ms = started.elapsed().as_millis(),
            objects = generation.snapshot.objects.len(),
            tables = generation.catalog.tables.len(),
            issues = generation.catalog.issues.len(),
            "metadata generation refreshed"
        );
        *self.current.write().await = Some(Arc::clone(&generation));
        Ok(generation)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::generation_is_fresh;

    #[test]
    fn ttl_marks_only_nonexpired_generations_fresh() {
        assert!(generation_is_fresh(Instant::now(), Duration::from_secs(60)));
        assert!(!generation_is_fresh(
            Instant::now() - Duration::from_secs(61),
            Duration::from_secs(60)
        ));
    }
}
