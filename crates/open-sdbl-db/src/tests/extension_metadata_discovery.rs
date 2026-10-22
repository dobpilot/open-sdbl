use super::*;
use crate::MetadataProgress;
use crate::pipeline::ConfigMetadata;
use futures_util::stream;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Source {
    blobs: Vec<Vec<u8>>,
    detects: usize,
    commits: usize,
    rollbacks: usize,
}
impl MetadataSource for Source {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        Ok(())
    }
    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        self.detects += 1;
        Ok(StorageLayout::MODERN)
    }
    async fn read_db_names(&mut self, _: &StorageLayout) -> Result<DbNames, DbError> {
        panic!("no DBNames")
    }
    async fn read_config(
        &mut self,
        _: &StorageLayout,
        _: &mut dyn MetadataProgress,
    ) -> Result<ConfigMetadata, DbError> {
        panic!("no Config")
    }
    async fn read_extension_resources(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        panic!("no ConfigCAS")
    }
    async fn read_schema(&mut self) -> Result<SchemaStorage, DbError> {
        panic!("no SchemaStorage")
    }
    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
        panic!("no live catalog")
    }
    async fn read_bounded_extension_restructures(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<Vec<u8>>, DbError> {
        Ok(self.blobs.clone())
    }
    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        self.commits += 1;
        Ok(())
    }
    async fn rollback_readonly(&mut self, error: DbError) -> DbError {
        self.rollbacks += 1;
        error
    }
}
#[test]
fn extension_physical_discovery_reuses_layout_and_does_not_read_config() {
    let blob =
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/fixtures/service_tables/extension_attrs/restruct_reference14574.txt",
        ))
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut source = Source {
        blobs: vec![blob],
        detects: 0,
        commits: 0,
        rollbacks: 0,
    };
    let (layout, metadata) = runtime
        .block_on(acquire_extensions(&mut source, None))
        .unwrap();
    assert_eq!(source.detects, 1);
    assert_eq!(metadata.len(), 1);
    assert!(!metadata[0].descriptors.is_empty());
    runtime
        .block_on(acquire_extensions(&mut source, Some(layout)))
        .unwrap();
    assert_eq!(source.detects, 1);
    assert_eq!(source.commits, 2);
    source.blobs = vec![vec![0xff]];
    assert!(
        runtime
            .block_on(acquire_extensions(&mut source, Some(layout)))
            .is_err()
    );
    assert_eq!(source.rollbacks, 1);
    let _: fn(&mut crate::BlockingDatabaseSession) -> Result<Vec<ExtensionMetadata>, DbError> =
        crate::BlockingDatabaseSession::configuration_extension_metadata;
}
#[test]
fn extension_structure_stream_stops_at_retention_limit() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let rows = stream::iter([Ok(vec![1; 3]), Ok(vec![2; 3]), Ok(vec![3; 3])]).inspect(move |_| {
        observed.fetch_add(1, Ordering::Relaxed);
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let limits = Limits {
        config_retained_byte_limit: 5,
        ..Limits::default()
    };
    assert!(
        runtime
            .block_on(collect_restructures(rows, limits))
            .is_err()
    );
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert!(open_sdbl::metadata::parse_extension_restructure_bounded(b"{0}", 2).is_err());
}
