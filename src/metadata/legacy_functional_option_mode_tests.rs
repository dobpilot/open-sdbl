use super::*;
const ID: &str = "bf75c4de-b36a-4a7f-8d84-9fa3d020a6cb";
const LOCATION: &str = "a300ecc3-8031-4a50-a17d-b08cf3a72139";
const RAW: &[u8] = include_bytes!("../../tests/fixtures/buh-functional-option-mode/owner.deflate");
#[test]
fn native_legacy_option_preserves_identity_location_and_true_mode() {
    let ds = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let own = ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap();
    assert_eq!(own.name, "ИспользуютсяПрофилиБезопасности");
    assert_eq!(own.functional_option_privileged_get_mode, Some(true));
    assert_eq!(
        own.functional_option_location_guid
            .as_ref()
            .unwrap()
            .as_str(),
        LOCATION
    );
}

#[test]
fn six_native_text_comments_keep_complete_option_settings() {
    let raws: &[&[u8]] = &[
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/d95fcb37-fc65-466d-93e2-489f41683276.deflate"
        ),
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/c6b0d31e-7b68-4c7f-a565-0ac8a7c11143.deflate"
        ),
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/bde7dd9b-41ad-44c0-b0d4-04220beb8697.deflate"
        ),
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/53f45594-043e-4804-be88-78e79addde82.deflate"
        ),
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/d5588c0d-3e94-485b-a7e9-45f5cf74df9e.deflate"
        ),
        include_bytes!(
            "../../tests/fixtures/buh-functional-option-mode/text-comments/2f5e5cba-0086-4410-869c-65e4d3b24302.deflate"
        ),
    ];
    for (line, raw) in
        include_str!("../../tests/fixtures/buh-functional-option-mode/text-comments/native.tsv")
            .lines()
            .zip(raws)
    {
        let row = line.split('\t').collect::<Vec<_>>();
        let ds = super::super::parse_config_descriptors(row[0], raw).unwrap();
        let d = ds
            .iter()
            .find(|d| d.object_guid.as_str() == row[0])
            .unwrap();
        assert_eq!(d.name, row[1]);
        assert_eq!(
            d.functional_option_location_guid.as_ref().unwrap().as_str(),
            row[2]
        );
        assert_eq!(d.functional_option_privileged_get_mode, Some(true));
        let v = super::super::parse_serialized(&super::super::inflate_raw_deflate(raw).unwrap())
            .unwrap();
        assert_eq!(at(&v, &[1, 1, 4]).and_then(Value::as_string), Some(row[3]));
    }
}
#[test]
fn existing_native_false_and_true_options_are_unchanged() {
    for (id,raw,mode) in [
        ("a11dfe2b-ef68-4701-9977-08fb0e680ee1",include_bytes!("../../tests/fixtures/functional-option-content-native/resources/a11dfe2b-ef68-4701-9977-08fb0e680ee1.deflate").as_slice(),false),
        ("87c6ea4c-6187-4c3f-92ac-b691c1007e3e",include_bytes!("../../tests/fixtures/functional-option-content-native/resources/87c6ea4c-6187-4c3f-92ac-b691c1007e3e.deflate").as_slice(),true),
    ] {
        let mut ds=super::super::parse_config_descriptors(id,raw).unwrap();
        let owner=ds.iter().find(|d|d.object_guid.as_str()==id).unwrap();
        assert_eq!(owner.functional_option_privileged_get_mode,Some(mode));
        assert_eq!(owner.functional_option_location_guid.as_ref().unwrap().as_str(),"9893e2d6-f3f8-4d73-bb06-19bf26d216ab");
        let before=format!("{ds:?}");
        let v=super::super::parse_serialized(&super::super::inflate_raw_deflate(raw).unwrap()).unwrap();
        project(&v,&id.parse().unwrap(),&mut ds);assert_eq!(format!("{ds:?}"),before);
    }
}
fn values(value: &mut Value) -> &mut Vec<Value> {
    match value {
        Value::List(values) => values,
        _ => panic!("ожидается список"),
    }
}
fn at_mut<'a>(mut value: &'a mut Value, path: &[usize]) -> &'a mut Value {
    for i in path {
        value = &mut values(value)[*i];
    }
    value
}
#[test]
fn malformed_old_profiles_publish_no_partial_mode_or_location() {
    let base = super::super::parse_config_descriptors(ID, RAW).unwrap();
    let source =
        super::super::parse_serialized(&super::super::inflate_raw_deflate(RAW).unwrap()).unwrap();
    for mode in 0..32 {
        let mut v = source.clone();
        let mut ds = base.clone();
        for d in &mut ds {
            d.functional_option_location_guid = None;
            d.functional_option_privileged_get_mode = None;
            d.functional_option_location = None;
        }
        match mode {
            0 => {
                values(&mut v).pop();
            }
            1 => *at_mut(&mut v, &[0]) = Value::Atom("9".into()),
            2 => *at_mut(&mut v, &[2]) = Value::Atom("1".into()),
            3 => {
                values(at_mut(&mut v, &[1])).pop();
            }
            4 => *at_mut(&mut v, &[1, 0]) = Value::Atom("9".into()),
            5 => {
                values(at_mut(&mut v, &[1, 1])).pop();
            }
            6 => *at_mut(&mut v, &[1, 1, 0]) = Value::Atom("0".into()),
            7 => *at_mut(&mut v, &[1, 1, 1, 0]) = Value::Atom("1".into()),
            8 => *at_mut(&mut v, &[1, 1, 1, 1]) = Value::Atom("1".into()),
            9 => {
                *at_mut(&mut v, &[1, 1, 1, 2]) =
                    Value::Atom("00000000-0000-0000-0000-000000000000".into())
            }
            10 => *at_mut(&mut v, &[1, 1, 1, 2]) = Value::Atom(LOCATION.into()),
            11 => *at_mut(&mut v, &[1, 1, 2]) = Value::Atom("name".into()),
            12 => *at_mut(&mut v, &[1, 1, 2]) = Value::String("".into()),
            13 => *at_mut(&mut v, &[1, 1, 3]) = Value::Null,
            14 => *at_mut(&mut v, &[1, 1, 4]) = Value::Atom("".into()),
            15 => *at_mut(&mut v, &[1, 1, 4]) = Value::Atom("text".into()),
            16 => *at_mut(&mut v, &[1, 1, 4]) = Value::Null,
            17 => *at_mut(&mut v, &[1, 1, 4]) = Value::List(vec![]),
            18 => *at_mut(&mut v, &[1, 1, 5]) = Value::Atom("1".into()),
            19 => *at_mut(&mut v, &[1, 1, 6]) = Value::Atom("1".into()),
            20 => *at_mut(&mut v, &[1, 2]) = Value::Atom("unknown".into()),
            21 => *at_mut(&mut v, &[1, 4]) = Value::Atom("2".into()),
            22 => *at_mut(&mut v, &[1, 4]) = Value::String("1".into()),
            23 => *at_mut(&mut v, &[1, 3, 1]) = Value::Atom("5".into()),
            24 => {
                values(at_mut(&mut v, &[1, 3, 7])).pop();
            }
            25 => *at_mut(&mut v, &[1, 3, 7, 0]) = Value::Atom("#".into()),
            26 => *at_mut(&mut v, &[1, 3, 7, 1]) = Value::Atom(ID.into()),
            27 => {
                *at_mut(&mut v, &[1, 3, 7, 2, 1, 2, 1]) =
                    Value::Atom("00000000-0000-0000-0000-000000000000".into())
            }
            28 => *at_mut(&mut v, &[1, 4]) = Value::Null,
            29 => ds.clear(),
            30 => ds.push(ds[0].clone()),
            31 => *at_mut(&mut v, &[1, 1, 3, 2]) = Value::Atom("synonym".into()),
            _ => unreachable!(),
        }
        let before = format!("{ds:?}");
        project(&v, &ID.parse().unwrap(), &mut ds);
        assert_eq!(format!("{ds:?}"), before, "режим {mode}");
    }
}
