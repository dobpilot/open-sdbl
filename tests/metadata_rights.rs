#[path = "support/metadata_right_native.rs"]
mod native;

use open_sdbl::metadata::{
    MetadataRightApplicability, MetadataRightError, Right, resolve_metadata, resolve_metadata_right,
};

#[test]
fn replays_all_492_native_observations_without_combining_role_fragments() {
    let snapshot = native::snapshot();
    let cases = native::cases();
    assert_eq!(cases.len(), 492);
    let fallback_role = cases
        .iter()
        .flat_map(|case| &case.roles)
        .next()
        .unwrap()
        .clone();
    let mut mismatches = Vec::new();
    for case in cases {
        let target = resolve_metadata_right(
            &snapshot,
            &native::owner(),
            &case.right,
            case.standard.as_deref(),
        );
        // Только проверочный потребитель выбирает политику оператора и привилегий.
        let outcome = match target {
            Err(MetadataRightError::UnknownRight(_)) => "unknown_right",
            Err(MetadataRightError::InvalidStandardAttribute(_)) => "invalid_standard_path",
            Err(error) => panic!("{}: {error}", case.context),
            Ok(target) if target.applicability() == MetadataRightApplicability::NotApplicable => {
                assert_eq!(
                    target.granted_by(
                        &case
                            .roles
                            .first()
                            .cloned()
                            .unwrap_or_else(|| fallback_role.clone())
                    ),
                    Err(MetadataRightError::NotApplicable)
                );
                if case.method == "verify" {
                    "allowed"
                } else {
                    "not_applicable"
                }
            }
            Ok(target) => {
                if case.privileged
                    || case.roles.iter().any(|role| {
                        if case.method == "verify" {
                            target.granted_for_verification(role).unwrap()
                        } else {
                            target.granted_by(role).unwrap()
                        }
                    })
                {
                    "allowed"
                } else {
                    "denied"
                }
            }
        };
        if outcome != case.outcome {
            mismatches.push(format!(
                "{} {} {} {:?}: got {outcome}, native {}",
                case.context, case.method, case.right, case.standard, case.outcome
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn rejects_missing_owners_settings_and_unmeasured_rights_without_grant_inference() {
    let snapshot = native::snapshot();
    let owner = native::owner();
    assert!(matches!(
        resolve_metadata_right(
            &snapshot,
            &"00000000-0000-0000-0000-000000000000".parse().unwrap(),
            "Read",
            None
        ),
        Err(MetadataRightError::UnknownOwner(_))
    ));
    assert!(matches!(
        resolve_metadata_right(&snapshot, &owner, "Edit", Some("Code")),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
    assert_eq!(
        resolve_metadata_right(&snapshot, &owner, "пРоСмОтР", Some("кОд"))
            .unwrap()
            .right(),
        &Right::View
    );
    let mut descriptors = snapshot.descriptors().to_vec();
    for descriptor in &mut descriptors {
        descriptor.catalog_standard_attributes = None;
    }
    let missing = resolve_metadata(
        snapshot.db_names().clone(),
        descriptors,
        snapshot.schema().clone(),
        vec![],
    )
    .snapshot;
    assert!(matches!(
        resolve_metadata_right(&missing, &owner, "Read", Some("Code")),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
}

#[test]
fn independent_attribute_defaults_without_parent_remain_explicitly_unsupported() {
    let snapshot = native::snapshot();
    let mut role = native::cases()
        .into_iter()
        .find(|case| case.context.contains("independent-owner-no-code-yes"))
        .unwrap()
        .roles
        .remove(0);
    role.set_for_attributes_by_default = true;
    let target = resolve_metadata_right(&snapshot, &native::owner(), "View", Some("Code")).unwrap();
    assert!(!target.granted_by(&role).unwrap());
    assert!(matches!(
        target.granted_for_verification(&role),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
}

#[test]
fn zero_length_standard_settings_are_not_inferred_as_applicable_or_nonexistent() {
    let snapshot = native::snapshot();
    let mut descriptors = snapshot.descriptors().to_vec();
    for descriptor in &mut descriptors {
        if let Some(settings) = &mut descriptor.catalog_standard_attributes {
            settings.code_length = 0;
        }
    }
    let zero = resolve_metadata(
        snapshot.db_names().clone(),
        descriptors,
        snapshot.schema().clone(),
        vec![],
    )
    .snapshot;
    assert!(matches!(
        resolve_metadata_right(&zero, &native::owner(), "Read", Some("Code")),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
}
