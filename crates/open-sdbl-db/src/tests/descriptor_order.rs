use super::*;

#[allow(clippy::duplicate_mod)]
#[path = "../../../../tests/support/mod.rs"]
mod support;

#[test]
fn index_permutation_preserves_stable_descriptor_order_and_duplicate_ties() {
    let root = support::guid("11111111-1111-4111-8111-111111111111");
    let other = support::guid("22222222-2222-4222-8222-222222222222");
    let mut source = Vec::new();
    for index in 0..64 {
        let mut descriptor = support::descriptor(
            if index % 3 == 0 { &root } else { &other },
            if index % 2 == 0 { &root } else { &other },
            if index % 5 == 0 { "А" } else { "A" },
        );
        descriptor.comment = Some(index.to_string());
        source.push(descriptor);
    }
    for count in 0..=source.len() {
        let mut input = source[..count].to_vec();
        for _ in 0..count.max(1) {
            let mut expected = input.clone();
            expected.sort_by(|a, b| {
                a.resource_guid
                    .as_str()
                    .cmp(b.resource_guid.as_str())
                    .then_with(|| a.object_guid.as_str().cmp(b.object_guid.as_str()))
                    .then_with(|| a.name.cmp(&b.name))
            });
            let mut actual = input.clone();
            sort_descriptors(&mut actual);
            assert_eq!(actual, expected);
            input.rotate_left(usize::from(count > 0));
        }
    }
}
