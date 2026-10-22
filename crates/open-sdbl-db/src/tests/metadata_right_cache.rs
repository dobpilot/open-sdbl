#[path = "../../../../tests/support/metadata_right_native.rs"]
mod native;

use super::*;
use open_sdbl::metadata::resolve_metadata_right;

#[test]
fn unreadable_roles_cannot_be_hidden_by_another_grant() {
    let snapshot = native::snapshot();
    let mut store = AccessStore::new(&snapshot);
    let one: Guid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let two: Guid = "00000000-0000-0000-0000-000000000002".parse().unwrap();
    let role = native::cases()
        .into_iter()
        .find(|case| case.context == "standard-paths/standard-admin.oracle")
        .unwrap()
        .roles
        .remove(0);
    store.insert_rights(one.clone(), role);
    store.unreadable.push(two.clone());
    let target = resolve_metadata_right(&snapshot, &native::owner(), "Read", None).unwrap();
    assert!(
        store
            .metadata_right(&[one.clone(), two.clone()], &target)
            .is_err()
    );
    assert!(
        store
            .metadata_right_for_verification(&[one, two], &target)
            .is_err()
    );
}
