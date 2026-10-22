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
async fn selected_configuration_preserves_report_metadata_after_discarding_resource_bytes() {
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
    let id = "8bac013d-e4dc-43c2-980d-6190972b6678";
    let resources = futures_util::stream::iter([Ok(ConfigResource {
        file_name: id.into(),
        compressed: include_bytes!("../../../../tests/fixtures/report-object/ЗамерОтчета.deflate")
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
    assert_eq!(d.report_attributes.as_ref().unwrap().len(), 2);
    assert_eq!(d.report_tabular_sections.as_ref().unwrap().len(), 1);
    assert_eq!(
        d.report_object_type.as_ref().unwrap().as_str(),
        "1ece1a7b-59c8-4f1a-9c70-5577fbae18f4"
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.name == "СтрокиПробы")
            .unwrap()
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .len(),
        2
    );
}
