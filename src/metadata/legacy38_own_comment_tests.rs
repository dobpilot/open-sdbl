use super::super::parse_config_descriptors;
use super::*;
const ID: &str = "c63f584a-d27d-42f2-b656-79f7a64d9e93";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-controlled-transactions/owner.deflate");
fn scalar(v: &Value) -> &str {
    match v {
        Value::String(s) | Value::Atom(s) => s,
        _ => panic!("scalar"),
    }
}
#[test]
fn native_nine_own_one_section_eighteen_fields_match_thirty_two_xml_types() {
    let raw = RAW;
    let ds = parse_config_descriptors("c63f584a-d27d-42f2-b656-79f7a64d9e93", raw).unwrap();
    type XmlType = (&'static str, &'static str, &'static str);
    type XmlAttribute = (&'static str, &'static str, &'static [XmlType]);
    let expected: &[XmlAttribute] = &[
        (
            "c6647ff5-c5ae-48dd-8383-7c3d0bcda7dd",
            "УведомлениеОКонтролируемойСделке",
            &[("#", "67e952d8-c694-4237-98d4-948020588410", "")],
        ),
        (
            "d4ec55fa-826a-44af-ab34-ac13188cbca0",
            "Организация",
            &[("#", "c92a1124-c41e-42a4-bda7-3bbac9c6039e", "")],
        ),
        (
            "4774bf52-5125-4528-90f0-7e94ba03e36a",
            "Контрагент",
            &[("#", "9f6206b2-1ed6-423c-9b08-fd4978930c49", "")],
        ),
        (
            "35059bda-e949-460f-b48e-bdbb57049d95",
            "ДоговорКонтрагента",
            &[("#", "376807bc-0d88-4c06-9eb2-42b72b970afb", "")],
        ),
        (
            "0a7240d8-3dc9-429c-882f-c4bd97e81fe0",
            "СделкаСовершенаЧерезКомиссионера",
            &[("B", "", "")],
        ),
        (
            "408b64f2-69a6-4169-8e71-072c27ab6ac7",
            "Комиссионер",
            &[("#", "9f6206b2-1ed6-423c-9b08-fd4978930c49", "")],
        ),
        (
            "97dd3c18-828d-4463-86d9-05a9e6df9dd6",
            "ВалютаДокумента",
            &[("#", "655fd168-ab6a-4863-a2d5-401497aceec3", "")],
        ),
        (
            "65a26459-6333-478b-ac01-ce07b97823cc",
            "СуммаДокумента",
            &[("N", "", "15,2,0")],
        ),
        (
            "576eb7ad-702b-4f05-81a4-841a2e5b172c",
            "Комментарий",
            &[("S", "", "")],
        ),
        (
            "434011ad-e1e8-4d70-bb2f-0cc01a606cd3",
            "ДатаСовершенияСделки",
            &[("D", "", "D")],
        ),
        (
            "899bc343-3cfd-461b-aeed-b3fdb056efc0",
            "ПредметСделки",
            &[
                ("#", "1433c81a-03ff-4319-91c9-379dad7ac0e9", ""),
                ("#", "333415fd-35e0-47f1-88cc-720e5ebdb593", ""),
                ("#", "8f1c87a0-7591-43e3-bbdb-d91040748838", ""),
                ("#", "e18d5670-dd2f-4528-8897-d3f07ca77f21", ""),
            ],
        ),
        (
            "626b7f57-06e5-4083-be80-2077614eba1b",
            "НаименованиеПредметаСделки",
            &[("S", "", "255,1")],
        ),
        (
            "bda0cd26-ef08-4823-a94e-04e6db8428c4",
            "Количество",
            &[("N", "", "10,3,0")],
        ),
        (
            "b0a3ddae-c78e-45b7-8c6d-fb37db1ab84a",
            "ЕдиницаИзмерения",
            &[("#", "a6be8b19-1f08-4c5f-8636-0dd052c33b35", "")],
        ),
        (
            "a040b4ec-e707-4cec-a4a1-00209147fc48",
            "СуммаБезНДСВРублях",
            &[("N", "", "15,2,0")],
        ),
        (
            "6996a549-46fa-41f0-a393-d13df08ca41f",
            "СуммаБезНДСВВалютеРасчетов",
            &[("N", "", "15,2,0")],
        ),
        (
            "5073cb0f-1a47-4475-b5c4-bc2ab0706e90",
            "СтавкаНДС",
            &[("#", "d3a0be91-8172-4fd4-9de8-6c72c72127b1", "")],
        ),
        (
            "929ed2ba-4643-4163-8eea-a825f58ecd2c",
            "СуммаНДСВРублях",
            &[("N", "", "15,2,0")],
        ),
        (
            "5ada169d-a8c3-4b73-8526-8f2c63aaeac8",
            "СуммаНДСВВалютеРасчетов",
            &[("N", "", "15,2,0")],
        ),
        (
            "91039791-0d30-42d0-92ed-dc7f62fc5269",
            "ПроцентнаяСтавка",
            &[("N", "", "7,4,0")],
        ),
        (
            "e8bd452f-815a-4828-8d67-d4b5dbda2582",
            "ДатаПроцентнойСтавки",
            &[("D", "", "D")],
        ),
        (
            "a3d11594-1a44-48ea-854f-87a0e40377e5",
            "ТипПредметаСделки",
            &[("#", "93a89ea9-16e8-4820-8758-2bebaa734e62", "")],
        ),
        (
            "4c6ebb66-c808-4e45-a314-4348c064d3d3",
            "ТипКонтролируемойСделки",
            &[("#", "96c62fa5-08d3-4c14-a5ac-e87047736dbd", "")],
        ),
        (
            "99db388e-de0f-405f-ad04-e139126158b3",
            "СтранаПроисхожденияПредметаСделки",
            &[("#", "4ee4f6c6-683e-4abc-8890-ed6cde5a53cb", "")],
        ),
        (
            "4cd9e5ba-e9f4-44d0-869d-682bae1294c8",
            "Грузоотправитель",
            &[
                ("#", "9f6206b2-1ed6-423c-9b08-fd4978930c49", ""),
                ("#", "c92a1124-c41e-42a4-bda7-3bbac9c6039e", ""),
            ],
        ),
        (
            "d88232e3-e7cc-4bcb-a79a-e9c74ffccf03",
            "Грузополучатель",
            &[
                ("#", "9f6206b2-1ed6-423c-9b08-fd4978930c49", ""),
                ("#", "c92a1124-c41e-42a4-bda7-3bbac9c6039e", ""),
            ],
        ),
        (
            "8eda192d-8887-4edb-81c7-6669a5a5f54a",
            "ОперацияОблагаетсяЕНВД",
            &[("B", "", "")],
        ),
    ];
    let mut n = 0;
    for (gid, name, types) in expected {
        let d = ds.iter().find(|d| d.object_guid.as_str() == *gid).unwrap();
        assert_eq!(&d.name, name);
        let ts = d.declared_types.as_ref().unwrap();
        assert_eq!(ts.len(), types.len());
        for (ty, (tag, r, q)) in ts.iter().zip(*types) {
            assert_eq!(&ty.tag, tag);
            assert_eq!(ty.reference.as_ref().map(|g| g.as_str()).unwrap_or(""), *r);
            assert_eq!(
                ty.qualifiers
                    .iter()
                    .map(scalar)
                    .collect::<Vec<_>>()
                    .join(","),
                *q
            );
            n += 1;
        }
    }
    let o = ds
        .iter()
        .find(|d| d.object_guid.as_str() == "c63f584a-d27d-42f2-b656-79f7a64d9e93")
        .unwrap();
    let ids = o.document_attributes.as_ref().unwrap();
    assert_eq!(
        ids.iter().map(|g| g.as_str()).collect::<Vec<_>>(),
        expected[..9].iter().map(|e| e.0).collect::<Vec<_>>()
    );
    let parts = o.document_tabular_sections.as_ref().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].as_str(), "05bc3681-dde2-412e-9ed0-e8069d1cf55b");
    let section = ds.iter().find(|d| d.object_guid == parts[0]).unwrap();
    assert_eq!(
        section
            .tabular_section_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(|g| g.as_str())
            .collect::<Vec<_>>(),
        expected[9..].iter().map(|e| e.0).collect::<Vec<_>>()
    );
    assert_eq!(n, 32);
    println!(
        "PASS causal exact 9 own + 1 section/18 fields, 32 XML types/references/qualifiers and ordered UUID/name membership"
    );
}

fn vals(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается список"),
    }
}
fn source() -> Value {
    super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap()
}
fn own_rows(v: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(v)[5])
}
fn own_identity(row: &mut Value) -> &mut Vec<Value> {
    vals(&mut vals(&mut vals(&mut vals(&mut vals(row)[0])[1])[1])[1])
}

fn mutate(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let mut v = source();
    let mut ds = parse_config_descriptors(ID, RAW).unwrap();
    for d in &mut ds {
        d.declared_types = None;
        d.document_attributes = None;
        d.document_tabular_sections = None;
        d.tabular_section_attributes = None;
    }
    edit(&mut v, &mut ds);
    project(&v, &ID.parse().unwrap(), &mut ds);
    ds
}
#[test]
fn malformed_comments_and_late_own_rows_publish_no_prefix() {
    for mode in 0..11 {
        let ds = mutate(|v, ds| match mode {
            0 => own_identity(own_rows(v).last_mut().unwrap())[4] = Value::Atom(String::new()),
            1 => own_identity(own_rows(v).last_mut().unwrap())[4] = Value::Atom("text".into()),
            2 => own_identity(own_rows(v).last_mut().unwrap())[4] = Value::Null,
            3 => own_identity(own_rows(v).last_mut().unwrap())[4] = Value::List(vec![]),
            4 => own_rows(v)[1] = Value::Atom("8".into()),
            5 => {
                let row = own_rows(v).last_mut().unwrap();
                vals(&mut vals(&mut vals(row)[0])[1])[1] = Value::Null;
            }
            6 => {
                let id = own_identity(&mut own_rows(v)[2])[1].clone();
                own_identity(own_rows(v).last_mut().unwrap())[1] = id;
            }
            7 => {
                let gid = own_identity(own_rows(v).last_mut().unwrap())[1].clone();
                let id = guid(&list(&gid).unwrap()[2]).unwrap();
                ds.retain(|d| d.object_guid != id);
            }
            8 => {
                let gid = own_identity(own_rows(v).last_mut().unwrap())[1].clone();
                let id = guid(&list(&gid).unwrap()[2]).unwrap();
                let d = ds.iter().find(|d| d.object_guid == id).unwrap().clone();
                ds.push(d);
            }
            9 => own_identity(own_rows(v).last_mut().unwrap())[5] = Value::Atom("1".into()),
            _ => own_identity(own_rows(v).last_mut().unwrap())[3] = Value::Null,
        });
        let o = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
        assert!(o.document_attributes.is_none(), "mode {mode}");
        assert!(o.document_tabular_sections.is_none(), "mode {mode}");
        assert!(ds.iter().all(|d| d.declared_types.is_none()), "mode {mode}");
    }
}
#[test]
fn strict_identity_callers_keep_rejecting_nonempty_comments() {
    let ds = parse_config_descriptors(ID, RAW).unwrap();
    for profile in [
        DocumentAttributeProfile::Legacy37,
        DocumentAttributeProfile::Legacy38Section,
    ] {
        let mut v = source();
        for row in &mut own_rows(&mut v)[2..] {
            own_identity(row)[4] = Value::String(String::new());
            let wrapper = vals(&mut vals(row)[0]);
            if matches!(profile, DocumentAttributeProfile::Legacy37) {
                wrapper[0] = Value::Atom("3".into());
                wrapper.truncate(4);
                wrapper[2] = Value::Atom("0".into());
                wrapper[3] = Value::Atom("1".into());
            } else {
                wrapper[0] = Value::Atom("7".into());
            }
        }
        assert!(
            staged_document_attributes(
                list(&v).unwrap(),
                &ds,
                "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
                profile
            )
            .known()
            .is_some()
        );
        let h = own_identity(own_rows(&mut v).last_mut().unwrap());
        assert!(legacy_data_processor::identity(&Value::List(h.clone())).is_some());
        h[4] = Value::String("измеренный текст".into());
        assert!(legacy_data_processor::identity(&Value::List(h.clone())).is_none());
        assert!(
            staged_document_attributes(
                list(&v).unwrap(),
                &ds,
                "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
                profile
            )
            .known()
            .is_none()
        );
    }
}
#[test]
fn owner_and_section_identity_comments_remain_strict() {
    for owner in [true, false] {
        let ds = mutate(|v, _| {
            let h = if owner {
                vals(&mut vals(&mut vals(v)[1])[9])
            } else {
                let row = &mut vals(&mut vals(v)[3])[2];
                vals(&mut vals(&mut vals(&mut vals(row)[0])[1])[5])
            };
            vals(&mut h[1])[4] = Value::String("текст".into());
        });
        let o = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
        assert!(o.document_tabular_sections.is_none());
        if owner {
            assert!(o.document_attributes.is_none());
        } else {
            assert_eq!(o.document_attributes.as_ref().unwrap().len(), 9);
        }
    }
}
