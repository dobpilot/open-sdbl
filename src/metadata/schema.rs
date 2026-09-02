use super::{MetadataError, Value, parse_serialized};

/// A physical column type declaration from SchemaStorage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnType {
    /// 1C schema tag such as `S`, `N`, `R`, or `V`.
    pub tag: String,
    /// Referenced canonical table name for an `R` declaration.
    pub reference_target: Option<String>,
}

/// A physical column declaration from SchemaStorage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    /// Canonical name as stored by 1C, without the SQL leading underscore.
    pub name: String,
    /// All declared storage types for this logical schema column.
    pub types: Vec<ColumnType>,
}

impl SchemaColumn {
    /// Returns the canonical PostgreSQL/MSSQL physical name.
    #[must_use]
    pub fn physical_name(&self) -> String {
        format!("_{}", self.name)
    }
}

/// An index declaration from SchemaStorage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIndex {
    /// Canonical index name as stored by 1C.
    pub name: String,
    /// Ordered canonical column names, without leading underscores.
    pub columns: Vec<String>,
    /// Whether SchemaStorage marks the index as unique.
    pub unique: bool,
}

/// A physical table declaration from SchemaStorage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaTable {
    /// Canonical table name as stored by 1C, without the SQL leading underscore.
    pub name: String,
    /// Numeric platform identifier from the table header.
    pub number: u32,
    /// Declared columns.
    pub columns: Vec<SchemaColumn>,
    /// Declared indexes.
    pub indexes: Vec<SchemaIndex>,
}

impl SchemaTable {
    /// Returns the canonical physical table name including the SQL underscore.
    #[must_use]
    pub fn physical_name(&self) -> String {
        format!("_{}", self.name)
    }
}

/// Parsed current physical schema of an information base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaStorage {
    /// Tables in SchemaStorage source order.
    pub tables: Vec<SchemaTable>,
}

impl SchemaStorage {
    /// Finds a table using its canonical physical name, with or without `_`.
    #[must_use]
    pub fn table(&self, physical_name: &str) -> Option<&SchemaTable> {
        let name = physical_name.strip_prefix('_').unwrap_or(physical_name);
        self.tables.iter().find(|table| table.name == name)
    }
}

/// Parses plaintext `SchemaStorage.CurrentSchema` for `SchemaID = 0`.
///
/// # Errors
///
/// Returns [`MetadataError`] for malformed serialization or when no table
/// declarations with the verified SchemaStorage signature are present.
pub fn parse_schema_storage(input: &[u8]) -> Result<SchemaStorage, MetadataError> {
    let value = parse_serialized(input)?;
    let mut tables = Vec::new();
    collect_tables(&value, &mut tables);
    if tables.is_empty() {
        return Err(MetadataError::new(
            "SchemaStorage contains no recognized table declarations",
        ));
    }
    Ok(SchemaStorage { tables })
}

fn collect_tables(value: &Value, tables: &mut Vec<SchemaTable>) {
    let Value::List(values) = value else {
        return;
    };
    if let Some(table) = project_table(values).or_else(|| project_inline_table(values)) {
        tables.push(table);
    }
    for value in values {
        collect_tables(value, tables);
    }
}

fn project_table(values: &[Value]) -> Option<SchemaTable> {
    if values.len() < 7 || values.get(1)?.as_string()? != "N" {
        return None;
    }
    let name = values.first()?.as_string()?.to_owned();
    let number = values.get(2)?.as_u32()?;
    let columns = project_counted(values.get(4)?, project_column)?;
    let indexes = project_counted(values.get(6)?, project_index)?;
    Some(SchemaTable {
        name,
        number,
        columns,
        indexes,
    })
}

fn project_inline_table(values: &[Value]) -> Option<SchemaTable> {
    if values.len() < 7 || values.get(1)?.as_string()? != "I" || values.get(2)?.as_u32()? != 0 {
        return None;
    }
    let inline_name = values.first()?.as_string()?;
    let number = inline_name.strip_prefix("VT")?.parse::<u32>().ok()?;
    let parent = values.get(3)?.as_string()?;
    if parent.is_empty() {
        return None;
    }
    let mut columns = project_counted(values.get(4)?, project_column)?;
    let owner_name = format!("{parent}_IDRRef");
    if !columns
        .iter()
        .any(|column| column.name.eq_ignore_ascii_case(&owner_name))
    {
        columns.insert(
            0,
            SchemaColumn {
                name: owner_name,
                types: vec![ColumnType {
                    tag: "R".to_owned(),
                    reference_target: Some(parent.to_owned()),
                }],
            },
        );
    }
    let indexes = project_counted(values.get(6)?, project_index)?;
    Some(SchemaTable {
        name: format!("{parent}_{inline_name}"),
        number,
        columns,
        indexes,
    })
}

fn project_counted<T>(value: &Value, project: fn(&[Value]) -> Option<T>) -> Option<Vec<T>> {
    let values = value.as_list()?;
    let count = values.first()?.as_u32()? as usize;
    if values.len() < count + 1 {
        return None;
    }
    let projected: Vec<T> = values[1..=count]
        .iter()
        .map(Value::as_list)
        .map(|value| value.and_then(project))
        .collect::<Option<_>>()?;
    Some(projected)
}

fn project_column(values: &[Value]) -> Option<SchemaColumn> {
    if values.len() < 3 || values.get(1)?.as_u32().is_none() {
        return None;
    }
    let name = values.first()?.as_string()?.to_owned();
    let type_collection = values.get(2)?.as_list()?;
    let count = type_collection.first()?.as_u32()? as usize;
    if type_collection.len() < count + 1 {
        return None;
    }
    let mut types = Vec::with_capacity(count);
    for declaration in &type_collection[1..=count] {
        let declaration = declaration.as_list()?;
        let tag = declaration.first()?.as_string()?.to_owned();
        if !matches!(tag.as_str(), "S" | "N" | "T" | "B" | "L" | "R" | "V" | "E") {
            return None;
        }
        let reference_target = if tag == "R" {
            declaration.get(3)?.as_string().map(str::to_owned)
        } else {
            None
        };
        types.push(ColumnType {
            tag,
            reference_target,
        });
    }
    Some(SchemaColumn { name, types })
}

fn project_index(values: &[Value]) -> Option<SchemaIndex> {
    if values.len() < 3 {
        return None;
    }
    let name = values.first()?.as_string()?.to_owned();
    let unique = values.get(1)?.as_u32()? != 0;
    let fields = values.get(2)?.as_list()?;
    let count = fields.first()?.as_u32()? as usize;
    if fields.len() < count + 1 {
        return None;
    }
    let columns = fields[1..=count]
        .iter()
        .map(Value::as_string)
        .map(|value| value.map(str::to_owned))
        .collect::<Option<_>>()?;
    Some(SchemaIndex {
        name,
        columns,
        unique,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_schema_storage;

    #[test]
    fn parses_tables_columns_references_and_indexes() {
        let source = br#"{0,{1,{"Reference35","N",35,"",{2,{"ID",0,{1,{"R",0,0,"Reference35",3}},"",0},{"Fld36",0,{1,{"S",10,0,"",0}},"",0}},{0},{1,{"ByID",1,{1,"ID"},0,0,0,{0},0,0}},1,"R",{0},{0},"",0}}}"#;
        let schema = parse_schema_storage(source).unwrap();
        assert_eq!(schema.tables.len(), 1);
        let table = &schema.tables[0];
        assert_eq!(table.physical_name(), "_Reference35");
        assert_eq!(table.columns[0].types[0].tag, "R");
        assert_eq!(
            table.columns[0].types[0].reference_target.as_deref(),
            Some("Reference35")
        );
        assert!(table.indexes[0].unique);
        assert_eq!(table.indexes[0].columns, ["ID"]);
    }

    #[test]
    fn canonicalizes_inline_tabular_section_tables_and_owner_references() {
        let source = br#"{0,{1,{"Document53","N",53,"",{1,{"IDRRef",0,{1,{"B",0,0,"",0}},"",0}},{0},{0},{"VT54","I",0,"Document53",{2,{"LineNo55",0,{1,{"N",5,0,"",0}},"",0},{"Fld56",0,{1,{"T",0,0,"",0}},"",0}},{0},{0},1,"S",{0},{0},"",0,0}}}}"#;
        let schema = parse_schema_storage(source).unwrap();

        assert_eq!(schema.tables.len(), 2);
        assert_eq!(schema.tables[0].name, "Document53");
        let table = &schema.tables[1];
        assert_eq!(table.name, "Document53_VT54");
        assert_eq!(table.number, 54);
        assert_eq!(table.physical_name(), "_Document53_VT54");
        assert_eq!(table.columns[0].name, "Document53_IDRRef");
        assert_eq!(table.columns[0].types[0].tag, "R");
        assert_eq!(
            table.columns[0].types[0].reference_target.as_deref(),
            Some("Document53")
        );
        assert_eq!(table.columns[1].name, "LineNo55");
        assert_eq!(table.columns[2].name, "Fld56");
    }

    #[test]
    fn rejects_non_schema_values() {
        assert!(parse_schema_storage(br#"{1,"not a schema"}"#).is_err());
    }
}
