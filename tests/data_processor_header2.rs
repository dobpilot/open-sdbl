mod support;
use open_sdbl::metadata::{ConfigDescriptor, parse_config_descriptors};
const ID: &str = "4507a92e-abb6-4329-88b2-3429bc694bc6";
const TEXT: &str = include_str!("fixtures/data-processor-header2/owner.config");
const IDS: [&str; 3] = [
    "2694c1b4-536b-4187-adf6-0f8757aaae76",
    "e523f88b-b3de-476e-a3cc-592970492dec",
    "0e2107e2-8648-4b2e-9dfb-c23b1ff5b007",
];
fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}
#[test]
fn exact_native_legacy_owner_retains_order_types_and_empty_sections() {
    let ds = parse_config_descriptors(
        ID,
        include_bytes!("fixtures/data-processor-header2/owner.deflate"),
    )
    .unwrap();
    let d = owner(&ds);
    assert_eq!(
        d.data_processor_attributes
            .as_ref()
            .unwrap()
            .iter()
            .map(|g| g.as_str())
            .collect::<Vec<_>>(),
        IDS
    );
    assert!(
        d.data_processor_tabular_sections
            .as_ref()
            .unwrap()
            .is_empty()
    );
    let xml = include_str!("fixtures/data-processor-header2/owner.xml");
    assert!(xml.contains("<v8:TypeSet>cfg:ExchangePlanRef</v8:TypeSet>"));
    for id in IDS {
        assert!(xml.contains(id));
    }
    let names = [
        "URLВебСервиса",
        "НаименованиеАвтономногоРабочегоМеста",
        "АвтономноеРабочееМесто",
    ];
    for (i, id) in IDS.iter().enumerate() {
        let a = ds.iter().find(|d| d.object_guid.as_str() == *id).unwrap();
        assert_eq!(a.name, names[i]);
        let t = a.declared_types.as_ref().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].tag, if i == 2 { "#" } else { "S" });
        if i == 2 {
            assert_eq!(
                t[0].reference.as_ref().unwrap().as_str(),
                "0a52f9de-73ea-4507-81e8-66217bead73a"
            );
            assert!(t[0].qualifiers.is_empty());
        } else {
            assert_eq!(
                t[0].qualifiers
                    .iter()
                    .map(|v| match v {
                        open_sdbl::metadata::Value::Atom(s) => s.as_str(),
                        _ => panic!("atom qualifier"),
                    })
                    .collect::<Vec<_>>(),
                if i == 0 { vec![] } else { vec!["150", "1"] }
            );
        }
    }
}
#[test]
fn malformed_legacy_identity_and_declaration_remain_unknown() {
    let first = format!("{{1,0,{}}}", IDS[0]);
    for text in [
        TEXT.replacen(&first, &format!("{{0,0,{}}}", IDS[0]), 1),
        TEXT.replace(IDS[1], IDS[0]),
        TEXT.replace(IDS[0], "00000000-0000-0000-0000-000000000000"),
        TEXT.replacen("{27,", "{28,", 1),
        TEXT.replacen("{27,", "{27,0,", 1),
        {
            let index = TEXT.find(IDS[0]).unwrap();
            format!(
                "{}{}",
                &TEXT[..index],
                TEXT[index..].replacen(
                    "00000000-0000-0000-0000-000000000000}",
                    "00000000-0000-0000-0000-000000000000,0,0}",
                    1
                )
            )
        },
        TEXT.replacen("{17,", "{19,", 1),
        TEXT.replacen("{17,", "{56,", 1),
        TEXT.replacen(
            "0,0,00000000-0000-0000-0000-000000000000}",
            "0,0,00000000-0000-0000-0000-000000000000,0,0}",
            1,
        ),
        TEXT.replacen("{\"Pattern\",", "{\"Unknown\",", 1),
        TEXT.replacen(
            "{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,3,",
            "{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,4,",
            1,
        ),
    ] {
        let ds = parse_config_descriptors(ID, &support::stored_deflate(text.as_bytes())).unwrap();
        assert!(owner(&ds).data_processor_attributes.is_none());
    }
}

#[test]
fn legacy_identity_is_not_enabled_for_other_owners_or_modern_owner_headers() {
    let normalized = TEXT.replace("\r\n", "\n");
    let modern_owner = normalized
        .replacen(
            &format!("{{2,\n{{1,0,{ID}}}"),
            &format!("{{3,\n{{1,0,{ID}}}"),
            1,
        )
        .replacen(
            "0,0,00000000-0000-0000-0000-000000000000}",
            "0,0,00000000-0000-0000-0000-000000000000,0}",
            1,
        );
    assert!(modern_owner.contains(&format!("{{3,\n{{1,0,{ID}}}")));
    for text in [
        TEXT.replacen("{17,", "{19,", 1),
        TEXT.replacen("{17,", "{56,", 1),
        modern_owner,
    ] {
        let ds = parse_config_descriptors(ID, &support::stored_deflate(text.as_bytes())).unwrap();
        assert!(owner(&ds).data_processor_attributes.is_none());
        for id in IDS {
            assert!(
                ds.iter()
                    .find(|d| d.object_guid.as_str() == id)
                    .unwrap()
                    .declared_types
                    .is_none()
            );
        }
    }
}

#[test]
fn invalid_last_legacy_declaration_does_not_mutate_earlier_attributes() {
    let index = TEXT.rfind("\"Pattern\"").unwrap();
    let text = format!(
        "{}{}",
        &TEXT[..index],
        TEXT[index..].replacen("\"Pattern\"", "\"Unknown\"", 1)
    );
    let ds = parse_config_descriptors(ID, &support::stored_deflate(text.as_bytes())).unwrap();
    assert!(owner(&ds).data_processor_attributes.is_none());
    assert!(
        owner(&ds)
            .data_processor_tabular_sections
            .as_ref()
            .unwrap()
            .is_empty()
    );
    for id in IDS {
        assert!(
            ds.iter()
                .find(|d| d.object_guid.as_str() == id)
                .unwrap()
                .declared_types
                .is_none()
        );
    }
}
