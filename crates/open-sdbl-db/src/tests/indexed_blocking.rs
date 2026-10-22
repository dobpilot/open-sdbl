use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct IndexedSession(Arc<AtomicUsize>);
impl SessionOperations for IndexedSession {
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
    async fn configuration_bootstrap(
        &mut self,
        _: &mut dyn ConfigSelector,
    ) -> Result<crate::ConfigurationBootstrap, DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(DbError::Data("bootstrap forwarded".into()))
    }
    async fn configuration_resources(
        &mut self,
        names: &[&str],
    ) -> Result<Vec<crate::ConfigResource>, DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(names
            .iter()
            .map(|name| crate::ConfigResource {
                file_name: (*name).into(),
                compressed: vec![1].into(),
            })
            .collect())
    }
    async fn extension_resources(
        &mut self,
        _: &[open_sdbl::metadata::ContentKey],
    ) -> Result<Vec<crate::ConfigResource>, DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
    async fn close(self) -> Result<(), DbError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct Selector;
impl ConfigSelector for Selector {
    fn root_resource_name(&mut self, _: &crate::ConfigResource) -> Result<String, DbError> {
        unreachable!()
    }
    fn configure(&mut self, _: &crate::ConfigResource) -> Result<(), DbError> {
        unreachable!()
    }
    fn retain(&mut self, _: &str) -> bool {
        unreachable!()
    }
}

#[test]
fn indexed_blocking_calls_forward_and_keep_lifecycle_precedence() {
    let calls = Arc::new(AtomicUsize::new(0));
    let credentials = Credentials::new(None, None, None);
    let mut driver =
        BlockingDriver::<IndexedSession>::connect(&calls, &credentials, Limits::default()).unwrap();
    assert_eq!(
        driver.configuration_resources(&["a", "b"]).unwrap().len(),
        2
    );
    assert!(
        driver
            .configuration_bootstrap(&mut Selector)
            .unwrap_err()
            .to_string()
            .contains("forwarded")
    );
    assert!(driver.extension_resources(&[]).unwrap().is_empty());
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        assert!(matches!(
            driver.configuration_resources(&[]),
            Err(DbError::AsyncContext)
        ));
        assert!(matches!(
            driver.extension_resources(&[]),
            Err(DbError::AsyncContext)
        ));
        assert!(matches!(
            driver.configuration_bootstrap(&mut Selector),
            Err(DbError::AsyncContext)
        ));
    });
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    driver.close().unwrap();
    runtime.block_on(async {
        assert!(matches!(
            driver.configuration_resources(&[]),
            Err(DbError::SessionClosed)
        ));
        assert!(matches!(
            driver.extension_resources(&[]),
            Err(DbError::SessionClosed)
        ));
        assert!(matches!(
            driver.configuration_bootstrap(&mut Selector),
            Err(DbError::SessionClosed)
        ));
    });
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}
