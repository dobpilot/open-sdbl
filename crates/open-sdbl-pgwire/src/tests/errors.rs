use std::time::Duration;

use super::*;

#[test]
fn positions_count_characters_not_bytes() {
    let text = "ВЫБРАТЬ ИЗ";
    let offset = text.find("ИЗ").unwrap();
    assert_eq!(offset, 15);
    assert_eq!(character_position(text, offset), Some(9));
    assert_eq!(character_position(text, 0), Some(1));
    assert_eq!(character_position(text, 1), None);
    assert_eq!(character_position(text, 100), None);
}

#[test]
fn database_errors_hide_the_dbms_message_unless_verbose() {
    let lost = database_error(&DbError::SessionClosed, false);
    assert_eq!(lost.code, "08006");
    let timeout = database_error(
        &DbError::DatabaseTimeout {
            operation: "query".to_owned(),
            duration: Duration::from_secs(1),
        },
        false,
    );
    assert_eq!(timeout.code, "57014");
    let error = DbError::Database("relation \"_reference42\" does not exist".to_owned());
    let quiet = database_error(&error, false);
    assert_eq!(quiet.code, "XX000");
    assert!(!quiet.message.contains("_reference42") && quiet.detail.is_none());
    let verbose = database_error(&error, true);
    assert!(verbose.detail.unwrap().contains("_reference42"));
}
