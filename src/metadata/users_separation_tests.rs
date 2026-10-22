use super::*;

fn record(bindings: Vec<Value>) -> Vec<Value> {
    let mut fields = vec![Value::Atom("0".into()); 38];
    for index in [0, 4, 6] {
        fields[index] = Value::Atom("11111111-1111-1111-1111-111111111111".into());
    }
    fields[5] = Value::List(vec![Value::Atom("0".into())]);
    fields[17] = Value::Atom(bindings.len().to_string());
    fields[30] = Value::Atom("1".into());
    fields[31] = Value::String("sanitized-own-field".into());
    fields[33] = Value::String(String::new());
    fields[37] = Value::String(String::new());
    fields.splice(18..18, bindings);
    fields
}

fn binding() -> Value {
    Value::List(vec![
        Value::String("НаименованиеЯзык1".into()),
        Value::String("own-separator-value".into()),
    ])
}

#[test]
fn known_empty_is_distinct_from_unknown() {
    assert_eq!(decode_data_separation(&record(vec![])), Some(vec![]));
    assert_eq!(decode_data_separation(&[]), None);
}

#[test]
fn retains_the_complete_string_binding() {
    assert_eq!(
        decode_data_separation(&record(vec![binding()])),
        Some(vec![(
            "НаименованиеЯзык1".into(),
            "own-separator-value".into()
        )])
    );
}

#[test]
fn rejects_incomplete_counts_and_unknown_layouts() {
    let mut fields = record(vec![binding()]);
    fields[17] = Value::Atom("2".into());
    assert_eq!(decode_data_separation(&fields), None);
    fields[17] = Value::Atom("1".into());
    fields.push(Value::Null);
    assert_eq!(decode_data_separation(&fields), None);
}

#[test]
fn rejects_duplicates_and_unsupported_values_without_partial_results() {
    assert_eq!(
        decode_data_separation(&record(vec![binding(), binding()])),
        None
    );
    assert_eq!(
        decode_data_separation(&record(vec![
            binding(),
            Value::List(vec![Value::String("Other".into()), Value::Atom("1".into())])
        ])),
        None
    );
}

#[test]
fn rejects_missing_layout_sentinels() {
    let mut fields = record(vec![]);
    fields[4] = Value::Null;
    assert_eq!(decode_data_separation(&fields), None);
}

#[test]
fn historical_layouts_retain_empty_and_nonempty_bindings() {
    for bindings in [vec![], vec![binding()]] {
        let count = bindings.len();
        for base in [31, 32, 37, 38] {
            let mut fields = record(bindings.clone());
            if base == 32 {
                fields.truncate(32 + count);
            } else if base != 38 {
                fields.remove(31 + count);
                fields[30 + count] = Value::Atom("0".into());
                if base == 31 {
                    fields.truncate(31 + count);
                }
            }
            assert_eq!(fields.len(), base + count);
            assert_eq!(decode_data_separation(&fields).unwrap().len(), count);
        }
    }
}

#[test]
fn inconsistent_optional_fields_and_suffix_kinds_remain_unknown() {
    let mut fields = record(vec![]);
    fields[30] = Value::Atom("0".into());
    assert_eq!(decode_data_separation(&fields), None);
    fields[30] = Value::Atom("1".into());
    fields[33] = Value::Atom("0".into());
    assert_eq!(decode_data_separation(&fields), None);
}
