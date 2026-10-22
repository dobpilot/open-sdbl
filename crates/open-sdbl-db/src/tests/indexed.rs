use super::*;
use open_sdbl::metadata::{MsSqlMetadataQueries, PostgresMetadataQueries};

#[test]
fn addressed_queries_escape_names_and_preserve_indexed_columns() {
    for mut layout in [StorageLayout::MODERN, StorageLayout::LEGACY] {
        layout.extension_store = true;
        for (base, mssql) in [
            (PostgresMetadataQueries::config_all(&layout), false),
            (MsSqlMetadataQueries::config_all(&layout), true),
            (
                PostgresMetadataQueries::extension_resources(&layout).unwrap(),
                false,
            ),
            (
                MsSqlMetadataQueries::extension_resources(&layout).unwrap(),
                true,
            ),
        ] {
            let query = sql(base, &["a'quoted".into(), "back\\slash".into()], mssql);
            assert!(query.contains("a''quoted"));
            assert!(query.contains(if mssql {
                "N'back\\slash'"
            } else {
                "E'back\\\\slash'"
            }));
            assert!(query.contains(if mssql {
                "WHERE [FileName] IN ("
            } else {
                "WHERE filename IN ("
            }));
            assert_eq!(query.matches(" WHERE ").count(), 1);
            assert!(!query.contains("WHERE rtrim"));
            let totals = totals_sql(base, &["selected".into()], mssql);
            assert!(totals.contains(if mssql {
                "WHERE [FileName] IN (N'selected')"
            } else {
                "WHERE filename IN (E'selected')"
            }));
            assert!(!totals.contains("ORDER BY"));
        }
    }
}

#[test]
fn requests_are_bounded_deduplicated_and_right_trimmed() {
    let limits = Limits {
        config_resource_limit: 2,
        ..Limits::default()
    };
    assert_eq!(names(&["b ", "a", "b"], limits).unwrap(), ["b", "a"]);
    assert!(names(&["a", "b", "c"], limits).is_err());
    assert!(names(&[""], limits).is_err());
    assert!(names(&["a\0b"], limits).is_err());
    assert!(names(&[], limits).unwrap().is_empty());
}

#[test]
fn addressed_results_are_complete_and_in_request_order() {
    let resource = |name: &str| ConfigResource {
        file_name: name.into(),
        compressed: vec![1].into(),
    };
    let requested = vec!["b".into(), "a".into()];
    let result = ordered(&requested, vec![resource("a"), resource("b")]).unwrap();
    assert_eq!(result[0].file_name, "b");
    assert!(
        ordered(&requested, vec![resource("a")])
            .unwrap_err()
            .to_string()
            .contains("\"b\"")
    );
    assert!(ordered(&["a".into()], vec![resource("a"), resource("extra")]).is_err());
}

#[tokio::test]
async fn multipart_limit_is_checked_before_assembly_and_gaps_fail() {
    use futures_util::{StreamExt, stream};
    let limits = Limits {
        config_retained_byte_limit: 3,
        ..Limits::default()
    };
    let mut held = 0;
    let parts = stream::iter(vec![
        Ok(("a".into(), 0, vec![1, 2])),
        Ok(("a".into(), 1, vec![3, 4])),
    ])
    .map(|part| bound_part(part, &mut held, limits));
    assert!(
        crate::pipeline::collect_bounded_resources(
            crate::pipeline::assemble_parts(parts),
            limits,
            &mut crate::NoProgress
        )
        .await
        .is_err()
    );
    let parts = stream::iter(vec![
        Ok(("a".into(), 0, vec![1])),
        Ok(("a".into(), 2, vec![2])),
    ]);
    assert!(
        crate::pipeline::collect_bounded_resources(
            crate::pipeline::assemble_parts(parts),
            limits,
            &mut crate::NoProgress
        )
        .await
        .is_err()
    );
}

#[derive(Default)]
struct IndexedSource {
    probes: usize,
    reads: Vec<(Vec<String>, bool)>,
    byte_budgets: Vec<usize>,
    commits: usize,
    rollbacks: usize,
    fail: bool,
}

impl MetadataSource for IndexedSource {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        Ok(())
    }
    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        self.probes += 1;
        Ok(StorageLayout::MODERN)
    }
    async fn read_indexed_resources(
        &mut self,
        _: &StorageLayout,
        requested: &[String],
        extension: bool,
        limits: Limits,
    ) -> Result<Vec<ConfigResource>, DbError> {
        self.reads.push((requested.to_vec(), extension));
        self.byte_budgets.push(limits.config_retained_byte_limit);
        crate::check_retention_totals(limits, requested.len() as u64, requested.len() as u64)?;
        if self.fail {
            return Err(DbError::Data("missing addressed resource".into()));
        }
        Ok(requested
            .iter()
            .map(|name| ConfigResource {
                file_name: name.clone(),
                compressed: vec![1].into(),
            })
            .collect())
    }
    async fn read_extension_catalog(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<ExtensionCatalogRow>, DbError> {
        Ok(Vec::new())
    }
    async fn read_db_names(
        &mut self,
        _: &StorageLayout,
    ) -> Result<open_sdbl::metadata::DbNames, DbError> {
        panic!("bootstrap must not read DBNames")
    }
    async fn read_config(
        &mut self,
        _: &StorageLayout,
        _: &mut dyn crate::MetadataProgress,
    ) -> Result<crate::pipeline::ConfigMetadata, DbError> {
        panic!("must not scan Config")
    }
    async fn read_extension_resources(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        panic!("must not scan ConfigCAS")
    }
    async fn read_schema(&mut self) -> Result<open_sdbl::metadata::SchemaStorage, DbError> {
        panic!("must not read SchemaStorage")
    }
    async fn read_live_tables(&mut self) -> Result<Vec<open_sdbl::metadata::LiveTable>, DbError> {
        panic!("must not read live catalog")
    }
    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        self.commits += 1;
        Ok(())
    }
    async fn rollback_readonly(&mut self, original: DbError) -> DbError {
        self.rollbacks += 1;
        original
    }
}

#[derive(Default)]
struct BootstrapSelector {
    configured: bool,
}
impl ConfigSelector for BootstrapSelector {
    fn root_resource_name(&mut self, root: &ConfigResource) -> Result<String, DbError> {
        assert_eq!(root.file_name, "root");
        Ok("12345678-1234-1234-1234-123456789abc".into())
    }
    fn configure(&mut self, descriptor: &ConfigResource) -> Result<(), DbError> {
        assert_eq!(descriptor.file_name, "12345678-1234-1234-1234-123456789abc");
        self.configured = true;
        Ok(())
    }
    fn retain(&mut self, _: &str) -> bool {
        panic!("bootstrap never calls retain")
    }
}

#[tokio::test]
async fn bootstrap_and_repeated_reads_only_address_requested_resources_and_reuse_layout() {
    let mut source = IndexedSource::default();
    let mut selector = BootstrapSelector::default();
    let acquired = bootstrap(&mut source, None, &mut selector, Limits::default())
        .await
        .unwrap();
    assert!(selector.configured);
    let (_, two) = resources(
        &mut source,
        Some(acquired.layout),
        &["a".into(), "b".into()],
        false,
        Limits::default(),
    )
    .await
    .unwrap();
    let (_, one) = resources(
        &mut source,
        Some(acquired.layout),
        &["cas-key".into()],
        true,
        Limits::default(),
    )
    .await
    .unwrap();
    assert_eq!((two.len(), one.len()), (2, 1));
    assert_eq!(source.probes, 1);
    assert_eq!(
        source.byte_budgets[1],
        Limits::default().config_retained_byte_limit - 1
    );
    assert_eq!(
        source.reads,
        [
            (vec!["root".into()], false),
            (vec!["12345678-1234-1234-1234-123456789abc".into()], false),
            (vec!["a".into(), "b".into()], false),
            (vec!["cas-key".into()], true),
        ]
    );
    assert_eq!((source.commits, source.rollbacks), (3, 0));
}

#[tokio::test]
async fn indexed_failure_rolls_back_and_bootstrap_counts_both_resources() {
    let mut source = IndexedSource {
        fail: true,
        ..Default::default()
    };
    assert!(
        resources(&mut source, None, &["a".into()], false, Limits::default())
            .await
            .is_err()
    );
    assert_eq!((source.commits, source.rollbacks), (0, 1));
    let mut source = IndexedSource::default();
    let limits = Limits {
        config_retained_byte_limit: 1,
        ..Limits::default()
    };
    assert!(
        bootstrap(&mut source, None, &mut BootstrapSelector::default(), limits)
            .await
            .is_err()
    );
    assert_eq!((source.commits, source.rollbacks), (0, 1));
}
