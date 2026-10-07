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
async fn selected_configuration_preserves_process_and_register_metadata_even_when_bytes_are_discarded()
 {
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
    let bp = "6db4bbe0-ce4c-4389-ad25-9a10c1857192";
    let register = "c72d739a-05e3-4e9e-bcc7-3bf8baf5dc63";
    let resources = futures_util::stream::iter([
        Ok(ConfigResource {
            file_name: bp.into(),
            compressed: include_bytes!(
                "../../../../tests/fixtures/runtime-object-099/tabular-section.native.deflate"
            )
            .as_slice()
            .into(),
        }),
        Ok(ConfigResource {
            file_name: register.into(),
            compressed: include_bytes!(
                "../../../../tests/fixtures/runtime-object-099/ОстаткиПорядок.deflate"
            )
            .as_slice()
            .into(),
        }),
    ]);
    let ((descriptors, _, _, _), retained) = decode_selected_config(
        resources,
        bootstrap,
        &mut RetainNone,
        Limits::default(),
        &mut NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(retained.len(), 2);
    let process = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == bp)
        .unwrap();
    assert_eq!(process.business_process_number.unwrap().length, 11);
    assert_eq!(
        process.business_process_attributes.as_ref().unwrap().len(),
        2
    );
    assert_eq!(
        process
            .business_process_tabular_sections
            .as_ref()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        process
            .business_process_standard_fill_checking
            .as_ref()
            .unwrap()
            .len(),
        7
    );
    let register = descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == register)
        .unwrap();
    assert_eq!(
        register
            .accumulation_register_dimensions
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        register
            .accumulation_register_resources
            .as_ref()
            .unwrap()
            .len(),
        2
    );
}
