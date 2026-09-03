#![allow(dead_code)]

mod hex;

use std::str::FromStr;

pub(crate) use hex::hex;

use open_sdbl::metadata::{
    ColumnType, ConfigDescriptor, ConfigFieldPurpose, ConfigPredefinedValue, Guid, LiveColumn,
    LiveIndex, LiveTable, MetadataKind, SchemaColumn, SchemaStorage, SchemaTable,
    parse_config_descriptors, parse_db_names, parse_schema_storage, resolve_metadata,
    resolve_metadata_with_predefined_values,
};

pub(crate) fn snapshot() -> open_sdbl::metadata::MetadataSnapshot {
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
        br#"{0,{1,{"Reference53","N",53,"",{4,{"ID",0,{1,{"R",0,0,"Reference53",2}},"",0},{"Code",0,{1,{"S",2147483657,0,"",0}},"",0},{"Date_Time",0,{1,{"T",0,0,"",0}},"",0},{"Fld54",0,{1,{"B",16,0,"",0}},"",0}},{0},{1,{"Code",1,{2,"Code","ID"},0,0,0,{0},0,0}},1,"R",{0},{0},"",0}}}"#,
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
                name: "_date_time".to_owned(),
                data_type: "timestamp without time zone".to_owned(),
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
    resolve_metadata(db_names, descriptors, schema, live_tables).snapshot
}

pub(crate) fn tabular_section_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let db_names = parse_db_names(&hex(
        "4d8dbb6a03311045ff45b5062cefac34ea4d20ad09e9f721b9b1d7109c6ad97fcf48cc4d728a832e9c417b087e9f659e9614675a729889d72424533a51add390abac2566f6eef25cbe1f657b393f0e87df83415ddc2498c0bbcf0fcd59f3b3415ddc2498c0bbb7fbaafda8fd605017370926401fb56783fe247801f449fbd1a02e6e124c805eb48f067571936002f459fb645017370926f0ee7dabcfebcdf978d21331a88b7f5ff20ffb2206edb3415ddc2498c0bb6ba9e5ab6c4bd1abb35e4d067571936002fc321cc70f",
    ))
    .unwrap();
    let parent = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let information_register = guid("77777777-7777-4777-8777-777777777777");
    let catalog = guid("99999999-9999-4999-8999-999999999999");
    let descriptors = vec![
        descriptor(&parent, &parent, "бит_ДополнительныеУсловияПоДоговору"),
        descriptor(
            &parent,
            &guid("11111111-1111-4111-8111-111111111111"),
            "ГрафикНачислений",
        ),
        descriptor(
            &parent,
            &guid("22222222-2222-4222-8222-222222222222"),
            "ЦФО",
        ),
        descriptor(
            &parent,
            &guid("33333333-3333-4333-8333-333333333333"),
            "СуммаБезНДС",
        ),
        descriptor(
            &parent,
            &guid("44444444-4444-4444-8444-444444444444"),
            "Сумма",
        ),
        descriptor(
            &parent,
            &guid("55555555-5555-4555-8555-555555555555"),
            "Период",
        ),
        descriptor(
            &parent,
            &guid("66666666-6666-4666-8666-666666666666"),
            "ДоговорКонтрагента",
        ),
        descriptor(
            &information_register,
            &information_register,
            "бит_СтатусыОбъектов",
        ),
        descriptor(
            &information_register,
            &guid("88888888-8888-4888-8888-888888888888"),
            "Объект",
        ),
        descriptor(&catalog, &catalog, "ЦентрыФинансовойОтветственности"),
        descriptor(
            &catalog,
            &guid("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            "Сам_БизнесРегион",
        ),
    ];
    let schema = SchemaStorage {
        tables: vec![
            schema_table(
                "Document53",
                53,
                vec![
                    schema_column("ID", "R", Some("Document53")),
                    schema_column("Fld59", "R", Some("Reference62")),
                ],
            ),
            schema_table(
                "Document53_VT54X1",
                54,
                vec![
                    schema_column("Document53_IDRRef", "R", Some("Document53")),
                    schema_column("LineNo54", "N", None),
                    schema_column("Fld55", "R", Some("Reference62")),
                    schema_column("Fld56", "N", None),
                    schema_column("Fld57", "N", None),
                    schema_column("Fld58", "T", None),
                ],
            ),
            schema_table(
                "InfoRg60",
                60,
                vec![schema_column("Fld61", "R", Some("Document53"))],
            ),
            schema_table(
                "Reference62",
                62,
                vec![
                    schema_column("ID", "R", Some("Reference62")),
                    schema_column("Fld63", "B", None),
                ],
            ),
        ],
        anomalies: Vec::new(),
    };
    let live_tables = vec![
        live_table("_document53", &["_idrref", "_fld59"]),
        live_table(
            "_document53_vt54X1",
            &[
                "_document53_idrref",
                "_lineno54",
                "_fld55",
                "_fld56",
                "_fld57",
                "_fld58",
            ],
        ),
        live_table(
            "_inforg60",
            &["_fld61_type", "_fld61_rtref", "_fld61_rrref"],
        ),
        live_table("_reference62", &["_idrref", "_fld63"]),
    ];
    resolve_metadata(db_names, descriptors, schema, live_tables).snapshot
}

pub(crate) fn dereferenced_presentation_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = tabular_section_snapshot();
    let business_region = snapshot
        .schema
        .tables
        .iter_mut()
        .find(|table| table.name == "Reference62")
        .unwrap()
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld63")
        .unwrap();
    business_region.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Reference62".to_owned()),
    }];
    snapshot
}

pub(crate) fn universal_dereferenced_presentation_snapshot() -> open_sdbl::metadata::MetadataSnapshot
{
    let mut snapshot = tabular_section_snapshot();
    let agreement = snapshot
        .schema
        .tables
        .iter_mut()
        .find(|table| table.name == "Document53")
        .unwrap()
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld59")
        .unwrap();
    agreement.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some(String::new()),
    }];
    let document = snapshot
        .live_tables
        .iter_mut()
        .find(|table| table.name == "_document53")
        .unwrap();
    document.columns.retain(|column| column.name != "_fld59");
    document.columns.extend(
        ["_fld59_type", "_fld59_rtref", "_fld59_rrref"]
            .into_iter()
            .map(|name| LiveColumn {
                name: name.to_owned(),
                data_type: "bytea".to_owned(),
            }),
    );
    snapshot
}

pub(crate) fn enumeration_value_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let owner = guid("c8b21fea-1e3d-4ae9-8719-7ff4db08af97");
    let value = guid("d2f8bde9-fadd-4be8-9022-249e3a1ac4b9");
    let db_names = parse_db_names(&hex(
        "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500",
    ))
    .unwrap();
    let mut status = descriptor(&owner, &value, "Статус");
    status.enumeration_value = true;
    resolve_metadata(
        db_names,
        vec![
            descriptor(&owner, &owner, "бит_ВидыСтатусовОбъектов"),
            status,
        ],
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        vec![live_table("_enum99", &["_idrref", "_enumorder"])],
    )
    .snapshot
}

pub(crate) fn catalog_value_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let base = snapshot();
    let owner = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let mut live_tables = base.live_tables.clone();
    live_tables[0].columns.push(LiveColumn {
        name: "_predefinedid".to_owned(),
        data_type: "bytea".to_owned(),
    });
    resolve_metadata_with_predefined_values(
        base.db_names.clone(),
        base.descriptors.clone(),
        vec![
            ConfigPredefinedValue {
                owner_guid: owner.clone(),
                value_guid: guid("2e22ad88-32b5-4456-a3da-e56fa2f94623"),
                name: "Утвержден".to_owned(),
            },
            ConfigPredefinedValue {
                owner_guid: owner,
                value_guid: guid("f6fa6a92-32a3-4378-a161-ed47a2787c5a"),
                name: "ДополнительныеУсловияПоДоговору_Проверен".to_owned(),
            },
        ],
        base.schema.clone(),
        live_tables,
    )
    .snapshot
}

pub(crate) fn guid(value: &str) -> Guid {
    Guid::from_str(value).unwrap()
}

pub(crate) fn descriptor(resource: &Guid, object: &Guid, name: &str) -> ConfigDescriptor {
    ConfigDescriptor {
        resource_guid: resource.clone(),
        object_guid: object.clone(),
        marker: "1".to_owned(),
        name: name.to_owned(),
        synonyms: Vec::new(),
        comment: None,
        field_purpose: None,
        enumeration_value: false,
    }
}

pub(crate) fn schema_table(name: &str, number: u32, columns: Vec<SchemaColumn>) -> SchemaTable {
    SchemaTable {
        name: name.to_owned(),
        number,
        columns,
        indexes: Vec::new(),
    }
}

pub(crate) fn schema_column(name: &str, tag: &str, reference_target: Option<&str>) -> SchemaColumn {
    SchemaColumn {
        name: name.to_owned(),
        types: vec![ColumnType {
            tag: tag.to_owned(),
            reference_target: reference_target.map(str::to_owned),
        }],
    }
}

pub(crate) fn live_table(name: &str, columns: &[&str]) -> LiveTable {
    LiveTable {
        name: name.to_owned(),
        columns: columns
            .iter()
            .map(|column| LiveColumn {
                name: (*column).to_owned(),
                data_type: "bytea".to_owned(),
            })
            .collect(),
        indexes: Vec::new(),
    }
}

pub(crate) fn mssql_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    for table in &mut snapshot.live_tables {
        for column in &mut table.columns {
            column.data_type = match column.data_type.as_str() {
                "bytea" => "binary(16)".to_owned(),
                "mvarchar(9)" => "nvarchar(9)".to_owned(),
                "timestamp without time zone" => "datetime2".to_owned(),
                other => other.to_owned(),
            };
        }
    }
    snapshot
}

pub(crate) fn reference_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    snapshot.fields[0].name = Some("Организация".to_owned());
    let source_field = snapshot.schema.tables[0]
        .columns
        .iter_mut()
        .find(|column| column.name == "Fld54")
        .unwrap();
    source_field.types = vec![ColumnType {
        tag: "R".to_owned(),
        reference_target: Some("Reference57".to_owned()),
    }];

    let mut target_object = snapshot.objects[0].clone();
    target_object.name = Some("Организации".to_owned());
    target_object.number = Some(57);
    target_object.physical_table = Some("_Reference57".to_owned());
    snapshot.objects.push(target_object);

    let mut target_schema = snapshot.schema.tables[0].clone();
    target_schema.name = "Reference57".to_owned();
    target_schema.number = 57;
    target_schema
        .columns
        .retain(|column| column.name != "Fld54");
    snapshot.schema.tables.push(target_schema);

    let mut target_live = snapshot.live_tables[0].clone();
    target_live.name = "_reference57".to_owned();
    target_live.columns.retain(|column| column.name != "_fld54");
    snapshot.live_tables.push(target_live);
    snapshot
}

pub(crate) fn information_register_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    let object = &mut snapshot.objects[0];
    object.kind = Some(MetadataKind::InformationRegister);
    object.name = Some("Prices".to_owned());
    object.physical_table = Some("_InfoRg53".to_owned());

    let schema = &mut snapshot.schema.tables[0];
    schema.name = "InfoRg53".to_owned();
    let date = schema
        .columns
        .iter_mut()
        .find(|column| column.name == "Date_Time")
        .unwrap();
    date.name = "Period".to_owned();

    let live = &mut snapshot.live_tables[0];
    live.name = "_inforg53".to_owned();
    let date = live
        .columns
        .iter_mut()
        .find(|column| column.name == "_date_time")
        .unwrap();
    date.name = "_period".to_owned();

    let field = &mut snapshot.fields[0];
    field.owner_tables = vec!["_InfoRg53".to_owned()];
    field.purpose = Some(ConfigFieldPurpose::InformationRegisterDimension);
    snapshot
}

pub(crate) fn accumulation_register_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let mut snapshot = snapshot();
    snapshot.db_names = parse_db_names(&hex(
        "95cbb11142210c00d05da8933bf9249094360ee0b94012c0464b2b8edd3d0b07f8fd7b8b60b9b845ab8ea1d9917a13146b179cd38a4ee9a32a41ba467cdef767022efbe47924e0ba611d1c5abd179c1684748c895e95b151a84d2a2cea906eaf9e8069c36a5d8af33170fe12556ba8ae8c212377b1c12de7bfe7bdbf",
    ))
    .unwrap();
    let object = &mut snapshot.objects[0];
    object.kind = Some(MetadataKind::AccumulationRegister);
    object.name = Some("Остатки".to_owned());
    object.physical_table = Some("_AccumRg53".to_owned());

    let schema = &mut snapshot.schema.tables[0];
    schema.name = "AccumRg53".to_owned();
    schema
        .columns
        .retain(|column| !matches!(column.name.as_str(), "ID" | "Code"));
    let date = schema
        .columns
        .iter_mut()
        .find(|column| column.name == "Date_Time")
        .unwrap();
    date.name = "Period".to_owned();
    schema.columns.extend([
        SchemaColumn {
            name: "Active".to_owned(),
            types: vec![ColumnType {
                tag: "L".to_owned(),
                reference_target: None,
            }],
        },
        SchemaColumn {
            name: "RecordKind".to_owned(),
            types: vec![ColumnType {
                tag: "N".to_owned(),
                reference_target: None,
            }],
        },
        SchemaColumn {
            name: "Fld55".to_owned(),
            types: vec![ColumnType {
                tag: "N".to_owned(),
                reference_target: None,
            }],
        },
    ]);

    let live = &mut snapshot.live_tables[0];
    live.name = "_accumrg53".to_owned();
    live.columns
        .retain(|column| !matches!(column.name.as_str(), "_idrref" | "_code"));
    let date = live
        .columns
        .iter_mut()
        .find(|column| column.name == "_date_time")
        .unwrap();
    date.name = "_period".to_owned();
    live.columns.extend([
        LiveColumn {
            name: "_active".to_owned(),
            data_type: "boolean".to_owned(),
        },
        LiveColumn {
            name: "_recordkind".to_owned(),
            data_type: "numeric(1,0)".to_owned(),
        },
        LiveColumn {
            name: "_fld55".to_owned(),
            data_type: "numeric(10,2)".to_owned(),
        },
    ]);

    let dimension = &mut snapshot.fields[0];
    dimension.name = Some("Номенклатура".to_owned());
    dimension.owner_tables = vec!["_AccumRg53".to_owned()];
    dimension.purpose = Some(ConfigFieldPurpose::AccumulationRegisterDimension);
    let mut resource = dimension.clone();
    resource.name = Some("Количество".to_owned());
    resource.number = 55;
    resource.physical_name = "_Fld55".to_owned();
    resource.purpose = Some(ConfigFieldPurpose::AccumulationRegisterResource);
    snapshot.fields.push(resource);

    let mut totals_schema = snapshot.schema.tables[0].clone();
    totals_schema.name = "AccumRgT56".to_owned();
    totals_schema.number = 56;
    totals_schema
        .columns
        .retain(|column| matches!(column.name.as_str(), "Period" | "Fld54" | "Fld55"));
    totals_schema.columns.push(SchemaColumn {
        name: "Splitter".to_owned(),
        types: vec![ColumnType {
            tag: "N".to_owned(),
            reference_target: None,
        }],
    });
    totals_schema.indexes.clear();
    snapshot.schema.tables.push(totals_schema);

    let mut totals_live = snapshot.live_tables[0].clone();
    totals_live.name = "_accumrgt56".to_owned();
    totals_live
        .columns
        .retain(|column| matches!(column.name.as_str(), "_period" | "_fld54" | "_fld55"));
    totals_live.columns.push(LiveColumn {
        name: "_splitter".to_owned(),
        data_type: "numeric(10,0)".to_owned(),
    });
    totals_live.indexes.clear();
    snapshot.live_tables.push(totals_live);
    snapshot
}

pub(crate) fn presentation_reference_snapshot(
    multiple: bool,
) -> open_sdbl::metadata::MetadataSnapshot {
    let db_names = parse_db_names(&hex(if multiple {
        "55ce3112c3200c04c0bf5073333612207d200fc80f10a02a9322adc77f0f850b7bebbbb93b381e26d67a2d86aebb81471548ab1bdc1ba9cb98453986f7f4f99bdf3e43cc74c623e5aec506c15b67709a0e2b9a51b96b73a62c6a31bc3e63e579e5f70bd2022622083323df3c57ea6a950bea021659df5415ede6d992f3fc03"
    } else {
        "55cd310e82210c40e1bb30d3c49f16682fe001bc012ded641c5c097797c141bff9256f615eca3aac3705934b816667e0d16f10315082a737a19c1e1efef69779ca15775ea59a349d08318c808a0768930a9d4c46105616cde9fe9ca7a7d35f5f500e2044042622a83ffe2f7def0f"
    }))
    .unwrap();
    let descriptors = parse_config_descriptors(
        "b8bac76b-c91b-4d78-8a70-ffa39f8de694",
        &hex(
            "4d8d4b0ac3201400af22ae7d9018a3be650f505ae809def303857e426256c1bb37d850ba9e6166d36adb7ad529f64cc15986803d8389ce8327d741ce3460f631593455c9cb945eb7c88f732a14a9d0757e73926a4fc879955f2e965d10cfc31053536a3d467a0c68390e902918303a65608b23381390b219468fbc8f5af854ca7ce7b5fc1f1a10f423b5d60f",
        ),
    )
    .unwrap();
    let extra_type = if multiple {
        ",{\"R\",0,0,\"Reference58\",2}"
    } else {
        ""
    };
    let third_table = if multiple {
        r#",{"Reference58","N",58,"",{2,{"ID",0,{1,{"R",0,0,"Reference58",2}},"",0},{"Code",0,{1,{"S",10,0,"",0}},"",0}},{0},{0},1,"R",{0},{0},"",0}"#
    } else {
        ""
    };
    let table_count = if multiple { 3 } else { 2 };
    let schema_text = format!(
        r#"{{0,{{{table_count},{{"Reference53","N",53,"",{{2,{{"ID",0,{{1,{{"R",0,0,"Reference53",2}}}},"",0}},{{"Fld54",0,{{{},{{"R",0,0,"Reference57",2}}{extra_type}}},"",0}}}},{{0}},{{0}},1,"R",{{0}},{{0}},"",0}},{{"Reference57","N",57,"",{{2,{{"ID",0,{{1,{{"R",0,0,"Reference57",2}}}},"",0}},{{"Code",0,{{1,{{"S",10,0,"",0}}}},"",0}}}},{{0}},{{0}},1,"R",{{0}},{{0}},"",0}}{third_table}}}}}"#,
        if multiple { 2 } else { 1 }
    );
    let schema = parse_schema_storage(schema_text.as_bytes()).unwrap();
    let mut live_tables = vec![
        LiveTable {
            name: "_reference53".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_fld54_rtref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_fld54_rrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
            ],
            indexes: Vec::new(),
        },
        LiveTable {
            name: "_reference57".to_owned(),
            columns: vec![
                LiveColumn {
                    name: "_idrref".to_owned(),
                    data_type: "bytea".to_owned(),
                },
                LiveColumn {
                    name: "_code".to_owned(),
                    data_type: "mvarchar(10)".to_owned(),
                },
            ],
            indexes: Vec::new(),
        },
    ];
    if multiple {
        let mut target = live_tables[1].clone();
        target.name = "_reference58".to_owned();
        live_tables.push(target);
    }
    resolve_metadata(db_names, descriptors, schema, live_tables).snapshot
}

pub(crate) fn stored_deflate(value: &[u8]) -> Vec<u8> {
    let length = u16::try_from(value.len()).expect("test fixture must fit one stored block");
    let mut compressed = Vec::with_capacity(value.len() + 5);
    compressed.push(1);
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(value);
    compressed
}

pub(crate) fn ambiguous_object_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let first = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let second = guid("11111111-1111-4111-8111-111111111111");
    let serialized = format!("{{2,{{{first},\"Reference\",53}},{{{second},\"Reference\",57}}}}");
    let db_names = parse_db_names(&stored_deflate(serialized.as_bytes())).unwrap();
    let descriptors = vec![
        descriptor(&first, &first, "Duplicate"),
        descriptor(&second, &second, "Duplicate"),
    ];
    let schema = SchemaStorage {
        tables: vec![
            schema_table(
                "Reference53",
                53,
                vec![schema_column("ID", "R", Some("Reference53"))],
            ),
            schema_table(
                "Reference57",
                57,
                vec![schema_column("ID", "R", Some("Reference57"))],
            ),
        ],
        anomalies: Vec::new(),
    };
    resolve_metadata(
        db_names,
        descriptors,
        schema,
        vec![
            live_table("_reference53", &["_idrref"]),
            live_table("_reference57", &["_idrref"]),
        ],
    )
    .snapshot
}

pub(crate) fn ambiguous_field_snapshot() -> open_sdbl::metadata::MetadataSnapshot {
    let owner = guid("b8bac76b-c91b-4d78-8a70-ffa39f8de694");
    let field = guid("03bd775a-e0a1-4205-82ce-6068e73ad134");
    let serialized =
        format!("{{3,{{{owner},\"Reference\",53}},{{{field},\"Fld\",54}},{{{field},\"Fld\",55}}}}");
    let db_names = parse_db_names(&stored_deflate(serialized.as_bytes())).unwrap();
    let schema = SchemaStorage {
        tables: vec![schema_table(
            "Reference53",
            53,
            vec![
                schema_column("ID", "R", Some("Reference53")),
                schema_column("Fld54", "B", None),
                schema_column("Fld55", "B", None),
            ],
        )],
        anomalies: Vec::new(),
    };
    resolve_metadata(
        db_names,
        vec![
            descriptor(&owner, &owner, "OpenSdblMetadataProbe"),
            descriptor(&owner, &field, "DuplicateField"),
        ],
        schema,
        vec![live_table("_reference53", &["_idrref", "_fld54", "_fld55"])],
    )
    .snapshot
}
