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
