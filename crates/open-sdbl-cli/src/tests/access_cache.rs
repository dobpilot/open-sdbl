//! Tests of the session parameters read from the base.

use open_sdbl::metadata::{DEFAULT_OUTPUT_LIMIT, decode_stored_value};

use super::*;

fn content(text: &str) -> Vec<u8> {
    let mut bytes = vec![0x01, 0x01];
    bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// The cache of the УНФ demo, in its shape: the versions beside a nested
/// map of the lists.
const CACHE: &str = r##"{"#",3ee983d7-ace7-40f9-bb7e-2e916fcddd56,
{3,
{
{"S","ВерсияСтруктурыКэша"},
{"S","28.2"}
},
{
{"S","ВерсииШаблонов"},
{"S",",ДляОбъекта9,"}
},
{
{"S","ПараметрыШаблонов"},
{"#",3ee983d7-ace7-40f9-bb7e-2e916fcddd56,
{2,
{
{"S","СпискиСОграничениемПоПолям"},
{"S","РегистрНакопления.Х:Вариант1Поле1=Поле2;"}
},
{
{"S","СпискиСОтключеннымОграничениемЧтения"},
{"S","БизнесПроцесс.Х;"}
}
}
}
}
}
}"##;

#[test]
fn names_the_parameters_the_cache_carries() {
    let value = decode_stored_value(&content(CACHE), DEFAULT_OUTPUT_LIMIT).unwrap();
    let parameters = template_parameters(&value);
    assert_eq!(
        parameters,
        [
            (
                "ВерсииШаблоновОграниченияДоступа".to_owned(),
                ",ДляОбъекта9,".to_owned()
            ),
            (
                "СпискиСОграничениемПоПолям".to_owned(),
                "РегистрНакопления.Х:Вариант1Поле1=Поле2;".to_owned()
            ),
            (
                "СпискиСОтключеннымОграничениемЧтения".to_owned(),
                "БизнесПроцесс.Х;".to_owned()
            ),
        ]
    );
    // The cache version is not a session parameter.
    assert!(
        !parameters
            .iter()
            .any(|(name, _)| name.contains("ВерсияСтруктурыКэша"))
    );
}

#[test]
fn reads_the_hexadecimal_a_provider_may_answer() {
    assert_eq!(decode_hex("0x0A0b"), Some(vec![0x0a, 0x0b]));
    assert_eq!(decode_hex("0X00"), Some(vec![0]));
    assert_eq!(decode_hex("0x0"), None);
    assert_eq!(decode_hex("00"), None);
    assert_eq!(decode_hex("0xzz"), None);
}
