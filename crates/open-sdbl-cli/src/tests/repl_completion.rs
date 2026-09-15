//! Tests of the console `completion` module.

use std::borrow::Cow;
use std::collections::HashSet;

use open_sdbl::metadata::MetadataKind;
use rustyline::highlight::Highlighter;

use super::*;

#[test]
fn completes_commands_keywords_and_cyrillic_metadata_case_insensitively() {
    let helper = ConsoleHelper::for_test(
        vec![
            "\\refresh".to_owned(),
            "ВЫБРАТЬ".to_owned(),
            "Справочник.Договоры".to_owned(),
            "Организация.Код".to_owned(),
        ],
        vec!["Справочник.Договоры".to_owned()],
        HashSet::new(),
    );

    let (_, commands) = helper.complete_values("\\REF", "\\REF".len());
    assert_eq!(commands[0].replacement, "\\refresh");
    let (_, objects) = helper.complete_values("из справ", "из справ".len());
    assert_eq!(objects[0].replacement, "Справочник.Договоры");
    let (_, fields) = helper.complete_values("Организация.к", "Организация.к".len());
    assert_eq!(fields[0].replacement, "Организация.Код");
}

#[test]
fn candidate_deduplication_keeps_the_first_case_insensitive_spelling() {
    let mut candidates = vec!["Код".to_owned()];
    let mut keys = HashSet::from(["код".to_owned()]);
    push_unique(&mut candidates, &mut keys, "КОД");
    push_unique(&mut candidates, &mut keys, "Description");
    push_unique(&mut candidates, &mut keys, "description");
    assert_eq!(candidates, ["Код", "Description"]);
}

#[test]
fn stores_reference_completion_aliases_linearly_and_expands_only_a_typed_path() {
    let path = CompletionPath::new(
        vec!["Организация".to_owned(), "Organization".to_owned()],
        vec![
            "Код".to_owned(),
            "Code".to_owned(),
            "Description".to_owned(),
        ],
    );
    assert_eq!(path.stored_names(), 2 + 3);
    assert_ne!(path.stored_names(), 2 * 3);
    let mut helper = ConsoleHelper::for_test(Vec::new(), Vec::new(), HashSet::new());
    helper.paths.push(path);

    let (_, empty) = helper.complete_values("SELECT ", "SELECT ".len());
    assert!(empty.is_empty());
    let source = "SELECT Организация.к";
    let (_, values) = helper.complete_values(source, source.len());
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].replacement, "Организация.Код");
}

#[test]
fn completes_virtual_tables_by_resolved_register_kind() {
    let accumulation_names = [
        "Остатки".to_owned(),
        "AccumulationRegister.Остатки".to_owned(),
        "РегистрНакопления.Остатки".to_owned(),
    ];
    let information_names = [
        "Цены".to_owned(),
        "InformationRegister.Цены".to_owned(),
        "РегистрСведений.Цены".to_owned(),
    ];
    let mut candidates = Vec::new();
    let mut candidate_keys = HashSet::new();
    push_virtual_table_candidates(
        &mut candidates,
        &mut candidate_keys,
        MetadataKind::AccumulationRegister,
        &accumulation_names,
    );
    push_virtual_table_candidates(
        &mut candidates,
        &mut candidate_keys,
        MetadataKind::InformationRegister,
        &information_names,
    );
    let helper = ConsoleHelper::for_test(candidates.clone(), candidates, HashSet::new());

    let russian = "из регистрнакопления.остатки.ос";
    let (_, values) = helper.complete_values(russian, russian.len());
    assert_eq!(values[0].replacement, "РегистрНакопления.Остатки.Остатки()");
    let english = "FROM AccumulationRegister.Остатки.ba";
    let (_, values) = helper.complete_values(english, english.len());
    assert_eq!(
        values[0].replacement,
        "AccumulationRegister.Остатки.Balance()"
    );
    let slice = "ИЗ РегистрСведений.Цены.срезп";
    let (_, values) = helper.complete_values(slice, slice.len());
    assert_eq!(values.len(), 2);
    assert!(
        values
            .iter()
            .any(|value| value.replacement.ends_with("СрезПервых()"))
    );
    assert!(
        values
            .iter()
            .any(|value| value.replacement.ends_with("СрезПоследних()"))
    );
}

#[test]
fn completes_service_sources_under_their_owners() {
    let mut candidates = Vec::new();
    let mut keys = HashSet::new();
    push_service_table_candidates(
        &mut candidates,
        &mut keys,
        MetadataKind::AccumulationRegister,
        &["РегистрНакопления.RegisteredTotals".to_owned()],
        true,
    );
    push_service_table_candidates(
        &mut candidates,
        &mut keys,
        MetadataKind::ChartOfCalculationTypes,
        &["ChartOfCalculationTypes.Payroll".to_owned()],
        false,
    );
    push_service_table_candidates(
        &mut candidates,
        &mut keys,
        MetadataKind::ChartOfAccounts,
        &["ChartOfAccounts.Main".to_owned()],
        false,
    );

    assert!(candidates.contains(&"РегистрНакопления.RegisteredTotals.Изменения".to_owned()));
    assert!(
        candidates.contains(&"ChartOfCalculationTypes.Payroll.LeadingCalculationKinds".to_owned())
    );
    assert!(candidates.contains(&"ChartOfAccounts.Main.ExtraDimensions".to_owned()));
}

#[test]
fn restricts_source_completion_to_the_qualified_metadata_hierarchy() {
    let helper = ConsoleHelper::for_test(
        vec![
            "Код".to_owned(),
            "Договоры".to_owned(),
            "_Референс42".to_owned(),
            "Организация.Код".to_owned(),
        ],
        vec![
            "Catalog.Contracts".to_owned(),
            "Document.Sale".to_owned(),
            "РегистрНакопления.Остатки".to_owned(),
            "РегистрНакопления.Остатки.Остатки()".to_owned(),
            "Справочник.Договоры".to_owned(),
        ],
        HashSet::new(),
    );

    let (_, empty_source) = helper.complete_values("FROM ", "FROM ".len());
    assert_eq!(empty_source.len(), 4);
    assert!(
        empty_source
            .iter()
            .all(|candidate| candidate.replacement.matches('.').count() == 1)
    );
    assert!(empty_source.iter().all(|candidate| {
        !matches!(
            candidate.replacement.as_str(),
            "Код" | "Договоры" | "_Референс42"
        )
    }));

    let (_, catalogs) = helper.complete_values("из спр", "из спр".len());
    assert_eq!(catalogs.len(), 1);
    assert_eq!(catalogs[0].replacement, "Справочник.Договоры");

    let virtual_prefix = "JOIN РегистрНакопления.Остатки.ос";
    let (_, virtual_sources) = helper.complete_values(virtual_prefix, virtual_prefix.len());
    assert_eq!(virtual_sources.len(), 1);
    assert_eq!(
        virtual_sources[0].replacement,
        "РегистрНакопления.Остатки.Остатки()"
    );

    let (_, fields) =
        helper.complete_values("ВЫБРАТЬ Организация.к", "ВЫБРАТЬ Организация.к".len());
    assert_eq!(fields[0].replacement, "Организация.Код");
}

#[test]
fn does_not_attach_virtual_tables_to_catalogs() {
    let mut candidates = Vec::new();
    let mut candidate_keys = HashSet::new();
    push_virtual_table_candidates(
        &mut candidates,
        &mut candidate_keys,
        MetadataKind::Catalog,
        &["Справочник.Номенклатура".to_owned()],
    );
    assert!(candidates.is_empty());
}

#[test]
fn finds_completion_boundary_without_splitting_utf8_or_dotted_names() {
    let line = "ВЫБРАТЬ Организация.Ко";
    assert_eq!(completion_start(line, line.len()), "ВЫБРАТЬ ".len());
    assert_eq!(completion_start("\\d Спр", "\\d Спр".len()), "\\d ".len());
}

#[test]
fn highlights_lexer_tokens_without_changing_display_text() {
    let helper = ConsoleHelper::for_test(
        Vec::new(),
        Vec::new(),
        HashSet::from(["договоры".to_owned()]),
    );
    let line = "ВЫБРАТЬ Договоры // test";
    let Cow::Owned(highlighted) = helper.highlight(line, line.len()) else {
        panic!("expected styled output");
    };
    assert!(highlighted.contains("\x1b[1;34mВЫБРАТЬ\x1b[0m"));
    assert!(highlighted.contains("\x1b[36mДоговоры\x1b[0m"));
    assert!(highlighted.contains("\x1b[2;37m// test\x1b[0m"));
    let plain = highlighted
        .replace("\x1b[1;34m", "")
        .replace("\x1b[36m", "")
        .replace("\x1b[2;37m", "")
        .replace("\x1b[0m", "");
    assert_eq!(plain, line);
}
