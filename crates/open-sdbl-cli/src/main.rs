use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use open_sdbl::metadata::{
    LiveColumn, LiveIndex, LiveTable, MetadataError, MetadataSnapshot, PostgresMetadataQueries,
    parse_config_descriptors, parse_db_names, parse_schema_storage, resolve_metadata,
};
use open_sdbl::{Diagnostic, tokenize};
use tokio_postgres::{IsolationLevel, NoTls, Row, Transaction};

mod repl;

const HELP: &str = "open-sdbl — tooling for the 1C query language\n\n\
Usage:\n  open-sdbl lex [FILE|-]\n  open-sdbl metadata postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl console postgres --host HOST --database DB --user USER [OPTIONS]\n  open-sdbl --help\n\n\
Commands:\n  lex       Print lexical tokens; reads standard input when FILE is '-' or omitted\n  metadata  Read and resolve 1C information-base metadata\n\n\
  console   Run 1C queries and inspect resolved metadata interactively\n\n\
PostgreSQL options:\n  --port PORT   PostgreSQL port (default: 5432)\n\n\
Authentication:\n  PGPASSWORD, PGPASSFILE, or $HOME/.pgpass\n";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run() -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print!("{HELP}");
        return Ok(());
    };

    match command.as_str() {
        "-h" | "--help" => {
            print!("{HELP}");
            Ok(())
        }
        "lex" => {
            let path = arguments.next().unwrap_or_else(|| "-".to_owned());
            if let Some(unexpected) = arguments.next() {
                return Err(CliError::Usage(format!(
                    "unexpected argument {unexpected:?}\n\n{HELP}"
                )));
            }
            lex(&path)
        }
        "metadata" => metadata(arguments).await,
        "console" | "repl" => console(arguments).await,
        unknown => Err(CliError::Usage(format!(
            "unknown command {unknown:?}\n\n{HELP}"
        ))),
    }
}

async fn metadata(mut arguments: impl Iterator<Item = String>) -> Result<(), CliError> {
    let Some(connection) = parse_postgres_connection(&mut arguments, "metadata")? else {
        return Ok(());
    };

    let mut session = PostgresSession::connect(&connection).await?;
    let result = session.metadata().await;
    let close_result = session.close().await;
    let snapshot = result?;
    close_result?;
    print_snapshot(snapshot);
    Ok(())
}

async fn console(mut arguments: impl Iterator<Item = String>) -> Result<(), CliError> {
    let Some(connection) = parse_postgres_connection(&mut arguments, "console")? else {
        return Ok(());
    };
    let mut session = PostgresSession::connect(&connection).await?;
    let result = async {
        let snapshot = session.metadata().await?;
        repl::run(&mut session, snapshot).await
    }
    .await;
    let close_result = session.close().await;
    result?;
    close_result
}

fn parse_postgres_connection(
    arguments: &mut impl Iterator<Item = String>,
    command: &str,
) -> Result<Option<PostgresConnection>, CliError> {
    let Some(provider) = arguments.next() else {
        return Err(CliError::Usage(format!(
            "missing {command} provider\n\n{HELP}"
        )));
    };
    if matches!(provider.as_str(), "-h" | "--help") {
        print!("{HELP}");
        return Ok(None);
    }
    if provider != "postgres" {
        return Err(CliError::Usage(format!(
            "unsupported {command} provider {provider:?}\n\n{HELP}"
        )));
    }

    let mut connection = PostgresConnection::default();
    while let Some(option) = arguments.next() {
        if matches!(option.as_str(), "-h" | "--help") {
            print!("{HELP}");
            return Ok(None);
        }
        let value = arguments
            .next()
            .ok_or_else(|| CliError::Usage(format!("missing value for {option:?}\n\n{HELP}")))?;
        match option.as_str() {
            "--host" => connection.host = value,
            "--database" => connection.database = value,
            "--user" => connection.user = value,
            "--port" => {
                connection.port = value.parse().map_err(|_| {
                    CliError::Usage(format!("invalid PostgreSQL port {value:?}\n\n{HELP}"))
                })?;
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown {command} option {option:?}\n\n{HELP}"
                )));
            }
        }
    }
    if connection.host.is_empty() || connection.database.is_empty() || connection.user.is_empty() {
        return Err(CliError::Usage(format!(
            "--host, --database, and --user are required\n\n{HELP}"
        )));
    }

    Ok(Some(connection))
}

#[derive(Debug)]
struct PostgresConnection {
    host: String,
    port: u16,
    database: String,
    user: String,
}

impl Default for PostgresConnection {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 5432,
            database: String::new(),
            user: String::new(),
        }
    }
}

struct PostgresSession {
    client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
}

impl PostgresSession {
    async fn connect(connection: &PostgresConnection) -> Result<Self, CliError> {
        let mut configuration = tokio_postgres::Config::new();
        configuration
            .host(&connection.host)
            .port(connection.port)
            .dbname(&connection.database)
            .user(&connection.user)
            .connect_timeout(Duration::from_secs(10));
        if let Some(password) = postgres_password(connection)? {
            configuration.password(password);
        }

        let (client, connection_driver) = configuration
            .connect(NoTls)
            .await
            .map_err(CliError::database_connection)?;
        Ok(Self {
            client,
            driver: tokio::spawn(connection_driver),
        })
    }

    async fn metadata(&mut self) -> Result<MetadataSnapshot, CliError> {
        let transaction = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .read_only(true)
            .start()
            .await?;
        let snapshot = acquire_metadata(&transaction).await;
        match snapshot {
            Ok(snapshot) => {
                transaction.commit().await?;
                Ok(snapshot)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn query(&mut self, sql: &str) -> Result<Vec<Row>, CliError> {
        let transaction = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .read_only(true)
            .start()
            .await?;
        if let Err(error) = verify_transaction(&transaction).await {
            let _ = transaction.rollback().await;
            return Err(error);
        }
        match transaction.query(sql, &[]).await {
            Ok(rows) => {
                transaction.commit().await?;
                Ok(rows)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error.into())
            }
        }
    }

    async fn close(self) -> Result<(), CliError> {
        drop(self.client);
        self.driver
            .await
            .map_err(|error| {
                CliError::Database(format!("PostgreSQL connection task failed: {error}"))
            })?
            .map_err(CliError::database_connection)
    }
}

async fn acquire_metadata(transaction: &Transaction<'_>) -> Result<MetadataSnapshot, CliError> {
    verify_transaction(transaction).await?;

    let db_names_rows = transaction
        .query(PostgresMetadataQueries::DB_NAMES, &[])
        .await?;
    let db_names_data: Vec<u8> = exactly_one_row(&db_names_rows, "DBNames")?.try_get(0)?;
    let db_names = parse_db_names(&db_names_data)?;

    let mut descriptors = Vec::new();
    for row in transaction
        .query(PostgresMetadataQueries::CONFIG, &[])
        .await?
    {
        let file_name: String = row.try_get(0)?;
        let data: Vec<u8> = row.try_get(1)?;
        descriptors.extend(parse_config_descriptors(&file_name, &data)?);
    }

    let schema_rows = transaction
        .query(PostgresMetadataQueries::SCHEMA, &[])
        .await?;
    let schema_data: Vec<u8> = exactly_one_row(&schema_rows, "SchemaStorage")?.try_get(0)?;
    let schema = parse_schema_storage(&schema_data)?;

    let catalog_rows = transaction
        .query(PostgresMetadataQueries::CATALOG, &[])
        .await?;
    let live_tables = decode_catalog_rows(catalog_rows)?;
    Ok(resolve_metadata(db_names, descriptors, schema, live_tables))
}

async fn verify_transaction(transaction: &Transaction<'_>) -> Result<(), CliError> {
    let transaction_mode = transaction
        .query_one(PostgresMetadataQueries::VERIFY_TRANSACTION, &[])
        .await?;
    let read_only: String = transaction_mode.try_get(0)?;
    let isolation: String = transaction_mode.try_get(1)?;
    if read_only != "on" || !isolation.eq_ignore_ascii_case("read committed") {
        return Err(CliError::Data(format!(
            "unsafe PostgreSQL transaction mode: read_only={read_only:?}, isolation={isolation:?}"
        )));
    }
    Ok(())
}

fn exactly_one_row<'rows>(rows: &'rows [Row], name: &str) -> Result<&'rows Row, CliError> {
    match rows {
        [row] => Ok(row),
        [] => Err(CliError::Data(format!("{name} resource is missing"))),
        _ => Err(CliError::Data(format!(
            "more than one {name} resource was returned"
        ))),
    }
}

fn decode_catalog_rows(rows: Vec<Row>) -> Result<Vec<LiveTable>, CliError> {
    let mut tables = BTreeMap::<String, LiveTable>::new();
    for row in rows {
        let tag: String = row.try_get(0)?;
        let table_name: String = row.try_get(1)?;
        let value: String = row.try_get(2)?;
        let detail: String = row.try_get(3)?;
        let columns: String = row.try_get(4)?;
        let table = tables
            .entry(table_name.clone())
            .or_insert_with(|| LiveTable {
                name: table_name,
                columns: Vec::new(),
                indexes: Vec::new(),
            });
        match tag.as_str() {
            "T" => {}
            "C" => table.columns.push(LiveColumn {
                name: value,
                data_type: detail,
            }),
            "I" => table.indexes.push(LiveIndex {
                name: value,
                unique: detail == "true" || detail == "t",
                columns: columns
                    .split(',')
                    .filter(|column| !column.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }),
            _ => {
                return Err(CliError::Data(format!(
                    "unknown PostgreSQL catalog row tag {tag:?}"
                )));
            }
        }
    }
    Ok(tables.into_values().collect())
}

fn postgres_password(connection: &PostgresConnection) -> Result<Option<String>, CliError> {
    if let Some(password) = env::var_os("PGPASSWORD") {
        return password
            .into_string()
            .map(Some)
            .map_err(|_| CliError::Data("PGPASSWORD is not valid UTF-8".to_owned()));
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

fn read_password_file(
    path: &Path,
    connection: &PostgresConnection,
    explicit: bool,
) -> Result<Option<String>, CliError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliError::Io(
                format!("cannot inspect PostgreSQL password file {path:?}"),
                error,
            ));
        }
    };
    reject_insecure_password_file(path, &metadata)?;
    let contents = fs::read_to_string(path).map_err(|error| {
        CliError::Io(
            format!("cannot read PostgreSQL password file {path:?}"),
            error,
        )
    })?;
    Ok(contents.lines().find_map(|line| {
        let fields = parse_password_line(line)?;
        matches_password_field(&fields[0], &connection.host)
            .then_some(())
            .filter(|_| matches_password_field(&fields[1], &connection.port.to_string()))
            .filter(|_| matches_password_field(&fields[2], &connection.database))
            .filter(|_| matches_password_field(&fields[3], &connection.user))
            .map(|()| fields[4].clone())
    }))
}

#[cfg(unix)]
fn reject_insecure_password_file(path: &Path, metadata: &fs::Metadata) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CliError::Data(format!(
            "PostgreSQL password file {path:?} must have permissions 0600 or stricter"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_password_file(_path: &Path, _metadata: &fs::Metadata) -> Result<(), CliError> {
    Ok(())
}

fn parse_password_line(line: &str) -> Option<[String; 5]> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = Vec::with_capacity(5);
    let mut field = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            field.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' && fields.len() < 4 {
            fields.push(std::mem::take(&mut field));
        } else {
            field.push(character);
        }
    }
    if escaped {
        field.push('\\');
    }
    fields.push(field);
    fields.try_into().ok()
}

fn matches_password_field(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

fn print_snapshot(snapshot: MetadataSnapshot) {
    println!("RECORD\tGUID\tKIND\tNAME\tPHYSICAL_NAME\tOWNER\tSCHEMA\tLIVE\tDETAIL");
    for object in snapshot.objects {
        let mut details = Vec::new();
        if let Some(allowed_length) = object.code_allowed_length {
            details.push(format!("Code={}", allowed_length.as_str()));
        }
        if let Some(allowed_length) = object.number_allowed_length {
            details.push(format!("Number={}", allowed_length.as_str()));
        }
        println!(
            "OBJECT\t{}\t{}\t{}\t{}\t\t{}\t{}\t{}",
            object.guid,
            object.kind.map_or("NonTabular", |kind| kind.as_str()),
            escape_field(object.name.as_deref().unwrap_or("")),
            object.physical_table.as_deref().unwrap_or(""),
            yes_no(object.declared),
            yes_no(object.live),
            details.join(","),
        );
    }
    for field in snapshot.fields {
        println!(
            "FIELD\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            field.guid,
            if field.data_separator {
                "DataSeparator"
            } else {
                "Field"
            },
            escape_field(field.name.as_deref().unwrap_or("")),
            field.physical_name,
            field.owner_tables.join(","),
            yes_no(field.declared),
            yes_no(field.live),
        );
    }
    for index in snapshot.indexes {
        println!(
            "INDEX\t\tIndex\t{}\t{}\t{}\tyes\t{}\t{}",
            escape_field(&index.declared_name),
            index.live_name.as_deref().unwrap_or(""),
            index.table,
            yes_no(index.live_name.is_some() && index.unique_matches),
            index.logical_key.join(","),
        );
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn lex(path: &str) -> Result<(), CliError> {
    let source = if path == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::Io("cannot read standard input".to_owned(), error))?;
        source
    } else {
        fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path:?}"), error))?
    };

    for token in tokenize(&source).map_err(CliError::Lexical)? {
        println!(
            "{}:{}\t{}\t{}",
            token.span.line,
            token.span.column,
            token.kind,
            escape_lexeme(token.lexeme)
        );
    }
    Ok(())
}

fn escape_lexeme(lexeme: &str) -> String {
    let mut escaped = String::with_capacity(lexeme.len());
    for character in lexeme.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String, io::Error),
    Lexical(Diagnostic),
    Metadata(MetadataError),
    Data(String),
    Database(String),
    Terminal(String),
}

impl CliError {
    const fn exit_code(&self) -> u8 {
        match self {
            Self::Lexical(_) | Self::Metadata(_) | Self::Data(_) => 1,
            Self::Usage(_) | Self::Io(_, _) | Self::Database(_) | Self::Terminal(_) => 2,
        }
    }

    fn database_connection(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL connection failed: {error}"))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io(context, error) => write!(formatter, "{context}: {error}"),
            Self::Lexical(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
            Self::Data(message) | Self::Database(message) | Self::Terminal(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl From<MetadataError> for CliError {
    fn from(error: MetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<tokio_postgres::Error> for CliError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(format!("PostgreSQL query failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{PostgresConnection, parse_password_line, read_password_file};

    #[test]
    fn parses_password_file_escaping_and_wildcards() {
        let fields = parse_password_line(r"host\:part:5432:*:reader:pa\\ss\:word").unwrap();
        assert_eq!(fields[0], "host:part");
        assert_eq!(fields[2], "*");
        assert_eq!(fields[4], r"pa\ss:word");
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
            host: "db".to_owned(),
            port: 5432,
            database: "test".to_owned(),
            user: "reader".to_owned(),
        };

        assert_eq!(
            read_password_file(&path, &connection, true).unwrap(),
            Some("secret".to_owned())
        );
        std::fs::remove_file(path).unwrap();
    }
}
