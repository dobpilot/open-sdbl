use open_sdbl::metadata::{
    AllowedLength, LiveColumn, LiveIndex, LiveTable, MetadataKind, parse_config_descriptors,
    parse_db_names, parse_schema_storage, resolve_metadata,
};

/// Resolves minimal projections captured after publishing
/// `Catalog.OpenSdblMetadataProbe` to the disposable 8.3.27/PostgreSQL
/// conformance information base.
#[test]
fn resolves_the_catalog_and_attribute_verified_in_the_test_infobase() {
    let db_names = parse_db_names(&hex(
        "0dcab10d03310800c05d5c83f46f631b16c800d9003054518a6f2def9e5c7dbbc23636f5390c5d6e435a9391755e98a94d92570c2128efc878e2eb51a0b703bb769761ab61aa13528d441bd271928b26b5ce62505e9ff5ff74ce0f",
    ))
    .unwrap();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        ),
    )
    .unwrap();
    let schema = parse_schema_storage(
        br#"{0,{1,{"Reference53","N",53,"",{3,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0}},{0},{1,{"Code",1,{2,"Code","ID"},0,0,0,{0},0,0}},1,"R",{0},{0},"",0}}}"#,
    )
    .unwrap();
    let live_tables = vec![LiveTable {
        name: "_reference53".to_owned(),
        columns: vec![
            LiveColumn {
                name: "_idrref".to_owned(),
                data_type: "bytea".to_owned(),
            },
            LiveColumn {
                name: "_code".to_owned(),
                data_type: "mvarchar(9)".to_owned(),
            },
            LiveColumn {
                name: "_fld54".to_owned(),
                data_type: "bytea".to_owned(),
            },
        ],
        indexes: vec![LiveIndex {
            name: "_reference53_2".to_owned(),
            columns: vec!["_code".to_owned(), "_idrref".to_owned()],
            unique: true,
        }],
    }];

    let snapshot = resolve_metadata(db_names, descriptors, schema, live_tables);
    let object = snapshot
        .objects
        .iter()
        .find(|object| object.name.as_deref() == Some("OpenSdblMetadataProbe"))
        .unwrap();
    assert_eq!(object.kind, Some(MetadataKind::Catalog));
    assert_eq!(object.physical_table.as_deref(), Some("_Reference53"));
    assert!(object.declared && object.live);
    assert_eq!(object.code_allowed_length, Some(AllowedLength::Variable));

    let field = &snapshot.fields[0];
    assert_eq!(field.name.as_deref(), Some("ProbeAttribute"));
    assert_eq!(field.physical_name, "_Fld54");
    assert_eq!(field.owner_tables, ["_Reference53"]);
    assert!(field.declared && field.live);

    assert_eq!(snapshot.indexes[0].logical_key, ["Code", "ID"]);
    assert_eq!(
        snapshot.indexes[0].live_name.as_deref(),
        Some("_reference53_2")
    );
    assert!(snapshot.indexes[0].unique_matches);
}

fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
