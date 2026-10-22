use super::*;
use crate::pipeline::{ConfigMetadata, ConfigSelector};
use crate::{AccessStore, MetadataProgress};
use open_sdbl::metadata::{
    DbNames, Guid, MetadataSnapshot, parse_config_descriptors, parse_role_rights, resolve_metadata,
    resolve_metadata_right,
};
use open_sdbl::query::{PostgresBackend, QueryCompiler};
#[allow(clippy::duplicate_mod)]
#[path = "../../../../tests/support/mod.rs"]
mod support;

struct DiscoverySource {
    snapshot: MetadataSnapshot,
    reads: [usize; 4],
    commits: usize,
}
impl DiscoverySource {
    fn new() -> Self {
        Self {
            snapshot: support::snapshot(),
            reads: [0; 4],
            commits: 0,
        }
    }
}
impl MetadataSource for DiscoverySource {
    async fn begin_readonly(&mut self) -> Result<(), DbError> {
        Ok(())
    }
    async fn detect_layout(&mut self) -> Result<StorageLayout, DbError> {
        self.reads[0] += 1;
        Ok(StorageLayout::MODERN)
    }
    async fn read_db_names(&mut self, _: &StorageLayout) -> Result<DbNames, DbError> {
        self.reads[1] += 1;
        Ok(self.snapshot.db_names().clone())
    }
    async fn read_schema(&mut self) -> Result<SchemaStorage, DbError> {
        self.reads[2] += 1;
        Ok(self.snapshot.schema().clone())
    }
    async fn read_live_tables(&mut self) -> Result<Vec<LiveTable>, DbError> {
        self.reads[3] += 1;
        Ok(self.snapshot.live_tables().to_vec())
    }
    async fn read_config(
        &mut self,
        _: &StorageLayout,
        _: &mut dyn MetadataProgress,
    ) -> Result<ConfigMetadata, DbError> {
        panic!("no Config body read")
    }
    async fn read_selected_config(
        &mut self,
        _: &StorageLayout,
        _: &mut dyn MetadataProgress,
        _: &mut dyn ConfigSelector,
    ) -> Result<(ConfigMetadata, Vec<ConfigResource>), DbError> {
        panic!("no hidden selected Config scan")
    }
    async fn read_extension_resources(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<ConfigResource>, DbError> {
        panic!("no ConfigCAS read")
    }
    async fn read_configuration_resource_names(
        &mut self,
        _: &StorageLayout,
    ) -> Result<Vec<String>, DbError> {
        Ok(vec!["root".into(), "own.0".into()])
    }
    async fn commit_readonly(&mut self) -> Result<(), DbError> {
        self.commits += 1;
        Ok(())
    }
    async fn rollback_readonly(&mut self, original: DbError) -> DbError {
        original
    }
}

#[tokio::test]
async fn physical_discovery_is_reused_without_config_scans_or_repeated_sql() {
    let mut source = DiscoverySource::new();
    let inputs = acquire_inputs(&mut source, None).await.unwrap();
    assert_eq!(source.reads, [1, 1, 1, 1]);
    let (_, names) = acquire_names(&mut source, Some(inputs.layout))
        .await
        .unwrap();
    assert_eq!(names, ["root", "own.0"]);
    assert_eq!(source.reads, [1, 1, 1, 1]);
    let one = resolve_configuration_resources(&inputs, &[], vec![]).unwrap();
    let two = resolve_configuration_resources(&inputs, &[], vec![]).unwrap();
    assert!(one.snapshot.objects().is_empty());
    assert!(two.snapshot.objects().is_empty());
    assert_eq!(source.reads, [1, 1, 1, 1]);
    assert_eq!(source.commits, 2);
}

#[tokio::test]
async fn filename_discovery_is_bounded_and_never_selects_binarydata() {
    use futures_util::stream;
    for mssql in [false, true] {
        let sql = names_sql(mssql).to_ascii_lowercase();
        assert!(sql.contains("distinct"));
        assert!(!sql.contains("binarydata"));
    }
    let limits = Limits {
        config_resource_limit: 1,
        ..Limits::default()
    };
    assert!(
        collect_names(stream::iter(vec![Ok("a".into()), Ok("b".into())]), limits)
            .await
            .is_err()
    );
    let limits = Limits {
        config_retained_byte_limit: 1,
        ..Limits::default()
    };
    assert!(
        collect_names(stream::iter(vec![Ok("long".into())]), limits)
            .await
            .is_err()
    );
    assert_eq!(
        collect_names(
            stream::iter(vec![Ok("b".into()), Ok("a".into())]),
            Limits::default()
        )
        .await
        .unwrap(),
        ["a", "b"]
    );
}

fn fixture_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}
fn separator_resources() -> Vec<ConfigResource> {
    std::fs::read_dir(fixture_root().join("separators/config"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            ConfigResource {
                file_name: path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_suffix(".deflate")
                    .unwrap()
                    .into(),
                compressed: std::fs::read(path).unwrap().into(),
            }
        })
        .collect()
}

#[test]
fn explicit_resolution_reports_machine_readable_separator_dependencies() {
    let root = fixture_root().join("separators");
    let full = resolve_metadata(
        open_sdbl::metadata::parse_db_names(&std::fs::read(root.join("db_names.deflate")).unwrap())
            .unwrap(),
        separator_resources()
            .iter()
            .flat_map(|resource| {
                parse_config_descriptors(&resource.file_name, &resource.compressed).unwrap()
            })
            .collect(),
        open_sdbl::metadata::parse_schema_storage(
            &std::fs::read(root.join("schema_storage.txt")).unwrap(),
        )
        .unwrap(),
        vec![],
    )
    .snapshot;
    let inputs = ConfigurationMetadataInputs {
        layout: StorageLayout::MODERN,
        db_names: full.db_names().clone(),
        schema: full.schema().clone(),
        live_tables: full.live_tables().to_vec(),
    };
    let owner = full
        .object_id(open_sdbl::metadata::MetadataKind::Catalog, "Товары")
        .unwrap();
    let all = separator_resources();
    let mut resources = all
        .iter()
        .filter(|resource| resource.file_name == owner.to_string())
        .cloned()
        .collect::<Vec<_>>();
    let missing = match resolve_configuration_resources(&inputs, &resources, vec![]).unwrap_err() {
        DbError::ConfigurationDescriptorsMissing { objects } => objects,
        error => panic!("{error}"),
    };
    assert!(
        missing
            .iter()
            .all(|guid| full.separators().any(|field| &field.guid == guid))
    );
    let mut unsupported = resources.clone();
    unsupported.push(ConfigResource {
        file_name: missing[0].to_string(),
        compressed: support::stored_deflate(b"{0}").into(),
    });
    assert!(matches!(
        resolve_configuration_resources(&inputs, &unsupported, vec![]),
        Err(DbError::Data(_))
    ));
    resources.extend(
        all.iter()
            .filter(|resource| {
                missing
                    .iter()
                    .any(|guid| resource.file_name == guid.as_str())
            })
            .cloned(),
    );
    let parameters = match resolve_configuration_resources(&inputs, &resources, vec![]).unwrap_err()
    {
        DbError::ConfigurationDescriptorsMissing { objects } => objects,
        error => panic!("{error}"),
    };
    assert!(!parameters.is_empty());
    resources.extend(
        all.iter()
            .filter(|resource| {
                parameters
                    .iter()
                    .any(|guid| resource.file_name == guid.as_str())
            })
            .cloned(),
    );
    let complete = resolve_configuration_resources(&inputs, &resources, vec![]).unwrap();
    assert_eq!(
        complete.snapshot.separators().count(),
        full.separators().count()
    );
    let malformed = ConfigResource {
        file_name: owner.to_string(),
        compressed: vec![0xff].into(),
    };
    assert!(resolve_configuration_resources(&inputs, &[malformed], vec![]).is_err());
}

#[test]
fn role_only_access_cache_survives_growing_and_reordered_query_snapshots() {
    let role: Guid = "76702e9e-fa7a-4b98-befa-f9b37db2dae0".parse().unwrap();
    let bytes = std::fs::read(fixture_root().join(format!("access/unf_{role}.deflate"))).unwrap();
    let role_descriptors = parse_config_descriptors(role.as_str(), &bytes).unwrap();
    let mut roles = resolve_metadata(
        DbNames::from_entries(vec![]),
        role_descriptors,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    roles.attach_roles(vec![role.clone()]);
    let mut store = AccessStore::new(&roles);
    let users = std::fs::read_to_string(fixture_root().join("access/unf_v8users.tsv")).unwrap();
    let user_row = users.lines().nth(2).unwrap().split('|').collect::<Vec<_>>();
    let user = open_sdbl::metadata::InfoBaseUser::new(
        open_sdbl::metadata::UserRow {
            name: user_row[0],
            description: user_row[1],
            os_name: user_row[2],
            email: "",
            show_in_list: user_row[4] == "t",
            standard_authentication: user_row[5] == "true",
            administrative: user_row[6] == "true",
        },
        &support::hex(user_row[7]),
    )
    .unwrap();
    store.set_users(vec![user]);
    let full = support::reference_snapshot();
    let owner = full
        .object_id(
            open_sdbl::metadata::MetadataKind::Catalog,
            "OpenSdblMetadataProbe",
        )
        .unwrap();
    let guid: Guid = owner.to_string().parse().unwrap();
    // Перепривязка измеренного профиля проверяет идентичность кеша SDK,
    // а не объявляет результат новым платформенным замером прав.
    let native =
        std::fs::read(fixture_root().join("metadata-right-native-0922/RootAllow.rights.deflate"))
            .unwrap();
    let text = String::from_utf8(open_sdbl::metadata::inflate_raw_deflate(&native).unwrap())
        .unwrap()
        .replace("1316b080-3b2c-501b-ace3-a1234a3fedd1", guid.as_str());
    store.insert_rights(
        role.clone(),
        parse_role_rights(&support::stored_deflate(text.as_bytes())).unwrap(),
    );
    let first = resolve_selected_metadata(
        full.db_names(),
        full.descriptors()
            .iter()
            .filter(|descriptor| descriptor.resource_guid == guid)
            .cloned()
            .collect(),
        vec![],
        vec![],
        full.schema(),
        full.live_tables(),
    )
    .snapshot;
    let mut entries = full.db_names().entries().to_vec();
    entries.reverse();
    let reordered = DbNames::from_entries(entries);
    let second = resolve_selected_metadata(
        &reordered,
        full.descriptors().to_vec(),
        vec![],
        vec![],
        full.schema(),
        full.live_tables(),
    )
    .snapshot;
    assert!(second.objects().len() > first.objects().len());
    assert_eq!(
        first
            .object_id(
                open_sdbl::metadata::MetadataKind::Catalog,
                "OpenSdblMetadataProbe"
            )
            .unwrap(),
        second
            .object_id(
                open_sdbl::metadata::MetadataKind::Catalog,
                "OpenSdblMetadataProbe"
            )
            .unwrap()
    );
    assert_eq!(
        first.field_id(owner, "Организация").unwrap(),
        second.field_id(owner, "Организация").unwrap()
    );
    for snapshot in [&first, &second] {
        let target = resolve_metadata_right(snapshot, &guid, "Read", None).unwrap();
        assert!(
            store
                .metadata_right(std::slice::from_ref(&role), &target)
                .unwrap()
        );
        assert_eq!(store.rights_of(std::slice::from_ref(&role)).len(), 1);
        assert_eq!(store.users().unwrap()[0].name, user_row[0]);
    }
    let query = "SELECT Code FROM Catalog.OpenSdblMetadataProbe";
    assert_eq!(
        QueryCompiler::new(&first, PostgresBackend)
            .compile(query)
            .unwrap()
            .sql,
        QueryCompiler::new(&second, PostgresBackend)
            .compile(query)
            .unwrap()
            .sql
    );
    let prepared = QueryCompiler::new(&first, PostgresBackend)
        .prepare(query)
        .unwrap();
    assert!(prepared.compile(&second, &[]).is_err());
    assert!(prepared.compile(&first, &[]).is_ok());
}
