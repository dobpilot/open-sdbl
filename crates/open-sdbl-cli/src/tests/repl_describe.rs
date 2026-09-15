//! Tests of the console `describe` module.

use std::collections::HashSet;

use open_sdbl::metadata::{
    LiveTable, SchemaStorage, parse_db_names,
    resolve_metadata,
};
use open_sdbl::query::{CompileOptions, PostgresBackend, QueryCompiler};

use super::*;

#[test]
fn lists_and_completes_session_temporary_tables() {
    assert!(CONSOLE_HELP.contains("\\tables"));
    assert!(CONSOLE_HELP.contains("ПОМЕСТИТЬ"));

    let mut empty = Vec::new();
    print_temporary_tables(&mut empty, &TempTablesManager::new()).unwrap();
    assert_eq!(
        String::from_utf8(empty).unwrap(),
        "No temporary tables placed.\n"
    );

    let mut helper = ConsoleHelper::for_test(
        vec!["\\tables".to_owned()],
        vec!["Справочник.Договоры".to_owned()],
        HashSet::new(),
    );
    helper.set_temporary_tables(vec!["Обороты".to_owned(), "Остатки".to_owned()]);

    let (_, sources) = helper.complete_values("ИЗ ", "ИЗ ".len());
    assert_eq!(
        sources
            .iter()
            .map(|candidate| candidate.replacement.as_str())
            .collect::<Vec<_>>(),
        ["Обороты", "Остатки", "Справочник.Договоры"]
    );

    let (_, filtered) = helper.complete_values("ИЗ обо", "ИЗ обо".len());
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].replacement, "Обороты");

    // Temporary tables are sources, not expression candidates.
    let (_, projection) = helper.complete_values("ВЫБРАТЬ обо", "ВЫБРАТЬ обо".len());
    assert!(projection.is_empty());

    let (_, commands) = helper.complete_values("\\tab", "\\tab".len());
    assert_eq!(commands[0].replacement, "\\tables");
}

#[test]
fn lists_temporary_tables_placed_in_this_session() {
    let serialized = b"{1,{b8bac76b-c91b-4d78-8a70-ffa39f8de694,\"Reference\",53}}";
    let length = u16::try_from(serialized.len()).unwrap();
    let mut compressed = vec![1];
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(serialized);
    let snapshot = resolve_metadata(
        parse_db_names(&compressed).unwrap(),
        Vec::new(),
        SchemaStorage {
            tables: Vec::new(),
            anomalies: Vec::new(),
        },
        Vec::<LiveTable>::new(),
    )
    .snapshot;

    let mut temporary = TempTablesManager::new();
    QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_batch(
            "ВЫБРАТЬ 1 КАК Итог, ИСТИНА КАК Флаг ПОМЕСТИТЬ Обороты;",
            &CompileOptions::new(),
            &mut temporary,
        )
        .unwrap();

    let mut listing = Vec::new();
    print_temporary_tables(&mut listing, &temporary).unwrap();
    assert_eq!(
        String::from_utf8(listing).unwrap(),
        "Обороты  Итог [Number], Флаг [Boolean]\n"
    );
}

#[test]
fn labels_temporary_table_column_kinds() {
    assert_eq!(
        column_kind_label(&ColumnKind::String { length: Some(9) }),
        "String"
    );
    assert_eq!(
        column_kind_label(&ColumnKind::Reference {
            targets: Vec::new(),
            runtime_typed: true,
        }),
        "Reference*"
    );
    assert_eq!(column_kind_label(&ColumnKind::DateTime), "DateTime");
    assert_eq!(
        column_kind_label(&ColumnKind::Unknown {
            data_type: "bytea".to_owned(),
        }),
        "Unknown"
    );
}
