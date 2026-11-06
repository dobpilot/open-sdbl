//! Candidate identities are assigned before access filtering.
use super::*;
use crate::codec::FieldDescription;
use crate::values::describe_column;
use open_sdbl::metadata::{MetadataKind, MetadataSnapshot, Synonym};
use open_sdbl::query::{
    AccessDecision, Backend, CompileOptions, FieldUsageRequest, ParameterValue, PostgresBackend,
    PrepareOptions, QueryCompiler, QueryParameter, RestrictionRequest, SessionParameters,
    TempTablesManager, object_query_name,
};
use open_sdbl_db::DatabaseDialect;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) schema: String,
    pub(crate) name: String,
    pub(crate) query: String,
    pub(crate) oid: i64,
    pub(crate) schema_oid: i64,
    pub(crate) view: bool,
    pub(crate) comment: Option<String>,
    pub(crate) columns: Vec<(i64, FieldDescription, Option<String>)>,
}

pub(crate) struct CandidateAccess {
    pub(crate) request: RestrictionRequest,
    pub(crate) retain_empty: bool,
    pub(crate) columns: Vec<FieldUsageRequest>,
}

struct BuiltCandidate {
    candidate: Candidate,
    access: CandidateAccess,
}
pub(crate) fn synonym(s: &[Synonym]) -> Option<String> {
    s.iter()
        .find(|s| s.language.eq_ignore_ascii_case("ru") && !s.text.trim().is_empty())
        .or_else(|| s.iter().find(|s| !s.text.trim().is_empty()))
        .map(|s| s.text.trim().into())
}
pub(crate) fn candidates(snapshot: &MetadataSnapshot) -> Vec<Candidate> {
    let mut result = BTreeMap::new();
    let mut resources = HashMap::<_, Vec<_>>::new();
    for descriptor in snapshot.descriptors() {
        resources
            .entry(&descriptor.resource_guid)
            .or_default()
            .push(descriptor);
    }
    let sections = snapshot
        .db_names()
        .entries()
        .iter()
        .filter(|e| e.alias == "VT")
        .map(|e| &e.guid)
        .collect::<HashSet<_>>();
    for object in snapshot.objects() {
        let Some(query) = object_query_name(snapshot, object) else {
            continue;
        };
        let Some((schema, name)) = query.split_once('.') else {
            continue;
        };
        let mut add = |name: String, view, comment| {
            result
                .entry((schema.to_owned(), name.clone()))
                .or_insert(Candidate {
                    schema: schema.into(),
                    name: name.clone(),
                    query: format!(
                        "ВЫБРАТЬ * ИЗ {schema}.{name}{}",
                        if view { "()" } else { "" }
                    ),
                    oid: 0,
                    schema_oid: 0,
                    view,
                    comment,
                    columns: vec![],
                });
        };
        add(name.into(), false, synonym(&object.synonyms));
        // Section descriptors carry their owner's resource GUID.
        for d in resources
            .get(&object.guid)
            .into_iter()
            .flatten()
            .filter(|d| d.object_guid != object.guid)
        {
            if sections.contains(&d.object_guid) {
                add(format!("{name}.{}", d.name), false, synonym(&d.synonyms));
            }
        }
        let virtuals: &[&str] = match object.kind {
            Some(MetadataKind::AccumulationRegister) => &["Остатки", "Обороты", "ОстаткиИОбороты"],
            Some(MetadataKind::InformationRegister) => &["СрезПервых", "СрезПоследних"],
            Some(MetadataKind::AccountingRegister) => &[
                "Остатки",
                "Обороты",
                "ОстаткиИОбороты",
                "ОборотыДтКт",
                "ДвиженияССубконто",
            ],
            _ => &[],
        };
        for v in virtuals {
            add(format!("{name}.{v}"), true, None);
        }
    }
    let schemas = result
        .keys()
        .map(|(s, _)| s.clone())
        .collect::<BTreeSet<_>>();
    let schema_ids = schemas
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, 16384 + i as i64))
        .collect::<BTreeMap<_, _>>();
    let first = 16384 + schema_ids.len() as i64;
    result
        .into_values()
        .enumerate()
        .map(|(i, mut c)| {
            c.schema_oid = schema_ids[&c.schema];
            c.oid = first + 2 * i as i64;
            c
        })
        .collect()
}

pub(crate) fn build(
    snapshot: &MetadataSnapshot,
    dialect: DatabaseDialect,
    database: &str,
    revision: u32,
) -> Catalog {
    let candidates = candidates(snapshot);
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(candidates.len());
    let built = if workers <= 1 {
        compile_chunk(candidates.as_slice(), snapshot, dialect)
    } else {
        let chunk_size = candidates.len().div_ceil(workers);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for chunk in candidates.chunks(chunk_size) {
                handles.push(scope.spawn(move || compile_chunk(chunk, snapshot, dialect)));
            }
            let mut built = Vec::with_capacity(candidates.len());
            for handle in handles {
                built.extend(handle.join().expect("catalog worker panicked"));
            }
            built
        })
    };
    let mut failures = 0;
    let mut candidates = Vec::with_capacity(built.len());
    let mut access = BTreeMap::new();
    for (built, failed) in built {
        failures += failed;
        if let Some(built) = built {
            access.insert(built.candidate.oid, built.access);
            candidates.push(built.candidate);
        }
    }
    let mut catalog = finish(candidates, "", database, revision);
    catalog._build_failures = failures;
    catalog.access = Arc::new(access);
    catalog
}

fn compile_chunk(
    candidates: &[Candidate],
    snapshot: &MetadataSnapshot,
    dialect: DatabaseDialect,
) -> Vec<(Option<BuiltCandidate>, usize)> {
    candidates
        .iter()
        .cloned()
        .map(|candidate| match dialect {
            DatabaseDialect::Postgres => compile_candidate(snapshot, PostgresBackend, candidate),
            DatabaseDialect::MsSql { backend } => compile_candidate(snapshot, backend, candidate),
        })
        .collect()
}

fn compile_candidate<B: Backend>(
    snapshot: &MetadataSnapshot,
    backend: B,
    mut candidate: Candidate,
) -> (Option<BuiltCandidate>, usize) {
    let mut manager = TempTablesManager::new();
    let options = PrepareOptions::new()
        .temporary_tables(&manager)
        .restricted();
    let prepared = match QueryCompiler::new(snapshot, backend)
        .prepare_with_options(&candidate.query, &options)
    {
        Ok(prepared) => prepared,
        Err(_) => return (None, 1),
    };
    let request = prepared.restriction_request().clone();
    let retain_empty = request
        .targets
        .iter()
        .all(|target| target.table_part.is_none());
    let decisions = request
        .targets
        .iter()
        .cloned()
        .map(AccessDecision::unrestricted)
        .collect::<Vec<_>>();
    let mut parameters = SessionParameters::new();
    for field in snapshot.separators() {
        let Some(settings) = &field.separation else {
            continue;
        };
        if let Some(constant) = &settings.conditional_separation {
            parameters.set_separation_condition(constant.clone(), true);
        }
        if let Some(name) = &settings.use_parameter {
            parameters.set(QueryParameter::new(name, ParameterValue::Boolean(true)));
        }
        // This compilation only discovers columns; its SQL is never executed.
        // NULL is type-neutral, including for reference-valued separators.
        let name = settings
            .value_parameter
            .as_deref()
            .or(field.name.as_deref())
            .unwrap_or(&field.physical_name);
        parameters.set(QueryParameter::new(name, ParameterValue::Null));
    }
    let options = CompileOptions::new()
        .session(&parameters)
        .decisions(&decisions);
    let query = match prepared.compile_batch(snapshot, &options, &mut manager) {
        Ok(Some(query)) => query,
        _ if retain_empty => {
            return (
                Some(BuiltCandidate {
                    candidate,
                    access: CandidateAccess {
                        request,
                        retain_empty,
                        columns: Vec::new(),
                    },
                }),
                1,
            );
        }
        _ => return (None, 1),
    };
    let mut access = CandidateAccess {
        request,
        retain_empty,
        columns: Vec::new(),
    };
    for (index, column) in query.columns.iter().enumerate() {
        if query.service_columns.contains(&index) || query.forbidden_row_column == Some(index) {
            continue;
        }
        let usage = match &column.origin {
            Some(origin) => FieldUsageRequest {
                fields: vec![open_sdbl::query::FieldUse {
                    object: origin.object,
                    table_part: origin.table_part.clone(),
                    field: origin.field,
                    usage: open_sdbl::query::FieldUsage::Projection,
                }],
            },
            None => prepared.field_usage().clone(),
        };
        let comment = column.origin.as_ref().and_then(|origin| {
            if let open_sdbl::metadata::FieldId::Metadata(id) = origin.field {
                snapshot
                    .attribute_by_id(id)
                    .ok()
                    .and_then(|field| synonym(&field.synonyms))
            } else {
                None
            }
        });
        let (field, _) = describe_column(&column.label, &column.kind, 0);
        candidate.columns.push((index as i64 + 1, field, comment));
        access.columns.push(usage);
    }
    (Some(BuiltCandidate { candidate, access }), 0)
}

pub(crate) fn finish(
    candidates: Vec<Candidate>,
    user: &str,
    database: &str,
    revision: u32,
) -> Catalog {
    let candidates = Arc::<[Candidate]>::from(candidates);
    let mut catalog = Catalog {
        revision,
        _build_failures: 0,
        tables: relations::empty()
            .into_iter()
            .map(|(name, table)| (name, SharedTable::new(table)))
            .collect(),
        user: user.into(),
        database: database.into(),
        schemas: vec![
            "pg_catalog".into(),
            "information_schema".into(),
            "public".into(),
        ],
        views: BTreeMap::new(),
        candidates: Arc::clone(&candidates),
        access: Arc::new(BTreeMap::new()),
    };
    let mut schemas: BTreeMap<String, i64> = BTreeMap::from([
        ("pg_catalog".into(), 11),
        ("information_schema".into(), 13248),
        ("public".into(), 2200),
    ]);
    for c in candidates.iter() {
        schemas.insert(c.schema.clone(), c.schema_oid);
    }
    catalog.schemas = schemas.keys().cloned().collect();
    for (name, oid) in &schemas {
        catalog
            .tables
            .get_mut("pg_catalog.pg_namespace")
            .unwrap()
            .push(vec![
                (*oid).into(),
                name.clone().into(),
                10.into(),
                Value::Null,
            ]);
        catalog
            .tables
            .get_mut("information_schema.schemata")
            .unwrap()
            .push(vec![
                database.into(),
                name.clone().into(),
                user.into(),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ]);
    }
    let mut types = vec![];
    for oid in [
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 28, 30, 701, 1033, 1043, 1114, 1186, 1700,
        2203, 2205, 2206, 2950, 4089,
    ] {
        types.push((
            oid,
            sql::type_name(oid),
            0,
            sql::array_oid(oid).unwrap_or(0),
            0,
            11,
        ));
        if let Some(array) = sql::array_oid(oid) {
            types.push((array, format!("_{}", sql::type_name(oid)), oid, 0, 0, 11));
        }
    }
    // Include the actual catalog relations so casts and description class
    // identifiers resolve by name as PostgreSQL clients expect.
    let builtin = relations::BUILTINS;
    let mut class_rows = vec![];
    for &(name, oid) in builtin {
        class_rows.push(class_row(oid, name, 11, false, 0));
    }
    for c in candidates.iter().cloned() {
        let mut class = class_row(c.oid, &c.name, c.schema_oid, c.view, c.oid + 1);
        class[22] = (c.columns.len() as i64).into();
        class_rows.push(class);
        types.push((c.oid as u32 + 1, c.name.clone(), 0, 0, c.oid, c.schema_oid));
        if c.view {
            catalog.views.insert(c.oid, c.query.clone());
        }
        let kind = if c.view { "VIEW" } else { "BASE TABLE" };
        catalog
            .tables
            .get_mut("information_schema.tables")
            .unwrap()
            .push(vec![
                database.into(),
                c.schema.clone().into(),
                c.name.clone().into(),
                kind.into(),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                "NO".into(),
                "NO".into(),
                Value::Null,
            ]);
        if c.view {
            catalog
                .tables
                .get_mut("information_schema.views")
                .unwrap()
                .push(vec![
                    database.into(),
                    c.schema.clone().into(),
                    c.name.clone().into(),
                    c.query.clone().into(),
                    "NONE".into(),
                    "NO".into(),
                    "NO".into(),
                    "NO".into(),
                    "NO".into(),
                    "NO".into(),
                ]);
        }
        if let Some(comment) = c.comment {
            catalog
                .tables
                .get_mut("pg_catalog.pg_description")
                .unwrap()
                .push(vec![c.oid.into(), 1259.into(), 0.into(), comment.into()]);
        }
        for (num, field, comment) in c.columns {
            let typ = sql::format_type(field.type_oid, field.type_modifier);
            catalog
                .tables
                .get_mut("pg_catalog.pg_attribute")
                .unwrap()
                .push(vec![
                    c.oid.into(),
                    field.name.clone().into(),
                    i64::from(field.type_oid).into(),
                    num.into(),
                    i64::from(field.type_modifier).into(),
                    false.into(),
                    false.into(),
                    false.into(),
                    0.into(),
                    "".into(),
                    "".into(),
                    "x".into(),
                    (-1).into(),
                    0.into(),
                    false.into(),
                    i64::from(field.type_length).into(),
                    Value::Null,
                ]);
            let length = if field.type_oid == 1043 && field.type_modifier >= 4 {
                (i64::from(field.type_modifier) - 4).into()
            } else {
                Value::Null
            };
            let precision = if field.type_oid == 1700 && field.type_modifier >= 4 {
                i64::from((field.type_modifier - 4) >> 16).into()
            } else {
                Value::Null
            };
            let scale = if field.type_oid == 1700 && field.type_modifier >= 4 {
                i64::from((field.type_modifier - 4) & 65535).into()
            } else {
                Value::Null
            };
            catalog
                .tables
                .get_mut("information_schema.columns")
                .unwrap()
                .push(vec![
                    database.into(),
                    c.schema.clone().into(),
                    c.name.clone().into(),
                    field.name.into(),
                    num.into(),
                    Value::Null,
                    "YES".into(),
                    typ.into(),
                    length,
                    Value::Null,
                    precision,
                    10.into(),
                    scale,
                    Value::Null,
                    database.into(),
                    "pg_catalog".into(),
                    sql::type_name(field.type_oid).into(),
                    "NO".into(),
                    "NEVER".into(),
                    Value::Null,
                ]);
            if let Some(comment) = comment {
                catalog
                    .tables
                    .get_mut("pg_catalog.pg_description")
                    .unwrap()
                    .push(vec![c.oid.into(), 1259.into(), num.into(), comment.into()]);
            }
        }
    }
    catalog.tables.get_mut("pg_catalog.pg_class").unwrap().rows = class_rows;
    for (oid, name, element, array, relid, namespace) in types {
        let (length, byval, category, preferred, align, storage, collation) =
            type_properties(oid, element, relid);
        catalog
            .tables
            .get_mut("pg_catalog.pg_type")
            .unwrap()
            .push(vec![
                i64::from(oid).into(),
                name.into(),
                namespace.into(),
                10.into(),
                length.into(),
                byval.into(),
                if relid > 0 { "c" } else { "b" }.into(),
                category.into(),
                preferred.into(),
                true.into(),
                relid.into(),
                i64::from(element).into(),
                i64::from(array).into(),
                if element > 0 { 750.into() } else { 0.into() },
                0.into(),
                0.into(),
                0.into(),
                align.into(),
                storage.into(),
                false.into(),
                0.into(),
                (-1).into(),
                0.into(),
                collation.into(),
                Value::Null,
                Value::Null,
                Value::Null,
            ]);
    }
    catalog
        .tables
        .get_mut("pg_catalog.pg_database")
        .unwrap()
        .push(vec![
            16383.into(),
            database.into(),
            10.into(),
            6.into(),
            "C".into(),
            "C".into(),
            false.into(),
            true.into(),
            (-1).into(),
            1663.into(),
            Value::Null,
        ]);
    catalog
        .tables
        .get_mut("pg_catalog.pg_roles")
        .unwrap()
        .push(vec![
            10.into(),
            user.into(),
            false.into(),
            false.into(),
            false.into(),
            false.into(),
            true.into(),
            (-1).into(),
            Value::Null,
            Value::Null,
            Value::Null,
            false.into(),
            false.into(),
        ]);
    let functions = catalog.tables.get_mut("pg_catalog.pg_proc").unwrap();
    let mut array_in = vec![Value::Null; functions.columns.len()];
    for (name, value) in [
        ("oid", 750.into()),
        ("proname", "array_in".into()),
        ("pronamespace", 11.into()),
        ("proowner", 10.into()),
        ("prorettype", 2277.into()),
        ("pronargs", 3.into()),
        ("pronargdefaults", 0.into()),
        ("prokind", "f".into()),
        ("proretset", false.into()),
    ] {
        let index = functions
            .columns
            .iter()
            .position(|c| c.name == name)
            .unwrap();
        array_in[index] = value;
    }
    functions.push(array_in);
    for (name, table) in &mut catalog.tables {
        if name
            .strip_prefix("pg_catalog.")
            .is_some_and(relations::physical)
        {
            let mut column = super::sql::Column::new("xmin", 28);
            column.system = true;
            table.columns.push(column);
            for row in &mut table.rows {
                row.push(i64::from(revision).into());
            }
        }
    }
    catalog
        .tables
        .get_mut("pg_catalog.pg_user")
        .unwrap()
        .push(vec![
            user.into(),
            10.into(),
            false.into(),
            false.into(),
            false.into(),
            Value::Null,
            Value::Null,
            Value::Null,
        ]);
    catalog
}
fn class_row(oid: i64, name: &str, namespace: i64, view: bool, rowtype: i64) -> Vec<Value> {
    vec![
        oid.into(),
        name.into(),
        namespace.into(),
        rowtype.into(),
        0.into(),
        10.into(),
        0.into(),
        0.into(),
        0.into(),
        (-1).into(),
        0.into(),
        0.into(),
        false.into(),
        false.into(),
        false.into(),
        false.into(),
        false.into(),
        false.into(),
        false.into(),
        if view { "v" } else { "r" }.into(),
        "p".into(),
        "d".into(),
        0.into(),
        0.into(),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        false.into(),
        true.into(),
        0.into(),
    ]
}

fn type_properties(
    oid: u32,
    element: u32,
    relid: i64,
) -> (
    i64,
    bool,
    &'static str,
    bool,
    &'static str,
    &'static str,
    i64,
) {
    if element > 0 {
        return (-1, false, "A", false, "i", "x", 0);
    }
    if relid > 0 {
        return (-1, false, "C", false, "d", "x", 0);
    }
    match oid {
        16 => (1, true, "B", true, "c", "p", 0),
        17 => (-1, false, "U", false, "i", "x", 0),
        18 => (1, true, "S", false, "c", "p", 0),
        19 => (64, false, "S", false, "c", "p", 950),
        20 => (8, true, "N", false, "d", "p", 0),
        21 => (2, true, "N", false, "s", "p", 0),
        22 | 30 => (-1, false, "A", false, "i", "p", 0),
        23 | 24 | 26 | 2203 | 2205 | 2206 | 4089 => (4, true, "N", oid == 26, "i", "p", 0),
        1033 => (12, false, "U", false, "i", "p", 0),
        28 => (4, true, "U", false, "i", "p", 0),
        25 | 1043 => (-1, false, "S", oid == 25, "i", "x", 100),
        701 => (8, true, "N", true, "d", "p", 0),
        1114 => (8, true, "D", false, "d", "p", 0),
        1186 => (16, false, "T", false, "d", "p", 0),
        1700 => (-1, false, "N", false, "i", "m", 0),
        2950 => (16, false, "U", false, "c", "p", 0),
        _ => (-1, false, "U", false, "i", "x", 0),
    }
}
