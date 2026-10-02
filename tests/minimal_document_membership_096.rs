use open_sdbl::metadata::{Guid, parse_config_descriptors};

#[test]
fn native_minimal_document_has_two_independent_sections_and_a_verified_empty_one() {
    let owner = "4dbedad1-342b-54c1-9bcf-1d79091376ef";
    let descriptors = parse_config_descriptors(
        owner,
        include_bytes!("fixtures/runtime-metadata-096/membership-minimal.deflate"),
    )
    .unwrap();
    let mut lists = Vec::new();
    for line in include_str!("fixtures/runtime-metadata-096/membership-minimal.tsv").lines() {
        let cells = line.split('\t').collect::<Vec<_>>();
        let d = descriptors
            .iter()
            .find(|d| d.object_guid.as_str() == cells[0])
            .unwrap();
        assert_eq!(d.name, cells[1]);
        let expected = cells[2]
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<Guid>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(d.tabular_section_attributes, Some(expected.clone()));
        for id in &expected {
            let attr = descriptors.iter().find(|d| &d.object_guid == id).unwrap();
            assert!(attr.declared_types.is_some());
            assert!(attr.fill_from_filling_value.is_some());
            assert!(attr.filling_value.is_some());
        }
        lists.push(expected);
    }
    assert_eq!(
        lists.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 2, 0]
    );
    assert!(lists[0].iter().all(|id| !lists[1].contains(id)));
    assert_eq!(
        descriptors
            .iter()
            .find(|d| d.object_guid.as_str() == owner)
            .unwrap()
            .document_registers,
        Some(Vec::new())
    );
}
