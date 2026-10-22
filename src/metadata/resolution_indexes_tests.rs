use super::*;
fn table(name: &str, number: u32, inline: Option<&str>) -> SchemaTable {
    SchemaTable {
        name: name.into(),
        number,
        inline_name: inline.map(str::to_owned),
        owner: None,
        columns: vec![],
        indexes: vec![],
    }
}
fn entry(alias: &str, number: u32) -> DbNameEntry {
    DbNameEntry {
        guid: "11111111-1111-4111-8111-111111111111".parse().unwrap(),
        alias: alias.into(),
        number,
    }
}
#[test]
fn indexed_service_mapping_agrees_with_original_or_order_and_unicode_equality() {
    let mut schema = SchemaStorage {
        tables: vec![
            table("Owner_ExtDim7", 7, Some("eXTdIM7")),
            table("ExtDim", 99, None),
            table("EXTDIM7", 8, None),
            table("СЕРВИС", 1, None),
            table("сервис7", 7, None),
            table("İ", 2, None),
            table("i\u{307}", 3, None),
            table("Straße", 4, None),
            table("STRASSE", 5, None),
            table("K", 6, None),
        ],
        anomalies: vec![],
    };
    for _ in 0..schema.tables.len() {
        let index = PhysicalTableIndex::new(&schema);
        for alias in [
            "ExtDim",
            "EXTDIM",
            "сервис",
            "СЕРВИС",
            "İ",
            "i\u{307}",
            "Straße",
            "strasse",
            "k",
            "missing",
        ] {
            for number in [1, 2, 7, 99] {
                for kind in [
                    MetadataKind::ExtraDimension,
                    MetadataKind::ResolveOnlyService,
                    MetadataKind::Catalog,
                ] {
                    let entry = entry(alias, number);
                    assert_eq!(
                        index.table_for_entry(&entry, kind),
                        plain_physical_table_for_entry(&entry, kind, &schema),
                        "{alias}:{number}:{kind:?}"
                    );
                }
            }
        }
        for name in ["_СЕРВИС", "i\u{307}", "_k", "_missing"] {
            assert_eq!(
                index.schema_row(name).map(|p| &schema.tables[p]),
                schema.table(name)
            );
        }
        schema.tables.rotate_left(1);
    }
}
#[test]
fn dependency_indexes_preserve_first_eligible_owner_and_all_table_exclusions() {
    let names = DbNames::from_entries(vec![
        entry("Reference", 1),
        DbNameEntry {
            guid: "22222222-2222-4222-8222-222222222222".parse().unwrap(),
            ..entry("Reference", 1)
        },
    ]);
    let schema = SchemaStorage {
        tables: vec![],
        anomalies: vec![],
    };
    let mut objects = resolve_metadata(names, vec![], schema, vec![])
        .snapshot
        .objects()
        .to_vec();
    assert_eq!(objects.len(), 2);
    objects[0].kind = Some(MetadataKind::ResolveOnlyService);
    objects[0].physical_table = Some("_СЕРВИС".into());
    objects[1].physical_table = Some("_сервис".into());
    let mut duplicate = objects[1].clone();
    duplicate.physical_table = Some("_СЕРВИС".into());
    objects.push(duplicate);
    let (tables, owners) = dependency_indexes(&objects);
    for name in ["_сервис", "_СЕРВИС", "_absent"] {
        assert_eq!(
            tables.contains(&folded_name(name)),
            objects.iter().any(|o| o
                .physical_table
                .as_deref()
                .is_some_and(|table| names_equal(table, name)))
        );
        assert_eq!(
            owners.get(&folded_name(name)).copied(),
            objects
                .iter()
                .position(|o| o.kind.is_some_and(|kind| !kind.is_service())
                    && o.physical_table
                        .as_deref()
                        .is_some_and(|table| names_equal(table, name)))
        );
    }
    assert_eq!(owners.get(&folded_name("_СЕРВИС")), Some(&1));
}
