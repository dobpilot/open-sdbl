//! Tests of the `pipeline` module.

use std::time::Duration;

use futures_util::stream;

use futures_util::StreamExt;

use super::{
    ConfigDecodeLimits, ConfigResource, assemble_parts, assemble_single_resource,
    decode_catalog_values, decode_config_stream,
};
use crate::limits::Limits;
use crate::progress::{MetadataProgress, NoProgress};

use crate::hex_test_support::hex;

/// A progress reporter that only counts, so the test can assert what the
/// decoder reported without drawing anything.
#[derive(Default)]
struct CountingProgress {
    completed_resources: u64,
    completed_bytes: u64,
}

impl MetadataProgress for CountingProgress {
    fn advance_config(&mut self, resources: usize, bytes: usize) {
        self.completed_resources = self.completed_resources.saturating_add(resources as u64);
        self.completed_bytes = self.completed_bytes.saturating_add(bytes as u64);
    }
}

async fn assembled(rows: Vec<(&str, i32, &[u8])>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let rows = rows
        .into_iter()
        .map(|(name, part, data)| Ok((name.to_owned(), part, data.to_vec())))
        .collect::<Vec<_>>();
    let mut resources = Vec::new();
    let mut stream = std::pin::pin!(assemble_parts(stream::iter(rows)));
    while let Some(resource) = stream.next().await {
        let resource = resource.map_err(|error| error.to_string())?;
        resources.push((resource.file_name, resource.compressed));
    }
    Ok(resources)
}

#[tokio::test]
async fn assembles_ordered_parts_into_whole_resources() {
    let resources = assembled(vec![
        ("a", 0, b"ab"),
        ("a", 1, b"cd"),
        ("a", 2, b"e"),
        ("b", 0, b"x"),
        ("c", 0, b""),
    ])
    .await
    .unwrap();
    assert_eq!(
        resources,
        [
            ("a".to_owned(), b"abcde".to_vec()),
            ("b".to_owned(), b"x".to_vec()),
            ("c".to_owned(), Vec::new()),
        ]
    );
    assert!(assembled(Vec::new()).await.unwrap().is_empty());
}

#[tokio::test]
async fn rejects_gaps_and_late_starts_in_part_sequences() {
    let gap = assembled(vec![("a", 0, b"1"), ("a", 2, b"3")])
        .await
        .unwrap_err();
    assert!(gap.contains("\"a\" part 2 arrived where part 1 was expected"));
    let late = assembled(vec![("a", 1, b"1")]).await.unwrap_err();
    assert!(late.contains("part 1 arrived where part 0 was expected"));

    assert_eq!(
        assemble_single_resource("DBNames", vec![(0, b"ab".to_vec()), (1, b"c".to_vec())]).unwrap(),
        b"abc"
    );
    assert!(assemble_single_resource("DBNames", Vec::new()).is_err());
    assert!(assemble_single_resource("DBNames", vec![(1, Vec::new())]).is_err());
}

#[test]
fn decodes_provider_neutral_catalog_rows() {
    let tables = decode_catalog_values(vec![
        ["T", "_Reference1", "", "", ""].map(str::to_owned),
        ["C", "_Reference1", "_IDRRef", "binary(16)", ""].map(str::to_owned),
        ["I", "_Reference1", "_Reference1_PK", "true", "_IDRRef"].map(str::to_owned),
    ])
    .unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].columns[0].name, "_IDRRef");
    assert_eq!(tables[0].columns[0].data_type, "binary(16)");
    assert!(tables[0].indexes[0].unique);
}

#[tokio::test]
async fn streamed_config_decoding_preserves_order_and_propagates_errors() {
    let compressed = hex(
        "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
    );
    let resources = futures_util::stream::iter([
        Ok(ConfigResource {
            file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
            compressed: compressed.clone(),
        }),
        Ok(ConfigResource {
            file_name: "25c96bd3-fac4-42ef-b695-74c9af43589b".to_owned(),
            compressed: compressed.clone(),
        }),
    ]);
    let mut progress = CountingProgress::default();
    progress.config_totals(2, (compressed.len() * 2) as u64);
    let (descriptors, predefined_values, _criteria, _roles) = decode_config_stream(
        resources,
        2,
        2,
        ConfigDecodeLimits::default(),
        &mut progress,
        Limits::default().query_timeout,
    )
    .await
    .unwrap();
    assert!(!descriptors.is_empty());
    assert!(predefined_values.is_empty());
    assert_eq!(
        descriptors.first().unwrap().resource_guid.as_str(),
        "25c96bd3-fac4-42ef-b695-74c9af43589b"
    );
    assert_eq!(
        descriptors.last().unwrap().resource_guid.as_str(),
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694"
    );
    assert_eq!(progress.completed_resources, 2);
    assert_eq!(progress.completed_bytes, (compressed.len() * 2) as u64);

    let invalid = futures_util::stream::iter([Ok(ConfigResource {
        file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
        compressed: b"not deflate".to_vec(),
    })]);
    let error = decode_config_stream(
        invalid,
        2,
        2,
        ConfigDecodeLimits::default(),
        &mut CountingProgress::default(),
        Limits::default().query_timeout,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("DEFLATE"));

    let limited = futures_util::stream::iter([Ok(ConfigResource {
        file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
        compressed: compressed.clone(),
    })]);
    let error = decode_config_stream(
        limited,
        1,
        1,
        ConfigDecodeLimits {
            resource_bytes: 1,
            batch_bytes: 2,
            total_bytes: 2,
            in_flight_bytes: 2,
        },
        &mut CountingProgress::default(),
        Limits::default().query_timeout,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("decoded metadata exceeds 1 byte limit")
    );

    let total_limited = futures_util::stream::iter([Ok(ConfigResource {
        file_name: "b8bac76b-c91b-4d78-8a70-ffa39f8de694".to_owned(),
        compressed,
    })]);
    let error = decode_config_stream(
        total_limited,
        1,
        1,
        ConfigDecodeLimits {
            resource_bytes: 1024 * 1024,
            batch_bytes: 1024 * 1024,
            total_bytes: 1,
            in_flight_bytes: 1024 * 1024,
        },
        &mut CountingProgress::default(),
        Limits::default().query_timeout,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Config total decoded size exceeds 1 byte")
    );
}

#[tokio::test]
async fn times_out_only_when_the_config_pipeline_stops_making_progress() {
    let error = decode_config_stream(
        stream::pending(),
        128,
        2,
        ConfigDecodeLimits::default(),
        &mut NoProgress,
        Duration::from_millis(1),
    )
    .await
    .unwrap_err();
    assert!(error.is_database_timeout());
    assert!(error.to_string().contains("Config stream progress"));
}
