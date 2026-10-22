#[path = "support/document_right_native.rs"]
mod native;

use open_sdbl::metadata::{
    ConfigDescriptor, DbNameEntry, DbNames, Guid, MetadataRightApplicability, MetadataRightError,
    MetadataSnapshot, Right, resolve_document_attribute_right, resolve_metadata,
    resolve_metadata_right,
};

#[test]
fn acquired_document_owner_read_is_applicable() {
    let resolved =
        resolve_metadata_right(&native::snapshot(), &native::owner(), "Read", None).unwrap();
    assert_eq!(
        resolved.applicability(),
        MetadataRightApplicability::Applicable
    );
}

#[test]
fn replays_all_432_retained_native_document_observations() {
    let snapshot = native::snapshot();
    let cases = native::cases();
    assert_eq!(cases.len(), 432);
    for case in cases {
        let target = case.resolve(&snapshot).unwrap();
        // Политика BSL-оператора и привилегий остаётся у вызывающей стороны.
        let outcome = if target.applicability() == MetadataRightApplicability::NotApplicable {
            assert_eq!(
                target.granted_by(&case.role),
                Err(MetadataRightError::NotApplicable)
            );
            assert_eq!(
                target.granted_for_verification(&case.role),
                Err(MetadataRightError::NotApplicable)
            );
            if case.operation == "verify" {
                "allowed"
            } else {
                "not_applicable"
            }
        } else {
            let granted = if case.operation == "verify" {
                target.granted_for_verification(&case.role).unwrap()
            } else {
                target.granted_by(&case.role).unwrap()
            };
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

fn changed_snapshot(change: impl FnOnce(&mut Vec<ConfigDescriptor>)) -> MetadataSnapshot {
    let source = native::snapshot();
    let mut descriptors = source.descriptors().to_vec();
    change(&mut descriptors);
    resolve_metadata(
        source.db_names().clone(),
        descriptors,
        source.schema().clone(),
        vec![],
    )
    .snapshot
}

#[test]
fn acquired_membership_and_child_description_are_both_required() {
    let source = native::snapshot();
    let owner = native::owner();
    let attribute = native::attribute();
    let foreign: Guid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    assert!(matches!(
        resolve_document_attribute_right(&source, &owner, &foreign, "Read"),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
    for snapshot in [
        changed_snapshot(|ds| {
            ds.iter_mut()
                .find(|d| d.object_guid == owner)
                .unwrap()
                .document_attributes = None
        }),
        changed_snapshot(|ds| {
            ds.iter_mut()
                .find(|d| d.object_guid == owner)
                .unwrap()
                .document_attributes = Some(vec![])
        }),
        changed_snapshot(|ds| ds.retain(|d| d.object_guid != attribute)),
        changed_snapshot(|ds| {
            ds.push(
                ds.iter()
                    .find(|d| d.object_guid == attribute)
                    .unwrap()
                    .clone(),
            )
        }),
        changed_snapshot(|ds| ds.push(ds.iter().find(|d| d.object_guid == owner).unwrap().clone())),
        changed_snapshot(|ds| ds.retain(|d| d.object_guid != owner)),
    ] {
        assert!(matches!(
            resolve_document_attribute_right(&snapshot, &owner, &attribute, "Read"),
            Err(MetadataRightError::UnsupportedTarget(_))
        ));
    }
    assert!(matches!(
        resolve_document_attribute_right(&source, &foreign, &attribute, "Read"),
        Err(MetadataRightError::UnknownOwner(_))
    ));
    let wrong_kind = resolve_metadata(
        DbNames::from_entries(vec![DbNameEntry {
            guid: owner.clone(),
            alias: "Reference".to_owned(),
            number: 1,
        }]),
        source.descriptors().to_vec(),
        source.schema().clone(),
        vec![],
    )
    .snapshot;
    assert!(matches!(
        resolve_document_attribute_right(&wrong_kind, &owner, &attribute, "View"),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
}

#[test]
fn unsupported_rights_paths_and_number_settings_are_not_guessed() {
    let source = native::snapshot();
    let owner = native::owner();
    for name in ["", "Code", "Description", "UnknownAttribute"] {
        assert!(matches!(
            resolve_metadata_right(&source, &owner, "Read", Some(name)),
            Err(MetadataRightError::InvalidStandardAttribute(_))
        ));
    }
    assert!(matches!(
        resolve_metadata_right(&source, &owner, "UnknownRight", None),
        Err(MetadataRightError::UnknownRight(_))
    ));
    assert!(matches!(
        resolve_document_attribute_right(&source, &owner, &native::attribute(), "UnknownRight"),
        Err(MetadataRightError::UnknownRight(_))
    ));
    assert!(matches!(
        resolve_document_attribute_right(&source, &owner, &native::attribute(), "Edit"),
        Err(MetadataRightError::UnsupportedTarget(_))
    ));
    for snapshot in [
        changed_snapshot(|ds| {
            ds.iter_mut()
                .find(|d| d.object_guid == owner)
                .unwrap()
                .document_number = None
        }),
        changed_snapshot(|ds| {
            ds.iter_mut()
                .find(|d| d.object_guid == owner)
                .unwrap()
                .document_number
                .as_mut()
                .unwrap()
                .length = 0
        }),
    ] {
        assert!(matches!(
            resolve_metadata_right(&snapshot, &owner, "Read", Some("Number")),
            Err(MetadataRightError::UnsupportedTarget(_))
        ));
    }
    for (ru, en) in [("нОМЕР", "nUMBER"), ("дАТА", "dATE")] {
        assert_eq!(
            resolve_metadata_right(&source, &owner, "пРоСмОтР", Some(ru)).unwrap(),
            resolve_metadata_right(&source, &owner, "vIEW", Some(en)).unwrap()
        );
    }
    assert_eq!(
        resolve_document_attribute_right(&source, &owner, &native::attribute(), "пРоСмОтР")
            .unwrap()
            .right(),
        &Right::View
    );
}

#[test]
fn independent_document_defaults_with_explicit_children_remain_unsupported() {
    let snapshot = native::snapshot();
    let cases = native::cases().into_iter().filter(|case| {
        case.context.ends_with("/independent-child")
            && !case.privileged
            && case.operation == "access"
            && case.right == "View"
            && case.target != "owner"
    });
    let mut count = 0;
    for mut case in cases {
        count += 1;
        let target = case.resolve(&snapshot).unwrap();
        case.role.set_for_attributes_by_default = true;
        assert!(!target.granted_by(&case.role).unwrap());
        assert!(matches!(
            target.granted_for_verification(&case.role),
            Err(MetadataRightError::UnsupportedTarget(_))
        ));
        for object in &mut case.role.objects {
            for right in &mut object.rights {
                if right.right == Right::View {
                    right.allowed = false;
                }
            }
        }
        assert!(matches!(
            target.granted_for_verification(&case.role),
            Err(MetadataRightError::UnsupportedTarget(_))
        ));
    }
    assert_eq!(count, 5);
}
