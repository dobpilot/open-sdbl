//! The explicit catalog function and type registry.
use super::*;

/// Empty virtual inventory with the PostgreSQL OUT-column shape.
pub(super) fn table_function(name: &str) -> Option<Table> {
    (name == "pg_available_extension_versions").then(|| {
        Table::new(&[
            ("name", 19),
            ("version", 25),
            ("superuser", 16),
            ("trusted", 16),
            ("relocatable", 16),
            ("schema", 19),
            ("requires", 1003),
            ("comment", 25),
        ])
    })
}
pub(super) fn known(n: &str) -> bool {
    [
        "pg_table_is_visible",
        "pg_type_is_visible",
        "pg_function_is_visible",
        "pg_get_userbyid",
        "age",
        "pg_is_in_recovery",
        "txid_current",
        "pg_available_extension_versions",
        "array_agg",
        "pg_tablespace_location",
        "format_type",
        "obj_description",
        "col_description",
        "shobj_description",
        "pg_get_expr",
        "pg_get_indexdef",
        "pg_get_constraintdef",
        "pg_get_viewdef",
        "pg_relation_size",
        "pg_total_relation_size",
        "pg_relation_is_publishable",
        "pg_partition_ancestors",
        "has_table_privilege",
        "has_schema_privilege",
        "current_schema",
        "current_schemas",
        "current_database",
        "current_user",
        "session_user",
        "current_role",
        "current_catalog",
        "version",
        "quote_ident",
        "array_upper",
        "array_lower",
        "array_length",
        "array_to_string",
        "unnest",
        "generate_series",
        "_pg_expandarray",
        "string_agg",
        "count",
        "min",
        "max",
        "sum",
        "translate",
        "greatest",
        "lower",
        "upper",
        "substring",
        "concat",
        "left",
        "length",
        "char_length",
        "coalesce",
        "nullif",
        "any",
        "all",
        "pg_encoding_to_char",
        "pg_get_function_result",
        "pg_get_function_arguments",
        "pg_get_function_identity_arguments",
        "pg_get_partkeydef",
    ]
    .contains(&n)
}
pub(super) fn result_type(n: &str, args: &[u32]) -> Result<u32, ErrorFields> {
    let (min, max) = match n {
        "current_schema"
        | "current_database"
        | "current_user"
        | "session_user"
        | "current_role"
        | "current_catalog"
        | "version"
        | "pg_is_in_recovery"
        | "txid_current"
        | "pg_available_extension_versions" => (0, 0),
        "format_type" | "col_description" | "array_upper" | "array_lower" | "array_length"
        | "nullif" | "string_agg" | "left" => (2, 2),
        "pg_get_expr" | "substring" => (2, 3),
        "obj_description"
        | "shobj_description"
        | "pg_get_indexdef"
        | "pg_get_constraintdef"
        | "pg_get_viewdef"
        | "pg_relation_size"
        | "pg_total_relation_size" => (1, 3),
        "has_table_privilege" | "has_schema_privilege" => (2, 3),
        "generate_series" => (2, 3),
        "array_to_string" => (2, 3),
        "translate" => (3, 3),
        "coalesce" | "greatest" | "concat" => (1, usize::MAX),
        _ => (1, 1),
    };
    if !(min..=max).contains(&args.len()) {
        return Err(ErrorFields::error(
            "42883",
            format!("function {n} does not accept {} arguments", args.len()),
        ));
    }
    Ok(match n {
        "pg_table_is_visible"
        | "pg_type_is_visible"
        | "pg_function_is_visible"
        | "has_table_privilege"
        | "has_schema_privilege"
        | "pg_relation_is_publishable"
        | "pg_is_in_recovery" => 16,
        "count" | "pg_relation_size" | "pg_total_relation_size" => 20,
        "age" => 23,
        "txid_current" => 20,
        "pg_available_extension_versions" => 2249,
        "array_agg" => array_type(args[0])
            .ok_or_else(|| ErrorFields::error("0A000", "unsupported array_agg element type"))?,
        "array_upper" | "array_lower" | "array_length" | "length" | "char_length" => 23,
        "current_schemas" => 1003,
        "pg_partition_ancestors" => 2205,
        "unnest" => args.first().copied().unwrap_or(1009),
        "generate_series" => args.first().copied().unwrap_or(23),
        "any" | "all" => element_type(args[0]).unwrap_or(args[0]),
        "min" | "max" | "sum" | "coalesce" | "greatest" | "nullif" => {
            args.first().copied().unwrap_or(25)
        }
        _ => 25,
    })
}
pub(super) fn type_oid(n: &str) -> Option<u32> {
    Some(match n {
        "bool" | "boolean" => 16,
        "bytea" => 17,
        "char" => 18,
        "name" => 19,
        "int8" | "bigint" => 20,
        "int2" | "smallint" => 21,
        "int4" | "integer" | "int" => 23,
        "text" => 25,
        "oid" => 26,
        "xid" => 28,
        "int2vector" => 22,
        "oidvector" => 30,
        "float8" => 701,
        "varchar" => 1043,
        "numeric" | "decimal" => 1700,
        "timestamp" => 1114,
        "uuid" => 2950,
        "regclass" => 2205,
        "regtype" => 2206,
        "regproc" => 24,
        "regoper" => 2203,
        "regnamespace" => 4089,
        "aclitem" => 1033,
        n if n.ends_with("[]") => return array_type(type_oid(&n[..n.len() - 2])?),
        _ => return None,
    })
}
pub(super) fn array_type(n: u32) -> Option<u32> {
    Some(match n {
        16 => 1000,
        17 => 1001,
        18 => 1002,
        19 => 1003,
        20 => 1016,
        21 => 1005,
        23 => 1007,
        25 => 1009,
        26 => 1028,
        1033 => 1034,
        28 => 1011,
        1043 => 1015,
        1700 => 1231,
        1114 => 1115,
        2950 => 2951,
        _ => return None,
    })
}
pub(super) fn element_type(n: u32) -> Option<u32> {
    Some(match n {
        1000 => 16,
        1001 => 17,
        1002 => 18,
        1003 => 19,
        1016 => 20,
        1005 | 22 => 21,
        1007 => 23,
        1009 => 25,
        1028 | 30 => 26,
        1034 => 1033,
        1011 => 28,
        1015 => 1043,
        1231 => 1700,
        1115 => 1114,
        2951 => 2950,
        _ => return None,
    })
}
pub(crate) fn format_type(oid: u32, modifier: i32) -> String {
    match oid {
        16 => "boolean".into(),
        17 => "bytea".into(),
        18 => "\"char\"".into(),
        19 => "name".into(),
        20 => "bigint".into(),
        21 => "smallint".into(),
        23 => "integer".into(),
        25 => "text".into(),
        26 => "oid".into(),
        24 => "regproc".into(),
        2203 => "regoper".into(),
        28 => "xid".into(),
        701 => "double precision".into(),
        1043 => {
            if modifier >= 4 {
                format!("character varying({})", modifier - 4)
            } else {
                "character varying".into()
            }
        }
        1700 => {
            if modifier >= 4 {
                let m = modifier - 4;
                format!("numeric({},{})", m >> 16, m & 65535)
            } else {
                "numeric".into()
            }
        }
        1114 => "timestamp without time zone".into(),
        1186 => "interval".into(),
        2950 => "uuid".into(),
        2205 => "regclass".into(),
        2206 => "regtype".into(),
        4089 => "regnamespace".into(),
        n => element_type(n)
            .map_or_else(|| "unknown".into(), |e| format!("{}[]", format_type(e, -1))),
    }
}
pub(super) fn cast(v: Value, oid: u32, catalog: &Catalog) -> Result<Value, ErrorFields> {
    if v == Value::Null {
        return Ok(v);
    }
    if let Some(element) = element_type(oid) {
        return match v {
            Value::Array(vs) => Ok(Value::Array(
                vs.into_iter()
                    .map(|v| cast(v, element, catalog))
                    .collect::<Result<_, _>>()?,
            )),
            Value::Text(s) if s.starts_with('{') && s.ends_with('}') => {
                let values = crate::values::parse_text_array(&s)
                    .map_err(|_| ErrorFields::error("22P02", "invalid catalog array"))?
                    .into_iter()
                    .map(|s| match s {
                        None => Ok(Value::Null),
                        Some(s) => cast(Value::Text(s), element, catalog),
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Value::Array(values))
            }
            _ => Err(ErrorFields::error("22P02", "invalid catalog array")),
        };
    }
    Ok(match oid {
        16 => match v {
            Value::Bool(_) => v,
            Value::Text(s) if ["true", "t", "1", "false", "f", "0"].contains(&s.as_str()) => {
                Value::Bool(["true", "t", "1"].contains(&s.as_str()))
            }
            _ => return Err(ErrorFields::error("22P02", "invalid boolean")),
        },
        20 | 21 | 23 | 26 | 28 => {
            let n = if let Value::Decimal(n) = v {
                n.round_int()?
            } else {
                v.int()?
            };
            if (oid == 21 && i16::try_from(n).is_err())
                || (oid == 23 && i32::try_from(n).is_err())
                || (matches!(oid, 26 | 28) && u32::try_from(n).is_err())
            {
                return Err(ErrorFields::error("22003", "integer out of range"));
            }
            Value::Int(n)
        }
        24 | 2203 | 2205 | 2206 | 4089 => {
            let table = if oid == 2203 {
                "pg_operator"
            } else if oid == 24 {
                "pg_proc"
            } else if oid == 2205 {
                "pg_class"
            } else if oid == 2206 {
                "pg_type"
            } else {
                "pg_namespace"
            };
            let name_col = if oid == 2203 {
                "oprname"
            } else if oid == 24 {
                "proname"
            } else if oid == 2205 {
                "relname"
            } else if oid == 2206 {
                "typname"
            } else {
                "nspname"
            };
            let namespace_col = if oid == 2203 {
                "oprnamespace"
            } else if oid == 24 {
                "pronamespace"
            } else if oid == 2205 {
                "relnamespace"
            } else {
                "typnamespace"
            };
            let n = if let Ok(n) = v.int() {
                n
            } else {
                let text = v.text();
                let tokens = super::lex::lex(&text)?;
                let mut names = vec![];
                for token in tokens {
                    match token.kind {
                        super::lex::Kind::Word(w) | super::lex::Kind::Quoted(w) => names.push(w),
                        super::lex::Kind::Symbol(s) if s == "." => {}
                        super::lex::Kind::End => {}
                        _ => return Err(ErrorFields::error("22P02", "invalid catalog name")),
                    }
                }
                if names.is_empty() || names.len() > 2 {
                    return Err(ErrorFields::error("22P02", "invalid catalog name"));
                }
                let t = &catalog.tables[&format!("pg_catalog.{table}")];
                let ni = t.columns.iter().position(|c| c.name == name_col).unwrap();
                let ns = t.columns.iter().position(|c| c.name == namespace_col);
                let mut found = None;
                for row in &t.rows {
                    if row[ni] != Value::Text(names.last().unwrap().clone()) {
                        continue;
                    }
                    if names.len() == 2 {
                        let Some(ns) = ns else {
                            continue;
                        };
                        let schema = lookup(catalog, "pg_namespace", "oid", &row[ns], "nspname");
                        if schema != Some(Value::from(names[0].clone())) {
                            continue;
                        }
                    }
                    found = Some(row[0].int()?);
                    break;
                }
                found.ok_or_else(|| {
                    ErrorFields::error("42P01", format!("catalog name {text} does not exist"))
                })?
            };
            let name = lookup(catalog, table, "oid", &Value::Int(n), name_col)
                .map(|v| v.text())
                .unwrap_or_else(|| n.to_string());
            Value::Reg(n, name)
        }
        18 | 19 | 25 | 1043 | 1033 => Value::Text(v.text()),
        1700 => Value::Decimal(Decimal::parse(&v.text())?),
        _ => return Err(ErrorFields::error("0A000", "unsupported catalog cast")),
    })
}
fn lookup(c: &Catalog, table: &str, key: &str, value: &Value, result: &str) -> Option<Value> {
    let t = c.tables.get(&format!("pg_catalog.{table}"))?;
    let k = t.columns.iter().position(|c| c.name == key)?;
    let r = t.columns.iter().position(|c| c.name == result)?;
    t.rows
        .iter()
        .find(|row| row[k].cmp_sql(value).is_ok_and(|o| o.is_eq()))
        .map(|row| row[r].clone())
}
fn description(c: &Catalog, oid: i64, num: i64, class: Option<i64>) -> Value {
    let t = &c.tables["pg_catalog.pg_description"];
    t.rows
        .iter()
        .find(|r| {
            r[0] == Value::Int(oid)
                && r[2] == Value::Int(num)
                && class.is_none_or(|n| r[1] == Value::Int(n))
        })
        .map_or(Value::Null, |r| r[3].clone())
}
pub(super) fn call(
    n: &str,
    a: Vec<Value>,
    catalog: &Catalog,
    budget: &mut Budget,
) -> Result<Value, ErrorFields> {
    let v = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
    let text = |i| v(i).text();
    if ![
        "format_type",
        "coalesce",
        "nullif",
        "greatest",
        "concat",
        "current_schema",
        "current_schemas",
        "current_database",
        "current_user",
        "session_user",
        "current_role",
        "current_catalog",
        "version",
    ]
    .contains(&n)
        && a.contains(&Value::Null)
    {
        return Ok(Value::Null);
    }
    Ok(match n {
        "pg_is_in_recovery" => Value::Bool(false),
        "txid_current" => Value::Int(i64::from(catalog.revision)),
        "coalesce" => a
            .into_iter()
            .find(|v| *v != Value::Null)
            .unwrap_or(Value::Null),
        "nullif" => {
            if v(0).cmp_sql(&v(1)).is_ok_and(|c| c.is_eq()) {
                Value::Null
            } else {
                v(0)
            }
        }
        "pg_get_userbyid" => {
            if v(0) == Value::Int(10) {
                catalog.user.clone().into()
            } else {
                Value::Null
            }
        }
        "age" => {
            let xid = u32::try_from(v(0).int()?)
                .map_err(|_| ErrorFields::error("22003", "xid out of range"))?;
            Value::Int(if xid < 3 {
                i64::from(i32::MAX)
            } else {
                i64::from(catalog.revision.wrapping_sub(xid) as i32)
            })
        }
        "pg_tablespace_location" => Value::Null,
        "current_database" | "current_catalog" => catalog.database.clone().into(),
        "current_user" | "session_user" | "current_role" => catalog.user.clone().into(),
        "current_schema" => "public".into(),
        "current_schemas" => Value::Array(
            catalog
                .schemas
                .iter()
                .filter(|n| v(0) == Value::Bool(true) || n.as_str() != "pg_catalog")
                .cloned()
                .map(Value::from)
                .collect(),
        ),
        "version" => format!("PostgreSQL 13.0 (open-sdbl {})", env!("CARGO_PKG_VERSION")).into(),
        "format_type" => {
            if v(0) == Value::Null {
                Value::Null
            } else {
                format_type(
                    u32::try_from(v(0).int()?).unwrap_or(0),
                    v(1).int()
                        .ok()
                        .and_then(|n| i32::try_from(n).ok())
                        .unwrap_or(-1),
                )
                .into()
            }
        }
        "pg_table_is_visible" | "has_table_privilege" => Value::Bool(
            lookup(catalog, "pg_class", "oid", &v(0), "relname").is_some()
                || lookup(catalog, "pg_class", "relname", &v(0), "oid").is_some(),
        ),
        "pg_type_is_visible" => {
            Value::Bool(lookup(catalog, "pg_type", "oid", &v(0), "typname").is_some())
        }
        "pg_function_is_visible" => {
            Value::Bool(lookup(catalog, "pg_proc", "oid", &v(0), "proname").is_some())
        }
        "has_schema_privilege" => Value::Bool(
            lookup(catalog, "pg_namespace", "oid", &v(0), "nspname").is_some()
                || lookup(catalog, "pg_namespace", "nspname", &v(0), "oid").is_some(),
        ),
        "obj_description" => description(
            catalog,
            v(0).int()?,
            0,
            if a.len() > 1 {
                lookup(catalog, "pg_class", "relname", &v(1), "oid").and_then(|n| n.int().ok())
            } else {
                None
            },
        ),
        "col_description" => description(catalog, v(0).int()?, v(1).int()?, Some(1259)),
        "shobj_description" => Value::Null,
        "pg_get_expr"
        | "pg_get_indexdef"
        | "pg_get_constraintdef"
        | "pg_get_partkeydef"
        | "pg_get_function_result"
        | "pg_get_function_arguments"
        | "pg_get_function_identity_arguments" => Value::Null,
        "pg_get_viewdef" => catalog
            .views
            .get(&v(0).int()?)
            .cloned()
            .map_or(Value::Null, Value::from),
        "pg_relation_size" | "pg_total_relation_size" => Value::Int(0),
        "pg_relation_is_publishable" => Value::Bool(false),
        "pg_partition_ancestors" => Value::Array(vec![]),
        "translate" => {
            if a.contains(&Value::Null) {
                Value::Null
            } else {
                let mut mapping = std::collections::BTreeMap::new();
                let destination = text(2);
                let mut to = destination.chars();
                for ch in text(1).chars() {
                    budget.step()?;
                    let value = to.next();
                    if let std::collections::btree_map::Entry::Vacant(entry) = mapping.entry(ch) {
                        budget.allocate_bytes(64)?;
                        entry.insert(value);
                    }
                }
                let mut output = String::new();
                for ch in text(0).chars() {
                    budget.step()?;
                    let translated = mapping.get(&ch).copied().unwrap_or(Some(ch));
                    if let Some(ch) = translated {
                        budget.allocate_bytes(ch.len_utf8())?;
                        output.push(ch);
                    }
                }
                output.into()
            }
        }
        "greatest" => {
            let mut best = Value::Null;
            for value in a.iter().filter(|v| **v != Value::Null) {
                if best == Value::Null || value.cmp_sql(&best)?.is_gt() {
                    best = value.clone();
                }
            }
            best
        }
        "lower" => text(0).to_lowercase().into(),
        "upper" => text(0).to_uppercase().into(),
        "length" | "char_length" => {
            Value::Int(i64::try_from(text(0).chars().count()).unwrap_or(i64::MAX))
        }
        "quote_ident" => {
            let s = text(0);
            if !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                && !s.chars().next().unwrap().is_ascii_digit()
            {
                s.into()
            } else {
                format!("\"{}\"", s.replace('"', "\"\"")).into()
            }
        }
        "concat" => {
            let size = a.iter().try_fold(0, |size, value| {
                value.text_len(budget.bytes - size).map(|len| size + len)
            })?;
            budget.allocate_bytes(size)?;
            let mut text = String::with_capacity(size);
            for value in &a {
                value.write_text(&mut text).expect("writing to a String");
            }
            text.into()
        }
        "left" => text(0)
            .chars()
            .take(usize::try_from(v(1).int()?).unwrap_or(0))
            .collect::<String>()
            .into(),
        "substring" => text(0)
            .chars()
            .skip(usize::try_from(v(1).int()?.saturating_sub(1)).unwrap_or(0))
            .take(if a.len() > 2 {
                usize::try_from(v(2).int()?).unwrap_or(0)
            } else {
                usize::MAX
            })
            .collect::<String>()
            .into(),
        "array_upper" | "array_length" => match v(0) {
            Value::Array(vs) if v(1) == Value::Int(1) && !vs.is_empty() => {
                Value::Int(vs.len() as i64)
            }
            _ => Value::Null,
        },
        "array_lower" => match v(0) {
            Value::Array(vs) if v(1) == Value::Int(1) && !vs.is_empty() => Value::Int(1),
            _ => Value::Null,
        },
        "array_to_string" => match v(0) {
            Value::Array(vs) => {
                let sep = text(1);
                let values = vs
                    .iter()
                    .filter_map(|v| {
                        if *v == Value::Null {
                            a.get(2).map(Value::text)
                        } else {
                            Some(v.text())
                        }
                    })
                    .collect::<Vec<_>>();
                let size = values
                    .iter()
                    .map(String::len)
                    .fold(0, usize::saturating_add)
                    .saturating_add(sep.len().saturating_mul(values.len().saturating_sub(1)));
                if size > budget.bytes {
                    return Err(ErrorFields::error(
                        "54001",
                        "catalog string allocation budget exceeded",
                    ));
                }
                values.join(&sep).into()
            }
            _ => return Err(ErrorFields::error("42804", "expected array")),
        },
        "unnest" | "any" | "all" => v(0),
        "generate_series" => {
            let lo = v(0).int()?;
            let hi = v(1).int()?;
            let step = if a.len() > 2 { v(2).int()? } else { 1 };
            if step == 0 {
                return Err(ErrorFields::error("22023", "step cannot be zero"));
            }
            let mut vs = vec![];
            let mut n = lo;
            while if step > 0 { n <= hi } else { n >= hi } {
                budget.step()?;
                budget.rows(vs.len() + 1)?;
                vs.push(Value::Int(n));
                let Some(next) = n.checked_add(step) else {
                    break;
                };
                n = next;
            }
            Value::Array(vs)
        }
        "pg_encoding_to_char" => {
            if v(0) == Value::Int(6) {
                "UTF8".into()
            } else {
                "".into()
            }
        }
        _ => {
            return Err(ErrorFields::error(
                "42883",
                format!("function {n} is unavailable"),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_reserves_utf8_output_and_charges_discarded_results() {
        let catalog = crate::catalog::finish(vec![], "tester", "test", 3);
        let mut budget = Budget::new();
        budget.bytes = 5;
        let result = call(
            "concat",
            vec!["аб".into(), Value::Null, 7.into()],
            &catalog,
            &mut budget,
        )
        .unwrap();
        assert_eq!(result, Value::Text("аб7".into()));
        assert_eq!(budget.bytes, 0);
        drop(result);
        assert_eq!(
            call("concat", vec!["x".into()], &catalog, &mut budget)
                .unwrap_err()
                .code,
            "54001"
        );

        // Counting array escapes must also stop at the budget, before creating
        // an expanded representation of nested values.
        let mut array = Value::Text("\\".into());
        for _ in 0..32 {
            array = Value::Array(vec![array]);
        }
        budget.bytes = 100;
        assert_eq!(
            call("concat", vec![array], &catalog, &mut budget)
                .unwrap_err()
                .code,
            "54001"
        );
    }
}
