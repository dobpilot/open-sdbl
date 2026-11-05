//! PostgreSQL 13 relation shapes read by the recorded clients.
use super::sql::Table;
use std::collections::BTreeMap;
pub(super) const BUILTINS: &[(&str, i64)] = &[
    ("pg_namespace", 2615),
    ("pg_class", 1259),
    ("pg_attribute", 1249),
    ("pg_type", 1247),
    ("pg_description", 2609),
    ("pg_shdescription", 2396),
    ("pg_database", 1262),
    ("pg_roles", 12087),
    ("pg_user", 12098),
    ("pg_settings", 12165),
    ("pg_locks", 12073),
    ("pg_auth_members", 1261),
    ("pg_timezone_names", 12099),
    ("pg_timezone_abbrevs", 12097),
    ("pg_index", 2610),
    ("pg_constraint", 2606),
    ("pg_attrdef", 2604),
    ("pg_inherits", 2611),
    ("pg_trigger", 2620),
    ("pg_rewrite", 2618),
    ("pg_policy", 3256),
    ("pg_statistic_ext", 3381),
    ("pg_publication", 6104),
    ("pg_publication_rel", 6106),
    ("pg_publication_namespace", 6237),
    ("pg_foreign_table", 3118),
    ("pg_partitioned_table", 3350),
    ("pg_am", 2601),
    ("pg_collation", 3456),
    ("pg_extension", 3079),
    ("pg_proc", 1255),
    ("pg_tablespace", 1213),
    ("pg_enum", 3501),
    ("pg_range", 3541),
    ("pg_event_trigger", 3466),
    ("pg_foreign_data_wrapper", 2328),
    ("pg_foreign_server", 1417),
    ("pg_user_mappings", 12183),
    ("pg_language", 2612),
    ("pg_cast", 2605),
    ("pg_depend", 2608),
    ("pg_sequence", 2224),
    ("pg_operator", 2617),
    ("pg_opclass", 2616),
    ("pg_opfamily", 2753),
    ("pg_amop", 2602),
    ("pg_amproc", 2603),
    ("pg_aggregate", 2600),
];
pub(super) fn known(n: &str) -> bool {
    BUILTINS.iter().any(|(name, _)| *name == n)
        || [
            "schemata",
            "tables",
            "columns",
            "views",
            "table_constraints",
            "key_column_usage",
            "routines",
            "parameters",
        ]
        .contains(&n)
}
pub(super) fn empty() -> BTreeMap<String, Table> {
    let mut tables = BTreeMap::new();
    let mut add = |name: &str, columns: &[(&str, u32)]| {
        tables.insert(name.into(), Table::new(columns));
    };
    add(
        "pg_catalog.pg_sequence",
        &[
            ("seqrelid", 26),
            ("seqtypid", 26),
            ("seqstart", 20),
            ("seqincrement", 20),
            ("seqmax", 20),
            ("seqmin", 20),
            ("seqcache", 20),
            ("seqcycle", 16),
        ],
    );
    add(
        "pg_catalog.pg_operator",
        &[
            ("oid", 26),
            ("oprname", 19),
            ("oprnamespace", 26),
            ("oprowner", 26),
            ("oprkind", 18),
            ("oprcanmerge", 16),
            ("oprcanhash", 16),
            ("oprleft", 26),
            ("oprright", 26),
            ("oprresult", 26),
            ("oprcom", 26),
            ("oprnegate", 26),
            ("oprcode", 24),
            ("oprrest", 24),
            ("oprjoin", 24),
        ],
    );
    add(
        "pg_catalog.pg_opclass",
        &[
            ("oid", 26),
            ("opcmethod", 26),
            ("opcname", 19),
            ("opcnamespace", 26),
            ("opcowner", 26),
            ("opcfamily", 26),
            ("opcintype", 26),
            ("opcdefault", 16),
            ("opckeytype", 26),
        ],
    );
    add(
        "pg_catalog.pg_opfamily",
        &[
            ("oid", 26),
            ("opfmethod", 26),
            ("opfname", 19),
            ("opfnamespace", 26),
            ("opfowner", 26),
        ],
    );
    add(
        "pg_catalog.pg_amop",
        &[
            ("oid", 26),
            ("amopfamily", 26),
            ("amoplefttype", 26),
            ("amoprighttype", 26),
            ("amopstrategy", 21),
            ("amoppurpose", 18),
            ("amopopr", 26),
            ("amopmethod", 26),
            ("amopsortfamily", 26),
        ],
    );
    add(
        "pg_catalog.pg_amproc",
        &[
            ("oid", 26),
            ("amprocfamily", 26),
            ("amproclefttype", 26),
            ("amprocrighttype", 26),
            ("amprocnum", 21),
            ("amproc", 24),
        ],
    );
    add(
        "pg_catalog.pg_aggregate",
        &[
            ("aggfnoid", 24),
            ("aggkind", 18),
            ("aggnumdirectargs", 21),
            ("aggtransfn", 24),
            ("aggfinalfn", 24),
            ("aggcombinefn", 24),
            ("aggserialfn", 24),
            ("aggdeserialfn", 24),
            ("aggmtransfn", 24),
            ("aggminvtransfn", 24),
            ("aggmfinalfn", 24),
            ("aggfinalextra", 16),
            ("aggmfinalextra", 16),
            ("aggfinalmodify", 18),
            ("aggmfinalmodify", 18),
            ("aggsortop", 26),
            ("aggtranstype", 26),
            ("aggtransspace", 23),
            ("aggmtranstype", 26),
            ("aggmtransspace", 23),
            ("agginitval", 25),
            ("aggminitval", 25),
        ],
    );
    add(
        "pg_catalog.pg_namespace",
        &[
            ("oid", 26),
            ("nspname", 19),
            ("nspowner", 26),
            ("nspacl", 1034),
        ],
    );
    add(
        "pg_catalog.pg_class",
        &[
            ("oid", 26),
            ("relname", 19),
            ("relnamespace", 26),
            ("reltype", 26),
            ("reloftype", 26),
            ("relowner", 26),
            ("relam", 26),
            ("relfilenode", 26),
            ("relpages", 23),
            ("reltuples", 701),
            ("relallvisible", 23),
            ("reltoastrelid", 26),
            ("relhasindex", 16),
            ("relhasrules", 16),
            ("relhastriggers", 16),
            ("relrowsecurity", 16),
            ("relforcerowsecurity", 16),
            ("relispartition", 16),
            ("relhassubclass", 16),
            ("relkind", 18),
            ("relpersistence", 18),
            ("relreplident", 18),
            ("relnatts", 21),
            ("relchecks", 21),
            ("relacl", 1034),
            ("reloptions", 1009),
            ("relpartbound", 25),
            ("relfrozenxid", 28),
            ("relhasoids", 16),
            ("relispopulated", 16),
            ("reltablespace", 26),
        ],
    );
    add(
        "pg_catalog.pg_attribute",
        &[
            ("attrelid", 26),
            ("attname", 19),
            ("atttypid", 26),
            ("attnum", 21),
            ("atttypmod", 23),
            ("attnotnull", 16),
            ("atthasdef", 16),
            ("attisdropped", 16),
            ("attcollation", 26),
            ("attidentity", 18),
            ("attgenerated", 18),
            ("attstorage", 18),
            ("attstattarget", 23),
            ("attndims", 23),
            ("attbyval", 16),
            ("attlen", 21),
            ("attacl", 1034),
        ],
    );
    add(
        "pg_catalog.pg_type",
        &[
            ("oid", 26),
            ("typname", 19),
            ("typnamespace", 26),
            ("typowner", 26),
            ("typlen", 21),
            ("typbyval", 16),
            ("typtype", 18),
            ("typcategory", 18),
            ("typispreferred", 16),
            ("typisdefined", 16),
            ("typrelid", 26),
            ("typelem", 26),
            ("typarray", 26),
            ("typinput", 24),
            ("typoutput", 24),
            ("typreceive", 24),
            ("typsend", 24),
            ("typalign", 18),
            ("typstorage", 18),
            ("typnotnull", 16),
            ("typbasetype", 26),
            ("typtypmod", 23),
            ("typndims", 23),
            ("typcollation", 26),
            ("typdefaultbin", 25),
            ("typdefault", 25),
            ("typacl", 1034),
        ],
    );
    add(
        "pg_catalog.pg_description",
        &[
            ("objoid", 26),
            ("classoid", 26),
            ("objsubid", 23),
            ("description", 25),
        ],
    );
    add(
        "pg_catalog.pg_database",
        &[
            ("oid", 26),
            ("datname", 19),
            ("datdba", 26),
            ("encoding", 23),
            ("datcollate", 19),
            ("datctype", 19),
            ("datistemplate", 16),
            ("datallowconn", 16),
            ("datconnlimit", 23),
            ("dattablespace", 26),
            ("datacl", 1034),
        ],
    );
    add(
        "pg_catalog.pg_shdescription",
        &[("objoid", 26), ("classoid", 26), ("description", 25)],
    );
    add(
        "pg_catalog.pg_roles",
        &[
            ("oid", 26),
            ("rolname", 19),
            ("rolsuper", 16),
            ("rolinherit", 16),
            ("rolcreaterole", 16),
            ("rolcreatedb", 16),
            ("rolcanlogin", 16),
            ("rolconnlimit", 23),
            ("rolpassword", 25),
            ("rolvaliduntil", 1184),
            ("rolconfig", 1009),
            ("rolreplication", 16),
            ("rolbypassrls", 16),
        ],
    );
    add(
        "pg_catalog.pg_user",
        &[
            ("usename", 19),
            ("usesysid", 26),
            ("usecreatedb", 16),
            ("usesuper", 16),
            ("userepl", 16),
            ("passwd", 25),
            ("valuntil", 1184),
            ("useconfig", 1009),
        ],
    );
    add("pg_catalog.pg_settings", &[("name", 25), ("setting", 25)]);
    add(
        "pg_catalog.pg_event_trigger",
        &[
            ("oid", 26),
            ("evtname", 19),
            ("evtevent", 19),
            ("evtowner", 26),
            ("evtfoid", 26),
            ("evtenabled", 18),
            ("evttags", 1009),
        ],
    );
    add(
        "pg_catalog.pg_foreign_data_wrapper",
        &[
            ("oid", 26),
            ("fdwname", 19),
            ("fdwowner", 26),
            ("fdwhandler", 26),
            ("fdwvalidator", 26),
            ("fdwacl", 1034),
            ("fdwoptions", 1009),
        ],
    );
    add(
        "pg_catalog.pg_foreign_server",
        &[
            ("oid", 26),
            ("srvname", 19),
            ("srvowner", 26),
            ("srvfdw", 26),
            ("srvtype", 25),
            ("srvversion", 25),
            ("srvacl", 1034),
            ("srvoptions", 1009),
        ],
    );
    add(
        "pg_catalog.pg_user_mappings",
        &[
            ("umid", 26),
            ("srvid", 26),
            ("srvname", 19),
            ("umuser", 26),
            ("usename", 19),
            ("umoptions", 1009),
        ],
    );
    add(
        "pg_catalog.pg_language",
        &[
            ("oid", 26),
            ("lanname", 19),
            ("lanowner", 26),
            ("lanispl", 16),
            ("lanpltrusted", 16),
            ("lanplcallfoid", 26),
            ("laninline", 26),
            ("lanvalidator", 26),
            ("lanacl", 1034),
        ],
    );
    add(
        "pg_catalog.pg_cast",
        &[
            ("oid", 26),
            ("castsource", 26),
            ("casttarget", 26),
            ("castfunc", 26),
            ("castcontext", 18),
            ("castmethod", 18),
        ],
    );
    add(
        "pg_catalog.pg_depend",
        &[
            ("classid", 26),
            ("objid", 26),
            ("objsubid", 23),
            ("refclassid", 26),
            ("refobjid", 26),
            ("refobjsubid", 23),
            ("deptype", 18),
        ],
    );
    add(
        "pg_catalog.pg_locks",
        &[
            ("locktype", 25),
            ("database", 26),
            ("relation", 26),
            ("page", 23),
            ("tuple", 21),
            ("virtualxid", 25),
            ("transactionid", 28),
            ("classid", 26),
            ("objid", 26),
            ("objsubid", 21),
            ("virtualtransaction", 25),
            ("pid", 23),
            ("mode", 25),
            ("granted", 16),
            ("fastpath", 16),
        ],
    );
    add(
        "pg_catalog.pg_auth_members",
        &[
            ("roleid", 26),
            ("member", 26),
            ("grantor", 26),
            ("admin_option", 16),
        ],
    );
    add(
        "pg_catalog.pg_timezone_names",
        &[
            ("name", 25),
            ("abbrev", 25),
            ("utc_offset", 1186),
            ("is_dst", 16),
        ],
    );
    add(
        "pg_catalog.pg_timezone_abbrevs",
        &[("abbrev", 25), ("utc_offset", 1186), ("is_dst", 16)],
    );
    add(
        "pg_catalog.pg_attrdef",
        &[("oid", 26), ("adrelid", 26), ("adnum", 21), ("adbin", 25)],
    );
    add(
        "pg_catalog.pg_am",
        &[
            ("oid", 26),
            ("amname", 19),
            ("amtype", 18),
            ("amhandler", 24),
        ],
    );
    add(
        "pg_catalog.pg_collation",
        &[
            ("oid", 26),
            ("collname", 19),
            ("collnamespace", 26),
            ("collowner", 26),
            ("collprovider", 18),
            ("collisdeterministic", 16),
            ("collencoding", 23),
            ("collcollate", 19),
            ("collctype", 19),
            ("collversion", 25),
        ],
    );
    add(
        "pg_catalog.pg_policy",
        &[
            ("oid", 26),
            ("polname", 19),
            ("polrelid", 26),
            ("polcmd", 18),
            ("polpermissive", 16),
            ("polroles", 1028),
            ("polqual", 25),
            ("polwithcheck", 25),
        ],
    );
    add(
        "pg_catalog.pg_statistic_ext",
        &[
            ("oid", 26),
            ("stxrelid", 26),
            ("stxnamespace", 26),
            ("stxname", 19),
            ("stxkeys", 22),
            ("stxkind", 1002),
            ("stxstattarget", 23),
        ],
    );
    add(
        "pg_catalog.pg_publication",
        &[
            ("oid", 26),
            ("pubname", 19),
            ("pubowner", 26),
            ("puballtables", 16),
            ("pubinsert", 16),
            ("pubupdate", 16),
            ("pubdelete", 16),
            ("pubtruncate", 16),
        ],
    );
    add(
        "pg_catalog.pg_publication_rel",
        &[("oid", 26), ("prpubid", 26), ("prrelid", 26)],
    );
    add(
        "pg_catalog.pg_publication_namespace",
        &[("oid", 26), ("pnpubid", 26), ("pnnspid", 26)],
    );
    add(
        "pg_catalog.pg_inherits",
        &[("inhrelid", 26), ("inhparent", 26), ("inhseqno", 23)],
    );
    add(
        "pg_catalog.pg_constraint",
        &[
            ("oid", 26),
            ("conname", 19),
            ("connamespace", 26),
            ("contype", 18),
            ("condeferrable", 16),
            ("condeferred", 16),
            ("convalidated", 16),
            ("conrelid", 26),
            ("contypid", 26),
            ("conindid", 26),
            ("conparentid", 26),
            ("confrelid", 26),
            ("confupdtype", 18),
            ("confdeltype", 18),
            ("confmatchtype", 18),
            ("conislocal", 16),
            ("coninhcount", 23),
            ("connoinherit", 16),
            ("conkey", 1005),
            ("confkey", 1005),
            ("conbin", 25),
            ("conexclop", 1028),
        ],
    );
    add(
        "pg_catalog.pg_index",
        &[
            ("indexrelid", 26),
            ("indrelid", 26),
            ("indnatts", 21),
            ("indnkeyatts", 21),
            ("indisunique", 16),
            ("indisprimary", 16),
            ("indisexclusion", 16),
            ("indimmediate", 16),
            ("indisclustered", 16),
            ("indisvalid", 16),
            ("indisready", 16),
            ("indislive", 16),
            ("indisreplident", 16),
            ("indkey", 22),
            ("indcollation", 30),
            ("indclass", 30),
            ("indoption", 22),
            ("indexprs", 25),
            ("indpred", 25),
        ],
    );
    add(
        "pg_catalog.pg_trigger",
        &[
            ("oid", 26),
            ("tgrelid", 26),
            ("tgname", 19),
            ("tgfoid", 26),
            ("tgtype", 21),
            ("tgenabled", 18),
            ("tgisinternal", 16),
            ("tgconstrrelid", 26),
            ("tgconstraint", 26),
        ],
    );
    add(
        "pg_catalog.pg_rewrite",
        &[
            ("oid", 26),
            ("rulename", 19),
            ("ev_class", 26),
            ("ev_type", 18),
            ("ev_enabled", 18),
            ("is_instead", 16),
            ("ev_qual", 25),
            ("ev_action", 25),
        ],
    );
    add(
        "pg_catalog.pg_foreign_table",
        &[("ftrelid", 26), ("ftserver", 26), ("ftoptions", 1009)],
    );
    add(
        "pg_catalog.pg_partitioned_table",
        &[
            ("partrelid", 26),
            ("partstrat", 18),
            ("partnatts", 21),
            ("partdefid", 26),
            ("partattrs", 22),
        ],
    );
    add(
        "pg_catalog.pg_extension",
        &[
            ("oid", 26),
            ("extname", 19),
            ("extowner", 26),
            ("extnamespace", 26),
            ("extrelocatable", 16),
            ("extversion", 25),
            ("extconfig", 1028),
            ("extcondition", 1009),
        ],
    );
    add(
        "pg_catalog.pg_proc",
        &[
            ("oid", 26),
            ("proname", 19),
            ("pronamespace", 26),
            ("proowner", 26),
            ("prorettype", 26),
            ("proargtypes", 30),
            ("proallargtypes", 1028),
            ("proargmodes", 1002),
            ("proargnames", 1009),
            ("prokind", 18),
            ("proretset", 16),
            ("proacl", 1034),
            ("pronargs", 21),
            ("pronargdefaults", 21),
            ("proargdefaults", 25),
            ("prolang", 26),
            ("prosrc", 25),
            ("probin", 25),
            ("provariadic", 26),
            ("provolatile", 18),
            ("proisstrict", 16),
            ("prosecdef", 16),
            ("proconfig", 1009),
            ("procost", 700),
            ("prorows", 700),
            ("proleakproof", 16),
            ("proparallel", 18),
        ],
    );
    add(
        "pg_catalog.pg_tablespace",
        &[
            ("oid", 26),
            ("spcname", 19),
            ("spcowner", 26),
            ("spcacl", 1034),
            ("spcoptions", 1009),
        ],
    );
    add(
        "pg_catalog.pg_enum",
        &[
            ("oid", 26),
            ("enumtypid", 26),
            ("enumsortorder", 700),
            ("enumlabel", 19),
        ],
    );
    add(
        "pg_catalog.pg_range",
        &[
            ("rngtypid", 26),
            ("rngsubtype", 26),
            ("rngcollation", 26),
            ("rngsubopc", 26),
            ("rngcanonical", 24),
            ("rngsubdiff", 24),
        ],
    );
    add(
        "information_schema.schemata",
        &[
            ("catalog_name", 19),
            ("schema_name", 19),
            ("schema_owner", 19),
            ("default_character_set_catalog", 19),
            ("default_character_set_schema", 19),
            ("default_character_set_name", 19),
            ("sql_path", 25),
        ],
    );
    add(
        "information_schema.tables",
        &[
            ("table_catalog", 19),
            ("table_schema", 19),
            ("table_name", 19),
            ("table_type", 25),
            ("self_referencing_column_name", 19),
            ("reference_generation", 25),
            ("user_defined_type_catalog", 19),
            ("user_defined_type_schema", 19),
            ("user_defined_type_name", 19),
            ("is_insertable_into", 25),
            ("is_typed", 25),
            ("commit_action", 25),
        ],
    );
    add(
        "information_schema.columns",
        &[
            ("table_catalog", 19),
            ("table_schema", 19),
            ("table_name", 19),
            ("column_name", 19),
            ("ordinal_position", 23),
            ("column_default", 25),
            ("is_nullable", 25),
            ("data_type", 25),
            ("character_maximum_length", 23),
            ("character_octet_length", 23),
            ("numeric_precision", 23),
            ("numeric_precision_radix", 23),
            ("numeric_scale", 23),
            ("datetime_precision", 23),
            ("udt_catalog", 19),
            ("udt_schema", 19),
            ("udt_name", 19),
            ("is_identity", 25),
            ("is_generated", 25),
            ("generation_expression", 25),
        ],
    );
    add(
        "information_schema.views",
        &[
            ("table_catalog", 19),
            ("table_schema", 19),
            ("table_name", 19),
            ("view_definition", 25),
            ("check_option", 25),
            ("is_updatable", 25),
            ("is_insertable_into", 25),
            ("is_trigger_updatable", 25),
            ("is_trigger_deletable", 25),
            ("is_trigger_insertable_into", 25),
        ],
    );
    add(
        "information_schema.table_constraints",
        &[
            ("constraint_catalog", 19),
            ("constraint_schema", 19),
            ("constraint_name", 19),
            ("table_catalog", 19),
            ("table_schema", 19),
            ("table_name", 19),
            ("constraint_type", 25),
            ("is_deferrable", 25),
            ("initially_deferred", 25),
            ("enforced", 25),
        ],
    );
    add(
        "information_schema.key_column_usage",
        &[
            ("constraint_catalog", 19),
            ("constraint_schema", 19),
            ("constraint_name", 19),
            ("table_catalog", 19),
            ("table_schema", 19),
            ("table_name", 19),
            ("column_name", 19),
            ("ordinal_position", 23),
            ("position_in_unique_constraint", 23),
        ],
    );
    add(
        "information_schema.routines",
        &[
            ("routine_catalog", 19),
            ("routine_schema", 19),
            ("routine_name", 19),
            ("routine_type", 25),
            ("data_type", 25),
        ],
    );
    add(
        "information_schema.parameters",
        &[
            ("specific_catalog", 19),
            ("specific_schema", 19),
            ("specific_name", 19),
            ("ordinal_position", 23),
            ("parameter_mode", 25),
            ("parameter_name", 19),
            ("data_type", 25),
        ],
    );
    tables
}

pub(super) fn physical(name: &str) -> bool {
    BUILTINS.iter().any(|(n, _)| *n == name)
        && ![
            "pg_roles",
            "pg_user",
            "pg_settings",
            "pg_locks",
            "pg_timezone_names",
            "pg_timezone_abbrevs",
            "pg_user_mappings",
        ]
        .contains(&name)
}
