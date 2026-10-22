use open_sdbl::metadata::{Guid, Right, parse_role_rights};

const ROOT: &str = "26c30ece-33b5-498a-b409-a91a046ba66b";

#[test]
fn named_root_rights_match_independently_measured_native_identifiers() {
    for (right, guid) in [
        (Right::SaveUserData, "d8682bbb-7800-4aa0-8590-d3cb11fe2a29"),
        (
            Right::ConfigurationExtensionsAdministration,
            "399d7390-8d83-4a57-b4d7-c902c15b701f",
        ),
        (
            Right::InteractiveOpenExtDataProcessors,
            "7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6",
        ),
        (
            Right::InteractiveOpenExtReports,
            "eb29e198-c338-4a20-a253-be6fc3dd44d9",
        ),
    ] {
        let guid: Guid = guid.parse().unwrap();
        assert_eq!(Right::from_guid(&guid), right);
        assert_eq!(right.guid(), guid);
        assert_eq!(Right::parse(&right.name()), Some(right.clone()));
        assert_eq!(Right::parse(&right.russian_name()), Some(right));
    }
    let unassigned = "4df6d046-3bf8-4dda-991c-53ba664296a5".parse().unwrap();
    assert_eq!(Right::from_guid(&unassigned), Right::Other(unassigned));
}

#[test]
fn single_right_role_grants_match_native_positive_and_cross_negative_checks() {
    let oracle = include_str!("fixtures/native-root-right-guids-1015/deny-external.oracle")
        .trim_start_matches('\u{feff}');
    let root: Guid = ROOT.parse().unwrap();
    let save = parse_role_rights(include_bytes!(
        "fixtures/native-root-right-guids-1015/RootSaveOnly.rights.deflate"
    ))
    .unwrap();
    let extensions = parse_role_rights(include_bytes!(
        "fixtures/native-root-right-guids-1015/RootExtensionsOnly.rights.deflate"
    ))
    .unwrap();
    let processor = parse_role_rights(include_bytes!(
        "fixtures/native-root-right-guids-1015/RootExternalProcessorOnly.rights.deflate"
    ))
    .unwrap();
    let report = parse_role_rights(include_bytes!(
        "fixtures/native-root-right-guids-1015/RootExternalReportOnly.rights.deflate"
    ))
    .unwrap();
    for (role, right, key) in [
        (&save, Right::SaveUserData, "right.role.save"),
        (
            &save,
            Right::ConfigurationExtensionsAdministration,
            "right.role.save.ext",
        ),
        (&extensions, Right::SaveUserData, "right.role.ext.save"),
        (
            &extensions,
            Right::ConfigurationExtensionsAdministration,
            "right.role.ext.ext",
        ),
        (&processor, Right::SaveUserData, "right.role.processor.save"),
        (
            &processor,
            Right::ConfigurationExtensionsAdministration,
            "right.role.processor.extensions",
        ),
        (
            &processor,
            Right::InteractiveOpenExtDataProcessors,
            "right.role.processor",
        ),
        (
            &report,
            Right::InteractiveOpenExtReports,
            "right.role.report",
        ),
        (
            &report,
            Right::InteractiveOpenExtDataProcessors,
            "right.role.report.processor",
        ),
    ] {
        let prefix = format!("{key}=");
        let result = oracle
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap()
            .trim();
        let expected = match result {
            "YES" => true,
            "NO" => false,
            _ => panic!("not a measured boolean: {key}={result}"),
        };
        assert_eq!(role.grants(&root, &right), expected, "{key}");
    }
}
