use super::*;
use crate::metadata::{parse_config_resource_bounded, parse_serialized};
const OWNER: &str = "6db4bbe0-ce4c-4389-ad25-9a10c1857192";
fn graph() -> Value {
    parse_serialized(include_bytes!(
        "../../tests/fixtures/native-098/route-points.config"
    ))
    .unwrap()
}
#[test]
fn native_graph_matches_every_xml_member_and_preserves_owner() {
    let owner: Guid = OWNER.parse().unwrap();
    let points = parse(&graph(), &owner).unwrap();
    let xml = include_str!("../../tests/fixtures/native-098/Flowchart.xml");
    assert_eq!(points.len(), xml.matches(" uuid=").count());
    assert_eq!(
        points.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        [
            "Действие1",
            "Действие2",
            "Условие1",
            "Старт",
            "Завершение",
            "ВыборВарианта1",
            "Разделение1",
            "Слияние1",
            "Обработка1",
            "ВложенныйПроцесс1"
        ]
    );
    for point in &points {
        assert_eq!(point.owner_guid, owner);
        assert_ne!(point.point_guid, owner);
        assert!(xml.contains(&format!("uuid=\"{}\"", point.point_guid)));
        assert!(xml.contains(&format!("<Name>{}</Name>", point.name)));
    }
    let other: Guid = "12345678-1234-4321-abcd-0123456789ab".parse().unwrap();
    let other_points = parse(&graph(), &other).unwrap();
    assert!(other_points.iter().all(|p| p.owner_guid == other));
    assert_eq!(points[0].point_guid, other_points[0].point_guid);
    assert_ne!(points[0].owner_guid, other_points[0].owner_guid);
}
#[test]
fn known_empty_and_complete_resource_keep_owner_separate_from_type() {
    let name = format!("{OWNER}.7");
    let empty = parse_config_resource_bounded(
        &name,
        include_bytes!("../../tests/fixtures/native-098/empty-route-points.deflate"),
        100000,
    )
    .unwrap();
    assert_eq!(empty.route_points.as_ref().unwrap().0.as_str(), OWNER);
    assert!(empty.descriptors.is_empty());
    let owner = parse_config_resource_bounded(
        OWNER,
        include_bytes!("../../tests/fixtures/native-098/business-process.deflate"),
        100000,
    )
    .unwrap();
    let d = owner
        .descriptors
        .iter()
        .find(|d| d.object_guid.as_str() == OWNER)
        .unwrap();
    assert_eq!(
        d.business_process_route_point_reference_type
            .as_ref()
            .unwrap()
            .as_str(),
        "bf5448cf-35b6-48b0-ba80-5b9ae9f506c5"
    );
    assert!(d.business_process_route_points.is_none());
}
#[test]
fn unknown_count_member_identity_and_kind_never_produce_partial_membership() {
    let owner = OWNER.parse().unwrap();
    for which in 0..4 {
        let mut value = graph();
        let Value::List(root) = &mut value else {
            panic!()
        };
        match which {
            0 => root[2] = Value::Atom("8".into()),
            1 => root[7] = Value::Atom("999".into()),
            2 => {
                let Value::List(body) = &mut root[8] else {
                    panic!()
                };
                let Value::List(header) = &mut body[0] else {
                    panic!()
                };
                header[2] = Value::Atom("bad-guid".into());
            }
            _ => root[0] = Value::Atom("999".into()),
        }
        assert!(parse(&value, &owner).is_none());
    }
}

#[test]
fn native_four_variant_switch_preserves_complete_consideration_membership() {
    let resource = parse_config_resource_bounded(
        "53d146b8-d85f-4cfe-b5ee-af78f4e09445.7",
        include_bytes!("../../tests/fixtures/native-098/demo-consideration-route-points.deflate"),
        100000,
    )
    .unwrap();
    let (owner, points) = resource.route_points.unwrap();
    assert_eq!(points.len(), 8);
    assert!(points.iter().all(|p| p.owner_guid == owner));
    assert_eq!(points[4].name, "ВыборВариантаРассмотрения");
    assert_eq!(points[7].name, "ВыборВариантаОбработкиРезолюции");
}
