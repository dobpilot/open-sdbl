//! Tests of the whole-configuration acquisition, over an in-memory
//! `MetadataSource` rather than a live base.
//!
//! The source answers the same rows a provider would: `(name, part,
//! data)` triples ordered by name and part, a content-addressed store,
//! and the rows of `_ExtensionsInfo`. Everything the acquisition does
//! with them — assembling parts, deciding which resource the metadata
//! decoder sees, grouping the store by extension, committing or rolling
//! back — is exercised here without a database.

use open_sdbl::metadata::{
    ContentKey, DbNames, LiveColumn, LiveTable, SchemaStorage, StorageLayout, parse_db_names,
    parse_schema_storage,
};

use super::{
    AcquiredConfiguration, ConfigDecodeLimits, ConfigMetadata, ConfigResource, ExtensionCatalogRow,
    MetadataSource, ResourcePart, acquire_configuration, acquire_metadata, assemble_parts,
    check_retention_totals, collect_bounded_resources, config_pipeline_depth, decode_config_stream,
    decode_retained_config,
};
use crate::error::DbError;
use crate::hex_test_support::hex;
use crate::limits::Limits;
use crate::progress::{MetadataProgress, NoProgress};

/// The DBNames of one enumeration and its owner.
const DB_NAMES: &str = "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500";

/// One compressed bare-GUID descriptor resource.
const DESCRIPTOR: &str = "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f";

const SPLIT: &str = "b8bac76b-c91b-4d78-8a70-ffa39f8de694";
const WHOLE: &str = "25c96bd3-fac4-42ef-b695-74c9af43589b";
/// A resource the acquisition name filter rejects.
const UNFILTERED: &str = "commonpicture.bin";

/// Counts what a read reports, so the test can check the totals and the
/// advances line up.
#[derive(Default)]
struct CountingProgress {
    announced_resources: u64,
    announced_bytes: u64,
    completed_resources: u64,
    completed_bytes: u64,
}

impl MetadataProgress for CountingProgress {
    fn config_totals(&mut self, resources: u64, bytes: u64) {
        self.announced_resources = resources;
        self.announced_bytes = bytes;
    }

    fn advance_config(&mut self, resources: usize, bytes: usize) {
        self.completed_resources += resources as u64;
        self.completed_bytes += bytes as u64;
    }
}

/// A `MetadataSource` answering rows a test wrote, so that the whole
/// acquisition runs without a database.
struct FakeSource {
    limits: Limits,
    config_rows: Vec<ResourcePart>,
    store_rows: Vec<ResourcePart>,
    extensions: Vec<ExtensionCatalogRow>,
    /// When set, reading `Config` fails with this message.
    config_failure: Option<String>,
    committed: bool,
    rolled_back: bool,
}

impl FakeSource {
    fn new() -> Self {
        Self {
            limits: Limits::default(),
            config_rows: config_rows(),
            store_rows: Vec::new(),
            extensions: Vec::new(),
            config_failure: None,
            committed: false,
            rolled_back: false,
        }
    }

    fn with_extensions(mut self) -> Self {
        let (store, catalog) = two_extensions();
        self.store_rows = store;
        self.extensions = catalog;
        self
    }

    fn rows(&self) -> impl futures_util::Stream<Item = Result<ResourcePart, DbError>> + use<> {
        futures_util::stream::iter(self.config_rows.clone().into_iter().map(Ok))
    }
}

impl MetadataSource for FakeSource {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        Ok(())
    }

    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        Ok(StorageLayout::MODERN)
    }

    async fn read_db_names(&mut self, _layout: &StorageLayout) -> Result<DbNames, DbError> {
        parse_db_names(&hex(DB_NAMES)).map_err(DbError::from)
    }

    async fn read_config(
        &mut self,
        _layout: &StorageLayout,
        progress: &mut dyn MetadataProgress,
    ) -> Result<ConfigMetadata, DbError> {
        if let Some(failure) = &self.config_failure {
            return Err(DbError::Data(failure.clone()));
        }
        // The statement filters; the fake filters the same way.
        let filtered = self
            .config_rows
            .clone()
            .into_iter()
            .filter(|(name, _, _)| open_sdbl::metadata::is_config_metadata_resource(name))
            .map(Ok);
        decode_config_stream(
            assemble_parts(futures_util::stream::iter(filtered)),
            8,
            config_pipeline_depth(),
            ConfigDecodeLimits::default(),
            progress,
            Limits::default().query_timeout,
        )
        .await
    }

    async fn read_whole_config(
        &mut self,
        _layout: &StorageLayout,
        progress: &mut dyn MetadataProgress,
    ) -> Result<(ConfigMetadata, Vec<ConfigResource>), DbError> {
        if let Some(failure) = &self.config_failure {
            return Err(DbError::Data(failure.clone()));
        }
        let resource_count = distinct_names(&self.config_rows);
        let compressed_bytes = self
            .config_rows
            .iter()
            .map(|(_, _, data)| data.len() as u64)
            .sum();
        check_retention_totals(self.limits, resource_count, compressed_bytes)?;
        progress.config_totals(resource_count, compressed_bytes);
        let assembled =
            collect_bounded_resources(assemble_parts(self.rows()), self.limits, progress).await?;
        let metadata = decode_retained_config(
            &assembled,
            8,
            ConfigDecodeLimits::default(),
            Limits::default().query_timeout,
        )
        .await?;
        Ok((metadata, assembled))
    }

    async fn read_extension_resources(
        &mut self,
        _layout: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        let rows = futures_util::stream::iter(self.store_rows.clone().into_iter().map(Ok));
        let mut resources = std::pin::pin!(assemble_parts(rows));
        let mut assembled = Vec::new();
        while let Some(resource) = futures_util::StreamExt::next(&mut resources).await {
            assembled.push(resource?);
        }
        Ok(assembled)
    }

    async fn read_extension_catalog(
        &mut self,
        _layout: &StorageLayout,
    ) -> Result<Vec<ExtensionCatalogRow>, DbError> {
        Ok(self.extensions.clone())
    }

    async fn read_schema(&mut self) -> Result<SchemaStorage, DbError> {
        parse_schema_storage(b"{1,\n{0}\n}").map_or_else(
            |_| {
                Ok(SchemaStorage {
                    tables: Vec::new(),
                    anomalies: Vec::new(),
                })
            },
            Ok,
        )
    }

    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
        Ok(vec![LiveTable {
            name: "_enum99".to_owned(),
            columns: vec![LiveColumn {
                name: "_idrref".to_owned(),
                data_type: "bytea".to_owned(),
            }],
            indexes: Vec::new(),
        }])
    }

    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        self.committed = true;
        Ok(())
    }

    async fn rollback_readonly(&mut self, original: DbError) -> DbError {
        self.rolled_back = true;
        original
    }
}

fn distinct_names(rows: &[ResourcePart]) -> u64 {
    let mut names = rows
        .iter()
        .map(|(name, _, _)| name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names.len() as u64
}

/// The rows of `Config`, ordered by name and part as every statement
/// orders them: one resource split into three parts, one whole, and one
/// the acquisition name filter rejects.
fn config_rows() -> Vec<ResourcePart> {
    let descriptor = hex(DESCRIPTOR);
    let third = descriptor.len() / 3;
    let mut rows = vec![
        (WHOLE.to_owned(), 0, descriptor.clone()),
        (SPLIT.to_owned(), 0, descriptor[..third].to_vec()),
        (SPLIT.to_owned(), 1, descriptor[third..third * 2].to_vec()),
        (SPLIT.to_owned(), 2, descriptor[third * 2..].to_vec()),
        (
            UNFILTERED.to_owned(),
            0,
            b"a picture, not a descriptor".to_vec(),
        ),
    ];
    rows.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    rows
}

/// A raw-DEFLATE stream holding `data` in one stored block, which the
/// core inflater reads like any other.
fn stored_deflate(data: &[u8]) -> Vec<u8> {
    let length = u16::try_from(data.len()).expect("fixture fits one stored block");
    let mut block = vec![0x01];
    block.extend_from_slice(&length.to_le_bytes());
    block.extend_from_slice(&(!length).to_le_bytes());
    block.extend_from_slice(data);
    block
}

/// The base64 the root index spells a key with.
fn base64(key: &ContentKey) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = key.as_bytes();
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let mut block = [0_u8; 3];
        block[..chunk.len()].copy_from_slice(chunk);
        let value = u32::from_be_bytes([0, block[0], block[1], block[2]]);
        for index in 0..=chunk.len() {
            text.push(ALPHABET[(value >> (18 - 6 * index) & 0x3f) as usize] as char);
        }
    }
    while text.len() % 4 != 0 {
        text.push('=');
    }
    text
}

fn key(seed: u8) -> ContentKey {
    let mut bytes = [0_u8; 20];
    bytes[19] = seed;
    // `ContentKey` is built from the record that carries it; the fixture
    // spells one the same way the store does.
    open_sdbl::metadata::extension_root_key(&[&[0_u8; 4][..], &bytes[..]].concat())
        .expect("twenty bytes after the marker")
}

/// The info record of one extension, written the way the platform writes
/// it: the marker, the root key, the synonym, and a tail whose
/// second-to-last flag says whether the base applies the extension.
fn info_record(root: &ContentKey, active: bool) -> Vec<u8> {
    let mut record = vec![0x43, 0xc2, 0x9a, 0x14];
    record.extend_from_slice(root.as_bytes());
    record.push(0x97);
    let synonym = "расширение".encode_utf16().collect::<Vec<_>>();
    record.push(u8::try_from(synonym.len()).unwrap());
    for unit in synonym {
        record.extend_from_slice(&unit.to_le_bytes());
    }
    record.extend_from_slice(&[0x81, if active { 0x82 } else { 0x81 }, 0x82, 0x20]);
    record
}

/// Two extensions that both name a resource `shared.0`, with different
/// content, plus one resource of their own.
fn two_extensions() -> (Vec<ResourcePart>, Vec<ExtensionCatalogRow>) {
    let (alpha_root, beta_root) = (key(1), key(2));
    let (alpha_shared, beta_shared) = (key(3), key(4));
    let (alpha_own, beta_own) = (key(5), key(6));

    let index = |shared: &ContentKey, own: &ContentKey, own_name: &str| {
        stored_deflate(
            format!(
                "{{2,\"shared.0\",\"{}\",\"{own_name}\",\"{}\"}}",
                base64(shared),
                base64(own)
            )
            .as_bytes(),
        )
    };
    let mut store = vec![
        (
            alpha_root.as_hex(),
            0,
            index(&alpha_shared, &alpha_own, "alpha.0"),
        ),
        (
            beta_root.as_hex(),
            0,
            index(&beta_shared, &beta_own, "beta.0"),
        ),
        (
            alpha_shared.as_hex(),
            0,
            b"shared, as alpha wrote it".to_vec(),
        ),
        (
            beta_shared.as_hex(),
            0,
            b"shared, as beta wrote it".to_vec(),
        ),
        (alpha_own.as_hex(), 0, b"only alpha".to_vec()),
        (beta_own.as_hex(), 0, b"only beta".to_vec()),
    ];
    store.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let catalog = vec![
        ExtensionCatalogRow {
            identity: vec![0xaa; 16],
            order: 1,
            name: "Альфа".to_owned(),
            info: info_record(&alpha_root, true),
        },
        ExtensionCatalogRow {
            identity: vec![0xbb; 16],
            order: 2,
            name: "Бета".to_owned(),
            info: info_record(&beta_root, false),
        },
    ];
    (store, catalog)
}

async fn configuration(mut source: FakeSource) -> Result<AcquiredConfiguration, DbError> {
    acquire_configuration(&mut source, &mut NoProgress).await
}

#[tokio::test]
async fn answers_the_metadata_read_plus_every_resource() {
    let mut source = FakeSource::new();
    let (snapshot, _layout, report) = acquire_metadata(&mut source, &mut NoProgress)
        .await
        .unwrap();
    assert!(source.committed);

    let mut source = FakeSource::new();
    let mut progress = CountingProgress::default();
    let acquired = acquire_configuration(&mut source, &mut progress)
        .await
        .unwrap();
    assert!(source.committed);

    assert_eq!(acquired.metadata.fingerprint(), snapshot.fingerprint());
    assert_eq!(acquired.report, report);
    assert_eq!(acquired.layout, StorageLayout::MODERN);

    let names = acquired
        .config_resources
        .iter()
        .map(|resource| resource.file_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, [WHOLE, SPLIT, UNFILTERED]);
    // Progress announced the whole table and reported every resource of
    // it, the one the metadata decoder never sees included.
    assert_eq!(progress.announced_resources, 3);
    assert_eq!(progress.completed_resources, 3);
    assert_eq!(progress.announced_bytes, progress.completed_bytes);
}

#[tokio::test]
async fn keeps_a_resource_the_metadata_filter_rejects() {
    let acquired = configuration(FakeSource::new()).await.unwrap();
    let kept = acquired
        .config_resources
        .iter()
        .find(|resource| resource.file_name == UNFILTERED)
        .expect("the resource outside the filter is kept");
    assert_eq!(&*kept.compressed, b"a picture, not a descriptor");
    assert!(!open_sdbl::metadata::is_config_metadata_resource(
        UNFILTERED
    ));
}

#[tokio::test]
async fn assembles_a_split_resource_once() {
    let acquired = configuration(FakeSource::new()).await.unwrap();
    let split = acquired
        .config_resources
        .iter()
        .filter(|resource| resource.file_name == SPLIT)
        .collect::<Vec<_>>();
    assert_eq!(split.len(), 1, "one resource, not one per part");
    assert_eq!(&*split[0].compressed, hex(DESCRIPTOR).as_slice());
}

#[tokio::test]
async fn refuses_a_broken_part_sequence_naming_the_resource() {
    let descriptor = hex(DESCRIPTOR);
    for (rows, expected) in [
        (
            vec![
                (SPLIT.to_owned(), 0, descriptor.clone()),
                (SPLIT.to_owned(), 2, descriptor.clone()),
            ],
            "part 2 arrived where part 1 was expected",
        ),
        (
            vec![
                (SPLIT.to_owned(), 0, descriptor.clone()),
                (SPLIT.to_owned(), 0, descriptor.clone()),
            ],
            "part 0 arrived where part 1 was expected",
        ),
        (
            vec![(SPLIT.to_owned(), 1, descriptor.clone())],
            "part 1 arrived where part 0 was expected",
        ),
    ] {
        let mut source = FakeSource::new();
        source.config_rows = rows;
        let error = acquire_configuration(&mut source, &mut NoProgress)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(SPLIT), "{error}");
        assert!(error.contains(expected), "{error}");
        assert!(source.rolled_back && !source.committed, "{error}");
    }
}

#[tokio::test]
async fn rolls_back_and_publishes_nothing_when_a_read_fails() {
    let mut source = FakeSource::new();
    source.config_failure = Some("Config resource 3 could not be read".to_owned());
    let error = acquire_configuration(&mut source, &mut NoProgress)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("could not be read"));
    assert!(source.rolled_back);
    assert!(!source.committed, "nothing is published on a failure");
}

#[tokio::test]
async fn refuses_a_configuration_larger_than_the_ceiling_before_reading_it() {
    // The totals alone are enough to refuse: no resource row is read.
    let mut source = FakeSource::new();
    source.limits.config_resource_limit = 1;
    let error = acquire_configuration(&mut source, &mut NoProgress)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Config resources"), "{error}");
    assert!(error.contains("this read may hold"), "{error}");
    assert!(source.rolled_back && !source.committed);

    let mut source = FakeSource::new();
    source.limits.config_retained_byte_limit = 8;
    let error = acquire_configuration(&mut source, &mut NoProgress)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("compressed bytes"), "{error}");
    assert!(source.rolled_back && !source.committed);
}

#[tokio::test]
async fn refuses_a_table_that_outgrew_its_totals() {
    // The totals are a statement of their own and can be stale, so the
    // ceiling is checked again against what actually arrives.
    let limits = Limits {
        config_resource_limit: 2,
        ..Limits::default()
    };
    let rows = config_rows();
    let error = collect_bounded_resources(
        assemble_parts(futures_util::stream::iter(rows.into_iter().map(Ok))),
        limits,
        &mut NoProgress,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("more Config resources than the 2"),
        "{error}"
    );

    let limits = Limits {
        config_retained_byte_limit: 4,
        ..Limits::default()
    };
    let error = collect_bounded_resources(
        assemble_parts(futures_util::stream::iter(
            config_rows().into_iter().map(Ok),
        )),
        limits,
        &mut NoProgress,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("more than the 4 compressed bytes"),
        "{error}"
    );

    // Under a ceiling it fits, the same rows assemble as they always did.
    let resources = collect_bounded_resources(
        assemble_parts(futures_util::stream::iter(
            config_rows().into_iter().map(Ok),
        )),
        Limits::default(),
        &mut NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(resources.len(), 3);
}

#[tokio::test]
async fn answers_an_unknown_activity_rather_than_guessing_it() {
    let mut source = FakeSource::new().with_extensions();
    // A record the decoder cannot follow: the flag is not read out of it.
    let root = source.extensions[0].info[4..24].to_vec();
    let mut unreadable = vec![0x43, 0xc2, 0x9a, 0x14];
    unreadable.extend_from_slice(&root);
    unreadable.extend_from_slice(&[0x98, 0x02, 0x81, 0x82, 0x20]);
    source.extensions[0].info = unreadable;

    let acquired = acquire_configuration(&mut source, &mut NoProgress)
        .await
        .unwrap();
    assert_eq!(acquired.extensions[0].active, None);
    assert_eq!(acquired.extensions[1].active, Some(false));
    // The resources are still answered: only the activity is unknown.
    assert!(!acquired.extensions[0].resources.is_empty());
}

#[tokio::test]
async fn keeps_two_extensions_apart() {
    let acquired = configuration(FakeSource::new().with_extensions())
        .await
        .unwrap();
    assert_eq!(acquired.extensions.len(), 2);

    let alpha = &acquired.extensions[0];
    let beta = &acquired.extensions[1];
    assert_eq!(alpha.name, "Альфа");
    assert_eq!(beta.name, "Бета");
    assert_eq!(alpha.identity, "a".repeat(32));
    assert_eq!(beta.identity, "b".repeat(32));
    assert_ne!(alpha.identity, beta.identity);
    assert_eq!((alpha.order, beta.order), (1, 2));

    let names = |extension: &crate::pipeline::AcquiredExtension| {
        let mut names = extension
            .resources
            .iter()
            .map(|resource| resource.file_name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(names(alpha), ["alpha.0", "shared.0"]);
    assert_eq!(names(beta), ["beta.0", "shared.0"]);
}

#[tokio::test]
async fn never_merges_resources_two_extensions_name_alike() {
    let acquired = configuration(FakeSource::new().with_extensions())
        .await
        .unwrap();
    let shared = |extension: &crate::pipeline::AcquiredExtension| {
        extension
            .resources
            .iter()
            .filter(|resource| resource.file_name == "shared.0")
            .map(|resource| resource.compressed.clone())
            .collect::<Vec<_>>()
    };
    let alpha = shared(&acquired.extensions[0]);
    let beta = shared(&acquired.extensions[1]);
    assert_eq!(alpha.len(), 1);
    assert_eq!(beta.len(), 1);
    assert_eq!(&*alpha[0], b"shared, as alpha wrote it");
    assert_eq!(&*beta[0], b"shared, as beta wrote it");
}

#[tokio::test]
async fn keeps_the_activity_of_an_extension() {
    let acquired = configuration(FakeSource::new().with_extensions())
        .await
        .unwrap();
    assert_eq!(acquired.extensions[0].active, Some(true));
    assert_eq!(acquired.extensions[1].active, Some(false));
    // An inactive extension is answered with its resources all the same:
    // skipping it is the consumer's decision, not the reader's.
    assert!(!acquired.extensions[1].resources.is_empty());
}

#[tokio::test]
async fn reports_a_resource_the_store_does_not_carry() {
    let mut source = FakeSource::new().with_extensions();
    let missing = source.store_rows[0].0.clone();
    source.store_rows.retain(|(name, _, _)| *name != missing);
    let error = acquire_configuration(&mut source, &mut NoProgress)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains(&missing), "{error}");
    assert!(source.rolled_back && !source.committed);
}

#[tokio::test]
async fn a_metadata_read_asks_for_no_extension_catalog() {
    let mut source = FakeSource::new().with_extensions();
    let (_snapshot, _layout, _report) = acquire_metadata(&mut source, &mut NoProgress)
        .await
        .unwrap();
    // The catalog is only read by the whole-configuration plan; the
    // metadata read answers no extensions and retains no resource.
    let acquired = configuration(FakeSource::new().with_extensions())
        .await
        .unwrap();
    assert_eq!(acquired.extensions.len(), 2);
}

#[tokio::test]
async fn a_provider_without_the_whole_read_says_so() {
    struct Bare;

    impl MetadataSource for Bare {
        async fn begin_readonly(&mut self) -> Result<(), DbError> {
            Ok(())
        }
        async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
            Ok(StorageLayout::MODERN)
        }
        async fn read_db_names(&mut self, _layout: &StorageLayout) -> Result<DbNames, DbError> {
            parse_db_names(&hex(DB_NAMES)).map_err(DbError::from)
        }
        async fn read_config(
            &mut self,
            _layout: &StorageLayout,
            _progress: &mut dyn MetadataProgress,
        ) -> Result<ConfigMetadata, DbError> {
            Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()))
        }
        async fn read_schema(&mut self) -> Result<SchemaStorage, DbError> {
            Ok(SchemaStorage {
                tables: Vec::new(),
                anomalies: Vec::new(),
            })
        }
        async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
            Ok(Vec::new())
        }
        async fn commit_readonly(&mut self) -> Result<(), DbError> {
            Ok(())
        }
        async fn rollback_readonly(&mut self, original: DbError) -> DbError {
            original
        }
    }

    let error = acquire_configuration(&mut Bare, &mut NoProgress)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("cannot read the whole configuration"),
        "{error}"
    );
}
