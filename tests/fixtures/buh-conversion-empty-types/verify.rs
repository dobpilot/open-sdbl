use open_sdbl::metadata::parse_config_descriptors;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args().nth(1).ok_or("нужен каталог фикстуры")?;
    let directory = Path::new(&directory);
    let owner_id = "cae8a6f2-bbe9-4b73-b839-9bf8e5e2818d";
    let bytes = std::fs::read(directory.join(format!("{owner_id}.deflate")))?;
    let descriptors = parse_config_descriptors(owner_id, &bytes)?;
    let xml = std::fs::read_to_string(directory.join("xml-attributes.tsv"))?;
    let rows = xml
        .lines()
        .skip(1)
        .map(|line| {
            let mut fields = line.split('\t');
            (
                fields.next().unwrap(),
                fields.next().unwrap(),
                fields.next().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 43);
    assert_eq!(rows.iter().filter(|row| row.2.is_empty()).count(), 4);
    let owner = descriptors
        .iter()
        .find(|item| item.object_guid.as_str() == owner_id)
        .ok_or("нет описателя владельца")?;
    let expected = rows.iter().map(|row| row.0).collect::<Vec<_>>();
    let actual = owner
        .data_processor_attributes
        .as_ref()
        .map(|ids| ids.iter().map(|id| id.as_str()).collect::<Vec<_>>());
    let membership_matches = actual.as_ref() == Some(&expected);
    let mut missing_declarations = 0;
    let mut empty_matches = 0;
    let mut value_tables_match = 0;
    for (id, name, xml_type) in &rows {
        let attribute = descriptors
            .iter()
            .find(|item| item.object_guid.as_str() == *id)
            .ok_or("нет описателя реквизита из XML")?;
        assert_eq!(attribute.name, *name);
        let Some(types) = attribute.declared_types.as_ref() else {
            missing_declarations += 1;
            continue;
        };
        if xml_type.is_empty() && types.is_empty() {
            empty_matches += 1;
        }
        if *xml_type == "v8:ValueTable"
            && types.len() == 1
            && types[0].tag == "#"
            && types[0].reference.as_ref().is_some_and(|reference| {
                reference.as_str() == "acf6192e-81ca-46ef-93a6-5a6968b78663"
            })
        {
            value_tables_match += 1;
        }
    }
    println!(
        "xml.attributes={}; membership.matches={membership_matches}; missing.declarations={missing_declarations}; empty.matches={empty_matches}/4; value_table.matches={value_tables_match}/3",
        rows.len()
    );
    if !membership_matches
        || missing_declarations != 0
        || empty_matches != 4
        || value_tables_match != 3
    {
        return Err("приобретённые метаданные не сохраняют измеренный состав реквизитов".into());
    }
    Ok(())
}
