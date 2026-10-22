//! Frozen complete decoder results captured before the module refactor.
//! Each case also checks a damaged final collection and truncated DEFLATE input.
use open_sdbl::metadata::{
    Value, inflate_raw_deflate, parse_config_resource_bounded, parse_serialized,
};

fn serialize(value: &Value) -> String {
    match value {
        Value::List(values) => format!(
            "{{{}}}",
            values.iter().map(serialize).collect::<Vec<_>>().join(",")
        ),
        Value::String(text) => format!("\"{}\"", text.replace('"', "\"\"")),
        Value::Atom(text) => text.clone(),
        Value::Null => String::new(),
    }
}

fn stored_deflate(text: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut chunks = text.chunks(u16::MAX as usize).peekable();
    while let Some(chunk) = chunks.next() {
        output.push(u8::from(chunks.peek().is_none()));
        let len = u16::try_from(chunk.len()).unwrap();
        output.extend_from_slice(&len.to_le_bytes());
        output.extend_from_slice(&(!len).to_le_bytes());
        output.extend_from_slice(chunk);
    }
    output
}

fn damaged_collection(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut value = parse_serialized(&inflate_raw_deflate(bytes).ok()?).ok()?;
    let Value::List(root) = &mut value else {
        return None;
    };
    // Corrupt the final member, keeping earlier members available to exercise
    // family-specific partial-result behavior rather than just parse failures.
    if let Some(Value::List(last)) = root.last_mut() {
        if let Some(member) = last.last_mut() {
            *member = Value::Null;
        }
    } else if let Some(last) = root.last_mut() {
        *last = Value::Null;
    }
    Some(stored_deflate(serialize(&value).as_bytes()))
}

fn fingerprint(resource: &str, bytes: &[u8]) -> u64 {
    let result = parse_config_resource_bounded(resource, bytes, 256 * 1024 * 1024);
    // FNV-1a over the entire deterministic Debug representation includes field
    // values, None versus empty, order, decoded size and full error diagnostics.
    format!("{result:?}")
        .bytes()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[test]
fn complete_native_results_and_partial_failures_match_before_refactor() {
    let manifest = include_str!("fixtures/runtime-projection-parity/before.tsv");
    let mut count = 0;
    for line in manifest.lines().filter(|line| !line.starts_with('#')) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let [path, resource, original, damaged, truncated] = fields.as_slice() else {
            panic!("{line}")
        };
        let bytes =
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(
            format!("{:016x}", fingerprint(resource, &bytes)),
            *original,
            "{path}: original"
        );
        if let Some(changed) = damaged_collection(&bytes) {
            assert_eq!(
                format!("{:016x}", fingerprint(resource, &changed)),
                *damaged,
                "{path}: damaged collection"
            );
        } else {
            assert_eq!(*damaged, "-");
        }
        assert_eq!(
            format!(
                "{:016x}",
                fingerprint(resource, &bytes[..bytes.len().saturating_sub(1)])
            ),
            *truncated,
            "{path}: truncated"
        );
        count += 1;
    }
    assert!(
        count >= 400,
        "the frozen native corpus must not shrink silently"
    );
}
