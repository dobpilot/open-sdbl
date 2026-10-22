use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct DiscoverySession(Arc<AtomicUsize>);
impl SessionOperations for DiscoverySession {
    type Target = Arc<AtomicUsize>;
    async fn connect(target: &Self::Target, _: &Credentials, _: Limits) -> Result<Self, DbError> {
        Ok(Self(Arc::clone(target)))
    }
    async fn configuration(
        &mut self,
        _: &mut dyn MetadataProgress,
    ) -> Result<AcquiredConfiguration, DbError> {
        unreachable!()
    }
    async fn configuration_selected(
        &mut self,
        _: &mut dyn MetadataProgress,
        _: &mut dyn ConfigSelector,
    ) -> Result<AcquiredConfiguration, DbError> {
        unreachable!()
    }
    async fn ensure_users(&mut self, _: &mut AccessStore) -> Result<(), DbError> {
        unreachable!()
    }
    async fn ensure_rights(
        &mut self,
        _: &mut AccessStore,
        _: &[Guid],
    ) -> Result<Vec<Guid>, DbError> {
        unreachable!()
    }
    async fn metadata(
        &mut self,
        _: &mut dyn MetadataProgress,
    ) -> Result<(MetadataSnapshot, ResolutionReport), DbError> {
        unreachable!()
    }
    async fn query(&mut self, _: &str, _: usize) -> Result<QueryRows, DbError> {
        unreachable!()
    }
    async fn query_each(
        &mut self,
        _: &str,
        _: usize,
        _: impl FnMut(Vec<Cell>) -> Result<RowFlow, DbError>,
    ) -> Result<(), DbError> {
        unreachable!()
    }
    async fn configuration_metadata_inputs(
        &mut self,
    ) -> Result<crate::ConfigurationMetadataInputs, DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(crate::ConfigurationMetadataInputs {
            layout: open_sdbl::metadata::StorageLayout::MODERN,
            db_names: open_sdbl::metadata::DbNames::from_entries(vec![]),
            schema: open_sdbl::metadata::SchemaStorage {
                tables: vec![],
                anomalies: vec![],
            },
            live_tables: vec![],
        })
    }
    async fn configuration_resource_names(&mut self) -> Result<Vec<String>, DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec!["root".into()])
    }
    async fn close(self) -> Result<(), DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn discovery_forwarding_preserves_closed_and_async_precedence() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut driver = BlockingDriver::<DiscoverySession>::connect(
        &calls,
        &Credentials::new(None, None, None),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        driver.configuration_metadata_inputs().unwrap().layout,
        open_sdbl::metadata::StorageLayout::MODERN
    );
    assert_eq!(driver.configuration_resource_names().unwrap(), ["root"]);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        assert!(matches!(
            driver.configuration_metadata_inputs(),
            Err(DbError::AsyncContext)
        ));
        assert!(matches!(
            driver.configuration_resource_names(),
            Err(DbError::AsyncContext)
        ));
    });
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    driver.close().unwrap();
    runtime.block_on(async {
        assert!(matches!(
            driver.configuration_metadata_inputs(),
            Err(DbError::SessionClosed)
        ));
        assert!(matches!(
            driver.configuration_resource_names(),
            Err(DbError::SessionClosed)
        ));
    });
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}
