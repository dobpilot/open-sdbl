//! Synthetic credentials only; positions mirror the measured record structure.
#![allow(dead_code)]
pub fn fields(h1: &str, h2: &str, algorithm: &str, salt: &str, optional: usize) -> Vec<String> {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut fields = vec!["0".to_owned(); 31 + optional + 6];
    fields[0] = "3a442462-4e9b-4d59-8561-f445712ab12f".into();
    fields[1] = quote("Synthetic password fixture");
    fields[3] = quote("Synthetic password fixture");
    fields[4] = "00000000-0000-0000-0000-000000000000".into();
    fields[5] = "{1,76702e9e-fa7a-4b98-befa-f9b37db2dae0}".into();
    fields[6] = "00000000-0000-0000-0000-000000000000".into();
    fields[12] = quote(h1);
    fields[13] = quote(h2);
    fields[14] = "2".into();
    fields[30] = optional.to_string();
    for field in &mut fields[31..31 + optional] {
        *field = quote("synthetic optional field");
    }
    let tail = 31 + optional;
    fields[tail] = algorithm.into();
    fields[tail + 1] = quote(salt);
    fields[tail + 5] = quote("");
    fields
}
pub fn encode(fields: &[String]) -> Vec<u8> {
    let text = format!("\u{feff}{{{}}}", fields.join(","));
    let key = [0x11_u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let mut bytes = vec![key.len() as u8];
    bytes.extend(key);
    bytes.extend(
        text.bytes()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()]),
    );
    bytes
}
pub fn blob(h1: &str, h2: &str, algorithm: &str, salt: &str, optional: usize) -> Vec<u8> {
    encode(&fields(h1, h2, algorithm, salt, optional))
}
