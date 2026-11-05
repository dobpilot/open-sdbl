//! Native invoice descriptors with synthetic physical placement for query tests.
#![allow(dead_code)]
use open_sdbl::metadata::{
    ColumnType, DbNameEntry, DbNames, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaColumn,
    SchemaStorage, SchemaTable, parse_config_descriptors, resolve_metadata,
};

pub const OWNER: &str = "bfd8a708-f45e-44e7-bcd0-d6d6b985ef12";
pub const SECTION: &str = "e971571b-9a5a-49eb-b1ac-5fad520ff270";
pub const ITEM: &str = "bdea0c48-64db-4877-bf49-28b5e3d7938e";
pub const QUERY: &str = "выбрать уникальныйИдентификатор(Ссылка) КАК Гуид, ПредставлениеСсылки(Ссылка) ИЗ Документ.ПриходнаяНакладная.Запасы;";
pub const ITEM_QUERY: &str = "выбрать уникальныйИдентификатор(Ссылка) КАК Гуид, ПредставлениеСсылки(Ссылка),Номенклатура ИЗ Документ.ПриходнаяНакладная.Запасы;";

pub fn snapshot() -> MetadataSnapshot {
    let pack = include_bytes!("../fixtures/unf/config.pack");
    let mut offset = 0;
    let descriptors = loop {
        let end = offset + pack[offset..].iter().position(|b| *b == b'\n').unwrap();
        let (id, size) = std::str::from_utf8(&pack[offset..end])
            .unwrap()
            .split_once('\t')
            .unwrap();
        let size: usize = size.parse().unwrap();
        offset = end + 1;
        let bytes = &pack[offset..offset + size];
        offset += size;
        if id == OWNER {
            break parse_config_descriptors(id, bytes).unwrap();
        }
        assert!(offset < pack.len(), "native invoice resource exists");
    };
    let owner: Guid = OWNER.parse().unwrap();
    let section: Guid = SECTION.parse().unwrap();
    let tables = [
        (
            "Document1",
            vec![
                ("ID", "R", "bytea"),
                ("Number", "S", "mvarchar(11)"),
                ("Date_Time", "T", "timestamp without time zone"),
            ],
        ),
        (
            "Document1_VT2",
            vec![
                ("Document1_IDRRef", "R", "bytea"),
                ("LineNo2", "N", "numeric(9,0)"),
                ("Fld3", "R", "bytea"),
            ],
        ),
    ];
    let schema = SchemaStorage {
        tables: tables
            .iter()
            .enumerate()
            .map(|(i, (name, columns))| SchemaTable {
                name: (*name).into(),
                number: i as u32 + 1,
                owner: None,
                inline_name: None,
                columns: columns
                    .iter()
                    .map(|(name, tag, _)| SchemaColumn {
                        name: (*name).into(),
                        types: vec![ColumnType {
                            tag: (*tag).into(),
                            reference_target: (*tag == "R" && *name != "Fld3")
                                .then(|| "Document1".into()),
                        }],
                    })
                    .collect(),
                indexes: vec![],
            })
            .collect(),
        anomalies: vec![],
    };
    let live = tables
        .iter()
        .map(|(name, columns)| LiveTable {
            name: format!("_{name}").to_ascii_lowercase(),
            columns: columns
                .iter()
                .map(|(name, _, data_type)| LiveColumn {
                    name: if *name == "ID" {
                        "_idrref".into()
                    } else {
                        format!("_{name}").to_ascii_lowercase()
                    },
                    data_type: (*data_type).into(),
                })
                .collect(),
            indexes: vec![],
        })
        .collect();
    resolve_metadata(
        DbNames::from_entries(vec![
            DbNameEntry {
                guid: owner,
                alias: "Document".into(),
                number: 1,
            },
            DbNameEntry {
                guid: section,
                alias: "VT".into(),
                number: 2,
            },
            DbNameEntry {
                guid: ITEM.parse().unwrap(),
                alias: "Fld".into(),
                number: 3,
            },
        ]),
        descriptors,
        schema,
        live,
    )
    .snapshot
}
