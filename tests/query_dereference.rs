//! Reference paths of more than one hop.

mod support;

use support::*;

use open_sdbl::metadata::MetadataSnapshot;
use open_sdbl::query::{
    CompiledQuery, MsSqlBackend, PostgresBackend, QueryCompiler, QueryDiagnosticKind,
};

fn postgres(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
    QueryCompiler::new(snapshot, PostgresBackend)
        .compile(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn assert_contains(sql: &str, needle: &str) {
    assert!(sql.contains(needle), "expected {needle:?} in\n{sql}");
}

const CATALOG: &str = "Справочник.OpenSdblMetadataProbe";

#[test]
fn walks_a_reference_chain() {
    let snapshot = chained_reference_snapshot();
    let two = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Т.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"),
    );
    // Each hop joins the target of the previous one.
    assert_contains(
        &two.sql,
        "LEFT JOIN \"_reference57\" AS \"__ref1\" ON \"Т\".\"_fld54\" = \"__ref1\".\"_idrref\"",
    );
    assert_contains(
        &two.sql,
        "LEFT JOIN \"_reference57\" AS \"__ref2\" ON \"__ref1\".\"_fld54\" = \"__ref2\".\"_idrref\"",
    );
    assert_contains(&two.sql, "\"__ref2\".\"_code\"::text AS \"К\"");

    let three = postgres(
        &snapshot,
        &format!("ВЫБРАТЬ Т.Организация.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"),
    );
    assert_contains(
        &three.sql,
        "AS \"__ref3\" ON \"__ref2\".\"_fld54\" = \"__ref3\".\"_idrref\"",
    );
}

#[test]
fn shares_the_joins_of_a_repeated_prefix() {
    let snapshot = chained_reference_snapshot();
    let compiled = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К, Т.Организация.Организация.Дата КАК Д,
                    Т.Организация.Код КАК К2 ИЗ {CATALOG} КАК Т;"
        ),
    );
    assert_eq!(
        compiled.sql.matches("LEFT JOIN").count(),
        2,
        "{}",
        compiled.sql
    );
}

#[test]
fn walks_the_chain_in_every_clause() {
    let snapshot = chained_reference_snapshot();
    let filtered = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Код КАК К ИЗ {CATALOG} КАК Т
             ГДЕ Т.Организация.Организация.Код = \"A\"
             УПОРЯДОЧИТЬ ПО Т.Организация.Организация.Дата;"
        ),
    );
    assert_contains(&filtered.sql, "WHERE (\"__ref2\".\"_code\" = 'A')");
    assert_contains(&filtered.sql, "ORDER BY \"__ref2\".\"_date_time\" ASC");

    let grouped = postgres(
        &snapshot,
        &format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К, КОЛИЧЕСТВО(*) КАК Ч ИЗ {CATALOG} КАК Т
             СГРУППИРОВАТЬ ПО Т.Организация.Организация.Код;"
        ),
    );
    assert_contains(&grouped.sql, "GROUP BY \"__ref2\".\"_code\"");

    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile(&format!(
            "ВЫБРАТЬ Т.Организация.Организация.Код КАК К ИЗ {CATALOG} КАК Т;"
        ))
        .unwrap();
    assert_contains(
        &mssql.sql,
        "LEFT JOIN [_reference57] AS [__ref2] ON [__ref1].[_fld54] = [__ref2].[_idrref]",
    );
}

#[test]
fn walks_the_chain_through_a_composite_reference() {
    // Measured on 8.3.27: the platform joins every target of the composite
    // hop under a guard on the stored type, walks the rest of the path
    // inside that target with plain joins, and selects the branches with a
    // `CASE` over the same type.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ Д.ДоговорКонтрагента.Ссылка.Ссылка КАК К
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д;",
    );
    // The first hop guards each target by type…
    assert_contains(
        &compiled.sql,
        "LEFT JOIN \"_document53\" AS \"__ref1\" ON \"Д\".\"_fld59_rrref\" = \"__ref1\".\"_idrref\" \
         AND \"Д\".\"_fld59_rtref\" = decode('00000035', 'hex')",
    );
    // …and the next hop joins from that target without a guard.
    assert_contains(
        &compiled.sql,
        "LEFT JOIN \"_document53\" AS \"__ref2\" ON \"__ref1\".\"_idrref\" = \"__ref2\".\"_idrref\"",
    );
    assert_contains(
        &compiled.sql,
        "CASE WHEN \"Д\".\"_fld59_rtref\" = decode('00000035', 'hex') \
         THEN (decode('00000035', 'hex') || \"__ref2\".\"_idrref\")",
    );

    // A target in which the rest of the path does not resolve contributes
    // no branch; when no target does, the field is reported unknown.
    let missing = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ Д.ДоговорКонтрагента.Ссылка.НетТакого КАК К
             ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д;",
        )
        .unwrap_err();
    assert_eq!(missing.kind(), QueryDiagnosticKind::UnknownField);

    // A hop through a value that is not a reference keeps its diagnostic.
    let scalar = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ Д.ДоговорКонтрагента.Ссылка.Наименование.Код КАК К
             ИЗ Документ.бит_ДополнительныеУсловияПоДоговору КАК Д;",
        )
        .unwrap_err();
    assert!(
        matches!(
            scalar.kind(),
            QueryDiagnosticKind::UnsupportedFeature | QueryDiagnosticKind::UnknownField
        ),
        "{scalar}"
    );
}

#[test]
fn dereferences_standard_fields_through_a_composite_reference() {
    // SchemaStorage names no target for a reference of several tables, so
    // the candidates are scanned; standard fields are not attributes and
    // have to be recognized by name.
    let snapshot = universal_dereferenced_presentation_snapshot();
    let compiled = postgres(
        &snapshot,
        "ВЫБРАТЬ ДоговорКонтрагента.Ссылка КАК С
         ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
    );
    // Each candidate target is joined under its own type guard.
    assert_contains(&compiled.sql, "\"__src\".\"_fld59_rtref\"");
    assert!(compiled.sql.contains("LEFT JOIN"), "{}", compiled.sql);

    let unknown = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile(
            "ВЫБРАТЬ ДоговорКонтрагента.НетТакого КАК Н
             ИЗ Документ.бит_ДополнительныеУсловияПоДоговору;",
        )
        .unwrap_err();
    assert!(unknown.message().contains("was not found"), "{unknown}");
}

#[test]
fn resolves_the_computed_standard_fields() {
    fn compile_separated(snapshot: &MetadataSnapshot, source: &str) -> CompiledQuery {
        let mut session = open_sdbl::query::SessionParameters::new();
        session.set(open_sdbl::query::QueryParameter::new(
            "ЗначениеРазделителя",
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
        session.set(open_sdbl::query::QueryParameter::new(
            "ИспользованиеРазделителя",
            open_sdbl::query::ParameterValue::Boolean(false),
        ));
        session.set(open_sdbl::query::QueryParameter::new(
            "ОбластьДанныхОсновныеДанные",
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
        QueryCompiler::new(snapshot, PostgresBackend)
            .compile_with(
                source,
                &open_sdbl::query::CompileOptions::new().session(&session),
            )
            .unwrap_or_else(|error| panic!("{source}: {error}"))
    }

    // `ЭтоГруппа` is the negation of the stored `Folder` column, which is
    // true for an item, and `Предопределенный` says the item has a
    // predefined identity.
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let compiled = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.Наименование КАК Имя, Т.ЭтоГруппа КАК Гр, Т.Предопределенный КАК Пред
         ИЗ Справочник.ГруппыДоступа КАК Т ГДЕ Т.ЭтоГруппа;",
    );
    assert_contains(&compiled.sql, "(\"Т\".\"_folder\" = FALSE) AS \"Гр\"");
    assert_contains(
        &compiled.sql,
        "(\"Т\".\"_predefinedid\" <> decode('00000000000000000000000000000000', 'hex')) AS \"Пред\"",
    );
    assert_contains(&compiled.sql, "AND (\"Т\".\"_folder\" = FALSE)");
    assert_eq!(
        compiled.columns[1].kind,
        open_sdbl::query::ColumnKind::Boolean
    );

    // SQL Server has no boolean type, so the value is spelled as a bit.
    let mut session = open_sdbl::query::SessionParameters::new();
    session.set(open_sdbl::query::QueryParameter::new(
        "ЗначениеРазделителя",
        open_sdbl::query::ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    ));
    session.set(open_sdbl::query::QueryParameter::new(
        "ИспользованиеРазделителя",
        open_sdbl::query::ParameterValue::Boolean(false),
    ));
    session.set(open_sdbl::query::QueryParameter::new(
        "ОбластьДанныхОсновныеДанные",
        open_sdbl::query::ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    ));
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile_with(
            "ВЫБРАТЬ Т.ЭтоГруппа КАК Гр ИЗ Справочник.ГруппыДоступа КАК Т;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .unwrap();
    assert_contains(
        &mssql.sql,
        "CASE WHEN [Т].[_folder] = 0x00 THEN 0x01 ELSE 0x00 END AS [Гр]",
    );

    // They also answer through a reference.
    let dereferenced = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.Родитель.ЭтоГруппа КАК Гр ИЗ Справочник.ГруппыДоступа КАК Т;",
    );
    assert_contains(
        &dereferenced.sql,
        "(\"__ref1\".\"_folder\" = FALSE) AS \"Гр\"",
    );

    // `ИмяПредопределенныхДанных` names the predefined item the row is,
    // read from the predefined values of the owning catalog. A row that is
    // not predefined answers an empty string, and a row an outer join
    // missed keeps NULL, both measured on the platform.
    let named = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.ИмяПредопределенныхДанных КАК П ИЗ Справочник.ГруппыДоступа КАК Т;",
    );
    assert_contains(
        &named.sql,
        "CASE WHEN \"Т\".\"_predefinedid\" IS NULL THEN NULL::text \
         WHEN \"Т\".\"_predefinedid\" = decode('b5786f0d29e6182246ca59783c39be2b', 'hex') \
         THEN 'Администраторы' ELSE '' END AS \"П\"",
    );
    assert_eq!(
        named.columns[0].kind,
        open_sdbl::query::ColumnKind::String { length: None }
    );

    // The English spelling answers the same, and the name reads through a
    // reference as well.
    let english = compile_separated(
        &snapshot,
        "ВЫБРАТЬ Т.Родитель.PredefinedDataName КАК П ИЗ Справочник.ГруппыДоступа КАК Т
         ГДЕ Т.ИмяПредопределенныхДанных = \"Администраторы\";",
    );
    assert_contains(
        &english.sql,
        "CASE WHEN \"__ref1\".\"_predefinedid\" IS NULL",
    );
    assert_contains(
        &english.sql,
        "THEN 'Администраторы' ELSE '' END = 'Администраторы')",
    );

    // SQL Server spells the literals as national strings.
    let mssql = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile_with(
            "ВЫБРАТЬ Т.ИмяПредопределенныхДанных КАК П ИЗ Справочник.ГруппыДоступа КАК Т;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .unwrap();
    assert_contains(&mssql.sql, "THEN N'Администраторы' ELSE N'' END AS [П]");

    // A document stores no predefined identity, so the name is unknown
    // there, as it is on the platform.
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Д.ИмяПредопределенныхДанных ИЗ Документ.ВходящееПисьмо КАК Д;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .unwrap_err();
    assert!(
        format!("{error}").contains("ИмяПредопределенныхДанных"),
        "{error}"
    );
}

#[test]
fn reads_a_document_journal() {
    // A journal is a table of its own: `Ссылка` is the reference of the
    // registered document, `Тип` its type. Measured on the probe base,
    // where a journal of one document kind stores a bare reference and the
    // demo journals store the `RTRef ‖ RRRef` pair.
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let mut session = open_sdbl::query::SessionParameters::new();
    session.set(open_sdbl::query::QueryParameter::new(
        "ОбластьДанныхОсновныеДанные",
        open_sdbl::query::ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    ));
    let options = open_sdbl::query::CompileOptions::new().session(&session);
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Ж.Ссылка КАК С, Ж.Тип КАК Т, Ж.Дата КАК Д, Ж.Номер КАК Н,
                    Ж.ПометкаУдаления КАК П, Ж.Проведен КАК Пр
             ИЗ ЖурналДокументов.УчетРабочегоВремени КАК Ж;",
            &options,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert_contains(&compiled.sql, "FROM \"_documentjournal1240\" AS \"Ж\"");
    assert_contains(&compiled.sql, "\"Ж\".\"_documentrref\"");
    assert_contains(&compiled.sql, "\"Ж\".\"_documenttref\"");
    assert_contains(&compiled.sql, "\"Ж\".\"_date_time\" AS \"Д\"");
    assert_eq!(
        compiled.columns[1].kind,
        open_sdbl::query::ColumnKind::Type,
        "{}",
        compiled.sql
    );

    // The journal's own column answers under its metadata name, and the
    // reference joins like any other.
    let column = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Ж.Сотрудник КАК С, Ж.ДлительностьРабот КАК Д
             ИЗ ЖурналДокументов.УчетРабочегоВремени КАК Ж ГДЕ Ж.Проведен;",
            &options,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert_contains(&column.sql, "AS \"С\"");
    assert_contains(&column.sql, "AND \"Ж\".\"_posted\"");
}

#[test]
fn reads_a_filter_criterion() {
    // The platform answers every object whose listed field holds the
    // value: one selection per content field, united by UNION ALL, each
    // projecting the found object as a payload reference. Measured on the
    // probe base against the platform's own SQL.
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let mut session = open_sdbl::query::SessionParameters::new();
    session.set(open_sdbl::query::QueryParameter::new(
        "ОбластьДанныхОсновныеДанные",
        open_sdbl::query::ParameterValue::Number {
            unscaled: 0,
            scale: 0,
        },
    ));
    let parameters = [open_sdbl::query::QueryParameter::new(
        "Значение",
        open_sdbl::query::ParameterValue::Binary(vec![0x11; 16]),
    )];
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ К.Ссылка КАК С ИЗ КритерийОтбора.ДокументыПоВопросуДеятельности(&Значение) КАК К;",
            &open_sdbl::query::CompileOptions::new()
                .session(&session)
                .parameters(&parameters),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    // The criterion of the fixture reaches one live field, so the union
    // has a single branch; the shape is the same for several.
    assert_contains(&compiled.sql, "AS \"Ссылка\" FROM ");
    assert_contains(&compiled.sql, "decode('00000065', 'hex') || ");
    assert_contains(
        &compiled.sql,
        "= decode('11111111111111111111111111111111', 'hex')",
    );
    assert!(
        matches!(
            compiled.columns[0].kind,
            open_sdbl::query::ColumnKind::Reference {
                runtime_typed: true,
                ..
            }
        ),
        "{:?}",
        compiled.columns[0].kind
    );

    // An unknown criterion is named as such.
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ К.Ссылка ИЗ КритерийОтбора.НетТакого(&Значение) КАК К;",
            &open_sdbl::query::CompileOptions::new()
                .session(&session)
                .parameters(&parameters),
        )
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnknownObject, "{error}");
}

#[test]
fn reads_the_point_in_time_pair() {
    // `МоментВремени` is the pair that orders a row past the second its
    // date resolves: a document pairs its date with its reference, a
    // register record its period with its recorder. Measured on 8.3.27:
    // the platform spreads the pair over two columns in the projection,
    // over two terms in the ordering, the grouping and `РАЗЛИЧНЫЕ`, and
    // refuses the field where no such pair exists.
    fn session() -> open_sdbl::query::SessionParameters {
        let mut session = open_sdbl::query::SessionParameters::new();
        for name in ["ЗначениеРазделителя", "ОбластьДанныхОсновныеДанные"]
        {
            session.set(open_sdbl::query::QueryParameter::new(
                name,
                open_sdbl::query::ParameterValue::Number {
                    unscaled: 0,
                    scale: 0,
                },
            ));
        }
        session.set(open_sdbl::query::QueryParameter::new(
            "ИспользованиеРазделителя",
            open_sdbl::query::ParameterValue::Boolean(false),
        ));
        session
    }
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let compile = |source: &str| {
        QueryCompiler::new(&snapshot, PostgresBackend).compile_with(
            source,
            &open_sdbl::query::CompileOptions::new().session(&session()),
        )
    };

    // The date is labelled with the `_T` member suffix, the reference
    // keeps the alias itself, as a composite value does.
    let projected =
        compile("ВЫБРАТЬ Д.Номер КАК Н, Д.МоментВремени КАК М ИЗ Документ.ВходящееПисьмо КАК Д;")
            .expect("the point in time of a document must compile");
    assert_contains(
        &projected.sql,
        "\"Д\".\"_date_time\" AS \"М_T\", \"Д\".\"_idrref\" AS \"М\"",
    );
    assert_eq!(
        projected
            .columns
            .iter()
            .map(|column| column.label.as_str())
            .collect::<Vec<_>>(),
        ["Н", "М_T", "М"]
    );
    assert_eq!(
        projected.columns[1].kind,
        open_sdbl::query::ColumnKind::DateTime
    );

    // A register record pairs its period with its recorder, which is a
    // composite reference here and keeps its `RTRef ‖ RRRef` payload.
    let register = compile(
        "ВЫБРАТЬ Р.МоментВремени КАК М ИЗ РегистрНакопления.КоличествоПредметовВПапках КАК Р;",
    )
    .expect("the point in time of a register record must compile");
    assert_contains(
        &register.sql,
        "\"Р\".\"_period\" AS \"М_T\", (\"Р\".\"_recordertref\" || \"Р\".\"_recorderrref\") AS \"М\"",
    );

    // The ordering, the grouping and `РАЗЛИЧНЫЕ` all take both members,
    // the date first.
    let ordered = compile(
        "ВЫБРАТЬ Д.Номер КАК Н ИЗ Документ.ВходящееПисьмо КАК Д УПОРЯДОЧИТЬ ПО Д.МоментВремени;",
    )
    .expect("ordering by a point in time must compile");
    assert_contains(
        &ordered.sql,
        "ORDER BY \"Д\".\"_date_time\" ASC, \"Д\".\"_idrref\" ASC",
    );
    let grouped = compile(
        "ВЫБРАТЬ Д.МоментВремени КАК М, КОЛИЧЕСТВО(*) КАК К ИЗ Документ.ВходящееПисьмо КАК Д
         СГРУППИРОВАТЬ ПО Д.МоментВремени;",
    )
    .expect("grouping by a point in time must compile");
    assert_contains(
        &grouped.sql,
        "GROUP BY \"Д\".\"_date_time\", \"Д\".\"_idrref\"",
    );
    let distinct =
        compile("ВЫБРАТЬ РАЗЛИЧНЫЕ Д.МоментВремени КАК М ИЗ Документ.ВходящееПисьмо КАК Д;")
            .expect("a distinct point in time must compile");
    assert_contains(
        &distinct.sql,
        "SELECT DISTINCT \"Д\".\"_date_time\" AS \"М_T\", \"Д\".\"_idrref\" AS \"М\"",
    );

    // A catalog carries no such pair, and the platform refuses the field
    // there too.
    let unknown =
        compile("ВЫБРАТЬ С.МоментВремени ИЗ Справочник.ГруппыДоступа КАК С;").unwrap_err();
    assert_eq!(unknown.kind(), QueryDiagnosticKind::UnknownField);

    // The platform refuses to aggregate the pair and refuses to walk
    // through it; both are reported here as well.
    let aggregated =
        compile("ВЫБРАТЬ МАКСИМУМ(Д.МоментВремени) ИЗ Документ.ВходящееПисьмо КАК Д;").unwrap_err();
    assert_eq!(aggregated.kind(), QueryDiagnosticKind::UnsupportedFeature);
    let walked =
        compile("ВЫБРАТЬ Д.МоментВремени.Дата ИЗ Документ.ВходящееПисьмо КАК Д;").unwrap_err();
    assert_eq!(walked.kind(), QueryDiagnosticKind::UnknownField);

    // Comparing two points in time is lexicographic on the platform; that
    // is not compiled here, and the pair is refused in expressions.
    let compared = compile(
        "ВЫБРАТЬ Д.Номер ИЗ Документ.ВходящееПисьмо КАК Д ГДЕ Д.МоментВремени > ДАТАВРЕМЯ(2024, 1, 1);",
    )
    .unwrap_err();
    assert_eq!(compared.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn accepts_an_alias_written_without_as() {
    // `КАК` is optional in front of an alias. Measured on 8.3.27: the
    // short form names a field, an aggregate, a `ВЫБОР` and a constant,
    // and a word that opens the next clause is never read as one.
    let snapshot = support::snapshot();
    let named = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка Имя ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    assert_contains(&named.sql, "AS \"Имя\"");
    let aggregated = postgres(
        &snapshot,
        "ВЫБРАТЬ КОЛИЧЕСТВО(*) Кол ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    assert_contains(&aggregated.sql, "COUNT(*) AS \"Кол\"");
    let chosen = postgres(
        &snapshot,
        "ВЫБРАТЬ ВЫБОР КОГДА ИСТИНА ТОГДА 1 ИНАЧЕ 2 КОНЕЦ Пс ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    assert_contains(&chosen.sql, "END AS \"Пс\"");
    assert_contains(&postgres(&snapshot, "ВЫБРАТЬ 1 Один;").sql, "1 AS \"Один\"");

    // A contextual keyword may name the column, as it may after `КАК`.
    assert_contains(&postgres(&snapshot, "ВЫБРАТЬ 1 Сумма;").sql, "AS \"Сумма\"");

    // `ИТОГИ` opens the totals clause; it never names the source before
    // it, nor the projection.
    let totals = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка КАК С, 1 КАК Ч ИЗ Справочник.OpenSdblMetadataProbe КАК Т
         ИТОГИ СУММА(Ч) ПО ОБЩИЕ;",
    );
    assert_contains(&totals.sql, "__totals_rows");

    // The wildcard keeps refusing an alias in either form.
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ * Имя ИЗ Справочник.OpenSdblMetadataProbe КАК Т;")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::UnsupportedFeature);
}

#[test]
fn accepts_the_selection_modifiers_in_any_order() {
    // Measured on 8.3.27: `РАЗРЕШЕННЫЕ`, `РАЗЛИЧНЫЕ` and `ПЕРВЫЕ n` are
    // accepted in any order and answer what the canonical order answers.
    let snapshot = support::snapshot();
    let canonical = postgres(
        &snapshot,
        "ВЫБРАТЬ РАЗЛИЧНЫЕ ПЕРВЫЕ 1 Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    for source in [
        "ВЫБРАТЬ ПЕРВЫЕ 1 РАЗЛИЧНЫЕ Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
        "ВЫБРАТЬ РАЗЛИЧНЫЕ ПЕРВЫЕ 1 Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    ] {
        assert_eq!(postgres(&snapshot, source).sql, canonical.sql, "{source}");
    }

    // `РАЗРЕШЕННЫЕ` takes either side of `РАЗЛИЧНЫЕ`.
    let allowed = postgres(
        &snapshot,
        "ВЫБРАТЬ РАЗРЕШЕННЫЕ РАЗЛИЧНЫЕ Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    let swapped = postgres(
        &snapshot,
        "ВЫБРАТЬ РАЗЛИЧНЫЕ РАЗРЕШЕННЫЕ Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    assert_eq!(allowed.sql, swapped.sql);

    // A modifier written twice is refused.
    let error = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile("ВЫБРАТЬ РАЗЛИЧНЫЕ РАЗЛИЧНЫЕ Т.Ссылка ИЗ Справочник.OpenSdblMetadataProbe КАК Т;")
        .unwrap_err();
    assert_eq!(error.kind(), QueryDiagnosticKind::Syntax);
}

#[test]
fn renders_a_logical_expression_as_a_value() {
    // A logical expression is a value on the platform: measured on
    // 8.3.27, `ВЫБРАТЬ Т.Наименование ПОДОБНО "%а%"` answers a boolean and
    // answers NULL when an operand is NULL. PostgreSQL has boolean values,
    // SQL Server has none, so the value form is a three-way CASE there.
    let snapshot = support::snapshot();
    let postgres_value = postgres(
        &snapshot,
        "ВЫБРАТЬ Т.Ссылка = Т.Ссылка КАК П ИЗ Справочник.OpenSdblMetadataProbe КАК Т;",
    );
    assert_contains(
        &postgres_value.sql,
        "(\"Т\".\"_idrref\" = \"Т\".\"_idrref\") AS \"П\"",
    );
    let mssql_value = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile("ВЫБРАТЬ Т.Ссылка = Т.Ссылка КАК П ИЗ Справочник.OpenSdblMetadataProbe КАК Т;")
        .unwrap();
    assert_contains(
        &mssql_value.sql,
        "CASE WHEN ([Т].[_idrref] = [Т].[_idrref]) THEN 0x01 \
         WHEN NOT ([Т].[_idrref] = [Т].[_idrref]) THEN 0x00 END AS [П]",
    );

    // The same comparison in a filter stays a plain predicate on both.
    let filtered = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile(
            "ВЫБРАТЬ Т.Ссылка КАК С ИЗ Справочник.OpenSdblMetadataProbe КАК Т
             ГДЕ Т.Ссылка = Т.Ссылка;",
        )
        .unwrap();
    assert_contains(&filtered.sql, "WHERE ([Т].[_idrref] = [Т].[_idrref])");
    assert!(!filtered.sql.contains("WHERE CASE"), "{}", filtered.sql);

    // `НЕ` and the logical spine negate a predicate, not a value.
    let negated = QueryCompiler::new(&snapshot, MsSqlBackend::new(0).unwrap())
        .compile("ВЫБРАТЬ НЕ Т.Ссылка = Т.Ссылка КАК П ИЗ Справочник.OpenSdblMetadataProbe КАК Т;")
        .unwrap();
    assert_contains(
        &negated.sql,
        "CASE WHEN (NOT ([Т].[_idrref] = [Т].[_idrref])) THEN 0x01",
    );
}

#[test]
fn joins_only_the_declared_targets_of_a_composite() {
    // The Config type description names the targets a composite reference
    // may hold, so the dereference joins those and no others. Before, with
    // SchemaStorage silent, every object carrying an attribute of that name
    // was a candidate, and more than thirty of them were refused outright.
    let mut session = open_sdbl::query::SessionParameters::new();
    for name in ["ЗначениеРазделителя", "ОбластьДанныхОсновныеДанные"]
    {
        session.set(open_sdbl::query::QueryParameter::new(
            name,
            open_sdbl::query::ParameterValue::Number {
                unscaled: 0,
                scale: 0,
            },
        ));
    }
    session.set(open_sdbl::query::QueryParameter::new(
        "ИспользованиеРазделителя",
        open_sdbl::query::ParameterValue::Boolean(false),
    ));
    let snapshot = support::demo_resolved_at(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo"),
    )
    .snapshot;
    let compiled = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ О.ЭлектронныйДокумент.Комментарий КАК К
             ИЗ РегистрСведений.ОбъектыУчетаДокументовЭДО КАК О;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .expect("the declared targets make the dereference compile");
    let joins = compiled.sql.matches("LEFT JOIN").count();
    assert!(
        (1..=4).contains(&joins),
        "only the declared targets are joined, not every object with that attribute: {joins}\n{}",
        compiled.sql
    );
    assert_contains(&compiled.sql, "\"О\".\"_fld4324_rtref\" = decode(");

    // A type description may name a whole kind instead of one object —
    // «любой бизнес-процесс» and its nine siblings are platform constants,
    // measured on 8.3.27 by declaring an attribute of each category in a
    // probe configuration. Such a field reaches every object of that kind.
    let category = QueryCompiler::new(&snapshot, PostgresBackend)
        .compile_with(
            "ВЫБРАТЬ Н.БизнесПроцесс.Наименование КАК Имя
             ИЗ РегистрСведений.НастройкаПовторенияБизнесПроцессов КАК Н;",
            &open_sdbl::query::CompileOptions::new().session(&session),
        )
        .expect("a reference category resolves to the objects of its kind");
    let joined = category.sql.matches("LEFT JOIN").count();
    assert!(
        (1..=12).contains(&joined),
        "only the business processes are joined: {joined}\n{}",
        category.sql
    );
    assert!(
        category.sql.contains("\"_bpr"),
        "the joined tables are business processes: {}",
        category.sql
    );
}
