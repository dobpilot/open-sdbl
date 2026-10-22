use super::*;

const ID: &str = "cae8a6f2-bbe9-4b73-b839-9bf8e5e2818d";
const RAW: &[u8] = include_bytes!(
    "../../tests/fixtures/buh-conversion-empty-types/cae8a6f2-bbe9-4b73-b839-9bf8e5e2818d.deflate"
);
const EXPECTED: &str =
    include_str!("../../tests/fixtures/buh-conversion-empty-types/xml-declarations.tsv");
const COLLECTION: &str = "ec6bb5e5-b7a8-4d75-bec9-658107a699cf";

fn acquired() -> Vec<ConfigDescriptor> {
    super::super::parse_config_descriptors(ID, RAW).unwrap()
}

fn owner(ds: &[ConfigDescriptor]) -> &ConfigDescriptor {
    ds.iter().find(|d| d.object_guid.as_str() == ID).unwrap()
}

#[test]
fn native_conversion_processor_preserves_every_ordered_declaration_and_qualifier() {
    let ds = acquired();
    let expected = EXPECTED.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(expected.len(), 43);
    let own = owner(&ds);
    assert_eq!(own.name, "КонвертацияОбъектовИнформационныхБаз");
    let ids = own.data_processor_attributes.as_ref().unwrap();
    assert_eq!(ids.len(), expected.len());
    let mut empty = 0;
    let mut tables = 0;
    for (id, row) in ids.iter().zip(expected) {
        let columns = row.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 5);
        assert_eq!(id.as_str(), columns[0]);
        let d = ds.iter().find(|d| &d.object_guid == id).unwrap();
        assert_eq!(d.name, columns[1]);
        let types = d.declared_types.as_ref().unwrap();
        if columns[2].is_empty() {
            assert!(types.is_empty(), "{}", d.name);
            empty += 1;
            continue;
        }
        assert_eq!(types.len(), 1, "{}", d.name);
        let ty = &types[0];
        assert_eq!(ty.tag, columns[2], "{}", d.name);
        assert_eq!(
            ty.reference.as_ref().map(Guid::as_str).unwrap_or(""),
            columns[3],
            "{}",
            d.name
        );
        assert_eq!(
            ty.qualifiers
                .iter()
                .map(|v| v.as_string().or_else(|| atom(v)).unwrap())
                .collect::<Vec<_>>()
                .join(","),
            columns[4],
            "{}",
            d.name
        );
        if columns[3] == "acf6192e-81ca-46ef-93a6-5a6968b78663" {
            tables += 1;
        }
    }
    assert_eq!(empty, 4);
    assert_eq!(tables, 3);
    // Комментарии взяты из исходной нативной XML-выгрузки.
    assert_eq!(
        ds.iter()
            .find(|d| d.object_guid.as_str() == "3e427c09-79ed-45f7-b4c9-d9aa7ccc01b2")
            .unwrap()
            .comment
            .as_deref(),
        Some(
            "Флаг определяет контекст выполнения загрузки данных. Истина – загрузка данных выполняется во внешнем соединении; Ложь – загрузка выполняется в контексте текущей ИБ."
        )
    );
    assert_eq!(
        ds.iter()
            .find(|d| d.object_guid.as_str() == "1a4dd1f5-c2d6-4e06-b428-ab7870810bf3")
            .unwrap()
            .comment
            .as_deref(),
        Some(
            "Флаг определяет режим загрузки данных. Истина – загрузка данных выполняется через внешнее соединение. Ложь – загрузка данных выполняется как обычно, через файл."
        )
    );
}

fn values(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::List(v) => v,
        _ => panic!("ожидается измеренная коллекция"),
    }
}

fn rows(v: &mut Value) -> &mut Vec<Value> {
    values(v)
        .iter_mut()
        .find_map(|v| match v {
            Value::List(rows) if atom(&rows[0]) == Some(COLLECTION) => Some(rows),
            _ => None,
        })
        .unwrap()
}

fn typed(row: &mut Value) -> &mut Vec<Value> {
    values(&mut values(&mut values(&mut values(row)[0])[1])[1])
}

fn mutated(edit: impl FnOnce(&mut Value, &mut Vec<ConfigDescriptor>)) -> Vec<ConfigDescriptor> {
    let text = super::super::inflate_raw_deflate(RAW).unwrap();
    let mut value = super::super::parse_serialized(&text).unwrap();
    let mut ds = acquired();
    for d in &mut ds {
        d.data_processor_attributes = None;
        d.data_processor_tabular_sections = None;
        d.data_processor_object_type = None;
        d.tabular_section_attributes = None;
        d.declared_types = None;
    }
    edit(&mut value, &mut ds);
    project(&value, &ID.parse().unwrap(), &mut ds);
    ds
}

#[test]
fn malformed_conversion_declarations_do_not_publish_membership_or_a_prefix() {
    for mode in 0..9 {
        let ds = mutated(|v, ds| {
            let rows = rows(v);
            // Последняя строка следует за всеми тремя таблицами и пустыми типами.
            let last = rows.len() - 1;
            match mode {
                0 => typed(&mut rows[last])[2] = Value::Null,
                1 => {
                    values(&mut typed(&mut rows[last])[2])[0] =
                        Value::String("UnsupportedPattern".into())
                }
                2 => values(&mut typed(&mut rows[last])[2]).push(Value::Null),
                3 => values(&mut rows[last]).push(Value::Atom("0".into())),
                4 => rows[last] = rows[2].clone(),
                5 => {
                    rows.pop();
                }
                6 => ds.retain(|d| d.name != "СсылкаДляПереходаПриВозникновенииОшибки"),
                7 => {
                    let duplicate = ds
                        .iter()
                        .find(|d| d.name == "СсылкаДляПереходаПриВозникновенииОшибки")
                        .unwrap()
                        .clone();
                    ds.push(duplicate);
                }
                8 => values(&mut typed(&mut rows[last])[1])[4] = Value::Null,
                _ => unreachable!(),
            }
        });
        assert!(
            owner(&ds).data_processor_attributes.is_none(),
            "mode {mode}"
        );
        assert!(
            ds.iter().all(|d| d.declared_types.is_none()),
            "mode {mode}: частичная декларация"
        );
    }
}
