use open_sdbl::metadata::{decode_common_setting, DecodedCommonSetting};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("каталог нативных payload"));
    for (index, expected) in [(0,"29"), (2,"31"), (4,"37")] {
        let decoded = decode_common_setting(&std::fs::read(root.join(format!("_systemsettings-{index}.settingsdata")))?, 1_048_576)?;
        let DecodedCommonSetting::Value(text) = decoded else { panic!("скаляр"); };
        assert_eq!(text, format!("{{\"N\",{expected}}}"));
        println!("system.scalar.{index}={text}");
    }
    for index in [1,3,5] {
        let decoded = decode_common_setting(&std::fs::read(root.join(format!("_systemsettings-{index}.settingsdata")))?, 1_048_576)?;
        let DecodedCommonSetting::ValueStorage(text) = decoded else { panic!("хранилище"); };
        let expected = r##"{"#",4238019d-7e49-4fc9-91db-b6b951d5cf8e,{1,{{"S","Probe"},{"N",73}}}}"##;
        assert_eq!(open_sdbl::metadata::parse_serialized(text.as_bytes())?, open_sdbl::metadata::parse_serialized(expected.as_bytes())?);
        println!("system.storage.{index}={text}");
    }
    Ok(())
}
