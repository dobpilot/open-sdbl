#[path = "../../../tests/support/document_right_native.rs"]
mod native;

use open_sdbl::metadata::{Guid, MetadataRightApplicability};
use open_sdbl_db::AccessStore;

#[test]
fn replays_432_document_observations_through_existing_complete_role_cache() {
    let snapshot = native::snapshot();
    let role_id: Guid = "d0c7f3bf-7e25-5068-9d5a-35fa41923ea2".parse().unwrap();
    let missing: Guid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let cases = native::cases();
    assert_eq!(cases.len(), 432);
    for case in cases {
        let mut store = AccessStore::new(&snapshot);
        store.insert_rights(role_id.clone(), case.role.clone());
        let target = case.resolve(&snapshot).unwrap();
        let roles = std::slice::from_ref(&role_id);
        let outcome = if target.applicability() == MetadataRightApplicability::NotApplicable {
            assert!(store.metadata_right(roles, &target).is_err());
            assert!(
                store
                    .metadata_right_for_verification(roles, &target)
                    .is_err()
            );
            if case.operation == "verify" {
                "allowed"
            } else {
                "not_applicable"
            }
        } else {
            let granted = if case.operation == "verify" {
                store
                    .metadata_right_for_verification(roles, &target)
                    .unwrap()
            } else {
                store.metadata_right(roles, &target).unwrap()
            };
            assert!(
                store
                    .metadata_right(&[role_id.clone(), missing.clone()], &target)
                    .is_err()
            );
            assert!(
                store
                    .metadata_right_for_verification(&[role_id.clone(), missing.clone()], &target)
                    .is_err()
            );
            if case.privileged || granted {
                "allowed"
            } else {
                "denied"
            }
        };
        assert_eq!(
            outcome, case.outcome,
            "{} {} {} {}",
            case.context, case.target, case.right, case.operation
        );
    }
}
