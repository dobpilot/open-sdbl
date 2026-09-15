use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::args::PostgresConnection;
use crate::error::CliError;

#[derive(Clone)]
pub(crate) enum EnvironmentSecret {
    Missing,
    InvalidUnicode,
    Present(Zeroizing<String>),
}

impl EnvironmentSecret {
    fn take(name: &'static str) -> Self {
        let value = env::var_os(name);
        // SAFETY: this is called before the Tokio runtime and any application
        // worker threads are created. No other thread can concurrently access
        // the process environment at this point in this binary.
        unsafe { env::remove_var(name) };
        match value {
            None => Self::Missing,
            Some(value) => value
                .into_string()
                .map(Zeroizing::new)
                .map_or(Self::InvalidUnicode, Self::Present),
        }
    }

    pub(crate) fn optional(&self, name: &str) -> Result<Option<Zeroizing<String>>, CliError> {
        match self {
            Self::Missing => Ok(None),
            Self::InvalidUnicode => Err(CliError::Data(format!("{name} is not valid UTF-8"))),
            Self::Present(value) => Ok(Some(value.clone())),
        }
    }

    pub(crate) fn required(&self, name: &str) -> Result<Zeroizing<String>, CliError> {
        self.optional(name)?.ok_or_else(|| {
            CliError::Data(format!(
                "{name} is required for the selected authentication method"
            ))
        })
    }
}

pub(crate) struct Credentials {
    pub(crate) postgres: EnvironmentSecret,
    pub(crate) mssql: EnvironmentSecret,
    pub(crate) socks5: EnvironmentSecret,
}

impl Credentials {
    pub(crate) fn take_from_environment() -> Self {
        Self {
            postgres: EnvironmentSecret::take("PGPASSWORD"),
            mssql: EnvironmentSecret::take("MSSQL_PASSWORD"),
            socks5: EnvironmentSecret::take("SOCKS5_PASSWORD"),
        }
    }
}

pub(crate) fn postgres_password(
    connection: &PostgresConnection,
    environment: &EnvironmentSecret,
) -> Result<Option<Zeroizing<String>>, CliError> {
    if let Some(password) = environment.optional("PGPASSWORD")? {
        return Ok(Some(password));
    }

    let explicit_path = env::var_os("PGPASSFILE");
    let path = explicit_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(default_password_file);
    let Some(path) = path else {
        return Ok(None);
    };
    read_password_file(&path, connection, explicit_path.is_some())
}

fn default_password_file() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".pgpass"))
}

pub(crate) fn read_password_file(
    path: &Path,
    connection: &PostgresConnection,
    explicit: bool,
) -> Result<Option<Zeroizing<String>>, CliError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliError::Io(
                format!("cannot open PostgreSQL password file {path:?}"),
                error,
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        CliError::Io(
            format!("cannot inspect PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    reject_insecure_password_file(path, &metadata)?;
    let mut contents = Zeroizing::new(String::new());
    file.read_to_string(&mut contents).map_err(|error| {
        CliError::Io(
            format!("cannot read PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    Ok(contents.lines().find_map(|line| {
        let record = parse_password_line(line)?;
        matches_password_field(&record.host, &connection.host)
            .then_some(())
            .filter(|_| matches_password_field(&record.port, &connection.port.to_string()))
            .filter(|_| matches_password_field(&record.database, &connection.database))
            .filter(|_| matches_password_field(&record.user, &connection.user))
            .map(|()| record.password)
    }))
}

#[cfg(unix)]
pub(crate) fn reject_insecure_password_file(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), CliError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !metadata.file_type().is_file() {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be a regular file"
        )));
    }
    // SAFETY: `geteuid` has no preconditions and does not retain pointers.
    let effective_uid = unsafe { libc::geteuid() };
    reject_password_file_owner(path, metadata.uid(), effective_uid)?;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must have permissions 0600 or stricter"
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn reject_password_file_owner(
    path: &Path,
    owner_uid: u32,
    effective_uid: u32,
) -> Result<(), CliError> {
    if owner_uid != effective_uid {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be owned by uid {effective_uid}, found uid {owner_uid}"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn reject_insecure_password_file(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), CliError> {
    if metadata.file_type().is_file() {
        Ok(())
    } else {
        Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must be a regular file"
        )))
    }
}

pub(crate) struct PasswordRecord {
    pub(crate) host: String,
    pub(crate) port: String,
    pub(crate) database: String,
    pub(crate) user: String,
    pub(crate) password: Zeroizing<String>,
}

pub(crate) fn parse_password_line(line: &str) -> Option<PasswordRecord> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = Vec::with_capacity(5);
    let mut field_start = 0;
    let mut escaped = false;
    for (offset, character) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' && fields.len() < 4 {
            fields.push(&line[field_start..offset]);
            field_start = offset + 1;
        }
    }
    fields.push(&line[field_start..]);
    let [host, port, database, user, password] = fields.try_into().ok()?;
    Some(PasswordRecord {
        host: decode_password_field(host),
        port: decode_password_field(port),
        database: decode_password_field(database),
        user: decode_password_field(user),
        // Exact preallocation prevents reallocations from leaving password
        // prefixes in abandoned allocator blocks.
        password: Zeroizing::new(decode_password_field(password)),
    })
}

fn decode_password_field(encoded: &str) -> String {
    let mut decoded = String::with_capacity(encoded.len());
    let mut escaped = false;
    for character in encoded.chars() {
        if escaped {
            decoded.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            decoded.push(character);
        }
    }
    if escaped {
        decoded.push('\\');
    }
    decoded
}

fn matches_password_field(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

#[cfg(test)]
mod tests {
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
}
