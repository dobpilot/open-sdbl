//! The accounting register on the UNF fixture: its fields, its chart, and
//! what its virtual tables answer while they are staged.

mod support;

use std::path::PathBuf;

use open_sdbl::metadata::{ConfigFieldPurpose, MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::query::{
    CompileOptions, ParameterDate, ParameterValue, PostgresBackend, QueryCompiler,
    QueryDiagnosticKind, QueryParameter, SessionParameters,
};
use support::*;

/// The UNF fixture carries the register `Управленческий`; without it the
/// tests have nothing to check and pass vacuously.
fn unf() -> Option<MetadataSnapshot> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unf");
    root.join("db_names.deflate")
        .is_file()
        .then(|| demo_resolved_at(&root).snapshot)
}

fn compile(
    snapshot: &MetadataSnapshot,
    source: &str,
) -> Result<String, open_sdbl::query::QueryDiagnostic> {
    let mut session = SessionParameters::new();
    let zero = ParameterValue::Number {
        unscaled: 0,
        scale: 0,
    };
    for name in ["ОбластьДанныхЗначение", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(QueryParameter::new(name, zero.clone()));
    }
    session.set(QueryParameter::new(
        "ОбластьДанныхИспользование",
        ParameterValue::Boolean(false),
    ));
    // Only the parameters the text names may be supplied.
    let parameters = open_sdbl::tokenize(source)
        .unwrap()
        .iter()
        .filter(|token| token.kind == open_sdbl::TokenKind::Parameter)
        .map(|token| {
            let name = token.lexeme.trim_start_matches('&');
            // The period bounds of a virtual table need a date.
            let value = match name {
                "Н" | "Д" => {
                    ParameterValue::Date(ParameterDate::new(2024, 1, 1, 0, 0, 0).unwrap())
                }
                "К" => ParameterValue::Date(ParameterDate::new(2024, 12, 31, 23, 59, 59).unwrap()),
                _ => ParameterValue::Null,
            };
            QueryParameter::new(name, value)
        })
        .collect::<Vec<_>>();
    // A parameter the text names twice is supplied once.
    let mut seen = std::collections::BTreeSet::new();
    let parameters = parameters
        .into_iter()
        .filter(|parameter| seen.insert(parameter.name().to_owned()))
        .collect::<Vec<_>>();
    let options = CompileOptions::new()
        .session(&session)
        .parameters(&parameters);
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile_with(source, &options)
        .map(|compiled| compiled.sql)
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

#[test]
fn resolves_purposes_balance_flags_and_the_chart() {
    let Some(snapshot) = unf() else {
        return;
    };
    let register = snapshot
        .object_id(MetadataKind::AccountingRegister, "Управленческий")
        .unwrap();
    let chart = snapshot
        .object_id(MetadataKind::ChartOfAccounts, "Управленческий")
        .unwrap();
    assert_eq!(
        snapshot.object_by_id(register).unwrap().chart_of_accounts,
        Some(chart)
    );
    let field = |name: &str| {
        snapshot
            .fields()
            .iter()
            .find(|field| {
                field.name.as_deref() == Some(name)
                    && field
                        .owner_tables
                        .iter()
                        .any(|table| table.to_lowercase().ends_with("accrg411"))
            })
            .unwrap_or_else(|| panic!("{name}"))
    };
    let organization = field("Организация");
    assert_eq!(
        organization.purpose,
        Some(ConfigFieldPurpose::AccountingRegisterDimension)
    );
    assert_eq!(organization.balance, Some(true));
    let currency = field("Валюта");
    assert_eq!(
        currency.purpose,
        Some(ConfigFieldPurpose::AccountingRegisterDimension)
    );
    assert_eq!(currency.balance, Some(false));
    let amount = field("Сумма");
    assert_eq!(
        amount.purpose,
        Some(ConfigFieldPurpose::AccountingRegisterResource)
    );
    assert_eq!(amount.balance, Some(true));
    let currency_amount = field("СуммаВал");
    assert_eq!(currency_amount.balance, Some(false));
    let content = field("Содержание");
    assert_eq!(
        content.purpose,
        Some(ConfigFieldPurpose::AccountingRegisterAttribute)
    );
    assert_eq!(content.balance, None);
    let _: ObjectId = register;
}

#[test]
fn exposes_accounts_and_the_sides_of_non_balance_fields() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Т.СчетДт КАК Дт, Т.СчетКт КАК Кт, Т.Организация КАК О, Т.Сумма КАК С,
                 Т.ВалютаДт КАК ВД, Т.СуммаВалКт КАК СВК, Т.Содержание КАК Текст
         ИЗ РегистрБухгалтерии.Управленческий КАК Т
         ГДЕ Т.Активность И Т.СчетДт.Вид = ЗНАЧЕНИЕ(ВидСчета.Активный);",
    )
    .unwrap();
    assert_contains(&sql, "\"Т\".\"_accountdtrref\" AS \"Дт\"");
    assert_contains(&sql, "\"Т\".\"_accountctrref\" AS \"Кт\"");
    assert_contains(&sql, "\"Т\".\"_fld412rref\" AS \"О\"");
    assert_contains(&sql, "\"Т\".\"_fld415\" AS \"С\"");
    assert_contains(&sql, "\"Т\".\"_fld414dtrref\" AS \"ВД\"");
    assert_contains(&sql, "\"Т\".\"_fld416ct\" AS \"СВК\"");
    assert_contains(&sql, "\"_kind\" = 0");
    let english = compile(
        &snapshot,
        "SELECT T.AccountDr AS Dr, T.СуммаВалCr AS Cr FROM AccountingRegister.Управленческий AS T;",
    )
    .unwrap();
    assert_contains(&english, "\"_accountdtrref\" AS \"Dr\"");
    assert_contains(&english, "\"_fld416ct\" AS \"Cr\"");
    // A non-balance field has no side-less name.
    let error = compile(
        &snapshot,
        "ВЫБРАТЬ Т.СуммаВал КАК С ИЗ РегистрБухгалтерии.Управленческий КАК Т;",
    )
    .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnknownField);
}

#[test]
fn accounting_virtual_tables_take_the_platform_arity() {
    let Some(snapshot) = unf() else {
        return;
    };
    // Without extra dimensions the two record-level tables take the
    // shorter layouts too.
    let dr_cr = compile(
        &snapshot,
        "ВЫБРАТЬ О.СчетДт КАК Дт, О.СчетКт КАК Кт, О.СуммаОборот КАК С ИЗ РегистрБухгалтерии.Управленческий.ОборотыДтКт(&Н, &К, , СчетДт = &Счет, , ) КАК О;",
    )
    .unwrap();
    assert_contains(
        &dr_cr,
        "GROUP BY \"__aggregate_base\".\"_accountdtrref\", \"__aggregate_base\".\"_accountctrref\"",
    );
    let records = compile(
        &snapshot,
        "ВЫБРАТЬ О.СчетДт КАК Дт, О.Сумма КАК С ИЗ РегистрБухгалтерии.Управленческий.ДвиженияССубконто(&Н, &К, Организация = &Орг) КАК О;",
    )
    .unwrap();
    assert_contains(&records, "FROM \"_accrg411\" AS \"__aggregate_base\" WHERE");
    let eight = compile(
        &snapshot,
        "ВЫБРАТЬ О.СчетДт ИЗ РегистрБухгалтерии.Управленческий.ОборотыДтКт(&Н, &К, , , , , , ) КАК О;",
    )
    .unwrap_err();
    assert_eq!(eight.kind(), QueryDiagnosticKind::Syntax);
    // The parser takes the counts of a register with extra dimensions;
    // the UNF register has none, so one more than its own count stops in
    // the compiler, also as a syntax error.
    for source in [
        "ВЫБРАТЬ О.Счет ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д, , , , ) КАК О;",
        "ВЫБРАТЬ О.Счет ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д, , , ) КАК О;",
        "ВЫБРАТЬ О.Счет ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, , , , , ) КАК О;",
    ] {
        let too_many = compile(&snapshot, source).unwrap_err();
        assert_eq!(too_many.kind(), QueryDiagnosticKind::Syntax, "{source}");
    }
    // An accumulation register keeps its own counts.
    let accumulation = compile(
        &snapshot,
        "ВЫБРАТЬ О.Организация ИЗ РегистрНакопления.ЗапасыНаСкладах.Остатки(&Д, , ) КАК О;",
    )
    .unwrap_err();
    assert_eq!(accumulation.kind(), QueryDiagnosticKind::Syntax);
}

#[test]
fn resolves_a_predefined_account() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ С.Код КАК Код ИЗ ПланСчетов.Управленческий КАК С
         ГДЕ С.Ссылка = ЗНАЧЕНИЕ(ПланСчетов.Управленческий.ПрочиеРасходы);",
    )
    .unwrap();
    assert_contains(&sql, "FROM \"_acc17\" AS \"__open_sdbl_value\"");
    assert_contains(&sql, "\"__open_sdbl_value\".\"_predefinedid\" = ");
    let absent = compile(
        &snapshot,
        "ВЫБРАТЬ С.Код КАК Код ИЗ ПланСчетов.Управленческий КАК С
         ГДЕ С.Ссылка = ЗНАЧЕНИЕ(ПланСчетов.Управленческий.НетТакогоСчета);",
    )
    .unwrap_err();
    assert_eq!(absent.kind(), QueryDiagnosticKind::UnknownValue);
}

#[test]
fn turnovers_fold_the_two_sides_of_a_record() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Организация КАК Орг, О.Валюта КАК Вал,
                 О.СуммаОборот КАК Об, О.СуммаОборотДт КАК Дт, О.СуммаВалОборотКт КАК ВалКт
         ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет = &Счет, Организация = &Орг) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    // Two branches, one per side, under one account column.
    assert_contains(
        &sql,
        "0 AS \"__side\", \"__aggregate_base\".\"_accountdtrref\" AS \"_account\"",
    );
    assert_contains(
        &sql,
        "1 AS \"__side\", \"__aggregate_base\".\"_accountctrref\" AS \"_account\"",
    );
    assert_contains(&sql, " UNION ALL ");
    // The non-balance dimension and resource take the side's column.
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_fld414dtrref\" AS \"_fld414rref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_fld414ctrref\" AS \"_fld414rref\"",
    );
    assert_contains(&sql, "\"__aggregate_base\".\"_fld416dt\" AS \"_fld416\"");
    assert_contains(&sql, "\"__aggregate_base\".\"_fld416ct\" AS \"_fld416\"");
    // The account condition applies to each side's account, the condition
    // to each side's row.
    assert_contains(&sql, "(\"__aggregate_base\".\"_accountdtrref\" = ");
    assert_contains(&sql, "(\"__aggregate_base\".\"_accountctrref\" = ");
    assert_eq!(
        sql.matches("\"__aggregate_base\".\"_fld412rref\" = ")
            .count(),
        2
    );
    // The three turnover columns per resource.
    assert_contains(
        &sql,
        "SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld415\" ELSE -\"__sides\".\"_fld415\" END) AS \"_fld415\"",
    );
    assert_contains(
        &sql,
        "SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld415\" ELSE 0 END) AS \"_fld415TurnoverDt\"",
    );
    assert_contains(
        &sql,
        "SUM(CASE WHEN \"__sides\".\"__side\" = 1 THEN \"__sides\".\"_fld416\" ELSE 0 END) AS \"_fld416TurnoverCt\"",
    );
    assert_contains(
        &sql,
        "GROUP BY \"__sides\".\"_account\", \"__sides\".\"_fld405\", \"__sides\".\"_fld412rref\"",
    );
}

#[test]
fn turnovers_prune_unread_dimensions_and_split_by_period() {
    let Some(snapshot) = unf() else {
        return;
    };
    let pruned = compile(
        &snapshot,
        "ВЫБРАТЬ О.СуммаОборотДт КАК Дт ИЗ РегистрБухгалтерии.Управленческий.Обороты КАК О;",
    )
    .unwrap();
    println!("SQL {pruned}");
    assert_contains(
        &pruned,
        "SUM(\"__aggregate_used\".\"_fld415TurnoverDt\") AS \"_fld415TurnoverDt\"",
    );
    assert!(
        !pruned.contains("GROUP BY \"__aggregate_used\""),
        "{pruned}"
    );
    let monthly = compile(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.Счет КАК Счет, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, Месяц) КАК О;",
    )
    .unwrap();
    assert_contains(
        &monthly,
        "date_trunc('month', \"__aggregate_base\".\"_period\") AS \"_period\"",
    );
    assert_contains(&monthly, ", \"__sides\".\"_period\" AS \"_period\"");
    // The chart of UNF has no extra dimensions, so the platform omits the
    // `Субконто` arguments: the condition is fifth and the balanced
    // account's condition sixth.
    let by_recorder = compile(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.Регистратор КАК Р, О.СуммаОборотКт КАК Кт
         ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, Регистратор, , Организация = &Орг) КАК О;",
    )
    .unwrap();
    assert_contains(
        &by_recorder,
        "\"__sides\".\"_recorderrref\" AS \"_recorderrref\"",
    );
    assert_contains(
        &by_recorder,
        "\"__sides\".\"_period\", \"__sides\".\"_recordertref\", \"__sides\".\"_recorderrref\")",
    );
    let too_many = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет ИЗ РегистрБухгалтерии.Управленческий.Обороты(, , , , , , ) КАК О;",
    )
    .unwrap_err();
    assert_eq!(too_many.kind(), QueryDiagnosticKind::Syntax);
    // `Авто` splits by what the statement reads, as for an accumulation
    // register.
    let auto = compile(
        &snapshot,
        "ВЫБРАТЬ О.ПериодМесяц КАК М, О.Регистратор КАК Р, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, Авто) КАК О;",
    )
    .unwrap();
    assert_contains(&auto, "\"__aggregate_used\".\"_PeriodMonth\"");
    assert_contains(&auto, "\"__aggregate_used\".\"_recorderrref\"");
    assert!(
        !auto.contains("\"__aggregate_used\".\"_PeriodYear\""),
        "{auto}"
    );
    // The balanced-account condition of a register without extra
    // dimensions takes the sixth position.
    let balanced = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК С, О.КорСчет КАК КС ИЗ РегистрБухгалтерии.Управленческий.Обороты(, , , , , КорСчет = &Счет) КАК О;",
    )
    .unwrap();
    assert_contains(&balanced, "\"__aggregate_used\".\"__cor_account\"");
}

#[test]
fn balances_derive_their_debit_and_credit_parts_at_the_grain_read() {
    let Some(snapshot) = unf() else {
        return;
    };
    let full = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Организация КАК Орг, О.СуммаОстаток КАК Ост,
                 О.СуммаОстатокДт КАК Дт, О.СуммаОстатокКт КАК Кт
         ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д, Счет = &Счет, Организация = &Орг) КАК О;",
    )
    .unwrap();
    println!("SQL {full}");
    assert_contains(&full, "(\"__aggregate_base\".\"_period\" < ");
    assert_contains(
        &full,
        "SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld415\" ELSE -\"__sides\".\"_fld415\" END) AS \"_fld415\"",
    );
    assert_contains(
        &full,
        "CASE WHEN SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld415\" ELSE -\"__sides\".\"_fld415\" END) > 0 THEN SUM(",
    );
    assert_contains(&full, " AS \"_fld415Dt\"");
    assert_contains(&full, "HAVING (SUM(CASE WHEN");
    // When the organization is summed away, the parts are recomputed from
    // the summed balance rather than summed themselves.
    let pruned = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаОстатокДт КАК Дт
         ИЗ РегистрБухгалтерии.Управленческий.Остатки(&Д) КАК О;",
    )
    .unwrap();
    println!("SQL {pruned}");
    assert_contains(
        &pruned,
        "CASE WHEN SUM(\"__aggregate_used\".\"_fld415\") > 0 THEN SUM(\"__aggregate_used\".\"_fld415\") ELSE 0 END AS \"_fld415Dt\"",
    );
    assert!(
        !pruned.contains("SUM(\"__aggregate_used\".\"_fld415Dt\")"),
        "{pruned}"
    );
}

#[test]
fn balance_and_turnovers_answer_opening_turnover_and_closing() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаНачальныйОстаток КАК НО, О.СуммаОборотДт КАК Дт,
                 О.СуммаКонечныйОстатокКт КАК КОКт, О.СуммаВалОборот КАК Вал
         ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, , , Счет = &Счет) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(&sql, "SUM(CASE WHEN \"__sides\".\"__record_period\" < ");
    assert_contains(&sql, " AS \"_fld415OpeningBalance\"");
    assert_contains(&sql, " AS \"_fld415ClosingBalanceCt\"");
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_period\" AS \"__record_period\"",
    );
    // Without a split the opening balance reads the records before the
    // interval, so only the end bound filters the rows.
    assert!(
        !sql.contains("(\"__aggregate_base\".\"_period\" >= "),
        "{sql}"
    );
    // `Авто` with a balance and no split field read answers the whole
    // interval; with a split field it refuses the balance.
    let auto = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Организация КАК Орг, О.СуммаКонечныйОстаток КАК КО
         ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, Авто, ДвиженияИГраницыПериода, , Организация = &Орг) КАК О;",
    )
    .unwrap();
    assert_contains(&auto, "\"_fld415ClosingBalance\"");
    // A split field with a balance takes the running sums over the
    // buckets of that grain; the parts stay derived from the running
    // balance.
    let split = compile(
        &snapshot,
        "ВЫБРАТЬ О.Регистратор КАК Р, О.СуммаКонечныйОстаток КАК КО, О.СуммаКонечныйОстатокКт КАК КОКт
         ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, Авто, , ) КАК О;",
    )
    .unwrap();
    println!("SQL {split}");
    assert_contains(
        &split,
        "OVER (PARTITION BY \"__sides\".\"_account\", \"__sides\".\"_fld405\"",
    );
    assert_contains(
        &split,
        "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS \"_fld415ClosingBalance\"",
    );
    assert_contains(
        &split,
        "CASE WHEN SUM(SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld415\" ELSE -\"__sides\".\"_fld415\" END)) OVER (",
    );
    assert_contains(&split, "\"__aggregate_used\".\"_recorderrref\"");
    let monthly = compile(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Управленческий.ОстаткиИОбороты(&Н, &К, Месяц, , ) КАК О;",
    )
    .unwrap();
    assert_contains(
        &monthly,
        "date_trunc('month', \"__aggregate_base\".\"_period\") AS \"_period\"",
    );
    assert_contains(&monthly, "(\"__aggregate_base\".\"_period\" >= ");
}

#[test]
fn conditions_of_virtual_tables_dereference_through_a_join() {
    let Some(snapshot) = unf() else {
        return;
    };
    // The account condition through the account's attribute joins the
    // chart to each branch on that branch's account.
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, , Счет.Вид = ЗНАЧЕНИЕ(ВидСчета.Активный)) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(
        &sql,
        "FROM \"_accrg411\" AS \"__aggregate_base\" LEFT JOIN \"_acc17\" AS \"__ref1\" ON \"__aggregate_base\".\"_accountdtrref\" = \"__ref1\".\"_idrref\"",
    );
    assert_contains(
        &sql,
        "FROM \"_accrg411\" AS \"__aggregate_base\" LEFT JOIN \"_acc17\" AS \"__ref1\" ON \"__aggregate_base\".\"_accountctrref\" = \"__ref1\".\"_idrref\"",
    );
    assert_contains(&sql, "(\"__ref1\".\"_kind\" = 0)");
    // The same for an accumulation register, in the movement branch of a
    // balance with a boundary and in the totals branch.
    let balance = compile(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОстаток КАК К
         ИЗ РегистрНакопления.ЗапасыНаСкладах.Остатки(&Д, Номенклатура.Родитель = &Р) КАК О;",
    )
    .unwrap();
    println!("SQL {balance}");
    assert_contains(
        &balance,
        "AS \"__totals_base\" LEFT JOIN \"_reference76\" AS \"__ref1\" ON \"__totals_base\".\"_fld7956rref\" = \"__ref1\".\"_idrref\"",
    );
    assert_contains(
        &balance,
        "AS \"__movement_base\" LEFT JOIN \"_reference76\" AS \"__ref1\" ON \"__movement_base\".\"_fld7956rref\" = \"__ref1\".\"_idrref\"",
    );
    let turnovers = compile(
        &snapshot,
        "ВЫБРАТЬ О.Номенклатура КАК Н, О.КоличествоОборот КАК К
         ИЗ РегистрНакопления.ЗапасыНаСкладах.Обороты(, , , Номенклатура.Родитель = &Р) КАК О;",
    )
    .unwrap();
    assert_contains(
        &turnovers,
        "AS \"__aggregate_base\" LEFT JOIN \"_reference76\" AS \"__ref1\" ON",
    );
}

/// The demo Бухгалтерия предприятия fixture: a register with three extra
/// dimensions.
fn buh() -> Option<MetadataSnapshot> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/buh");
    root.join("db_names.deflate")
        .is_file()
        .then(|| demo_resolved_at(&root).snapshot)
}

#[test]
fn a_long_named_composite_field_spreads_undefined_over_its_members() {
    let Some(snapshot) = buh() else {
        return;
    };
    // The label `СубконтоПоАмортизационнойПремии1_TYPE` is cut to the
    // identifier limit of PostgreSQL; the member is still told by the
    // requested name, so the other branch spreads over both members.
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СубконтоПоАмортизационнойПремии1 КАК СубконтоПоАмортизационнойПремии1 \
         ИЗ Документ.МодернизацияОС КАК Д \
         ОБЪЕДИНИТЬ ВСЕ \
         ВЫБРАТЬ НЕОПРЕДЕЛЕНО ИЗ Документ.МодернизацияОС КАК Д",
    )
    .unwrap();
    assert!(sql.contains("UNION ALL"), "{sql}");
    assert!(sql.contains("NULL AS \"column1\""), "{sql}");
    assert!(
        sql.contains("THEN decode('01', 'hex') END AS \"column1_TYPE\""),
        "{sql}"
    );
}

#[test]
fn compiles_the_extra_dimension_values_table() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ С.Вид КАК Вид, С.Значение КАК Значение, С.ВидДвижения КАК Сторона, С.НомерСтроки КАК Н
         ИЗ РегистрБухгалтерии.Хозрасчетный.Субконто КАК С ГДЕ С.Регистратор = &Д;",
    )
    .unwrap();
    assert_contains(&sql, "FROM \"_accrged38493\" AS \"С\"");
    assert_contains(&sql, "\"С\".\"_kindrref\" AS \"Вид\"");
    assert_contains(&sql, "\"С\".\"_correspond\" AS \"Сторона\"");
    assert_contains(&sql, "\"С\".\"_value_rrref\"");
    let kind = compile(
        &snapshot,
        "ВЫБРАТЬ С.Значение КАК З ИЗ РегистрБухгалтерии.Хозрасчетный.Субконто КАК С
         ГДЕ С.Вид = ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты);",
    )
    .unwrap();
    assert_contains(&kind, "\"_predefinedid\" = ");
}

#[test]
fn extra_dimensions_are_positional_dimensions_of_the_aggregates() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Субконто1 КАК С1, О.ВидСубконто2 КАК В2, О.СуммаОстаток КАК Ост
         ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, Счет = &Счет) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuedt1_rrref\" AS \"_value1_rrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuect1_rrref\" AS \"_value1_rrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_kinddt2rref\" AS \"_kind2rref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_used\".\"_value1_type\", \"__aggregate_used\".\"_value1_rtref\", \"__aggregate_used\".\"_value1_rrref\"",
    );
    assert!(
        !sql.contains("\"__aggregate_used\".\"_value3_rrref\""),
        "{sql}"
    );
    // The condition sees them as well.
    let filtered = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаОборотДт КАК Дт
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , ВидСубконто1 = &Вид) КАК О;",
    )
    .unwrap();
    assert_contains(&filtered, "(\"__aggregate_base\".\"_kinddt1rref\" = ");
    assert_contains(&filtered, "(\"__aggregate_base\".\"_kindct1rref\" = ");
}

#[test]
fn listed_kinds_pick_the_level_that_carries_them() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Субконто1 КАК Контрагент, О.СуммаОстатокДт КАК Дт
         ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты)) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(
        &sql,
        "CASE WHEN \"__aggregate_base\".\"_kinddt1rref\" = (SELECT",
    );
    assert_contains(
        &sql,
        "THEN \"__aggregate_base\".\"_valuedt1_rrref\" WHEN \"__aggregate_base\".\"_kinddt2rref\" = (SELECT",
    );
    assert_contains(
        &sql,
        "THEN \"__aggregate_base\".\"_valuect3_rrref\" END AS \"_value1_rrref\"",
    );
    // Records whose side lacks the kind are dropped.
    assert_contains(&sql, "AND (\"__aggregate_base\".\"_kinddt1rref\" = (SELECT");
    assert_contains(&sql, " OR \"__aggregate_base\".\"_kinddt3rref\" = (SELECT");
    let two = compile(
        &snapshot,
        "ВЫБРАТЬ О.Субконто2 КАК Д, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , (ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты), &Вид)) КАК О;",
    )
    .unwrap();
    assert_contains(&two, "AS \"_value2_rrref\"");
    // An unbound parameter in the list keeps the positions.
    let unbound = compile(
        &snapshot,
        "ВЫБРАТЬ О.Субконто2 КАК Д ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , &ВидыСубконто) КАК О;",
    )
    .unwrap();
    assert_contains(
        &unbound,
        "\"__aggregate_base\".\"_valuedt2_rrref\" AS \"_value2_rrref\"",
    );
    let too_many = compile(
        &snapshot,
        "ВЫБРАТЬ О.Субконто1 КАК Д ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , (ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты), ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Договоры), ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты), ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Договоры))) КАК О;",
    )
    .unwrap_err();
    assert_eq!(too_many.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn dr_cr_turnovers_group_by_both_accounts_and_both_sides() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.СчетДт КАК Дт, О.СчетКт КАК Кт, О.СубконтоДт1 КАК СД1, О.СубконтоКт1 КАК СК1,
                 О.ПодразделениеДт КАК П, О.СуммаОборот КАК С, О.КоличествоОборотКт КАК КК
         ИЗ РегистрБухгалтерии.Хозрасчетный.ОборотыДтКт(&Н, &К, , СчетДт В (&Счета), , СчетКт = &Счет, , Организация = &Орг) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_accountdtrref\" AS \"_accountdtrref\", \"__aggregate_base\".\"_accountctrref\" AS \"_accountctrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuedt1_rrref\" AS \"_valuedt1_rrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuect1_rrref\" AS \"_valuect1_rrref\"",
    );
    assert_contains(
        &sql,
        "SUM(\"__aggregate_base\".\"_fld38460\") AS \"_fld38460\"",
    );
    assert_contains(
        &sql,
        "SUM(\"__aggregate_base\".\"_fld38462ct\") AS \"_fld38462ct\"",
    );
    assert_contains(&sql, "(\"__aggregate_base\".\"_accountctrref\" = ");
    assert!(!sql.contains(" UNION ALL "), "{sql}");
    // Pruned to the two accounts and a resource.
    let pruned = compile(
        &snapshot,
        "ВЫБРАТЬ О.СчетДт КАК Дт, О.СуммаОборот КАК С
         ИЗ РегистрБухгалтерии.Хозрасчетный.ОборотыДтКт(&Н, &К, Регистратор, , , , , ) КАК О;",
    )
    .unwrap();
    assert_contains(
        &pruned,
        "GROUP BY \"__aggregate_used\".\"_accountdtrref\", \"__aggregate_used\".\"_period\", \"__aggregate_used\".\"_recordertref\"",
    );
    // Listed kinds per side.
    let listed = compile(
        &snapshot,
        "ВЫБРАТЬ О.СубконтоДт1 КАК ОС, О.СубконтоКт1 КАК К, О.СуммаОборот КАК С
         ИЗ РегистрБухгалтерии.Хозрасчетный.ОборотыДтКт(&Н, &К, , , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Договоры), , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты), ) КАК О;",
    )
    .unwrap();
    println!("SQL {listed}");
    assert_contains(
        &listed,
        "CASE WHEN \"__aggregate_base\".\"_kinddt1rref\" = (SELECT",
    );
    assert_contains(
        &listed,
        "CASE WHEN \"__aggregate_base\".\"_kindct1rref\" = (SELECT",
    );
}

#[test]
fn records_with_ext_dimensions_expose_both_sides() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Регистратор КАК Р, Д.СчетДт КАК Дт, Д.СубконтоКт1 КАК СК1, Д.ВидСубконтоКт1 КАК ВК1,
                 Д.Сумма КАК С, Д.Содержание КАК Т, Д.Активность КАК А
         ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Организация = &Орг И СубконтоКт1 = &Контрагент) КАК Д;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(&sql, "\"Д\".\"_valuect1_type\" AS \"СК1_TYPE\"");
    assert_contains(&sql, "\"Д\".\"_kindct1rref\" AS \"ВК1\"");
    assert_contains(&sql, "(\"__aggregate_base\".\"_period\" >= ");
    assert!(!sql.contains("GROUP BY"), "{sql}");
    let ordered = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, , Период, 10) КАК Д;",
    )
    .unwrap();
    assert_contains(
        &ordered,
        "ORDER BY \"__aggregate_base\".\"_period\" LIMIT 10) AS \"Д\"",
    );
}

#[test]
fn records_conditions_read_side_less_names_on_either_side() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Организация = &О И Счет = &С И Субконто1 = &К1) КАК Д;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(
        &sql,
        "(((\"__aggregate_base\".\"_fld38457rref\" = NULL) AND (\"__aggregate_base\".\"_accountdtrref\" = NULL)) AND (\"__aggregate_base\".\"_valuedt1_type\" = NULL)) OR (((\"__aggregate_base\".\"_fld38457rref\" = NULL) AND (\"__aggregate_base\".\"_accountctrref\" = NULL)) AND (\"__aggregate_base\".\"_valuect1_type\" = NULL))",
    );
    // A dereference through the account joins the chart once per side.
    let dereferenced = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Счет.Код = \"51\" И Организация.Наименование = \"А\") КАК Д;",
    )
    .unwrap();
    println!("SQL {dereferenced}");
    assert_contains(&dereferenced, "(\"__ref1\".\"_code\" = '51')");
    assert_contains(&dereferenced, "(\"__ref3\".\"_code\" = '51')");
    assert_contains(
        &dereferenced,
        "\"__aggregate_base\".\"_accountctrref\" = \"__ref3\".\"_idrref\"",
    );
    assert_eq!(
        dereferenced.matches("\"_description\" = 'А'").count(),
        2,
        "{dereferenced}"
    );
    // A condition without a side-less name is compiled once.
    let one_sided = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Регистратор = &Т И Организация.Наименование = \"А\") КАК Д;",
    )
    .unwrap();
    assert!(!one_sided.contains(" OR "), "{one_sided}");
    assert!(!one_sided.contains("__ref2"), "{one_sided}");
}

#[test]
fn grouped_statements_accept_expressions_of_grouped_fields() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт, -Д.Сумма КАК М, ЕСТЬNULL(Д.Содержание, \"\") КАК П
         ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         СГРУППИРОВАТЬ ПО Д.СчетДт, Д.Сумма, ЕСТЬNULL(Д.Содержание, \"\");",
    )
    .unwrap();
    assert_contains(&sql, "(-\"Д\".\"_fld38460\") AS \"М\"");
    assert_contains(
        &sql,
        "GROUP BY \"Д\".\"_accountdtrref\", \"Д\".\"_fld38460\", ",
    );
    let ungrouped = compile(
        &snapshot,
        "ВЫБРАТЬ Д.СчетДт КАК Дт, -Д.КоличествоДт КАК М ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д СГРУППИРОВАТЬ ПО Д.СчетДт, Д.Сумма;",
    )
    .unwrap_err();
    assert!(
        ungrouped.message().contains("must be grouped"),
        "{}",
        ungrouped.message()
    );
}

#[test]
fn long_aliases_of_nested_sources_resolve_by_their_text() {
    let Some(snapshot) = buh() else {
        return;
    };
    // 37 Cyrillic letters: 74 bytes, over the 63 of PostgreSQL.
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ ВЗ.БольничныйЗаСчетРаботодателяСпецРежим ПОМЕСТИТЬ ВТ
         ИЗ (ВЫБРАТЬ Д.Сумма КАК БольничныйЗаСчетРаботодателяСпецРежим
             ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д) КАК ВЗ;
         ВЫБРАТЬ ВТ.БольничныйЗаСчетРаботодателяСпецРежим КАК У ИЗ ВТ КАК ВТ;",
    )
    .unwrap();
    assert_contains(
        &sql,
        "\"ВЗ\".\"БольничныйЗаСчетРаботодателяСпе\" AS \"БольничныйЗаСчетРаботодателяСпе\"",
    );
    assert_contains(&sql, "\"ВТ\".\"БольничныйЗаСчетРаботодателяСпе\" AS \"У\"");
}

#[test]
fn turnovers_expose_the_correspondence_of_each_side() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК С, О.КорСчет КАК КС, О.КорСубконто1 КАК КС1, О.ПодразделениеКор КАК ПК,
                 О.Субконто1 КАК С1, О.СуммаОборотДт КАК Дт
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , Счет В (&Счета), , Организация = &Орг, НЕ КорСчет В (&КорСчета), ) КАК О;",
    )
    .unwrap();
    println!("SQL {sql}");
    // The debit branch reads the credit columns as the correspondence and
    // the credit branch the debit ones.
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_accountctrref\" AS \"__cor_account\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_accountdtrref\" AS \"__cor_account\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuect1_rrref\" AS \"__cor_value1_rrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_valuedt1_rrref\" AS \"__cor_value1_rrref\"",
    );
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_fld38459ctrref\" AS \"__cor_fld38459rref\"",
    );
    assert_contains(
        &sql,
        "NOT (\"__aggregate_base\".\"_accountctrref\" IN (NULL))",
    );
    assert_contains(
        &sql,
        "NOT (\"__aggregate_base\".\"_accountdtrref\" IN (NULL))",
    );
    assert_contains(&sql, "\"__aggregate_used\".\"__cor_account\", ");
    // A listed balanced kind is picked on the opposite side of each branch.
    let listed = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК С, О.КорСубконто1 КАК КС1, О.СуммаОборот КАК Об
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , , ЗНАЧЕНИЕ(ПланВидовХарактеристик.ВидыСубконтоХозрасчетные.Контрагенты)) КАК О;",
    )
    .unwrap();
    println!("SQL {listed}");
    assert_contains(
        &listed,
        "CASE WHEN \"__aggregate_base\".\"_kindct1rref\" = (SELECT",
    );
    assert_contains(
        &listed,
        "CASE WHEN \"__aggregate_base\".\"_kinddt1rref\" = (SELECT",
    );
    assert_contains(&listed, "AS \"__cor_value1_type\"");
    // Unread, the correspondence is summed away.
    let unread = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК С, О.СуммаОборот КАК Об ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , КорСчет = &КС, ) КАК О;",
    )
    .unwrap();
    assert!(
        !unread.contains("\"__aggregate_used\".\"__cor_account\""),
        "{unread}"
    );
    assert_contains(&unread, "(\"__aggregate_base\".\"_accountctrref\" = NULL)");
}

#[test]
fn turnovers_without_extra_dimensions_take_the_balanced_account_condition() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК С, О.КорСчет КАК КС, О.СуммаОборотКт КАК Кт ИЗ РегистрБухгалтерии.Управленческий.Обороты(&Н, &К, МЕСЯЦ, , , КорСчет = &КС) КАК О;",
    )
    .unwrap();
    assert_contains(
        &sql,
        "\"__aggregate_base\".\"_accountctrref\" AS \"__cor_account\"",
    );
    assert_contains(&sql, "\"__aggregate_used\".\"__cor_account\"");
}

#[test]
fn shared_names_of_a_nested_source_fall_back_to_labels() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ ВЗ.Организация КАК А, ВЗ.Организация_2 КАК Б
         ИЗ (ВЫБРАТЬ Д.Организация, Д.ПодразделениеДт КАК Организация
             ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д) КАК ВЗ;",
    )
    .unwrap();
    assert_contains(
        &sql,
        "\"ВЗ\".\"Организация\" AS \"А\", \"ВЗ\".\"Организация_2\" AS \"Б\"",
    );
}

#[test]
fn union_spreads_undefined_over_a_composite_extra_dimension() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.Субконто2 КАК Актив, О.СуммаОборот КАК С
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , , ) КАК О
         ОБЪЕДИНИТЬ ВСЕ
         ВЫБРАТЬ О.Счет, НЕОПРЕДЕЛЕНО, -О.СуммаОборот
         ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , , ) КАК О;",
    )
    .unwrap();
    // The second branch is spread over the type and reference members.
    assert_contains(
        &sql,
        "AS \"Актив_TYPE\", (\"О\".\"_value2_rtref\" || \"О\".\"_value2_rrref\") AS \"Актив\"",
    );
    assert_contains(&sql, "END AS \"column2_TYPE\", NULL AS \"column2\"");
}

#[test]
fn ordering_by_a_compound_field_spreads_over_its_columns() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Период КАК П, Д.Регистратор КАК Р, Д.СубконтоДт1 КАК С
         ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         УПОРЯДОЧИТЬ ПО Д.Период, Д.Регистратор, Д.СубконтоДт1 УБЫВ;",
    )
    .unwrap();
    assert_contains(
        &sql,
        "ORDER BY \"Д\".\"_period\" ASC, \"Д\".\"_recordertref\" ASC, \"Д\".\"_recorderrref\" ASC, \"Д\".\"_valuedt1_type\" DESC, \"Д\".\"_valuedt1_rtref\" DESC, \"Д\".\"_valuedt1_rrref\" DESC",
    );
    // By alias as well.
    let aliased = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Регистратор КАК Р ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д УПОРЯДОЧИТЬ ПО Р;",
    )
    .unwrap();
    assert_contains(
        &aliased,
        "ORDER BY \"Д\".\"_recordertref\" ASC, \"Д\".\"_recorderrref\" ASC",
    );
}

#[test]
fn tuple_membership_accepts_references_of_several_types() {
    let Some(snapshot) = buh() else {
        return;
    };
    // Both sides carry the RTRef ‖ RRRef payload of the recorder.
    let recorders = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         ГДЕ (Д.Регистратор, Д.НомерСтроки) В (ВЫБРАТЬ П.Регистратор, П.НомерСтроки ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Организация = &О) КАК П);",
    )
    .unwrap();
    println!("SQL {recorders}");
    assert_contains(
        &recorders,
        "\"__in\".\"Регистратор\" = (\"Д\".\"_recordertref\" || \"Д\".\"_recorderrref\")",
    );
    assert_contains(&recorders, "\"__in\".\"НомерСтроки\" = \"Д\".\"_lineno\"");
    // A composite extra dimension answers with its type and its payload.
    let composite = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         ГДЕ (Д.СубконтоДт1, Д.СубконтоДт2) В (ВЫБРАТЬ П.СубконтоДт1, П.СубконтоДт2 ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Организация = &О) КАК П);",
    )
    .unwrap();
    println!("SQL {composite}");
    assert_contains(
        &composite,
        "\"__in\".\"СубконтоДт1_TYPE\" = \"Д\".\"_valuedt1_type\"",
    );
    assert_contains(
        &composite,
        "\"__in\".\"СубконтоДт1\" = (\"Д\".\"_valuedt1_rtref\" || \"Д\".\"_valuedt1_rrref\")",
    );
    // A fixed reference is widened to the payload of the other side.
    let widened = compile(
        &snapshot,
        "ВЫБРАТЬ Т.Номер КАК Н ИЗ Документ.БольничныйЛист КАК Т
         ГДЕ (Т.Ссылка, Т.Организация) В (ВЫБРАТЬ Д.Регистратор, Д.Организация ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д);",
    )
    .unwrap();
    println!("SQL {widened}");
    assert_contains(&widened, "\"__in\".\"Регистратор\" = (");
    assert_contains(&widened, "|| \"Т\".\"_idrref\")");
}

#[test]
fn inner_and_left_joins_take_any_condition() {
    let Some(snapshot) = buh() else {
        return;
    };
    // `ПО (ИСТИНА)`, a comparison with a parameter, an inequality: no
    // equality binds the joined source, and the servers take them all.
    for (source, needle) in [
        (
            "ВЫБРАТЬ Д.Сумма КАК С, Х.Код КАК К ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
             ЛЕВОЕ СОЕДИНЕНИЕ ПланСчетов.Хозрасчетный КАК Х ПО (ИСТИНА);",
            "LEFT JOIN \"_acc69\" AS \"Х\" ON TRUE AND",
        ),
        (
            "ВЫБРАТЬ Д.Сумма КАК С, Х.Код КАК К ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
             ЛЕВОЕ СОЕДИНЕНИЕ ПланСчетов.Хозрасчетный КАК Х ПО (Х.Ссылка = &Счет);",
            "LEFT JOIN \"_acc69\" AS \"Х\" ON (\"Х\".\"_idrref\" = NULL) AND",
        ),
        (
            "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
             ВНУТРЕННЕЕ СОЕДИНЕНИЕ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Е ПО Д.Период < Е.Период И Д.Организация = Е.Организация;",
            "ON (\"Д\".\"_period\" < \"Е\".\"_period\") AND \"Д\".\"_fld38457rref\" = \"Е\".\"_fld38457rref\"",
        ),
    ] {
        let sql = compile(&snapshot, source).unwrap();
        assert_contains(&sql, needle);
    }
    // A FULL JOIN keeps needing the anchor equality.
    let full = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         ПОЛНОЕ СОЕДИНЕНИЕ ПланСчетов.Хозрасчетный КАК Х ПО (ИСТИНА);",
    )
    .unwrap_err();
    assert!(
        full.message().contains("FULL JOIN condition"),
        "{}",
        full.message()
    );
}

#[test]
fn a_join_condition_dereferences_through_a_cast() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С, Т.ДокументОснование КАК О
         ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Д
         ВНУТРЕННЕЕ СОЕДИНЕНИЕ Документ.СчетФактураВыданный.ДокументыОснования КАК Т
         ПО ВЫРАЗИТЬ(Д.Регистратор КАК Документ.РеализацияОтгруженныхТоваров).Организация = Т.Ссылка.Организация;",
    )
    .unwrap();
    println!("SQL {sql}");
    // The cast's target is joined to the record before the section's join,
    // whose condition reads the joined column.
    assert_contains(
        &sql,
        "AS \"Д\" LEFT JOIN \"_document1008\" AS \"__left_ref1\" ON",
    );
    assert_contains(
        &sql,
        "INNER JOIN (\"_document1063_vt33765\" AS \"Т\" LEFT JOIN \"_document1063\" AS \"__right_ref1\" ON",
    );
    assert_contains(
        &sql,
        "ON (\"__left_ref1\".\"_fld30486rref\" = \"__right_ref1\".\"_fld33699rref\")",
    );
}

#[test]
fn records_take_the_first_n_in_the_given_or_record_order() {
    let Some(snapshot) = buh() else {
        return;
    };
    let first = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, , Период, 1) КАК Д;",
    )
    .unwrap();
    assert_contains(
        &first,
        " ORDER BY \"__aggregate_base\".\"_period\" LIMIT 1) AS \"Д\"",
    );
    let default_order = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, Организация = &О, , 5) КАК Д;",
    )
    .unwrap();
    assert_contains(
        &default_order,
        " ORDER BY \"__aggregate_base\".\"_period\", \"__aggregate_base\".\"_recordertref\", \"__aggregate_base\".\"_recorderrref\", \"__aggregate_base\".\"_lineno\" LIMIT 5) AS \"Д\"",
    );
    // A parameter bound to NULL names no order; a bare order has no effect.
    let parameter_order = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, , &Порядок, 2) КАК Д;",
    )
    .unwrap();
    assert_contains(&parameter_order, "\"_lineno\" LIMIT 2) AS \"Д\"");
    let bare_order = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, , Период, ) КАК Д;",
    )
    .unwrap();
    assert!(!bare_order.contains("ORDER BY"), "{bare_order}");
    let text_top = compile(
        &snapshot,
        "ВЫБРАТЬ Д.Сумма КАК С ИЗ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, , , \"1\") КАК Д;",
    )
    .unwrap_err();
    assert!(
        text_top.message().contains("Первые"),
        "{}",
        text_top.message()
    );
}

#[test]
fn a_derived_fixed_reference_joins_a_runtime_typed_field() {
    let Some(snapshot) = buh() else {
        return;
    };
    // The derived column is a reference to one document; the recorder
    // carries its type, so the equality compares the type and the id.
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ Р.Сумма КАК С
         ИЗ (ВЫБРАТЬ Д.Ссылка КАК Ссылка ИЗ Документ.БольничныйЛист КАК Д) КАК Т
         ВНУТРЕННЕЕ СОЕДИНЕНИЕ РегистрБухгалтерии.Хозрасчетный.ДвиженияССубконто(&Н, &К, ) КАК Р
         ПО Р.Регистратор = Т.Ссылка;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(&sql, "\"_recordertref\" = decode('");
    assert_contains(&sql, "\"_recorderrref\" = \"Т\".\"Ссылка\"");
}

#[test]
fn the_extra_dimension_kinds_of_the_chart_answer_to_their_names() {
    let Some(snapshot) = buh() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ ВС.Ссылка КАК Счет, ВС.ВидСубконто КАК Вид, ВС.НомерСтроки КАК Н, ВС.ТолькоОбороты КАК О
         ИЗ ПланСчетов.Хозрасчетный.ВидыСубконто КАК ВС;",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(&sql, "\"_dimkindrref\" AS \"Вид\"");
    assert_contains(&sql, "\"_turnoveronly\" AS \"О\"");
}

#[test]
fn expanded_balances_are_parts_of_the_finest_grain_sums() {
    let Some(snapshot) = buh() else {
        return;
    };
    let balances = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаОстаток КАК Ост, О.СуммаРазвернутыйОстатокДт КАК РДт, О.СуммаРазвернутыйОстатокКт КАК РКт
         ИЗ РегистрБухгалтерии.Хозрасчетный.Остатки(&Д, , , ) КАК О;",
    )
    .unwrap();
    println!("SQL {balances}");
    assert_contains(
        &balances,
        "CASE WHEN SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld38460\" ELSE -\"__sides\".\"_fld38460\" END) > 0 THEN SUM(CASE WHEN \"__sides\".\"__side\" = 0 THEN \"__sides\".\"_fld38460\" ELSE -\"__sides\".\"_fld38460\" END) ELSE 0 END AS \"_fld38460ExpandedDt\"",
    );
    assert_contains(
        &balances,
        "SUM(\"__aggregate_used\".\"_fld38460ExpandedDt\") AS \"_fld38460ExpandedDt\"",
    );
    let whole = compile(
        &snapshot,
        "ВЫБРАТЬ О.Счет КАК Счет, О.СуммаНачальныйРазвернутыйОстатокДт КАК НДт, О.СуммаКонечныйРазвернутыйОстатокКт КАК ККт
         ИЗ РегистрБухгалтерии.Хозрасчетный.ОстаткиИОбороты(&Н, &К, , , , , ) КАК О;",
    )
    .unwrap();
    assert_contains(&whole, "AS \"_fld38460OpeningBalanceExpandedDt\"");
    assert_contains(&whole, "AS \"_fld38460ClosingBalanceExpandedCt\"");
    let periodic = compile(
        &snapshot,
        "ВЫБРАТЬ О.Период КАК П, О.СуммаНачальныйРазвернутыйОстатокДт КАК НДт
         ИЗ РегистрБухгалтерии.Хозрасчетный.ОстаткиИОбороты(&Н, &К, МЕСЯЦ, , , , ) КАК О;",
    )
    .unwrap_err();
    assert!(
        periodic.message().contains("expanded balance"),
        "{}",
        periodic.message()
    );
}

#[test]
fn a_value_dereferenced_across_targets_compares_by_its_reference_member() {
    let Some(snapshot) = unf() else {
        return;
    };
    let sql = compile(
        &snapshot,
        "ВЫБРАТЬ П.Сумма КАК С ИЗ РегистрНакопления.Продажи КАК П
         ГДЕ П.Регистратор.Организация = ЗНАЧЕНИЕ(Справочник.Организации.ПустаяСсылка);",
    )
    .unwrap();
    println!("SQL {sql}");
    assert_contains(&sql, "AND (CASE WHEN \"П\".\"_recordertref\" = ");
    assert_contains(
        &sql,
        " END = (decode('00000052', 'hex') || decode('00000000000000000000000000000000', 'hex')))",
    );
    let unbound = compile(
        &snapshot,
        "ВЫБРАТЬ П.Сумма КАК С ИЗ РегистрНакопления.Продажи КАК П ГДЕ П.Регистратор.Организация = &Организация;",
    )
    .unwrap();
    assert_contains(&unbound, " END = NULL)");
}
