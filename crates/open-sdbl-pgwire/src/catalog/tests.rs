use super::*;
use crate::codec::FieldDescription;
fn model() -> Catalog {
    let candidate =
        |schema: &str, name: &str, oid: i64, view, fields: Vec<(&str, u32, i32)>| Candidate {
            schema: schema.into(),
            name: name.into(),
            query: format!(
                "ВЫБРАТЬ * ИЗ {schema}.{name}{}",
                if view { "()" } else { "" }
            ),
            oid,
            schema_oid: if view { 16385 } else { 16384 },
            view,
            comment: Some("Номенклатура".into()),
            columns: fields
                .into_iter()
                .enumerate()
                .map(|(i, (name, type_oid, type_modifier))| {
                    (
                        i as i64 + 1,
                        FieldDescription {
                            name: name.into(),
                            type_oid,
                            type_length: -1,
                            type_modifier,
                            format: 0,
                        },
                        Some(format!("Комментарий {name}")),
                    )
                })
                .collect(),
        };
    finish(
        vec![
            candidate(
                "Справочник",
                "Номенклатура",
                16386,
                false,
                vec![
                    ("Ссылка", 17, -1),
                    ("Наименование", 1043, 154),
                    ("Цена", 1700, (15 << 16 | 2) + 4),
                ],
            ),
            candidate(
                "Справочник",
                "Номенклатура.Товары",
                16388,
                false,
                vec![("НомерСтроки", 1700, (5 << 16) + 4)],
            ),
            candidate(
                "РегистрНакопления",
                "ЗамерОстатков.Остатки",
                16398,
                true,
                vec![("КоличествоОстаток", 1700, (15 << 16 | 2) + 4)],
            ),
        ],
        "tester",
        "1c",
        3,
    )
}
fn run(text: &str) -> Result<sql::Table, crate::codec::ErrorFields> {
    let q = sql::route(text).unwrap_or_else(|| panic!("catalog route: {text}"))?;
    sql::evaluate(&q, &model(), &[], &[])
}
#[test]
fn psql_corpus() {
    for (client, corpus) in [
        (
            "psql13",
            include_str!("../../tests/fixtures/catalog-corpus/psql13.sql"),
        ),
        (
            "psql18",
            include_str!("../../tests/fixtures/catalog-corpus/psql18.sql"),
        ),
    ] {
        for (i, text) in corpus.split("\n-- next\n").enumerate() {
            let result = run(text);
            assert!(
                result.is_ok(),
                "{client} statement {}: {:?}\n{text}",
                i + 1,
                result.err()
            );
        }
    }
}

#[test]
fn datagrip_lists_schemas_with_snapshot_revision() {
    let query = include_str!("../../tests/fixtures/catalog-corpus/datagrip-list-schemas.sql");
    let table = run(query).unwrap();
    assert_eq!(
        table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c.oid))
            .collect::<Vec<_>>(),
        vec![
            ("id", 20),
            ("state_number", 28),
            ("name", 19),
            ("description", 25),
            ("owner", 25)
        ]
    );
    assert_eq!(table.rows.len(), model().schemas.len());
    assert_eq!(table.rows[0][2], Value::from("public"));
    for row in &table.rows {
        assert!(matches!(row[0], Value::Int(id) if id > 0));
        assert!(matches!(row[1], Value::Int(revision) if revision >= 3));
        assert_eq!(row[3], Value::Null);
        assert_eq!(row[4], Value::from("tester"));
    }
}

#[test]
fn datagrip_pre_schema_catalog_queries_execute() {
    let queries = include_str!("../../tests/fixtures/catalog-corpus/datagrip-pre-schemas.sql");
    for sql in queries.split("\n-- next\n") {
        if sql.trim().eq_ignore_ascii_case("show DateStyle") {
            continue;
        }
        let table = run(sql).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        assert!(table.rows.iter().all(|r| r.len() == table.columns.len()));
    }
    for relation in [
        "pg_locks",
        "pg_auth_members",
        "pg_timezone_names",
        "pg_timezone_abbrevs",
        "pg_tablespace",
    ] {
        assert!(
            run(&format!("SELECT * FROM {relation}"))
                .unwrap()
                .rows
                .is_empty()
        );
    }
    assert_eq!(
        run("SELECT rolbypassrls FROM pg_roles").unwrap().rows,
        vec![vec![Value::Bool(false)]]
    );
    assert_eq!(
        run("SELECT xmin FROM pg_type WHERE oid=28").unwrap().rows,
        vec![vec![3.into()]]
    );
}

#[test]
fn datagrip_schema_object_queries_route_to_catalog_and_execute() {
    let queries = include_str!("../../tests/fixtures/catalog-corpus/datagrip-schema-objects.sql");
    for sql in queries.split("\n-- next\n") {
        let table = run(sql).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
        assert!(
            table
                .rows
                .iter()
                .all(|row| row.len() == table.columns.len())
        );
    }
    assert_eq!(
        run("SELECT pg_catalog.pg_is_in_recovery(),pg_catalog.txid_current()")
            .unwrap()
            .rows,
        vec![vec![Value::Bool(false), 3.into()]]
    );
    for name in [
        "pg_event_trigger",
        "pg_foreign_data_wrapper",
        "pg_foreign_server",
        "pg_user_mappings",
        "pg_language",
        "pg_cast",
        "pg_depend",
    ] {
        assert!(
            run(&format!("SELECT * FROM {name}"))
                .unwrap()
                .rows
                .is_empty()
        );
        assert!(
            run(&format!(
                "SELECT 'pg_catalog.{name}'::regclass FROM pg_namespace LIMIT 1"
            ))
            .is_ok()
        );
    }
}

#[test]
fn datagrip_full_schema_corpus() {
    let catalog = model();
    let corpus = include_str!("../../tests/fixtures/catalog-corpus/datagrip-full-schema.sql");
    for namespace in [11, 16384, 16385, 999999] {
        for (i, sql) in corpus.split("\n-- next\n").enumerate() {
            let sql = sql
                .replace("?::oid", &format!("{namespace}::oid"))
                .replace("?::varchar", "'3'::varchar");
            let query = sql::route(&sql)
                .expect("catalog route")
                .unwrap_or_else(|e| panic!("query {i}: {e:?}\n{sql}"));
            let result = sql::evaluate(&query, &catalog, &[], &[])
                .unwrap_or_else(|e| panic!("query {i}: {e:?}\n{sql}"));
            if i == 3 && namespace == 16384 {
                assert_eq!(result.rows.len(), 2, "native DataGrip table listing");
            }
        }
    }
}

#[test]
fn extension_version_table_shape_and_array_aggregate_semantics() {
    let table = run("SELECT * FROM pg_catalog.pg_available_extension_versions() v").unwrap();
    assert_eq!(
        table
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c.oid))
            .collect::<Vec<_>>(),
        vec![
            ("name", 19),
            ("version", 25),
            ("superuser", 16),
            ("trusted", 16),
            ("relocatable", 16),
            ("schema", 19),
            ("requires", 1003),
            ("comment", 25)
        ]
    );
    assert!(table.rows.is_empty());
    let table = run("SELECT array_agg(version) FROM pg_available_extension_versions()").unwrap();
    assert_eq!(table.columns[0].oid, 1009);
    assert_eq!(table.rows, vec![vec![Value::Null]]);
    let table=run("SELECT array_agg(v) FROM (VALUES(1),(NULL),(2)) t(v), pg_namespace n WHERE n.nspname='public'").unwrap();
    assert_eq!(table.columns[0].oid, 1007);
    assert_eq!(
        table.rows,
        vec![vec![Value::Array(vec![1.into(), Value::Null, 2.into()])]]
    );
    for (sql, code) in [
        (
            "SELECT missing FROM pg_available_extension_versions()",
            "42703",
        ),
        ("SELECT * FROM pg_available_extension_versions(1)", "42883"),
        ("SELECT array_agg(sum(1)) FROM pg_namespace", "42803"),
        ("SELECT array_agg(*) FROM pg_namespace", "42601"),
    ] {
        assert_eq!(run(sql).unwrap_err().code, code);
    }
    let oversized = format!(
        "SELECT array_agg('{}') FROM pg_catalog.generate_series(1,1024) g",
        "x".repeat(128 * 1024)
    );
    assert_eq!(run(&oversized).unwrap_err().code, "54001");
}

#[test]
fn pgjdbc_type_info_identifies_xid_and_acl_arrays() {
    let text = include_str!("../../tests/fixtures/catalog-corpus/pgjdbc-type-info.sql");
    let query = sql::route(text).unwrap().unwrap();
    let catalog = model();
    let (columns, parameters) = sql::describe(&query, &catalog, &[26]).unwrap();
    assert_eq!(parameters, vec![26]);
    assert_eq!(
        columns.iter().map(|c| c.oid).collect::<Vec<_>>(),
        vec![16, 18, 19, 26]
    );
    for (oid, array, name) in [(28, false, "xid"), (1034, true, "_aclitem")] {
        let table = sql::evaluate(&query, &catalog, &[Value::Int(oid)], &parameters).unwrap();
        assert_eq!(
            table.rows,
            vec![vec![
                Value::Bool(array),
                "b".into(),
                name.into(),
                oid.into()
            ]]
        );
    }
}

#[test]
fn using_keys_match_nulls_and_keep_qualified_and_star_shapes() {
    let from = " FROM (VALUES(10,1),(20,2),(30,NULL)) a(v,k) LEFT JOIN (VALUES(1,100),(3,300),(NULL,400)) b(k,w) USING(k)";
    let table = run(&format!(
        "SELECT *{from}, pg_namespace n WHERE n.nspname='public' ORDER BY v"
    ))
    .unwrap();
    assert_eq!(
        table
            .columns
            .iter()
            .take(3)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["k", "v", "w"]
    );
    assert_eq!(
        table
            .rows
            .iter()
            .map(|r| r[..3].to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![1.into(), 10.into(), 100.into()],
            vec![2.into(), 20.into(), Value::Null],
            vec![Value::Null, 30.into(), Value::Null]
        ]
    );
    let table = run(&format!(
        "SELECT k,a.k,b.k,a.*,b.*{from}, pg_namespace n WHERE n.nspname='public' ORDER BY v"
    ))
    .unwrap();
    assert_eq!(
        table.rows[1],
        vec![
            2.into(),
            2.into(),
            Value::Null,
            20.into(),
            2.into(),
            Value::Null,
            Value::Null
        ]
    );
    assert_eq!(
        run("SELECT * FROM (SELECT * FROM pg_namespace a JOIN pg_namespace b USING(oid)) x")
            .unwrap()
            .columns
            .len(),
        7
    );
    for (sql, code) in [
        (
            "SELECT * FROM pg_namespace a JOIN pg_namespace b USING(missing)",
            "42703",
        ),
        (
            "SELECT * FROM pg_namespace a JOIN pg_namespace b USING(oid,oid)",
            "42701",
        ),
        (
            "SELECT * FROM pg_namespace a JOIN pg_namespace b ON a.oid=b.oid USING(oid)",
            "42601",
        ),
        (
            "SELECT * FROM pg_namespace a,pg_namespace b JOIN pg_namespace c USING(oid)",
            "42702",
        ),
        (
            "SELECT * FROM pg_namespace a JOIN (SELECT 1 AS oid FROM pg_namespace) b USING(oid)",
            "0A000",
        ),
    ] {
        assert_eq!(run(sql).unwrap_err().code, code, "{sql}");
    }
    let table=run("SELECT 1 AS n FROM pg_namespace UNION DISTINCT SELECT 1 FROM pg_namespace UNION SELECT 2 ORDER BY n").unwrap();
    assert_eq!(table.rows, vec![vec![1.into()], vec![2.into()]]);
}

#[test]
fn indexed_join_keeps_or_left_and_mixed_runtime_value_semantics() {
    let table=run("SELECT a.oid,b.oid FROM pg_namespace a,pg_namespace b WHERE a.oid=b.oid OR b.nspname='public'").unwrap();
    assert_eq!(table.rows.len(), 2 * model().schemas.len() - 1);
    assert!(run("SELECT a.oid,b.oid FROM pg_namespace a LEFT JOIN pg_namespace b ON false WHERE a.oid=b.oid").unwrap().rows.is_empty());
    let table=run("SELECT a.k,b.k FROM (VALUES('1'),(1)) a(k) JOIN (VALUES('1'),(1)) b(k) USING(k),pg_namespace n WHERE n.nspname='public'").unwrap();
    assert_eq!(table.rows.len(), 4);
}

#[test]
fn namespace_system_column_preserves_star_and_derived_table_shapes() {
    for query in [
        "SELECT * FROM pg_namespace",
        "SELECT n.* FROM pg_namespace n",
        "SELECT * FROM (SELECT * FROM pg_namespace) n",
        "SELECT n.* FROM pg_namespace n(id, name, owner, acl)",
    ] {
        let table = run(query).unwrap();
        assert_eq!(table.columns.len(), 4, "{query}");
        assert!(table.rows.iter().all(|row| row.len() == 4));
    }
    let table = run("SELECT * FROM (SELECT xmin FROM pg_namespace) n").unwrap();
    assert_eq!(table.columns[0].name, "xmin");
    assert_eq!(table.columns[0].oid, 28);
    assert!(table.rows.iter().all(|row| row == &[Value::Int(3)]));
    assert_eq!(
        run("SELECT * FROM pg_namespace n(a,b,c,d,e)")
            .unwrap_err()
            .code,
        "42601"
    );
    let table = run("SELECT oid, typname, typlen, typcategory FROM pg_type WHERE oid=28").unwrap();
    assert_eq!(
        table.rows,
        vec![vec![28.into(), "xid".into(), 4.into(), "U".into()]]
    );
}

#[test]
fn xid_encoding_retains_unsigned_range_and_rejects_malformed_parameters() {
    for value in [0_u32, 3, i32::MAX as u32 + 1, u32::MAX] {
        let sql = format!("SELECT '{value}'::xid AS xid FROM pg_namespace LIMIT 1");
        let table = run(&sql).unwrap();
        let (fields, rows) = encode(&table, &[1]).unwrap();
        assert_eq!(fields[0].type_oid, 28);
        assert_eq!(fields[0].type_length, 4);
        assert_eq!(&rows[0][rows[0].len() - 4..], &value.to_be_bytes());
        for (binary, bytes) in [
            (false, value.to_string().into_bytes()),
            (true, value.to_be_bytes().to_vec()),
        ] {
            assert_eq!(
                decode(28, binary, Some(&bytes)).unwrap(),
                Value::Int(i64::from(value))
            );
        }
    }
    for (binary, bytes) in [
        (false, b"-1".as_slice()),
        (false, b"4294967296"),
        (true, b"\0\0\0"),
        (true, b"\0\0\0\0\0"),
    ] {
        assert_eq!(decode(28, binary, Some(bytes)).unwrap_err().code, "22P02");
    }
    assert_eq!(
        run("SELECT '-1'::xid FROM pg_namespace").unwrap_err().code,
        "22003"
    );
}
#[test]
fn joins_types_comments_and_aggregates() {
    let comment =
        run("SELECT obj_description(16386,'pg_class') FROM pg_namespace LIMIT 1").unwrap();
    assert_eq!(comment.rows[0], vec![Value::from("Номенклатура")]);
    let t=run("SELECT c.relname FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='Справочник' ORDER BY 1").unwrap();
    assert_eq!(
        t.rows,
        vec![
            vec![Value::from("Номенклатура")],
            vec![Value::from("Номенклатура.Товары")]
        ]
    );
    let t=run("SELECT a.attname, format_type(a.atttypid,a.atttypmod), col_description(a.attrelid,a.attnum) FROM pg_attribute a WHERE attrelid=16386 ORDER BY attnum").unwrap();
    assert_eq!(
        t.rows[1],
        vec![
            "Наименование".into(),
            "character varying(150)".into(),
            "Комментарий Наименование".into()
        ]
    );
    assert_eq!(t.rows[2][1], Value::from("numeric(15,2)"));
    let t = run("SELECT count(*) FROM pg_attribute WHERE attrelid=16386").unwrap();
    assert_eq!(t.rows, vec![vec![Value::Int(3)]]);
}
#[test]
fn routing_and_errors() {
    assert!(sql::route("ВЫБРАТЬ Наименование ИЗ Справочник.Номенклатура").is_none());
    assert!(sql::route("SELECT * FROM pg_class, _Reference42").is_none());
    assert!(sql::route("SELECT (SELECT * FROM _Reference42) FROM pg_class").is_none());
    let err = run("SELECT relname FROM pg_class WHERE").unwrap_err();
    assert_eq!(err.code, "42601");
    assert_eq!(err.position, Some(35));
    assert_eq!(
        run("SELECT pg_catalog.pg_stat_get_live_tuples(c.oid) FROM pg_class c")
            .unwrap_err()
            .code,
        "42883"
    );
    assert_eq!(
        run("SELECT missing FROM pg_index").unwrap_err().code,
        "42703"
    );
}
#[test]
fn extended_typing_without_evaluation() {
    let q = sql::route("SELECT oid FROM pg_type WHERE typname=$1")
        .unwrap()
        .unwrap();
    let (cols, params) = sql::describe(&q, &model(), &[]).unwrap();
    assert_eq!(params, vec![19]);
    assert_eq!(cols[0].oid, 26);
    let t = sql::evaluate(&q, &model(), &["numeric".into()], &params).unwrap();
    assert_eq!(t.rows, vec![vec![Value::Int(1700)]]);
    let q = sql::route("SELECT 1/0 FROM pg_class").unwrap().unwrap();
    assert!(sql::describe(&q, &model(), &[]).is_ok());
}
#[test]
fn postgres_literals_nulls_and_arrays() {
    let t=run("SELECT E'it\\'s', $$literal$$, NULL = 1, NULL AND false, ARRAY[1,NULL,3][2] FROM pg_namespace LIMIT 1").unwrap();
    assert_eq!(
        t.rows[0],
        vec![
            "it's".into(),
            "literal".into(),
            Value::Null,
            Value::Bool(false),
            Value::Null
        ]
    );
}

#[test]
fn regex_matches_postgres_13_reference() {
    for line in include_str!("../../tests/fixtures/catalog-corpus/regex-pg13.tsv").lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        assert_eq!(parts.len(), 4);
        // COPY text doubles backslashes.
        let unescape = |s: &str| s.replace("\\\\", "\\");
        let actual = sql::regex::matches(
            &unescape(parts[1]),
            &unescape(parts[0]),
            parts[2] == "t",
            &mut sql::Budget::new(),
        )
        .unwrap();
        assert_eq!(actual, parts[3] == "t", "{line}");
    }
}
#[test]
fn bounded_queries_and_precise_numbers() {
    let e=run("SELECT count(*) FROM pg_catalog.generate_series(1,1001) a CROSS JOIN pg_catalog.generate_series(1,1001) b").unwrap_err();
    assert_eq!(e.code, "54001");
    let flat = format!("SELECT {} FROM pg_type", vec!["1"; 10000].join("+"));
    assert_eq!(run(&flat).unwrap_err().code, "54001");
    let nested = format!("SELECT {}1{} FROM pg_type", "(".repeat(70), ")".repeat(70));
    assert_eq!(run(&nested).unwrap_err().code, "54001");
    assert_eq!(run("SELECT 1.20 < 1.3, 10000000000000000.01 > 10000000000000000.00, 1.5::int FROM pg_namespace LIMIT 1").unwrap().rows[0],vec![Value::Bool(true),Value::Bool(true),Value::Int(2)]);
    assert_eq!(
        run("SELECT c.oid FROM pg_class c,pg_type c")
            .unwrap_err()
            .code,
        "42702"
    );
    assert!(run("SELECT \"true\" FROM (SELECT 7 AS \"true\" FROM pg_namespace LIMIT 1) t").is_ok());
}

#[test]
fn union_slices_the_complete_result() {
    for (query, expected) in [
        (
            "SELECT 1 AS n FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 2 ORDER BY 1 LIMIT 1",
            vec![1],
        ),
        (
            "SELECT 3 AS n FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 1 UNION ALL SELECT 2 ORDER BY n LIMIT 1 OFFSET 1",
            vec![2],
        ),
        (
            "SELECT 3 AS n FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 1 UNION ALL SELECT 2 ORDER BY n DESC OFFSET 1",
            vec![2, 1],
        ),
        (
            "SELECT 1 FROM pg_namespace UNION ALL SELECT 2 LIMIT 0",
            vec![],
        ),
        (
            "SELECT 1 FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 2 LIMIT ALL OFFSET 2",
            vec![],
        ),
        (
            "SELECT 1 FROM pg_namespace UNION SELECT 1 UNION SELECT 2 ORDER BY 1 LIMIT 1 OFFSET 1",
            vec![2],
        ),
        (
            "SELECT t.n FROM (SELECT 3 AS n UNION ALL SELECT 1 UNION ALL SELECT 2 ORDER BY n LIMIT 1 OFFSET 1) t, pg_namespace WHERE nspname='public'",
            vec![2],
        ),
        (
            "SELECT t.n FROM (SELECT 3 AS n UNION ALL SELECT 1 ORDER BY n LIMIT 1) t UNION ALL SELECT 2 FROM pg_namespace WHERE nspname='public' ORDER BY n DESC LIMIT 1",
            vec![2],
        ),
    ] {
        assert_eq!(
            run(query).unwrap().rows,
            expected
                .into_iter()
                .map(|n| vec![Value::Int(n)])
                .collect::<Vec<_>>(),
            "{query}"
        );
    }
}

#[test]
fn concat_checks_intermediate_allocations() {
    let literal = "x".repeat(900_000);
    let args = vec!["v"; 80].join(",");
    let query = format!("WITH x AS (SELECT '{literal}' AS v) SELECT length(concat({args})) FROM x");
    assert_eq!(run(&query).unwrap_err().code, "54001");
    assert_eq!(
        run("SELECT concat(NULL, 'аб', 42, true, ARRAY['a\"b', 'c\\d', NULL])")
            .unwrap()
            .rows,
        vec![vec![Value::Text(
            "аб42t{\"a\\\"b\",\"c\\\\d\",NULL}".into()
        )]]
    );
}

use crate::tests::support;

#[test]
fn candidate_kinds_sections_and_identifiers() {
    let snapshot = support::tabular_section_snapshot();
    let all = candidates(&snapshot);
    assert!(
        all.iter()
            .any(|c| c.schema == "Документ" && c.name.ends_with(".ГрафикНачислений"))
    );
    assert!(all.iter().any(|c| c.schema == "Справочник"));
    let ids = all
        .iter()
        .map(|c| c.oid)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), all.len());
    let second = candidates(&snapshot);
    assert_eq!(
        all.iter().map(|c| (&c.name, c.oid)).collect::<Vec<_>>(),
        second.iter().map(|c| (&c.name, c.oid)).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "release-mode catalog construction measurement"]
fn catalog_build_10000_relations() {
    use open_sdbl::metadata::{DbNameEntry, DbNames, SchemaStorage, resolve_metadata};
    use open_sdbl::query::{
        AccessDecision, CompileOptions, PostgresBackend, PrepareOptions, QueryCompiler,
    };
    let mut entries = vec![];
    let mut descriptors = vec![];
    let mut tables = vec![];
    let mut live = vec![];
    for i in 1..=10000 {
        let guid = support::guid(&format!("00000000-0000-4000-8000-{i:012x}"));
        entries.push(DbNameEntry {
            guid: guid.clone(),
            alias: "Reference".into(),
            number: i,
        });
        descriptors.push(support::descriptor(&guid, &guid, &format!("Каталог{i}")));
        let table = format!("Reference{i}");
        tables.push(support::schema_table(
            &table,
            i,
            vec![support::schema_column("ID", "R", Some(&table))],
        ));
        live.push(support::live_table(&format!("_reference{i}"), &["_idrref"]));
    }
    let snapshot = resolve_metadata(
        DbNames::from_entries(entries),
        descriptors,
        SchemaStorage {
            tables,
            anomalies: vec![],
        },
        live,
    )
    .snapshot;
    let started = std::time::Instant::now();
    let mut all = candidates(&snapshot);
    for c in &mut all {
        let prepared = QueryCompiler::new(&snapshot, PostgresBackend)
            .prepare_with_options(&c.query, &PrepareOptions::new().restricted())
            .unwrap();
        let decisions = prepared
            .restriction_request()
            .targets
            .iter()
            .cloned()
            .map(AccessDecision::unrestricted)
            .collect::<Vec<_>>();
        let compiled = prepared
            .compile_with(&snapshot, &CompileOptions::new().decisions(&decisions))
            .unwrap();
        for (i, column) in compiled.columns.iter().enumerate() {
            let (field, _) = crate::values::describe_column(&column.label, &column.kind, 0);
            c.columns.push((i as i64 + 1, field, None));
        }
    }
    let catalog = finish(all, "test", "1c", 3);
    assert_eq!(
        catalog.tables["information_schema.tables"].rows.len(),
        10000
    );
    let elapsed = started.elapsed();
    eprintln!("10000-relation catalog with restricted compilation: {elapsed:?}");
    assert!(elapsed < std::time::Duration::from_secs(if cfg!(debug_assertions) { 120 } else { 1 }));
}

#[test]
fn relational_semantics_and_group_validation() {
    let coalesce = run("SELECT coalesce(NULL,false) FROM pg_namespace LIMIT 1").unwrap();
    assert_eq!(coalesce.columns[0].oid, 16);
    assert_eq!(coalesce.rows[0], vec![Value::Bool(false)]);
    assert_eq!(
        run("SELECT 70000::int2 FROM pg_namespace")
            .unwrap_err()
            .code,
        "22003"
    );
    let t=run("WITH cats AS (SELECT relname,relnamespace FROM pg_class WHERE relkind='r') SELECT n.nspname,count(*) FROM cats c JOIN pg_namespace n ON c.relnamespace=n.oid WHERE n.nspname='Справочник' GROUP BY n.nspname HAVING count(*)>0 ORDER BY 1 LIMIT 1 OFFSET 0").unwrap();
    assert_eq!(t.rows, vec![vec!["Справочник".into(), Value::Int(2)]]);
    let t=run("SELECT c.relname,(SELECT count(*) FROM pg_attribute a WHERE a.attrelid=c.oid) FROM pg_class c WHERE relname='Номенклатура'").unwrap();
    assert_eq!(t.rows, vec![vec!["Номенклатура".into(), Value::Int(3)]]);
    let t=run("SELECT relname FROM pg_class WHERE oid IN (SELECT attrelid FROM pg_attribute) UNION SELECT 'extra' ORDER BY 1").unwrap();
    assert_eq!(t.rows.len(), 4);
    assert_eq!(
        run("SELECT 'pg_class'::regclass::text FROM pg_namespace LIMIT 1")
            .unwrap()
            .rows[0],
        vec!["pg_class".into()]
    );
    assert_eq!(
        run("SELECT CASE WHEN true THEN 1 ELSE false END FROM pg_type")
            .unwrap_err()
            .code,
        "0A000"
    );
    assert_eq!(
        run("SELECT typname,count(*) FROM pg_type")
            .unwrap_err()
            .code,
        "42803"
    );
}

#[test]
fn different_visible_subsets_keep_common_oids() {
    use open_sdbl::query::{
        AccessDecision, CompileOptions, PostgresBackend, PrepareOptions, QueryCompiler,
    };
    let snapshot = support::tabular_section_snapshot();
    let mut all = candidates(&snapshot);
    for c in &mut all {
        let Ok(prepared) = QueryCompiler::new(&snapshot, PostgresBackend)
            .prepare_with_options(&c.query, &PrepareOptions::new().restricted())
        else {
            continue;
        };
        let decisions = prepared
            .restriction_request()
            .targets
            .iter()
            .cloned()
            .map(AccessDecision::unrestricted)
            .collect::<Vec<_>>();
        let Ok(compiled) =
            prepared.compile_with(&snapshot, &CompileOptions::new().decisions(&decisions))
        else {
            continue;
        };
        for (i, c2) in compiled.columns.iter().enumerate() {
            let (field, _) = crate::values::describe_column(&c2.label, &c2.kind, 0);
            c.columns.push((i as i64 + 1, field, None));
        }
    }
    all.retain(|c| !c.columns.is_empty());
    assert!(all.len() >= 2);
    let limited = finish(all[1..].to_vec(), "reader", "1c", 3);
    let full = finish(all, "owner", "1c", 3);
    for row in &limited.tables["pg_catalog.pg_class"].rows {
        assert!(
            full.tables["pg_catalog.pg_class"]
                .rows
                .iter()
                .any(|r| r[0] == row[0] && r[1] == row[1] && r[2] == row[2])
        );
    }
    assert!(
        limited.tables["pg_catalog.pg_class"].rows.len()
            < full.tables["pg_catalog.pg_class"].rows.len()
    );
}

#[test]
fn ordered_aggregates_natural_joins_and_scalar_functions() {
    assert_eq!(run("SELECT array_agg(v ORDER BY k DESC NULLS LAST) FROM (VALUES (1,2),(2,NULL),(NULL,3),(4,1)) t(v,k)").unwrap().rows,
        vec![vec![Value::Array(vec![Value::Null,1.into(),4.into(),2.into()])]]);
    assert_eq!(
        run("SELECT array_agg(v ORDER BY 1) FROM (VALUES(2),(1)) t(v)")
            .unwrap()
            .rows,
        vec![vec![Value::Array(vec![2.into(), 1.into()])]]
    );
    assert_eq!(
        run("SELECT string_agg(v,',' ORDER BY k) FROM (VALUES('b',2),('a',1)) t(v,k)")
            .unwrap()
            .rows,
        vec![vec!["a,b".into()]]
    );
    assert_eq!(
        run("SELECT array_agg(oid ORDER BY oid) FROM pg_namespace WHERE false")
            .unwrap()
            .rows,
        vec![vec![Value::Null]]
    );
    assert_eq!(run("SELECT * FROM (VALUES(1,'a'),(2,'b')) a(k,v) NATURAL LEFT JOIN (VALUES(1,'c'),(NULL,'d')) b(k,w) WHERE current_schema()='public' ORDER BY k").unwrap().rows,
        vec![vec![1.into(),"a".into(),"c".into()], vec![2.into(),"b".into(),Value::Null]]);
    assert_eq!(run("SELECT * FROM (VALUES(1)) a(x) NATURAL JOIN (VALUES(2)) b(y) WHERE current_schema()='public'").unwrap().rows,
        vec![vec![1.into(),2.into()]]);
    assert_eq!(
        run("SELECT unnest(ARRAY[1,NULL,2]) FROM pg_namespace WHERE nspname='public'")
            .unwrap()
            .rows,
        vec![vec![1.into()], vec![Value::Null], vec![2.into()]]
    );
    assert_eq!(run("SELECT DISTINCT unnest(ARRAY[2,2,1]) FROM pg_namespace WHERE nspname='public' ORDER BY 1").unwrap().rows,
        vec![vec![1.into()],vec![2.into()]]);
    let table = run("SELECT translate('абракадабра','абв','XY'), translate('aba','aa','XY'), greatest(NULL,2,5,-1),greatest(NULL,NULL), translate(NULL,'x','y') FROM pg_namespace LIMIT 1").unwrap();
    assert_eq!(
        table.rows,
        vec![vec![
            "XYрXкXдXYрX".into(),
            "XbX".into(),
            5.into(),
            Value::Null,
            Value::Null
        ]]
    );
    for (sql, code) in [
        (
            "SELECT array_agg(oid ORDER BY missing) FROM pg_namespace WHERE false",
            "42703",
        ),
        (
            "SELECT array_agg(oid ORDER BY sum(oid)) FROM pg_namespace",
            "42803",
        ),
        (
            "SELECT array_agg(sum(oid) ORDER BY oid) FROM pg_namespace",
            "42803",
        ),
        (
            "SELECT greatest(1, true) FROM pg_namespace WHERE false",
            "0A000",
        ),
        (
            "SELECT translate('a','b') FROM pg_namespace WHERE false",
            "42883",
        ),
        (
            "SELECT * FROM pg_namespace a NATURAL JOIN pg_namespace b ON true",
            "42601",
        ),
        (
            "SELECT * FROM (VALUES(1,2)) a(k,k) NATURAL JOIN (VALUES(1)) b(k), pg_namespace n",
            "42701",
        ),
    ] {
        assert_eq!(run(sql).unwrap_err().code, code, "{sql}");
    }
}

#[test]
fn disjunctive_index_preserves_matches_and_left_join_nulls() {
    assert_eq!(run("SELECT a.x,b.y,b.z FROM (VALUES(1),(2),(NULL),(9)) a(x) LEFT JOIN (VALUES(1,1),(1,2),(NULL,2)) b(y,z) ON a.x=b.y OR a.x=b.z ORDER BY a.x,b.y,b.z").unwrap().rows,
        vec![vec![1.into(),1.into(),1.into()],vec![1.into(),1.into(),2.into()],vec![2.into(),1.into(),2.into()],vec![2.into(),Value::Null,2.into()],vec![9.into(),Value::Null,Value::Null],vec![Value::Null,Value::Null,Value::Null]]);
    // Predicate pushdown must not remove the row before the later LEFT JOIN.
    assert_eq!(run("SELECT a.x,b.y FROM (VALUES(1)) a(x) LEFT JOIN (VALUES(2)) b(y) ON false WHERE b.y IS NULL").unwrap().rows,
        vec![vec![1.into(),Value::Null]]);
}

#[test]
fn projection_pruning_keeps_correlated_ordering_context() {
    assert_eq!(run("SELECT n.nspname FROM pg_namespace n WHERE n.nspname IN ('public','pg_catalog') ORDER BY (SELECT n.oid)").unwrap().rows,
        vec![vec!["pg_catalog".into()],vec!["public".into()]]);
    assert_eq!(
        run("SELECT unnest(unnest(ARRAY[1])) FROM pg_namespace")
            .unwrap_err()
            .code,
        "0A000"
    );
}
