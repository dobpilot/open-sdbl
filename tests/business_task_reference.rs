mod support;
use open_sdbl::metadata::{
    ConfigDescriptor, Value, inflate_raw_deflate, parse_config_descriptors, parse_serialized,
};
use support::stored_deflate;
const BP: &str = "6db4bbe0-ce4c-4389-ad25-9a10c1857192";
const TASK: &str = "2adee501-a0c9-4849-9fd0-a3d10679d4f7";
const BP_TEXT: &str = include_str!(
    "fixtures/business-task-reference/6db4bbe0-ce4c-4389-ad25-9a10c1857192.native.config"
);
const TASK_TEXT: &str = include_str!(
    "fixtures/business-task-reference/2adee501-a0c9-4849-9fd0-a3d10679d4f7.native.config"
);
fn parse(id: &str, text: &str) -> Vec<ConfigDescriptor> {
    parse_config_descriptors(id, &stored_deflate(text.as_bytes())).unwrap()
}
fn owner<'a>(ds: &'a [ConfigDescriptor], id: &str) -> &'a ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == id).unwrap()
}
fn list(v: &Value) -> &[Value] {
    if let Value::List(v) = v {
        v
    } else {
        panic!("expected native list")
    }
}
#[test]
fn platform_compiled_references_match_source_xml_instead_of_object_types() {
    for (id, text, xml, kind, length) in [
        (
            BP,
            BP_TEXT,
            include_str!("fixtures/business-task-reference/ЗамерБизнесПроцесса.source.xml"),
            "30",
            49,
        ),
        (
            TASK,
            TASK_TEXT,
            include_str!("fixtures/business-task-reference/ЗамерЗадачи.source.xml"),
            "33",
            52,
        ),
    ] {
        let value = parse_serialized(text.as_bytes()).unwrap();
        let class = list(&list(&value)[1]);
        assert_eq!(class.len(), length);
        assert_eq!(class[0], Value::Atom(kind.into()));
        for section in xml.split("<xr:GeneratedType ").skip(1) {
            let category = section
                .split("category=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let pos = match category {
                "Object" => 3,
                "Ref" => 5,
                "Selection" => 7,
                "List" => 9,
                "Manager" => 11,
                "RoutePointRef" => 13,
                _ => panic!("unknown category"),
            };
            let type_id = section
                .split("<xr:TypeId>")
                .nth(1)
                .unwrap()
                .split('<')
                .next()
                .unwrap();
            let value_id = section
                .split("<xr:ValueId>")
                .nth(1)
                .unwrap()
                .split('<')
                .next()
                .unwrap();
            assert_eq!(class[pos], Value::Atom(type_id.into()));
            assert_eq!(class[pos + 1], Value::Atom(value_id.into()));
            if category == "Ref" {
                let ds = parse(id, text);
                assert_eq!(
                    owner(&ds, id)
                        .object_reference_type
                        .as_ref()
                        .unwrap()
                        .as_str(),
                    type_id
                );
            }
        }
    }
}
#[test]
fn all_eleven_demo_reference_targets_are_owned_by_the_right_descriptors() {
    let pack = include_bytes!("fixtures/demo/config.pack");
    let expected = include_str!("fixtures/business-task-reference/config-reference-mapping.tsv");
    let rows = expected
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 11);
    let mut offset = 0;
    let mut seen = 0;
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if let Some(row) = rows.iter().find(|row| row[0] == id) {
            let ds = parse_config_descriptors(id, &pack[offset..offset + n]).unwrap();
            let d = owner(&ds, id);
            assert_eq!(d.name, row[1]);
            assert_eq!(
                d.object_reference_type.as_ref().unwrap().as_str(),
                row[4],
                "{}",
                d.name
            );
            assert_ne!(row[4], row[5]);
            seen += 1;
        }
        offset += n;
    }
    assert_eq!(seen, 11);
}
#[test]
fn unknown_or_incomplete_header_first_forms_never_fall_back_to_object_guid() {
    for (id, text, object_type, selection_type) in [
        (
            BP,
            BP_TEXT,
            "3d8f31b8-65d2-4492-933e-1fd4c8a041f1",
            "2f1be866-984b-4692-8d9f-6a5403a7f9ce",
        ),
        (
            TASK,
            TASK_TEXT,
            "0108cf8d-8a15-4c82-98cc-d616a0d87d12",
            "a3f06f94-d6e9-47c2-9fd5-5637120eff2c",
        ),
    ] {
        for invalid in [
            text.replacen(
                &format!("}},1,{object_type}"),
                &format!("}},2,{object_type}"),
                1,
            ),
            text.replacen(selection_type, "not-a-guid", 1),
            text.replacen(&format!("{selection_type},"), "", 1),
            text.replacen("{1,", "{2,", 1),
            text.replacen(
                &format!("}},1,{object_type}"),
                &format!("}},,{object_type}"),
                1,
            ),
        ] {
            assert_ne!(invalid, text);
            let ds = parse(id, &invalid);
            assert!(owner(&ds, id).object_reference_type.is_none());
        }
        let wrong = text.replacen(id, "00000000-0000-0000-0000-000000000001", 1);
        assert!(
            parse(id, &wrong)
                .iter()
                .all(|d| d.object_reference_type.is_none())
        );
        assert!(
            parse_config_descriptors(id, &stored_deflate(&text.as_bytes()[..text.len() - 1]))
                .is_err()
        );
    }
}
#[test]
fn guid_prefix_class_33_retains_its_existing_projection() {
    let pack = include_bytes!("fixtures/demo/config.pack");
    let wanted = "0cdb1ab7-76eb-4729-9af4-418d0dc14920";
    let mut offset = 0;
    while offset < pack.len() {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, n) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let n: usize = n.parse().unwrap();
        offset = end + 1;
        if id == wanted {
            let bytes = &pack[offset..offset + n];
            let raw = inflate_raw_deflate(bytes).unwrap();
            let v = parse_serialized(&raw).unwrap();
            let c = list(&list(&v)[1]);
            assert_eq!(c[0], Value::Atom("33".into()));
            assert!(matches!(&c[1], Value::Atom(_)));
            let ds = parse_config_descriptors(id, bytes).unwrap();
            assert_eq!(
                owner(&ds, id)
                    .object_reference_type
                    .as_ref()
                    .unwrap()
                    .as_str(),
                "bd4adc16-6b29-46ce-a22f-e1e6bf10242d"
            );
            return;
        }
        offset += n;
    }
    panic!("original prefix fixture missing");
}

#[test]
fn independently_compiled_no_standard_commands_changes_only_marker_not_reference_identity() {
    for (id,original,raw,compressed,xml) in [
        (BP,BP_TEXT,include_bytes!("fixtures/business-task-reference/6db4bbe0-ce4c-4389-ad25-9a10c1857192.no-standard.native.config").as_slice(),include_bytes!("fixtures/business-task-reference/6db4bbe0-ce4c-4389-ad25-9a10c1857192.no-standard.native.deflate").as_slice(),include_str!("fixtures/business-task-reference/ЗамерБизнесПроцесса.no-standard.source.xml")),
        (TASK,TASK_TEXT,include_bytes!("fixtures/business-task-reference/2adee501-a0c9-4849-9fd0-a3d10679d4f7.no-standard.native.config").as_slice(),include_bytes!("fixtures/business-task-reference/2adee501-a0c9-4849-9fd0-a3d10679d4f7.no-standard.native.deflate").as_slice(),include_str!("fixtures/business-task-reference/ЗамерЗадачи.no-standard.source.xml")),
    ] {
        assert!(xml.contains("<UseStandardCommands>false</UseStandardCommands>"));
        let mut expected=parse_serialized(original.as_bytes()).unwrap();
        let Value::List(root)=&mut expected else {panic!("native root missing")};
        let Value::List(class)=&mut root[1] else {panic!("native class missing")};
        assert_eq!(class[2],Value::Atom("1".into()));class[2]=Value::Atom("0".into());
        assert_eq!(parse_serialized(raw).unwrap(),expected);
        assert_eq!(inflate_raw_deflate(compressed).unwrap(),raw);
        let ds=parse_config_descriptors(id,compressed).unwrap();
        let ref_id=xml.split("category=\"Ref\">").nth(1).unwrap().split("<xr:TypeId>").nth(1).unwrap().split('<').next().unwrap();
        assert_eq!(owner(&ds,id).object_reference_type.as_ref().unwrap().as_str(),ref_id);
    }
}
