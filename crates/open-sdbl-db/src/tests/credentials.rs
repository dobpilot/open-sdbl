//! Tests of the credential carrier.

use super::{Credentials, EnvironmentSecret};

const PASSWORD: &str = "s3cret-пароль-42";

#[test]
fn builds_credentials_from_owned_strings() {
    let credentials = Credentials::new(Some(PASSWORD.to_owned()), None, Some(String::new()));
    assert_eq!(
        credentials
            .postgres
            .optional("PGPASSWORD")
            .unwrap()
            .as_deref()
            .map(String::as_str),
        Some(PASSWORD)
    );
    assert!(matches!(credentials.mssql, EnvironmentSecret::Missing));
    // An empty string is a secret the caller has, not one it lacks.
    assert!(matches!(credentials.socks5, EnvironmentSecret::Present(_)));
}

#[test]
fn a_missing_secret_is_named_by_its_variable_only() {
    let error = EnvironmentSecret::from_option(None)
        .required("MSSQL_PASSWORD")
        .unwrap_err()
        .to_string();
    assert!(error.contains("MSSQL_PASSWORD"), "{error}");
}

#[test]
fn never_prints_a_secret() {
    let credentials = Credentials::new(
        Some(PASSWORD.to_owned()),
        Some(PASSWORD.to_owned()),
        Some(PASSWORD.to_owned()),
    );
    for printed in [
        format!("{credentials:?}"),
        format!("{credentials:#?}"),
        format!("{:?}", credentials.postgres),
    ] {
        assert!(!printed.contains(PASSWORD), "{printed}");
        assert!(printed.contains("Present(<redacted>)"), "{printed}");
    }
    let absent = format!("{:?}", Credentials::new(None, None, None));
    assert_eq!(
        absent,
        "Credentials { postgres: Missing, mssql: Missing, socks5: Missing }"
    );
}
