//! Проекция назначения из общей acquisition-ветки обоих SQL-провайдеров.

use super::configuration_tests::FakeSource;
use super::{AcquiredExtension, ExtensionCatalogRow, acquire_configuration};
use crate::hex_test_support::hex;
use crate::progress::NoProgress;
use open_sdbl::metadata::ConfigurationExtensionPurpose;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/extension/purpose-0915")
}

fn native_source() -> FakeSource {
    let mut source = FakeSource::new();
    source.extensions = std::fs::read_to_string(fixtures().join("rows.tsv"))
        .unwrap()
        .lines()
        .enumerate()
        .map(|(order, line)| {
            let parts = line.split('|').collect::<Vec<_>>();
            ExtensionCatalogRow {
                identity: vec![0; 16],
                order: order as i32,
                name: parts[0].to_owned(),
                scope: Some(parts[1].parse().unwrap()),
                used_in_distributed_infobase: Some(0),
                info: hex(parts[2]),
            }
        })
        .collect();
    source.store_rows = std::fs::read_dir(fixtures().join("cas"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                0,
                std::fs::read(&path).unwrap(),
            )
        })
        .collect();
    source
}

fn read(mut source: FakeSource) -> Vec<AcquiredExtension> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(acquire_configuration(&mut source, &mut NoProgress))
        .unwrap()
        .extensions
}

#[test]
fn shared_acquisition_preserves_all_three_native_purposes() {
    let extensions = read(native_source());
    for (name, purpose) in [
        ("Probe0915Patch", ConfigurationExtensionPurpose::Patch),
        (
            "Probe0915Customization",
            ConfigurationExtensionPurpose::Customization,
        ),
        ("Probe0915Addon", ConfigurationExtensionPurpose::AddOn),
    ] {
        let extension = extensions.iter().find(|e| e.name == name).unwrap();
        assert_eq!(extension.purpose, Some(purpose));
        assert_eq!(
            extension.catalog.as_ref().unwrap().purpose,
            extension.purpose
        );
    }
}

#[test]
fn renamed_inactive_and_unknown_scope_do_not_determine_purpose() {
    let mut source = native_source();
    let patch = source
        .extensions
        .iter_mut()
        .find(|e| e.name == "Probe0915Patch")
        .unwrap();
    patch.name = "NoSpecialPrefix".to_owned();
    patch.scope = Some(900);
    let flag = patch.info.len() - 3;
    patch.info[flag] = 0x81;
    let extensions = read(source);
    let patch = extensions
        .iter()
        .find(|e| e.name == "NoSpecialPrefix")
        .unwrap();
    assert_eq!(patch.purpose, Some(ConfigurationExtensionPurpose::Patch));
    assert_eq!(patch.active, Some(false));
    assert_eq!(patch.scope, Some(900));
    assert_eq!(patch.scope_kind, None);
}

#[test]
fn damaged_configuration_root_keeps_other_acquired_fields_and_other_extensions() {
    let mut source = native_source();
    let root = source
        .store_rows
        .iter_mut()
        .find(|r| r.0 == "7a2b6f1ab95361dda2de730a1b415edfa4feddf3")
        .unwrap();
    root.2 = b"broken stream".to_vec();
    let extensions = read(source);
    let patch = extensions
        .iter()
        .find(|e| e.name == "Probe0915Patch")
        .unwrap();
    assert_eq!(patch.purpose, None);
    assert!(patch.catalog.is_none());
    assert_eq!(patch.active, Some(true));
    assert_eq!(patch.safe_mode, Some(true));
    assert!(patch.hash_sum.is_some());
    assert_eq!(extensions.iter().filter(|e| e.purpose.is_some()).count(), 2);
}

#[test]
fn older_compatibility_generations_use_the_same_shared_acquisition() {
    for compatibility in ["80324", "80317"] {
        let mut source = native_source();
        let rows =
            std::fs::read_to_string(fixtures().join(format!("rows-{compatibility}.tsv"))).unwrap();
        for (row, line) in source.extensions.iter_mut().zip(rows.lines()) {
            let fields = line.split('|').collect::<Vec<_>>();
            assert_eq!(row.name, fields[0]);
            row.info = hex(fields[2]);
        }
        let extensions = read(source);
        assert_eq!(
            extensions.iter().map(|e| e.purpose).collect::<Vec<_>>(),
            vec![
                Some(ConfigurationExtensionPurpose::AddOn),
                Some(ConfigurationExtensionPurpose::Customization),
                Some(ConfigurationExtensionPurpose::Patch)
            ]
        );
    }
}
