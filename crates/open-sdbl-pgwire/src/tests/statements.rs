use super::*;

#[test]
fn batch_statements_are_classified() {
    use StatementKind::{Drop, Place, Select};
    let cases: [(&str, &[StatementKind]); 7] = [
        ("ВЫБРАТЬ 1", &[Select]),
        ("ВЫБРАТЬ 1;", &[Select]),
        (
            "ВЫБРАТЬ 1 КАК А ПОМЕСТИТЬ ВТ; ВЫБРАТЬ А ИЗ ВТ",
            &[Place, Select],
        ),
        ("SELECT 1 AS A INTO T; DROP T", &[Place, Drop]),
        ("УНИЧТОЖИТЬ ВТ", &[Drop]),
        ("ВЫБРАТЬ 1 // ; ПОМЕСТИТЬ\n;;", &[Select]),
        ("", &[]),
    ];
    for (text, expected) in cases {
        assert_eq!(statement_kinds(text), expected, "{text}");
    }
}

#[test]
fn completion_tags() {
    assert_eq!(StatementKind::Place.tag(), "SELECT 0");
    assert_eq!(StatementKind::Drop.tag(), "DROP TABLE");
}
