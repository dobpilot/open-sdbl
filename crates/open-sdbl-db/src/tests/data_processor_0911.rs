//! Проверка независимости приобретённых метаданных от удержания исходных ресурсов.
use super::{ConfigResource, ConfigSelector, decode_selected_config};
use crate::{DbError, Limits, NoProgress};
struct RetainNone;
impl ConfigSelector for RetainNone {
    fn root_resource_name(&mut self, _: &ConfigResource) -> Result<String, DbError> {
        Ok("00000000-0000-0000-0000-000000000001".into())
    }
    fn configure(&mut self, _: &ConfigResource) -> Result<(), DbError> {
        Ok(())
    }
    fn retain(&mut self, _: &str) -> bool {
        false
    }
}
#[tokio::test]
async fn selected_configuration_preserves_processor_metadata_after_discarding_resource_bytes() {
    let bootstrap = [
        ConfigResource {
            file_name: "root".into(),
            compressed: std::sync::Arc::from([]),
        },
        ConfigResource {
            file_name: "configuration.bin".into(),
            compressed: std::sync::Arc::from([]),
        },
    ];
    let id = "ff92317d-71e7-4664-a13a-18bd31fd3701";
    let resources = futures_util::stream::iter([Ok(ConfigResource {
        file_name: id.into(),
        compressed: include_bytes!("../../../../tests/fixtures/data-processor-0911/Замеры.deflate")
            .as_slice()
            .into(),
    })]);
    let ((ds, _, _, _), retained) = decode_selected_config(
        resources,
        bootstrap,
        &mut RetainNone,
        Limits::default(),
        &mut NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().all(|r| r.file_name != id));
    let d = ds.iter().find(|d| d.object_guid.as_str() == id).unwrap();
    assert_eq!(d.data_processor_attributes.as_ref().unwrap().len(), 2);
    assert_eq!(d.data_processor_tabular_sections.as_ref().unwrap().len(), 3);
    assert_eq!(
        d.data_processor_object_type.as_ref().unwrap().as_str(),
        "7992a8c3-af89-4b9f-ba1a-836068499055"
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "ПустаяЧасть")
            .unwrap()
            .tabular_section_attributes,
        Some(vec![])
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "ДругаяЧасть")
            .unwrap()
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .len(),
        2
    );
}
#[tokio::test]
async fn selected_configuration_preserves_runtime_options_and_full_inventory_without_resource_bytes()
 {
    use open_sdbl::metadata::{DbNames, SchemaStorage, resolve_metadata};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/runtime-options-0912/settings");
    let bootstrap = [
        ConfigResource {
            file_name: "root".into(),
            compressed: std::sync::Arc::from([]),
        },
        ConfigResource {
            file_name: "configuration.bin".into(),
            compressed: std::sync::Arc::from([]),
        },
    ];
    let rows = std::fs::read_to_string(base.join("resources.txt"))
        .unwrap()
        .lines()
        .map(|id| {
            Ok(ConfigResource {
                file_name: id.into(),
                compressed: std::fs::read(base.join(format!("{id}.deflate")))
                    .unwrap()
                    .into(),
            })
        })
        .collect::<Vec<_>>();
    let ((ds, _, _, _), retained) = decode_selected_config(
        futures_util::stream::iter(rows),
        bootstrap,
        &mut RetainNone,
        Limits::default(),
        &mut NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(retained.len(), 2);
    let snap = resolve_metadata(
        DbNames::from_entries(vec![]),
        ds,
        SchemaStorage {
            tables: vec![],
            anomalies: vec![],
        },
        vec![],
    )
    .snapshot;
    let info = snap
        .descriptors()
        .iter()
        .find(|d| d.name == "ОпцииПробы")
        .unwrap();
    assert_eq!(
        info.information_register_dimensions.as_ref().unwrap().len(),
        2
    );
    assert_eq!(
        info.information_register_resources.as_ref().unwrap().len(),
        2
    );
    assert_eq!(
        snap.descriptors()
            .iter()
            .find(|d| d.common_attribute_usage.is_some())
            .unwrap()
            .common_attribute_content
            .as_ref()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(
        snap.descriptors()
            .iter()
            .find(|d| d.name == "ПробаДвухСвязей")
            .unwrap()
            .functional_option_parameter_use
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert!(
        snap.descriptors()
            .iter()
            .find(|d| d.configuration_functional_option_parameters.is_some())
            .unwrap()
            .configuration_functional_option_parameters
            .as_ref()
            .unwrap()
            .contains(
                &snap
                    .descriptors()
                    .iter()
                    .find(|d| d.name == "ПробаДвухСвязей")
                    .unwrap()
                    .object_guid
            )
    );
}
