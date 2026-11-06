//! Catalog statements through real client protocols; the database journal
//! proves no client catalog SQL or generated SQL was executed.
use super::*;
use tokio_postgres::error::SqlState;
use tokio_postgres::{Config, NoTls, SimpleQueryMessage};

async fn connect(harness: &Harness) -> tokio_postgres::Client {
    let mut config = Config::new();
    config.user(USER).password(PASSWORD).dbname("catalog_test");
    let (client, connection) = config.connect_raw(harness.connect(), NoTls).await.unwrap();
    tokio::spawn(connection);
    client
}

#[derive(Debug, PartialEq, Eq)]
struct Xid(u32);
impl<'a> tokio_postgres::types::FromSql<'a> for Xid {
    fn from_sql(
        _: &tokio_postgres::types::Type,
        bytes: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(u32::from_be_bytes(bytes.try_into()?)))
    }
    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        *ty == tokio_postgres::types::Type::XID
    }
}

#[tokio::test]
async fn datagrip_schema_listing_keeps_visibility_types_and_refresh_isolation() {
    use tokio_postgres::types::Type;
    let sql = include_str!("../../tests/fixtures/catalog-corpus/datagrip-list-schemas.sql");
    let harness = Harness::new(Options::default());
    let mut first = connect(&harness).await;
    let second = connect(&harness).await;
    let old_revision = harness.state.current().catalog_revision();
    let statement = first.prepare(sql).await.unwrap();
    assert_eq!(
        statement
            .columns()
            .iter()
            .map(|c| c.type_())
            .collect::<Vec<_>>(),
        vec![
            &Type::INT8,
            &Type::XID,
            &Type::NAME,
            &Type::TEXT,
            &Type::TEXT
        ]
    );
    let before = first.query(&statement, &[]).await.unwrap();
    assert_eq!(before[0].get::<_, String>(2), "public");
    assert!(
        before
            .iter()
            .any(|r| r.get::<_, String>(2) == "РегистрНакопления")
    );
    assert!(
        before
            .iter()
            .all(|r| r.get::<_, Xid>(1) == Xid(old_revision))
    );
    let other = second.query(sql, &[]).await.unwrap();
    assert!(
        other
            .iter()
            .all(|r| r.get::<_, Xid>(1) == Xid(old_revision))
    );
    let simple = first.simple_query(sql).await.unwrap();
    assert_eq!(
        simple
            .iter()
            .filter(|m| matches!(m, SimpleQueryMessage::Row(_)))
            .count(),
        before.len()
    );
    let transaction = first.transaction().await.unwrap();
    let portal = transaction.bind(&statement, &[]).await.unwrap();
    let snapshot = fixture::snapshot();
    let access = fixture::store(&snapshot, false);
    harness.state.replace(GatewayState::from_parts(
        snapshot,
        access,
        DatabaseDialect::Postgres,
    ));
    let new_revision = harness.state.current().catalog_revision();
    assert_ne!(new_revision, old_revision);
    let after = transaction.query(sql, &[]).await.unwrap();
    assert_eq!(after.len(), 3);
    assert!(
        after
            .iter()
            .all(|r| r.get::<_, Xid>(1) == Xid(new_revision))
    );
    assert!(
        !after
            .iter()
            .any(|r| r.get::<_, String>(2) == "РегистрНакопления")
    );
    let retained = transaction.query_portal(&portal, 0).await.unwrap();
    assert_eq!(retained.len(), before.len());
    assert!(
        retained
            .iter()
            .all(|r| r.get::<_, Xid>(1) == Xid(old_revision))
    );
    transaction.rollback().await.unwrap();
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn catalog_parameter_types_survive_binary_bind() {
    use tokio_postgres::types::Type;
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    for sql in [
        "SELECT $1 FROM pg_namespace LIMIT 1",
        "SELECT (SELECT $1) FROM pg_namespace LIMIT 1",
        "SELECT v FROM (SELECT $1 AS v) t, pg_namespace LIMIT 1",
        "WITH t AS (SELECT $1 AS v) SELECT v FROM t, pg_namespace LIMIT 1",
    ] {
        let statement = client.prepare_typed(sql, &[Type::INT4]).await.unwrap();
        assert_eq!(statement.params(), &[Type::INT4], "{sql}");
        assert_eq!(statement.columns()[0].type_(), &Type::INT4, "{sql}");
        let row = client.query_one(&statement, &[&42_i32]).await.unwrap();
        assert_eq!(row.get::<_, i32>(0), 42, "{sql}");
        let row = client
            .query_one(&statement, &[&Option::<i32>::None])
            .await
            .unwrap();
        assert_eq!(row.get::<_, Option<i32>>(0), None, "{sql}");
    }
    let statement = client
        .prepare_typed(
            "SELECT $1::text, $1::int8, ARRAY[$1] FROM pg_namespace LIMIT 1",
            &[Type::INT4],
        )
        .await
        .unwrap();
    assert_eq!(statement.params(), &[Type::INT4]);
    assert_eq!(
        statement
            .columns()
            .iter()
            .map(|c| c.type_().clone())
            .collect::<Vec<_>>(),
        vec![Type::TEXT, Type::INT8, Type::INT4_ARRAY]
    );
    let row = client.query_one(&statement, &[&42_i32]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "42");
    assert_eq!(row.get::<_, i64>(1), 42);
    assert_eq!(row.get::<_, Vec<i32>>(2), vec![42]);
    let statement = client
        .prepare("SELECT $1 FROM pg_type WHERE oid=$1")
        .await
        .unwrap();
    assert_eq!(statement.params(), &[Type::OID]);
    assert_eq!(statement.columns()[0].type_(), &Type::OID);
    assert_eq!(
        client
            .query_one(&statement, &[&23_u32])
            .await
            .unwrap()
            .get::<_, u32>(0),
        23
    );
    let statement = client
        .prepare("SELECT $1::int8 FROM pg_namespace LIMIT 1")
        .await
        .unwrap();
    assert_eq!(statement.params(), &[Type::INT8]);
    assert_eq!(
        client
            .query_one(&statement, &[&i64::MAX])
            .await
            .unwrap()
            .get::<_, i64>(0),
        i64::MAX
    );
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn catalog_concat_budget_error_keeps_connection_usable() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let args = vec!["$1"; 80].join(",");
    let statement = client
        .prepare(&format!(
            "SELECT length(concat({args})) FROM pg_namespace LIMIT 1"
        ))
        .await
        .unwrap();
    let error = client
        .query(&statement, &[&"x".repeat(900_000)])
        .await
        .unwrap_err();
    assert_eq!(error.code().map(SqlState::code), Some("54001"));
    let row = client
        .query_one(
            "SELECT length(concat('a', NULL, 'б')) FROM pg_namespace LIMIT 1",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i32>(0), 2);
    let rows = client.query("SELECT 1 AS n FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 2 ORDER BY n LIMIT 1 OFFSET 1", &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, i32>(0), 2);
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
async fn catalog_protocol_types_and_visibility() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let schemas = client
        .simple_query("SELECT nspname FROM pg_catalog.pg_namespace ORDER BY 1")
        .await
        .unwrap();
    let names = schemas
        .iter()
        .filter_map(|m| {
            if let SimpleQueryMessage::Row(r) = m {
                r.get(0)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(names.contains(&"РегистрНакопления"), "{names:?}");
    let views = client
        .query("SELECT relname FROM pg_class WHERE relkind='v'", &[])
        .await
        .unwrap();
    let views = views
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert!(views.contains(&"ЗамерОстатков.Обороты".into()), "{views:?}");
    let rows = client
        .query(
            "SELECT oid, relname FROM pg_catalog.pg_class WHERE relname=$1",
            &[&"ЗамерОстатков"],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let oid: u32 = rows[0].get(0);
    assert!(oid >= 16384);
    let query=client.prepare("SELECT attname,atttypid,atttypmod FROM pg_attribute WHERE attrelid=$1 AND attnum>0 ORDER BY attnum").await.unwrap();
    assert_eq!(query.params()[0], tokio_postgres::types::Type::OID);
    let columns = client.query(&query, &[&oid]).await.unwrap();
    assert!(!columns.is_empty());
    for row in &columns {
        let name: &str = row.get(0);
        let oid: u32 = row.get(1);
        let sdbl = client
            .prepare(&format!(
                "ВЫБРАТЬ {name} ИЗ РегистрНакопления.ЗамерОстатков"
            ))
            .await
            .unwrap();
        assert_eq!(sdbl.columns()[0].type_().oid(), oid, "{name}");
    }
    let arr = client
        .query_one("SELECT ARRAY[1,2,3] FROM pg_namespace LIMIT 1", &[])
        .await
        .unwrap();
    assert_eq!(arr.get::<_, Vec<i32>>(0), vec![1, 2, 3]);
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
async fn denied_objects_never_appear_and_oids_stay_stable() {
    let allowed = Harness::new(Options::default());
    let denied = Harness::new(Options {
        read: false,
        ..Options::default()
    });
    let a = connect(&allowed).await;
    let d = connect(&denied).await;
    let query = "SELECT oid FROM pg_class WHERE relname='ЗамерОстатков'";
    let rows = a.query(query, &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(d.query(query, &[]).await.unwrap().is_empty());
    assert!(
        d.query(
            "SELECT * FROM information_schema.columns WHERE table_schema='РегистрНакопления'",
            &[]
        )
        .await
        .unwrap()
        .is_empty()
    );
    let first: u32 = rows[0].get(0);
    let a2 = connect(&allowed).await;
    assert_eq!(
        a2.query(query, &[]).await.unwrap()[0].get::<_, u32>(0),
        first
    );
    assert!(allowed.statements().is_empty());
    assert!(denied.statements().is_empty());
    allowed.stop();
    denied.stop();
}
#[tokio::test]
async fn catalog_errors_and_refresh() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let e = client
        .simple_query("SELECT relname FROM pg_class WHERE")
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some(&SqlState::SYNTAX_ERROR));
    let e = client
        .simple_query("SELECT pg_catalog.pg_stat_get_live_tuples(oid) FROM pg_class")
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some(&SqlState::UNDEFINED_FUNCTION));
    let before = client
        .query(
            "SELECT relname FROM pg_class WHERE relname='ЗамерОстатков'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(before.len(), 1);
    let snapshot = fixture::snapshot();
    let access = fixture::store(&snapshot, false);
    harness.state.replace(GatewayState::from_parts(
        snapshot,
        access,
        DatabaseDialect::Postgres,
    ));
    assert!(
        client
            .query(
                "SELECT relname FROM pg_class WHERE relname='ЗамерОстатков'",
                &[]
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
async fn catalog_inside_transaction() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    client.simple_query("BEGIN").await.unwrap();
    client
        .simple_query("SELECT relname FROM pg_class")
        .await
        .unwrap();
    let path = client.simple_query("SHOW search_path").await.unwrap();
    assert!(path.iter().any(|m|matches!(m,SimpleQueryMessage::Row(r) if r.get(0).is_some_and(|s|s.contains("РегистрНакопления")))));
    client.simple_query("ROLLBACK").await.unwrap();
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
#[ignore = "requires local psql and a loopback listener"]
async fn psql_catalog_introspection() {
    let harness = Harness::new(Options::default());
    let stdout=psql_script(&harness,"\\dn\n\\dt\n\\dv\n\\d \"РегистрНакопления\".\"ЗамерОстатков\"\n\\d+ \"РегистрНакопления\".\"ЗамерОстатков\"\n\\dv РегистрНакопления.*\n\\dT\nSELECT 1 AS union_review_marker FROM pg_namespace WHERE nspname='public' UNION ALL SELECT 2 ORDER BY 1 LIMIT 1 OFFSET 1;\n").await;
    assert!(stdout.contains("ЗамерОстатков"), "{stdout}");
    assert!(stdout.contains("Категория"), "{stdout}");
    assert!(stdout.contains("union_review_marker"), "{stdout}");
    assert!(harness.statements().is_empty());
    harness.stop();
}
pub(super) async fn psql_script(harness: &Harness, script: &str) -> String {
    let server = super::psql::Server::start(harness).await;
    let output = server
        .run(&["-v", "ON_ERROR_STOP=1", "-f", "-"], Some(script))
        .await;
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{}\n{stdout}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

#[tokio::test]
#[ignore = "requires psql and a loopback listener"]
async fn psql_datagrip_pre_schema_sequence() {
    let harness = Harness::new(Options::default());
    let queries = include_str!("../../tests/fixtures/catalog-corpus/datagrip-pre-schemas.sql");
    // Place delimiters on their own line: a recorded query may end with --.
    let script = queries
        .split("\n-- next\n")
        .collect::<Vec<_>>()
        .join("\n;\n")
        + "\n;\n";
    let output = psql_script(&harness, &script).await;
    assert!(output.contains("РегистрНакопления"));
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
#[ignore = "requires psql and a loopback listener"]
async fn psql_datagrip_full_schema_sequence() {
    let harness = Harness::new(Options::default());
    let queries = include_str!("../../tests/fixtures/catalog-corpus/datagrip-full-schema.sql");
    let script = queries
        .replace(
            "?::oid",
            "(SELECT oid FROM pg_namespace WHERE nspname='РегистрНакопления')::oid",
        )
        .replace("?::varchar", "'0'::varchar")
        .split("\n-- next\n")
        .collect::<Vec<_>>()
        .join("\n;\n")
        + "\n;\n";
    let output = psql_script(&harness, &script).await;
    assert!(output.contains("ЗамерОстатков"));
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn restricted_objects_and_hidden_fields() {
    let snapshot = fixture::snapshot();
    let access = fixture::restricted_store(&snapshot, "ГДЕ Категория = \"К0\"");
    let harness = Harness::new(Options {
        access: Some(access),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let relation = client
        .query(
            "SELECT oid FROM pg_class WHERE relname='ЗамерОстатков'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(relation.len(), 1);
    let columns = client
        .query(
            "SELECT column_name FROM information_schema.columns WHERE table_name='ЗамерОстатков'",
            &[],
        )
        .await
        .unwrap();
    let names = columns
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert!(names.contains(&"Категория".into()));
    assert!(!names.contains(&"Регистратор".into()));
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
async fn catalog_survives_database_session_loss() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    client.simple_query("BEGIN").await.unwrap();
    harness.journal.lock().unwrap().dead = true;
    let rows = client
        .query("SELECT oid FROM pg_type WHERE typname='numeric'", &[])
        .await
        .unwrap();
    assert_eq!(rows[0].get::<_, u32>(0), 1700);
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn budget_error_keeps_connection_usable() {
    let harness = Harness::new(Options::default());
    let client = connect(&harness).await;
    let error=client.simple_query("SELECT count(*) FROM pg_catalog.generate_series(1,1001) a CROSS JOIN pg_catalog.generate_series(1,1001) b").await.unwrap_err();
    assert_eq!(error.code().map(SqlState::code), Some("54001"));
    assert_eq!(
        client
            .query_one("SELECT oid FROM pg_type WHERE typname=$1", &[&"numeric"])
            .await
            .unwrap()
            .get::<_, u32>(0),
        1700
    );
    let array = vec![Some(1_i32), None, Some(3_i32)];
    let row = client
        .query_one("SELECT $1::int4[] FROM pg_namespace LIMIT 1", &[&array])
        .await
        .unwrap();
    assert_eq!(row.get::<_, Vec<Option<i32>>>(0), array);
    assert!(harness.statements().is_empty());
    harness.stop();
}

use super::support as populated_support;

fn populated_harness(objects: u32, attributes: u32) -> Harness {
    populated_harness_with_access(objects, attributes, false)
}

fn populated_harness_with_access(
    objects: u32,
    attributes: u32,
    unrestricted_access: bool,
) -> Harness {
    use open_sdbl::metadata::Guid;
    let snapshot = populated_support::populated_catalog_snapshot(objects, attributes);
    assert_eq!(snapshot.fields().len(), (objects * attributes) as usize);
    let base = fixture::snapshot();
    let known = fixture::store(&base, true);
    let mut access = AccessStore::new(&snapshot);
    access.set_users(known.users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, true);
    rights.set_for_new_objects = true;
    rights.set_for_attributes_by_default = true;
    rights.objects.clear();
    rights.templates.clear();
    access.insert_rights(fixture::ROLE.parse::<Guid>().unwrap(), rights);
    Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        unrestricted_access,
        ..Options::default()
    })
}

#[tokio::test]
async fn populated_table_listing_avoids_cartesian_catalog_allocation() {
    let harness = populated_harness(1000, 20);
    let client = connect(&harness).await;
    assert!(
        client
            .query_one("SELECT count(*) FROM pg_description", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
            >= 20_000
    );
    let rows=client.query("SELECT n.nspname,c.relname,d.description FROM pg_namespace n,pg_class c LEFT JOIN pg_description d ON c.oid=d.objoid AND d.objsubid=0 AND d.classoid='pg_class'::regclass WHERE c.relnamespace=n.oid AND n.nspname='Справочник' ORDER BY c.relname",&[]).await.unwrap();
    assert_eq!(rows.len(), 1000);
    assert!(rows.iter().all(|r| r.get::<_, Option<String>>(2).is_some()));
    assert!(harness.statements().is_empty());
    harness.stop();
}
#[tokio::test]
async fn populated_datagrip_full_schema_introspection() {
    let harness = populated_harness(1000, 20);
    let client = connect(&harness).await;
    let namespace: u32 = client
        .query_one(
            "SELECT oid FROM pg_namespace WHERE nspname='Справочник'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let corpus = include_str!("../../tests/fixtures/catalog-corpus/datagrip-full-schema.sql");
    for (i, text) in corpus.split("\n-- next\n").enumerate() {
        let sql = text
            .replace("?::oid", &format!("{namespace}::oid"))
            .replace("?::varchar", "'0'::varchar");
        let rows = client
            .query(&sql, &[])
            .await
            .unwrap_or_else(|e| panic!("full schema query {i}: {e:?}"));
        if i == 3 {
            assert_eq!(rows.len(), 1000, "native DataGrip tables");
        }
        if i == 22 {
            assert_eq!(rows.len(), 1000, "native DataGrip comment targets");
        }
        if i == 24 {
            assert_eq!(
                rows.len(),
                21_000,
                "attribute ACLs include reference fields"
            );
        }
    }
    assert!(harness.statements().is_empty());
    harness.stop();
}

async fn populated_catalog(
    objects: u32,
    attributes: u32,
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    populated_catalog_with_access(objects, attributes, false).await
}

async fn populated_catalog_with_access(
    objects: u32,
    attributes: u32,
    unrestricted: bool,
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    let started = std::time::Instant::now();
    let harness = populated_harness_with_access(objects, attributes, unrestricted);
    let initialization = started.elapsed();
    let text = "SELECT c.relname FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='Справочник' AND c.relkind='r' ORDER BY 1";
    let client = connect(&harness).await;
    let started = std::time::Instant::now();
    let first = client.simple_query(text).await.unwrap();
    let cold = started.elapsed();
    assert_eq!(
        first
            .iter()
            .filter(|m| matches!(m, SimpleQueryMessage::Row(_)))
            .count(),
        objects as usize
    );
    let started = std::time::Instant::now();
    let second = client.simple_query(text).await.unwrap();
    let warm = started.elapsed();
    assert_eq!(
        second
            .iter()
            .filter(|m| matches!(m, SimpleQueryMessage::Row(_)))
            .count(),
        objects as usize
    );
    let count = client
        .query_one(
            "SELECT count(*) FROM information_schema.columns WHERE table_schema='Справочник'",
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(count, i64::from(objects) * i64::from(attributes + 1));
    let other = connect(&harness).await;
    let rows = other.simple_query(text).await.unwrap();
    assert_eq!(
        rows.iter()
            .filter(|m| matches!(m, SimpleQueryMessage::Row(_)))
            .count(),
        objects as usize
    );
    assert!(harness.statements().is_empty());
    harness.stop();
    (initialization, cold, warm)
}
#[tokio::test]
async fn populated_catalog_reads_loaded_snapshot_only() {
    let _ = populated_catalog(64, 16).await;
}
#[tokio::test]
#[ignore = "release-mode prebuilt catalog benchmark with populated attributes and rights filtering"]
async fn populated_catalog_first_and_repeated_request() {
    let (initialization, cold, warm) = populated_catalog(1000, 20).await;
    eprintln!(
        "1000 relations / 20000 custom attributes: initialization {initialization:?}, first {cold:?}, repeat {warm:?}"
    );
    assert!(cold < std::time::Duration::from_secs(1));
    assert!(warm < std::time::Duration::from_secs(1));
}

#[tokio::test]
#[ignore = "release-mode shared catalog benchmark without rights filtering"]
async fn populated_unrestricted_catalog_first_and_repeated_request() {
    let (initialization, cold, warm) = populated_catalog_with_access(1000, 20, true).await;
    eprintln!(
        "unrestricted: 1000 relations / 20000 custom attributes: initialization {initialization:?}, first {cold:?}, repeat {warm:?}"
    );
    assert!(cold < std::time::Duration::from_secs(1));
    assert!(warm < std::time::Duration::from_secs(1));
}

#[tokio::test]
#[ignore = "requires local psql and a loopback listener; populated snapshot regression"]
async fn psql_populated_catalog_introspection() {
    let harness = populated_harness(1000, 20);
    let started = std::time::Instant::now();
    let stdout = psql_script(&harness, "\\dt\n\\d \"Справочник\".\"Каталог1000\"\n\\dt\n").await;
    assert!(harness.statements().is_empty());
    eprintln!(
        "real psql: first/repeated \\dt and \\d on 1000 tables / 20000 attributes: {:?}",
        started.elapsed()
    );
    assert!(stdout.contains("Каталог1000"));
    assert!(stdout.contains("Реквизит20"));
    harness.stop();
}

fn additional_metadata_harness(deny_task: bool) -> Harness {
    use super::support::{descriptor, guid, schema_column, schema_table};
    use open_sdbl::metadata::{
        DbNameEntry, DbNames, LiveColumn, LiveTable, MetadataKind, SchemaStorage, resolve_metadata,
    };
    let kinds = [
        MetadataKind::Task,
        MetadataKind::AccountingRegister,
        MetadataKind::BusinessProcess,
        MetadataKind::ExchangePlan,
    ];
    let mut entries = Vec::new();
    let mut descriptors = Vec::new();
    let mut tables = Vec::new();
    let mut live = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        let id = guid(&format!("40000000-0000-4000-8000-{:012x}", i + 1));
        let table = format!("{}{}", kind.alias(), i + 1);
        entries.push(DbNameEntry {
            guid: id.clone(),
            alias: kind.alias().into(),
            number: (i + 1) as u32,
        });
        descriptors.push(descriptor(&id, &id, &format!("Объект{}", i + 1)));
        let (name, tag, physical, ty) = if *kind == MetadataKind::AccountingRegister {
            ("Period", "D", "_period", "timestamp without time zone")
        } else {
            ("ID", "R", "_idrref", "bytea")
        };
        tables.push(schema_table(
            &table,
            (i + 1) as u32,
            vec![schema_column(name, tag, Some(&table))],
        ));
        live.push(LiveTable {
            name: format!("_{table}").to_lowercase(),
            columns: vec![LiveColumn {
                name: physical.into(),
                data_type: ty.into(),
            }],
            indexes: vec![],
        });
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
    metadata_visibility_harness(snapshot, deny_task)
}

fn metadata_visibility_harness(
    snapshot: open_sdbl::metadata::MetadataSnapshot,
    deny_task: bool,
) -> Harness {
    use open_sdbl::metadata::MetadataKind;
    let base = fixture::snapshot();
    let known = fixture::store(&base, true);
    let mut access = AccessStore::new(&snapshot);
    access.set_users(known.users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, true);
    let grant = rights.objects[0].clone();
    rights.objects = snapshot
        .objects()
        .iter()
        .map(|o| {
            let mut g = grant.clone();
            g.object = o.guid.clone();
            if deny_task && o.kind == Some(MetadataKind::Task) {
                g.rights[0].allowed = false;
            }
            g
        })
        .collect();
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights);
    Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        ..Options::default()
    })
}

#[tokio::test]
async fn readable_metadata_kinds_remain_visible_without_authorized_columns() {
    for denied in [false, true] {
        let harness = additional_metadata_harness(denied);
        let client = connect(&harness).await;
        let rows=client.query("SELECT n.nspname,c.relname,c.relnatts FROM pg_class c JOIN pg_namespace n ON c.relnamespace=n.oid WHERE n.nspname NOT IN ('pg_catalog','information_schema','public') ORDER BY n.nspname",&[]).await.unwrap();
        assert_eq!(rows.len(), if denied { 3 } else { 4 });
        for (schema, name) in [
            ("Задача", "Объект1"),
            ("РегистрБухгалтерии", "Объект2"),
            ("БизнесПроцесс", "Объект3"),
            ("ПланОбмена", "Объект4"),
        ] {
            assert_eq!(
                rows.iter()
                    .any(|r| r.get::<_, String>(0) == schema && r.get::<_, String>(1) == name),
                !(denied && schema == "Задача")
            );
        }
        assert!(rows.iter().all(|r| r.get::<_, i16>(2) == 0));
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM information_schema.columns", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        let error = client
            .query("ВЫБРАТЬ Ссылка ИЗ БизнесПроцесс.Объект3", &[])
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some(&SqlState::INSUFFICIENT_PRIVILEGE));
        assert!(harness.statements().is_empty());
        harness.stop();
    }
}

#[tokio::test]
async fn catalog_reference_columns_are_russian_and_queryable() {
    let harness = populated_harness(1, 1);
    let client = connect(&harness).await;
    let rows=client.query("SELECT column_name FROM information_schema.columns WHERE table_schema='Справочник' AND table_name='Каталог1' ORDER BY ordinal_position",&[]).await.unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r.get::<_, String>(0))
            .collect::<Vec<_>>(),
        vec!["Ссылка", "Реквизит1"]
    );
    let statement = client
        .prepare("ВЫБРАТЬ * ИЗ Справочник.Каталог1")
        .await
        .unwrap();
    assert_eq!(statement.columns()[0].name(), "Ссылка");
    let alias = client
        .prepare("ВЫБРАТЬ Ссылка КАК ID ИЗ Справочник.Каталог1")
        .await
        .unwrap();
    assert_eq!(alias.columns()[0].name(), "ID");
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn relation_visibility_does_not_require_row_compilation_context() {
    let snapshot = super::support::separators_resolved_at(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/separators"),
    )
    .snapshot;
    let object = snapshot
        .objects()
        .iter()
        .find(|o| o.kind == Some(open_sdbl::metadata::MetadataKind::Catalog) && o.live)
        .unwrap();
    let name = open_sdbl::query::object_query_name(&snapshot, object).unwrap();
    let sql = format!("ВЫБРАТЬ * ИЗ {name}");
    let error = open_sdbl::query::QueryCompiler::new(&snapshot, open_sdbl::query::PostgresBackend)
        .compile(&sql)
        .unwrap_err();
    assert!(error.to_string().contains("separator"), "{error}");
    let (schema, table) = name.split_once('.').unwrap();
    let schema = schema.to_owned();
    let table = table.to_owned();
    let harness = metadata_visibility_harness(snapshot, false);
    let client = connect(&harness).await;
    assert_eq!(client.query_one("SELECT count(*) FROM information_schema.tables WHERE table_schema=$1 AND table_name=$2",&[&schema,&table]).await.unwrap().get::<_,i64>(0),1);
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn independent_separator_does_not_remove_catalog_columns() {
    let snapshot = super::support::separators_resolved_at(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/separators"),
    )
    .snapshot;
    let mut parameters = SessionParameters::new();
    parameters.set(QueryParameter::new(
        "ИспользованиеРазделителя",
        ParameterValue::Boolean(true),
    ));
    parameters.set(QueryParameter::new(
        "ЗначениеРазделителя",
        ParameterValue::Number {
            unscaled: 7,
            scale: 0,
        },
    ));
    let expected =
        open_sdbl::query::QueryCompiler::new(&snapshot, open_sdbl::query::PostgresBackend)
            .compile_with(
                "ВЫБРАТЬ * ИЗ Справочник.Товары",
                &open_sdbl::query::CompileOptions::new().session(&parameters),
            )
            .unwrap()
            .columns
            .len();
    assert!(expected > 0);
    let base = fixture::snapshot();
    let known = fixture::store(&base, true);
    let mut access = AccessStore::new(&snapshot);
    access.set_users(known.users().unwrap().to_vec());
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        operator: parameters,
        unrestricted_access: true,
        ..Options::default()
    });
    let client = connect(&harness).await;
    let count: i64 = client.query_one("SELECT count(*) FROM information_schema.columns WHERE table_schema='Справочник' AND table_name='Товары'", &[]).await.unwrap().get(0);
    assert_eq!(count as usize, expected);
    harness.stop();
}

#[tokio::test]
async fn embedded_state_uses_configured_database_name() {
    let harness = Harness::new(Options {
        database_name: "erp_uf",
        unrestricted_access: true,
        ..Options::default()
    });
    let client = connect(&harness).await;
    let database: String = client
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(database, "erp_uf");
    for table in [
        "information_schema.schemata",
        "information_schema.tables",
        "information_schema.views",
        "information_schema.columns",
    ] {
        let sql = format!("SELECT count(*) FROM {table} WHERE table_catalog <> 'erp_uf'");
        let column = if table.ends_with("schemata") {
            "catalog_name"
        } else {
            "table_catalog"
        };
        let sql = sql.replace("table_catalog", column);
        let count: i64 = client.query_one(&sql, &[]).await.unwrap().get(0);
        assert_eq!(count, 0, "{table}");
    }
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_catalog.pg_database WHERE datname = 'erp_uf'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    harness.stop();
}

#[tokio::test]
async fn an_owner_read_grant_does_not_expose_a_denied_section() {
    let snapshot = super::section_reference::snapshot();
    let base = fixture::snapshot();
    let known = fixture::store(&base, true);
    let mut access = AccessStore::new(&snapshot);
    access.set_users(known.users().unwrap().to_vec());
    let mut rights = fixture::rights(&base, true);
    rights.objects[0].object = super::section_reference::OWNER.parse().unwrap();
    let mut denied = rights.objects[0].clone();
    denied.object = super::section_reference::SECTION.parse().unwrap();
    denied.rights[0].allowed = false;
    rights.objects.push(denied);
    access.insert_rights(fixture::ROLE.parse().unwrap(), rights);
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        ..Options::default()
    });
    let client = connect(&harness).await;
    let rows=client.query("SELECT relname FROM pg_class WHERE relname IN ('ПриходнаяНакладная','ПриходнаяНакладная.Запасы')",&[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), "ПриходнаяНакладная");
    assert!(harness.statements().is_empty());
    harness.stop();
}

#[tokio::test]
async fn dbeaver_columns_for_one_table_fit_catalog_budget() {
    let harness = populated_harness(5000, 20);
    let client = connect(&harness).await;
    let oid: u32 = client.query_one(
        "SELECT c.oid FROM pg_class c INNER JOIN pg_namespace n ON c.relnamespace=n.oid WHERE n.nspname='Справочник' ORDER BY c.oid LIMIT 1", &[]
    ).await.unwrap().get(0);
    let query = format!(
        r#"SELECT c.relname,a.*,pg_catalog.pg_get_expr(ad.adbin, ad.adrelid, true) as def_value,dsc.description,dep.objid
FROM pg_catalog.pg_attribute a
INNER JOIN pg_catalog.pg_class c ON (a.attrelid=c.oid)
LEFT OUTER JOIN pg_catalog.pg_attrdef ad ON (a.attrelid=ad.adrelid AND a.attnum = ad.adnum)
LEFT OUTER JOIN pg_catalog.pg_description dsc ON (c.oid=dsc.objoid AND a.attnum = dsc.objsubid)
LEFT OUTER JOIN pg_depend dep on dep.refobjid = a.attrelid AND dep.deptype = 'i' and dep.refobjsubid = a.attnum and dep.classid = dep.refclassid
WHERE NOT a.attisdropped AND c.relkind not in ('i','I','c') AND c.oid={oid}
ORDER BY a.attnum"#
    );
    let rows = client.simple_query(&query).await.unwrap();
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row, tokio_postgres::SimpleQueryMessage::Row(_)))
            .count(),
        21
    );
    harness.stop();
}

#[tokio::test]
async fn pushed_oid_filter_preserves_inner_and_outer_join_results() {
    let harness = populated_harness(4, 3);
    let client = connect(&harness).await;
    let oid: u32 = client.query_one("SELECT c.oid FROM pg_class c JOIN pg_namespace n ON c.relnamespace=n.oid WHERE n.nspname='Справочник' ORDER BY c.oid LIMIT 1", &[]).await.unwrap().get(0);
    let prefix = "SELECT a.attnum, a.attname, d.description FROM pg_attribute a JOIN pg_class c ON a.attrelid=c.oid LEFT JOIN pg_description d ON c.oid=d.objoid AND a.attnum=d.objsubid";
    let equal = client
        .query(
            &format!("{prefix} WHERE c.oid={oid} ORDER BY a.attnum"),
            &[],
        )
        .await
        .unwrap();
    let in_list = client
        .query(
            &format!("{prefix} WHERE c.oid IN ({oid}) ORDER BY a.attnum"),
            &[],
        )
        .await
        .unwrap();
    let project = |rows: &[tokio_postgres::Row]| {
        rows.iter()
            .map(|row| {
                (
                    row.get::<_, i16>(0),
                    row.get::<_, String>(1),
                    row.get::<_, Option<String>>(2),
                )
            })
            .collect::<Vec<_>>()
    };
    assert!(!equal.is_empty());
    assert_eq!(project(&equal), project(&in_list));
    let absent = client
        .query(&format!("{prefix} WHERE c.oid=0 ORDER BY a.attnum"), &[])
        .await
        .unwrap();
    assert!(absent.is_empty());
    harness.stop();
}

#[tokio::test]
async fn configured_catalog_memory_budget_applies_to_both_protocols() {
    let query = "SELECT c.oid FROM pg_class c";
    let limited = Harness::new(Options {
        limits: crate::GatewayLimits {
            catalog_allocation_bytes: 1,
            ..Options::default().limits
        },
        ..Options::default()
    });
    let client = connect(&limited).await;
    let error = client.simple_query(query).await.unwrap_err();
    assert_eq!(error.code().unwrap().code(), "54001");
    let error = client.query(query, &[]).await.unwrap_err();
    assert_eq!(error.code().unwrap().code(), "54001");
    limited.stop();

    let permitted = Harness::new(Options {
        limits: crate::GatewayLimits {
            catalog_allocation_bytes: 1024 * 1024,
            ..Options::default().limits
        },
        ..Options::default()
    });
    let client = connect(&permitted).await;
    assert!(!client.simple_query(query).await.unwrap().is_empty());
    assert!(!client.query(query, &[]).await.unwrap().is_empty());
    permitted.stop();
}

#[tokio::test]
async fn raising_catalog_budget_allows_statement_over_default_limit() {
    let query = format!(
        "SELECT length(concat({})) FROM pg_namespace WHERE nspname='public'",
        vec!["$1"; 80].join(",")
    );
    let input = "x".repeat(900_000);
    let standard = Harness::new(Options::default());
    let client = connect(&standard).await;
    let error = client.query(&query, &[&input]).await.unwrap_err();
    assert_eq!(error.code().unwrap().code(), "54001");
    standard.stop();

    let expanded = Harness::new(Options {
        limits: crate::GatewayLimits {
            catalog_allocation_bytes: 256 * 1024 * 1024,
            ..Options::default().limits
        },
        ..Options::default()
    });
    let client = connect(&expanded).await;
    let result: i32 = client.query_one(&query, &[&input]).await.unwrap().get(0);
    assert_eq!(result, 72_000_000);
    expanded.stop();
}

#[tokio::test]
async fn unrestricted_access_exposes_denied_objects_and_columns() {
    let snapshot = fixture::snapshot();
    let mut access = AccessStore::new(&snapshot);
    access.set_users(fixture::store(&snapshot, false).users().unwrap().to_vec());
    // No role rights are loaded, as in the unrestricted acquisition path.
    let harness = Harness::new(Options {
        snapshot: Some(snapshot),
        access: Some(access),
        unrestricted_access: true,
        ..Options::default()
    });
    let client = connect(&harness).await;
    let rows = client
        .query(
            "SELECT oid FROM pg_class WHERE relname='ЗамерОстатков'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let rows = client.query("SELECT column_name FROM information_schema.columns WHERE table_schema='РегистрНакопления' AND table_name='ЗамерОстатков'", &[]).await.unwrap();
    assert!(
        rows.iter()
            .any(|row| row.get::<_, String>(0) == "Категория")
    );
    assert!(
        rows.iter()
            .any(|row| row.get::<_, String>(0) == "Количество")
    );
    assert!(harness.statements().is_empty());
    harness.stop();
}
