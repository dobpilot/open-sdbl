#[path = "../../../tests/support/metadata_right_native.rs"]
mod native;

use open_sdbl::metadata::{MetadataRightApplicability, MetadataRightError, resolve_metadata_right};
use open_sdbl_db::AccessStore;

#[test]
fn replays_native_matrix_through_complete_cached_access_store() {
    let snapshot = native::snapshot();
    let mut count = 0;
    for case in native::cases() {
        count += 1;
        let mut store = AccessStore::new(&snapshot);
        let roles = case
            .roles
            .iter()
            .enumerate()
            .map(|(index, rights)| {
                let id: open_sdbl::metadata::Guid = format!("00000000-0000-0000-0000-{index:012}")
                    .parse()
                    .unwrap();
                store.insert_rights(id.clone(), rights.clone());
                id
            })
            .collect::<Vec<_>>();
        let outcome = match resolve_metadata_right(
            &snapshot,
            &native::owner(),
            &case.right,
            case.standard.as_deref(),
        ) {
            Err(MetadataRightError::UnknownRight(_)) => "unknown_right",
            Err(MetadataRightError::InvalidStandardAttribute(_)) => "invalid_standard_path",
            Err(error) => panic!("{}: {error}", case.context),
            Ok(target) if target.applicability() == MetadataRightApplicability::NotApplicable => {
                assert!(store.metadata_right(&roles, &target).is_err());
                if case.method == "verify" {
                    "allowed"
                } else {
                    "not_applicable"
                }
            }
            Ok(target) => {
                if case.privileged
                    || (if case.method == "verify" {
                        store.metadata_right_for_verification(&roles, &target)
                    } else {
                        store.metadata_right(&roles, &target)
                    })
                    .unwrap()
                {
                    "allowed"
                } else {
                    "denied"
                }
            }
        };
        assert_eq!(
            outcome, case.outcome,
            "{} {} {:?}",
            case.context, case.right, case.standard
        );
    }
    assert_eq!(count, 492);
}

#[test]
fn missing_role_is_an_error_even_beside_a_complete_grant() {
    let snapshot = native::snapshot();
    let mut store = AccessStore::new(&snapshot);
    let granted = native::cases()
        .into_iter()
        .find(|case| case.context == "standard-paths/standard-admin.oracle")
        .unwrap()
        .roles
        .remove(0);
    let one: open_sdbl::metadata::Guid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let two: open_sdbl::metadata::Guid = "00000000-0000-0000-0000-000000000002".parse().unwrap();
    store.insert_rights(one.clone(), granted);
    let target = resolve_metadata_right(&snapshot, &native::owner(), "Read", None).unwrap();
    assert!(
        store
            .metadata_right(std::slice::from_ref(&one), &target)
            .unwrap()
    );
    assert!(store.metadata_right(&[one, two], &target).is_err());
    assert!(!store.metadata_right(&[], &target).unwrap());
}
