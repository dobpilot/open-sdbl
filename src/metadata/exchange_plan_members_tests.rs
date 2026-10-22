const ID: &str = "d4becac6-bf5f-4947-a734-c702d399d5fc";
const ATTRS: &str = "1a1b4fea-e093-470d-94ff-1d2f16cda2ab";
const RAW: &[u8] =
    include_bytes!("../../tests/fixtures/exchange-plan-0943/ИнтегрированныеСистемы.deflate");
const TEXT: &[u8] =
    include_bytes!("../../tests/fixtures/exchange-plan-0943/ИнтегрированныеСистемы.config");

#[test]
fn native_exchange_plan_members_agree_with_designer_and_edt() {
    let ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let owner = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
    let ids = owner
        .exchange_plan_attributes
        .as_ref()
        .expect("complete native composition");
    let expected = [
        (
            "1e74bab7-dcee-4112-9e56-132faffce56c",
            "АдресВебСервиса",
            "S",
        ),
        (
            "5737d959-b812-45a3-9312-b88f4aee684b",
            "АдресХостаВнешнихНавигационныхСсылок",
            "S",
        ),
        ("34ccf388-90e9-4eee-a7a1-422906e975fb", "Идентификатор", "S"),
        (
            "d04c13ef-737b-47b2-9ffe-c2142514403d",
            "ПоддерживаетПравилаЗагрузкиДанныхВДО",
            "B",
        ),
        (
            "71db3b01-caf1-4965-9af3-8f8b853345d4",
            "СопоставлятьСотрудниковПоДолжности",
            "B",
        ),
        (
            "ac417619-f336-4876-862a-3739beecb7c3",
            "СопоставлятьСотрудниковПоПодразделению",
            "B",
        ),
        (
            "d547d6b0-1a0d-48cf-86da-3b731ec93f69",
            "ТипРасположенияХостаВнешнихНавигационныхСсылок",
            "#",
        ),
        (
            "e0e412d5-0748-4d4b-8190-213b866a2b87",
            "УзелДляСинхронизацииДанных",
            "#",
        ),
    ];
    assert_eq!(ids.len(), expected.len());
    let xml = include_str!("../../tests/fixtures/exchange-plan-0943/ИнтегрированныеСистемы.xml");
    let edt = include_str!("../../tests/fixtures/exchange-plan-0943/ИнтегрированныеСистемы.mdo");
    let mut xml_position = 0;
    let mut edt_position = 0;
    for (id, (wanted, name, code)) in ids.iter().zip(expected) {
        assert_eq!(id.as_str(), wanted);
        let member = ds.iter().find(|d| d.object_guid == *id).unwrap();
        assert_eq!(member.name, name);
        let xml_marker = format!("<Attribute uuid=\"{wanted}\">");
        let edt_marker = format!("<attributes uuid=\"{wanted}\">");
        xml_position += xml[xml_position..].find(&xml_marker).unwrap() + xml_marker.len();
        edt_position += edt[edt_position..].find(&edt_marker).unwrap() + edt_marker.len();
        assert_eq!(member.declared_types.as_ref().unwrap()[0].tag, code);
    }
    for (name, length) in [("АдресВебСервиса", "1024"), ("Идентификатор", "36")]
    {
        let member = ds.iter().find(|d| d.name == name).unwrap();
        assert_eq!(
            member.declared_types.as_ref().unwrap()[0]
                .qualifiers
                .iter()
                .map(super::atom)
                .collect::<Vec<_>>(),
            [Some(length), Some("1")]
        );
    }
}

fn values(v: &mut super::Value) -> &mut Vec<super::Value> {
    match v {
        super::Value::List(v) => v,
        _ => panic!("native list expected"),
    }
}
fn collection(v: &mut super::Value) -> &mut Vec<super::Value> {
    values(
        values(v)
            .iter_mut()
            .find(|v| super::list(v).is_some_and(|r| super::atom(&r[0]) == Some(ATTRS)))
            .unwrap(),
    )
}
fn mutated(
    edit: impl FnOnce(&mut super::Value, &mut Vec<super::ConfigDescriptor>),
) -> Vec<super::ConfigDescriptor> {
    let mut v = super::super::parse_serialized(TEXT).unwrap();
    let mut ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    for d in &mut ds {
        d.exchange_plan_attributes = None;
    }
    edit(&mut v, &mut ds);
    super::project(&v, &ID.parse().unwrap(), &mut ds);
    ds
}
fn owner(ds: &[super::ConfigDescriptor]) -> &super::ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn malformed_or_incomplete_exchange_members_never_publish_a_prefix() {
    for mode in 0..10 {
        let ds = mutated(|v, ds| match mode {
            0 => collection(v)[1] = super::Value::Atom("9".into()),
            1 => {
                collection(v).pop();
            }
            2 => {
                let row = collection(v)[2].clone();
                collection(v)[3] = row;
            }
            3 => ds.retain(|d| d.name != "АдресВебСервиса"),
            4 => {
                let d = ds
                    .iter()
                    .find(|d| d.name == "АдресВебСервиса")
                    .unwrap()
                    .clone();
                ds.push(d);
            }
            5 => {
                values(&mut values(&mut collection(v)[2])[0])[0] = super::Value::Atom("999".into())
            }
            6 => {
                let body = &mut values(&mut values(&mut collection(v)[2])[0])[1];
                values(body)[0] = super::Value::Atom("999".into());
                ds.iter_mut()
                    .find(|d| d.name == "АдресВебСервиса")
                    .unwrap()
                    .declared_types = None;
            }
            7 => {
                values(&mut values(v)[1]).pop();
            }
            8 => values(&mut values(v)[1])[1] = super::Value::Atom("unknown".into()),
            9 => {
                let duplicate = super::Value::List(collection(v).clone());
                values(v).push(duplicate);
            }
            _ => unreachable!(),
        });
        assert!(owner(&ds).exchange_plan_attributes.is_none(), "mode {mode}");
        assert!(owner(&ds).document_attributes.is_none());
    }
}
#[test]
fn verified_empty_exchange_members_are_known_empty() {
    let ds = mutated(|v, _| {
        let c = collection(v);
        c.truncate(2);
        c[1] = super::Value::Atom("0".into());
    });
    assert_eq!(owner(&ds).exchange_plan_attributes, Some(vec![]));
}
#[test]
fn failed_type_expansion_invalidates_exchange_membership() {
    let mut ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let member = ds.iter_mut().find(|d| d.name == "АдресВебСервиса").unwrap();
    member.defined_type_id = Some(member.object_guid.clone());
    member.declared_types = Some(vec![super::ConfigDeclaredType {
        tag: "#".into(),
        reference: Some(member.object_guid.clone()),
        qualifiers: vec![],
    }]);
    super::resolve_named_types(&mut ds);
    assert!(owner(&ds).exchange_plan_attributes.is_none());
}
