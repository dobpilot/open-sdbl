use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::TrinoType;

/// Complete logical catalog served to the connector.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MetadataCatalog {
    pub schemas: Vec<String>,
    pub tables: Vec<TableMetadata>,
    pub issues: Vec<MetadataIssue>,
}

impl MetadataCatalog {
    #[must_use]
    pub fn table(&self, schema: &str, name: &str) -> Option<&TableMetadata> {
        self.tables
            .iter()
            .find(|table| table.schema == schema && table.name == name)
    }
}

/// One Trino-visible 1C object.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMetadata {
    pub schema: String,
    pub name: String,
    pub logical_name: String,
    pub object_guid: String,
    #[serde(skip)]
    pub physical_table: String,
    pub columns: Vec<ColumnMetadata>,
}

impl TableMetadata {
    #[must_use]
    pub fn column(&self, name: &str) -> Option<&ColumnMetadata> {
        self.columns.iter().find(|column| column.name == name)
    }
}

/// One Trino-visible logical field and its private physical implementation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub name: String,
    #[serde(skip)]
    pub trino_type: TrinoType,
    #[serde(rename = "type")]
    pub type_signature: String,
    pub nullable: bool,
    pub predicate_pushdown: bool,
    pub comment: Option<String>,
    #[serde(skip)]
    pub physical: Vec<PhysicalColumn>,
    #[serde(skip)]
    pub reference_targets: Vec<String>,
}

/// One exact live PostgreSQL member of a logical field.
#[derive(Debug, Clone)]
pub struct PhysicalColumn {
    pub name: String,
    pub data_type: String,
    pub output_label: String,
}

/// An object or field that could not be exposed under its ordinary name.
#[derive(Debug, Clone, Serialize)]
pub struct MetadataIssue {
    pub code: String,
    pub object_guid: Option<String>,
    pub message: String,
}

/// Versioned request sent by the Java worker.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScanRequest {
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
    #[serde(default)]
    pub filters: Vec<crate::filters::FilterDomain>,
    pub limit: Option<u64>,
}

/// SDBL source analyzed by the polymorphic Trino table function.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SdblPrepareRequest {
    pub query: String,
}

/// One output column determined without executing SDBL query rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdblColumnMetadata {
    pub index: usize,
    pub name: String,
    #[serde(rename = "type")]
    pub type_signature: String,
    pub nullable: bool,
}

/// Dynamic result descriptor returned during Trino function analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SdblPrepareResponse {
    pub columns: Vec<SdblColumnMetadata>,
}

/// Revalidated worker request for one SDBL table-function scan.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SdblScanRequest {
    pub query: String,
    pub expected_columns: Vec<SdblColumnMetadata>,
    pub columns: Vec<usize>,
    pub limit: Option<u64>,
}

/// JSON object shape used for compound logical values.
pub type CompoundValue = BTreeMap<String, Option<String>>;

#[cfg(test)]
mod tests {
    use super::{ColumnMetadata, PhysicalColumn, ScanRequest, SdblScanRequest};
    use crate::types::TrinoType;

    #[test]
    fn metadata_and_scan_wire_contract_use_java_camel_case() {
        let column = ColumnMetadata {
            name: "ИНН".to_owned(),
            trino_type: TrinoType::Varchar,
            type_signature: "varchar".to_owned(),
            nullable: true,
            predicate_pushdown: true,
            comment: None,
            physical: vec![PhysicalColumn {
                name: "_Fld1".to_owned(),
                data_type: "text".to_owned(),
                output_label: "ИНН".to_owned(),
            }],
            reference_targets: vec![],
        };
        let json = serde_json::to_value(column).unwrap();
        assert_eq!(json["type"], "varchar");
        assert_eq!(json["predicatePushdown"], true);
        assert!(json.get("physical").is_none());

        let request: ScanRequest = serde_json::from_str(
            r#"{"schema":"Справочник","table":"Контрагенты","columns":["ИНН"],"filters":[{"column":"ИНН","all":false,"nullAllowed":false,"ranges":[]}],"limit":10}"#,
        )
        .unwrap();
        assert_eq!(request.filters[0].column, "ИНН");
        assert_eq!(request.limit, Some(10));

        let request: SdblScanRequest = serde_json::from_str(
            r#"{"query":"SELECT Code FROM Catalog.Items","expectedColumns":[{"index":0,"name":"Code","type":"varchar","nullable":true}],"columns":[0],"limit":5}"#,
        )
        .unwrap();
        assert_eq!(request.columns, [0]);
        assert_eq!(request.expected_columns[0].type_signature, "varchar");
    }
}
