use std::collections::{BTreeMap, BTreeSet, HashMap};

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::query::queryable_field_catalog;

use crate::model::{ColumnMetadata, MetadataCatalog, MetadataIssue, PhysicalColumn, TableMetadata};
use crate::types::map_logical_type;

/// Builds the deterministic Trino view of a resolved metadata generation.
#[must_use]
pub fn build_catalog(snapshot: &MetadataSnapshot) -> MetadataCatalog {
    let fields_by_object = queryable_field_catalog(snapshot);
    let mut issues = Vec::new();
    let mut candidates = Vec::new();

    for object in &snapshot.objects {
        let guid = object.guid.as_str().to_owned();
        let Some(kind) = object.kind else {
            // Config contains many auxiliary descriptors which are not 1C
            // objects. Only a descriptor tied to storage is an unsupported
            // tabular object worth surfacing to an operator.
            if object.physical_table.is_some() || object.live {
                issues.push(issue(
                    "unsupported_object_kind",
                    Some(guid),
                    "stored Config descriptor has no supported tabular metadata kind",
                ));
            }
            continue;
        };
        let Some(declared_physical_table) = object.physical_table.as_deref() else {
            issues.push(issue(
                "missing_physical_table",
                Some(guid),
                "tabular object has no DBNames physical table",
            ));
            continue;
        };
        let Some(physical_table) = snapshot
            .live_tables
            .iter()
            .find(|table| table.name.eq_ignore_ascii_case(declared_physical_table))
            .map(|table| table.name.clone())
        else {
            issues.push(issue(
                "physical_table_not_live",
                Some(guid),
                format!("physical table {declared_physical_table} is not present in PostgreSQL"),
            ));
            continue;
        };
        let Some(logical_name) = object
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
        else {
            issues.push(issue(
                "missing_object_name",
                Some(guid),
                format!("live object {physical_table} has no Config name"),
            ));
            continue;
        };
        let id = ObjectId::from(&object.guid);
        let Some(fields) = fields_by_object.get(&id) else {
            issues.push(issue(
                "missing_queryable_fields",
                Some(guid),
                format!("live object {physical_table} has no queryable field catalog"),
            ));
            continue;
        };
        let schema = russian_kind_name(kind).to_owned();
        let columns = build_columns(&guid, fields, &mut issues);
        candidates.push(TableMetadata {
            schema,
            name: logical_name.clone(),
            logical_name,
            object_guid: guid,
            physical_table,
            columns,
        });
    }

    disambiguate_tables(&mut candidates, &mut issues);
    candidates.sort_by(|left, right| {
        (&left.schema, &left.name, &left.object_guid).cmp(&(
            &right.schema,
            &right.name,
            &right.object_guid,
        ))
    });
    let schemas = candidates
        .iter()
        .map(|table| table.schema.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    MetadataCatalog {
        schemas,
        tables: candidates,
        issues,
    }
}

fn build_columns(
    object_guid: &str,
    fields: &[open_sdbl::query::QueryableField],
    issues: &mut Vec<MetadataIssue>,
) -> Vec<ColumnMetadata> {
    let mut counts = HashMap::<String, usize>::new();
    for field in fields {
        *counts
            .entry(public_field_name(field).to_lowercase())
            .or_default() += 1;
    }
    let mut used = BTreeSet::new();
    let mut columns = Vec::with_capacity(fields.len());
    for field in fields {
        let mut name = public_field_name(field);
        if counts
            .get(&name.to_lowercase())
            .copied()
            .unwrap_or_default()
            > 1
        {
            name = format!("{}__{}", name, field.schema_name);
            issues.push(issue(
                "duplicate_column_name",
                Some(object_guid.to_owned()),
                format!(
                    "logical field {:?} is ambiguous and is exposed as {:?}",
                    field.name, name
                ),
            ));
        }
        let base = name.clone();
        let mut suffix = 2;
        while !used.insert(name.to_lowercase()) {
            name = format!("{base}__{suffix}");
            suffix += 1;
        }
        let physical = field
            .columns
            .iter()
            .map(|column| PhysicalColumn {
                name: column.physical_name.clone(),
                data_type: column.data_type.clone(),
                output_label: column.output_label.clone(),
            })
            .collect::<Vec<_>>();
        let trino_type = map_logical_type(&physical, &field.reference_targets);
        columns.push(ColumnMetadata {
            name,
            type_signature: trino_type.signature(),
            trino_type,
            nullable: true,
            predicate_pushdown: physical.len() == 1,
            comment: None,
            physical,
            reference_targets: field.reference_targets.clone(),
        });
    }
    columns
}

fn public_field_name(field: &open_sdbl::query::QueryableField) -> String {
    field
        .aliases
        .iter()
        .find(|alias| {
            alias
                .chars()
                .any(|character| ('\u{0400}'..='\u{04ff}').contains(&character))
        })
        .cloned()
        .unwrap_or_else(|| field.name.clone())
}

fn disambiguate_tables(tables: &mut [TableMetadata], issues: &mut Vec<MetadataIssue>) {
    let mut groups = BTreeMap::<(String, String), Vec<usize>>::new();
    for (index, table) in tables.iter().enumerate() {
        groups
            .entry((table.schema.to_lowercase(), table.name.to_lowercase()))
            .or_default()
            .push(index);
    }
    for indexes in groups.values().filter(|indexes| indexes.len() > 1) {
        for &index in indexes {
            let table = &mut tables[index];
            let suffix = table.object_guid.get(..8).unwrap_or(&table.object_guid);
            table.name = format!("{}__{suffix}", table.logical_name);
            issues.push(issue(
                "duplicate_table_name",
                Some(table.object_guid.clone()),
                format!(
                    "duplicate object {:?}.{:?} is exposed as {:?}",
                    table.schema, table.logical_name, table.name
                ),
            ));
        }
    }
}

fn issue(
    code: impl Into<String>,
    object_guid: Option<String>,
    message: impl Into<String>,
) -> MetadataIssue {
    MetadataIssue {
        code: code.into(),
        object_guid,
        message: message.into(),
    }
}

/// Canonical Russian schema name for every metadata kind supported by core.
#[must_use]
pub const fn russian_kind_name(kind: MetadataKind) -> &'static str {
    match kind {
        MetadataKind::Catalog => "Справочник",
        MetadataKind::Document => "Документ",
        MetadataKind::Enumeration => "Перечисление",
        MetadataKind::InformationRegister => "РегистрСведений",
        MetadataKind::AccumulationRegister => "РегистрНакопления",
        MetadataKind::AccountingRegister => "РегистрБухгалтерии",
        MetadataKind::CalculationRegister => "РегистрРасчета",
        MetadataKind::ChartOfCharacteristicTypes => "ПланВидовХарактеристик",
        MetadataKind::ChartOfCalculationTypes => "ПланВидовРасчета",
        MetadataKind::ChartOfAccounts => "ПланСчетов",
        MetadataKind::Constant => "Константа",
        MetadataKind::ExchangePlan => "ПланОбмена",
        MetadataKind::BusinessProcess => "БизнесПроцесс",
        MetadataKind::Task => "Задача",
        MetadataKind::Sequence => "Последовательность",
    }
}

#[cfg(test)]
mod tests {
    use open_sdbl::metadata::MetadataKind;
    use open_sdbl::query::QueryableField;

    use crate::model::TableMetadata;

    use super::{disambiguate_tables, public_field_name, russian_kind_name};

    #[test]
    fn maps_every_supported_kind_to_a_russian_schema() {
        let mappings = [
            (MetadataKind::Catalog, "Справочник"),
            (MetadataKind::Document, "Документ"),
            (MetadataKind::Enumeration, "Перечисление"),
            (MetadataKind::InformationRegister, "РегистрСведений"),
            (MetadataKind::AccumulationRegister, "РегистрНакопления"),
            (MetadataKind::AccountingRegister, "РегистрБухгалтерии"),
            (MetadataKind::CalculationRegister, "РегистрРасчета"),
            (
                MetadataKind::ChartOfCharacteristicTypes,
                "ПланВидовХарактеристик",
            ),
            (MetadataKind::ChartOfCalculationTypes, "ПланВидовРасчета"),
            (MetadataKind::ChartOfAccounts, "ПланСчетов"),
            (MetadataKind::Constant, "Константа"),
            (MetadataKind::ExchangePlan, "ПланОбмена"),
            (MetadataKind::BusinessProcess, "БизнесПроцесс"),
            (MetadataKind::Task, "Задача"),
            (MetadataKind::Sequence, "Последовательность"),
        ];
        for (kind, expected) in mappings {
            assert_eq!(russian_kind_name(kind), expected);
        }
    }

    #[test]
    fn duplicate_table_names_receive_stable_guid_suffixes() {
        let mut tables = [
            table("Контрагенты", "aaaaaaaa-0000-0000-0000-000000000000"),
            table("контрагенты", "bbbbbbbb-0000-0000-0000-000000000000"),
        ];
        let mut issues = Vec::new();
        disambiguate_tables(&mut tables, &mut issues);
        assert_eq!(tables[0].name, "Контрагенты__aaaaaaaa");
        assert_eq!(tables[1].name, "контрагенты__bbbbbbbb");
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn standard_fields_use_their_russian_logical_alias() {
        let field = QueryableField {
            name: "Description".to_owned(),
            schema_name: "Description".to_owned(),
            aliases: vec!["Description".to_owned(), "Наименование".to_owned()],
            columns: vec![],
            reference_target: None,
            reference_targets: vec![],
        };
        assert_eq!(public_field_name(&field), "Наименование");
    }

    fn table(name: &str, guid: &str) -> TableMetadata {
        TableMetadata {
            schema: "Справочник".to_owned(),
            name: name.to_owned(),
            logical_name: name.to_owned(),
            object_guid: guid.to_owned(),
            physical_table: "_Reference1".to_owned(),
            columns: vec![],
        }
    }
}
