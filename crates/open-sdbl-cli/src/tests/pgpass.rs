//! Tests of the `pgpass` module.

use super::parse_password_line;
use super::read_password_file;
#[cfg(unix)]
use super::reject_password_file_owner;
use crate::args::{ConnectionOptions, PostgresConnection, PostgresSslMode};

#[test]
fn parses_escaped_password_records() {
    let record = parse_password_line(r"db:5432:test:user:p\:a\\ss").unwrap();
    assert_eq!(record.password.as_str(), r"p:a\ss");
}

#[test]
fn parses_password_file_escaping_and_wildcards() {
    let record = parse_password_line(r"host\:part:5432:*:reader:pa\\ss\:word").unwrap();
    assert_eq!(record.host, "host:part");
    assert_eq!(record.database, "*");
    assert_eq!(record.password.as_str(), r"pa\ss:word");
    assert!(parse_password_line("# comment").is_none());
}

#[cfg(unix)]
#[test]
fn reads_the_first_matching_secure_password_record() {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "open-sdbl-pgpass-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(
        &path,
        "other:5432:test:reader:wrong\n*:5432:test:reader:secret\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(&path, permissions).unwrap();
    let connection = PostgresConnection {
        options: ConnectionOptions {
            host: "db".to_owned(),
            port: 5432,
            database: "test".to_owned(),
            user: "reader".to_owned(),
            socks5_proxy: None,
        },
        sslmode: PostgresSslMode::VerifyFull,
        trust_ca_file: None,
    };

    let password = read_password_file(&path, &connection, true)
        .unwrap()
        .unwrap();
    assert_eq!(password.as_str(), "secret");
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn rejects_non_regular_and_wrong_owner_password_files() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, symlink};

    let path = std::env::temp_dir().join(format!(
        "open-sdbl-pgpass-fifo-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let path_bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: `path_bytes` is a valid NUL-terminated path and the mode is valid.
    assert_eq!(unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) }, 0);
    let connection = PostgresConnection {
        options: ConnectionOptions {
            host: "db".to_owned(),
            port: 5432,
            database: "test".to_owned(),
            user: "reader".to_owned(),
            socks5_proxy: None,
        },
        sslmode: PostgresSslMode::VerifyFull,
        trust_ca_file: None,
    };
    let metadata = std::fs::metadata(&path).unwrap();
    let error = read_password_file(&path, &connection, true).unwrap_err();
    assert!(error.to_string().contains("regular file"));
    std::fs::remove_file(&path).unwrap();

    let target = path.with_extension("target");
    let link = path.with_extension("link");
    std::fs::write(&target, "*:5432:test:reader:secret\n").unwrap();
    symlink(&target, &link).unwrap();
    let error = read_password_file(&link, &connection, true).unwrap_err();
    assert!(error.to_string().contains("cannot open"));
    std::fs::remove_file(link).unwrap();
    std::fs::remove_file(target).unwrap();

    let owner = metadata.uid();
    let error = reject_password_file_owner(&path, owner, owner.wrapping_add(1)).unwrap_err();
    assert!(error.to_string().contains("must be owned by uid"));
}
