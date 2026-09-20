//! Restriction templates as the manual describes them (5.5.4.8.7): what
//! may follow `#` in a template body, how a call substitutes its
//! arguments, and how the expanded text compiles into a query.

mod support;

use support::*;

use open_sdbl::access::{RestrictionScope, expand_restriction};
use open_sdbl::metadata::{MetadataSnapshot, ObjectId, RestrictionTemplate, Right};
use open_sdbl::query::{
    AccessRestriction, CompileOptions, PostgresBackend, QueryCompiler, QueryParameter,
    SessionParameters, find_metadata_object,
};

const TABLE: &str = "Catalog.OpenSdblMetadataProbe";

fn template(name: &str, parameters: &[&str], body: &str) -> RestrictionTemplate {
    RestrictionTemplate::new(name, parameters.iter().copied(), body)
}

/// The templates of the manual's examples.
fn templates() -> Vec<RestrictionTemplate> {
    vec![
        template("Шаблон", &[], "Итого = #Параметр(1)"),
        template("Шаблон1", &["ВидДокумента"], "ВидДокумента = #ВидДокумента"),
        template(
            "Шаблон2",
            &[],
            "ВидДокумента = #Параметр(1) ## #Параметр(2)",
        ),
        template("Шаблон3", &[], "ВидДокумента = #Параметр(3)"),
    ]
}

fn session(values: &[(&str, open_sdbl::query::ParameterValue)]) -> SessionParameters {
    let mut session = SessionParameters::new();
    for (name, value) in values {
        session.set(QueryParameter::new(*name, value.clone()));
    }
    session
}

/// Expands a text for the probe catalog and the reading right.
fn expand(text: &str) -> String {
    let session = session(&[]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    expand_restriction(text, &templates(), &scope)
        .unwrap_or_else(|error| panic!("{text}: {error}"))
        .condition
}

#[test]
fn a_call_substitutes_its_arguments_by_number() {
    // The manual: body `Итого = #Параметр(1)`, use `#Шаблон("10")`,
    // result `Итого = 10` — the quotes delimit the argument and are
    // dropped.
    assert_eq!(expand("ГДЕ #Шаблон(\"10\")"), "Итого = 10");

    // Arguments left empty are still counted: the third one is taken.
    assert_eq!(
        expand("ГДЕ #Шаблон3(\"\", \"\", \"\"\"Накладная\"\"\")"),
        "ВидДокумента = \"Накладная\""
    );
}

#[test]
fn a_call_substitutes_the_named_parameters_of_its_signature() {
    // The manual: `Шаблон1(ВидДокумента)` reads its parameter by name.
    // Two double quotes inside an argument stand for one.
    assert_eq!(
        expand("ГДЕ #Шаблон1(\"\"\"Накладная\"\"\")"),
        "ВидДокумента = \"Накладная\""
    );
    // An argument without quotes of its own arrives bare.
    assert_eq!(
        expand("ГДЕ #Шаблон1(\"Накладная\")"),
        "ВидДокумента = Накладная"
    );
}

#[test]
fn two_number_signs_stand_for_one() {
    // The manual: `#Параметр(1) ## #Параметр(2)` writes one `#` between
    // the arguments.
    assert_eq!(
        expand("ГДЕ #Шаблон2(\"\"\"Накладная\", \"1\"\"\")"),
        "ВидДокумента = \"Накладная # 1\""
    );
    // The escape is text wherever a directive is looked for.
    assert_eq!(expand("ГДЕ Поле = \"##Если\""), "Поле = \"#Если\"");
    assert_eq!(
        expand("ГДЕ Поле = \"##Параметр(1)\""),
        "Поле = \"#Параметр(1)\""
    );
}

#[test]
fn the_current_names_stand_for_the_table_and_the_right() {
    // `#ТекущаяТаблица` inserts the name of the table, and
    // `#ИмяТекущейТаблицы` the name as a string value, in quotes.
    assert_eq!(
        expand("ГДЕ #ТекущаяТаблица.Code = #ИмяТекущейТаблицы"),
        format!("{TABLE}.Code = \"{TABLE}\"")
    );
    // The right the restriction is built for.
    assert_eq!(
        expand("ГДЕ Право = #ИмяТекущегоПраваДоступа"),
        "Право = Чтение"
    );
}

#[test]
fn a_directive_reads_the_functions_of_the_manual() {
    let session = session(&[(
        "Списки",
        open_sdbl::query::ParameterValue::String(format!("{TABLE}:Code;")),
    )]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    // СтрСодержит over a concatenation of the current name and a literal.
    let text = "#Если СтрСодержит(&Списки, #ИмяТекущейТаблицы + \":Code;\") #Тогда ГДЕ ИСТИНА #Иначе ГДЕ ЛОЖЬ #КонецЕсли";
    let expanded = expand_restriction(text, &templates(), &scope).unwrap();
    assert_eq!(expanded.condition, "ИСТИНА");

    // The same text with a name the parameter does not carry.
    let other = RestrictionScope {
        table_name: "Catalog.Другой",
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction(text, &templates(), &other).unwrap();
    assert_eq!(expanded.condition, "ЛОЖЬ");
}

#[test]
fn a_nested_call_expands_within_the_body() {
    let templates = vec![
        template("Внешний", &[], "#Внутренний(\"#Параметр(1)\")"),
        template("Внутренний", &[], "Code = #Параметр(1)"),
    ];
    let session = session(&[]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    // The outer body passes its argument on, and the inner one reads it.
    let expanded = expand_restriction("ГДЕ #Внешний(\"HQ\")", &templates, &scope).unwrap();
    assert_eq!(expanded.condition, "Code = HQ");
}

/// Compiles a `РАЗРЕШЕННЫЕ` statement with one restriction of the probe
/// catalog and answers the SQL.
fn compile_with(snapshot: &MetadataSnapshot, condition: &str) -> String {
    let object = ObjectId::from(&find_metadata_object(snapshot, TABLE).unwrap().guid);
    let restrictions = [AccessRestriction::new(object, condition)];
    let options = CompileOptions::new().restrictions(&restrictions);
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile_with(&format!("ВЫБРАТЬ РАЗРЕШЕННЫЕ Code ИЗ {TABLE}"), &options)
        .unwrap_or_else(|error| panic!("{condition}: {error}"))
        .sql
}

#[test]
fn an_expanded_template_compiles_into_the_query() {
    let snapshot = snapshot();
    // A template whose body is the whole restriction, in the simple form.
    let templates = vec![template(
        "ПоКоду",
        &[],
        "ТекущаяТаблица ГДЕ Code = #Параметр(1)",
    )];
    let session = session(&[]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction("#ПоКоду(\"\"\"HQ\"\"\")", &templates, &scope).unwrap();
    assert_eq!(expanded.condition, "Code = \"HQ\"");

    let sql = compile_with(&snapshot, &expanded.text());
    assert!(
        sql.contains(
            "FROM \"_reference53\" AS \"__restricted\" WHERE (\"__restricted\".\"_code\" = 'HQ')"
        ),
        "{sql}"
    );
}

#[test]
fn a_template_naming_the_table_compiles_into_the_query() {
    let snapshot = snapshot();
    // The full form of the manual: the table describes itself after ИЗ,
    // and the alias it declares is the one the condition reads.
    let templates = vec![template(
        "ПоТаблице",
        &[],
        "ТекущаяТаблица ИЗ #ТекущаяТаблица КАК ТекущаяТаблица ГДЕ ТекущаяТаблица.Code = #Параметр(1)",
    )];
    let session = session(&[]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction("#ПоТаблице(\"\"\"HQ\"\"\")", &templates, &scope).unwrap();
    assert_eq!(expanded.alias.as_deref(), Some("ТекущаяТаблица"));

    let sql = compile_with(&snapshot, &expanded.text());
    assert!(
        sql.contains("WHERE (\"__restricted\".\"_code\" = 'HQ')"),
        "{sql}"
    );
}

#[test]
fn a_template_may_be_called_without_the_parenthesis() {
    // A role of «1С:Документооборот» restricts a catalog with a bare
    // `#ЧтениеШаблоновПроцессов`: a template taking no argument, called
    // without the parenthesis.
    let templates = vec![
        template("БезАргументов", &[], "ТекущаяТаблица ГДЕ Code = \"HQ\""),
        template("САргументом", &["Поле"], "#Поле = #Параметр(1)"),
    ];
    let session = session(&[]);
    let scope = RestrictionScope {
        table_name: TABLE,
        right: &Right::Read,
        session: &session,
    };
    let expanded = expand_restriction("#БезАргументов", &templates, &scope).unwrap();
    assert_eq!(expanded.condition, "Code = \"HQ\"");
    assert!(!expanded.condition.contains('#'));

    // The text around the call is kept.
    let expanded = expand_restriction("ГДЕ (#БезАргументов) И ИСТИНА", &templates, &scope).unwrap();
    assert_eq!(
        expanded.condition,
        "(ТекущаяТаблица ГДЕ Code = \"HQ\") И ИСТИНА"
    );

    // A template that reads arguments and is called without them reads
    // them as empty.
    let expanded = expand_restriction("ГДЕ #САргументом", &templates, &scope).unwrap();
    assert_eq!(expanded.condition, "=");

    // A name no template carries is left alone, as before.
    let expanded = expand_restriction("ГДЕ Поле = #Неизвестное", &templates, &scope).unwrap();
    assert_eq!(expanded.condition, "Поле = #Неизвестное");
}
