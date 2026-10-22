mod support;
use open_sdbl::metadata::{Guid, parse_config_descriptor, parse_config_header_bounded};

#[test]
fn own_headers_agree_with_native_config_corpora() {
    for base in ["demo", "buh", "unf"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(base)
            .join("config.pack");
        let pack = std::fs::read(path).unwrap();
        let mut offset = 0;
        let mut checked = 0;
        while offset < pack.len() && checked < 24 {
            let end = offset
                + pack[offset..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .unwrap();
            let header = std::str::from_utf8(&pack[offset..end]).unwrap();
            let (name, length) = header.split_once('\t').unwrap();
            let length: usize = length.parse().unwrap();
            let bytes = &pack[end + 1..end + 1 + length];
            offset = end + 1 + length;
            if name.parse::<Guid>().is_err() {
                continue;
            }
            let expected = parse_config_descriptor(name, bytes);
            if let Ok(expected) = expected {
                let actual = parse_config_header_bounded(name, bytes, 32 * 1024 * 1024).unwrap();
                assert_eq!(
                    actual
                        .as_ref()
                        .map(|own| (&own.object_guid, own.name.as_str())),
                    expected
                        .as_ref()
                        .map(|own| (&own.object_guid, own.name.as_str())),
                    "{base}: {name}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 12, "{base}: {checked}");
    }
}

#[test]
fn own_header_does_not_accept_a_nested_attribute_and_checks_the_rest() {
    let own = "11111111-1111-4111-8111-111111111111";
    let nested = "22222222-2222-4222-8222-222222222222";
    let body = format!("{{{{1,0,{nested}}},\"Child\",{{0}},{{{{1,0,{own}}},\"Own\",{{0}}}}}}");
    let compressed = support::stored_deflate(body.as_bytes());
    let header = parse_config_header_bounded(own, &compressed, 4096)
        .unwrap()
        .unwrap();
    assert_eq!(header.name, "Own");
    assert!(parse_config_header_bounded(own, &compressed, 2).is_err());
    let broken =
        support::stored_deflate(format!("{{{{1,0,{own}}},\"Own\",{{0}},{{unfinished").as_bytes());
    assert!(parse_config_header_bounded(own, &broken, 4096).is_err());
    assert!(
        parse_config_header_bounded(&format!("{own}.0"), &[0xff], 0)
            .unwrap()
            .is_none()
    );
}
