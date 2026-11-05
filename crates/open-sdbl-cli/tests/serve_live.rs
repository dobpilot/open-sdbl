//! Live checks of `open-sdbl serve` with `psql` against real 1C bases.
//!
//! Run with `-- --ignored`. Each test needs the gateway's own database
//! credentials in the environment the CLI reads (`PGPASSWORD` or
//! `MSSQL_PASSWORD`) and the base described by `OPEN_SDBL_SERVE_*`:
//!
//! - `OPEN_SDBL_SERVE_PG_HOST`, `OPEN_SDBL_SERVE_PG_DATABASE`,
//!   `OPEN_SDBL_SERVE_PG_USER` for PostgreSQL;
//! - `OPEN_SDBL_SERVE_MSSQL_HOST`, `OPEN_SDBL_SERVE_MSSQL_DATABASE`,
//!   `OPEN_SDBL_SERVE_MSSQL_USER` for SQL Server;
//! - `OPEN_SDBL_SERVE_IB_USER` and `OPEN_SDBL_SERVE_IB_PASSWORD`, an
//!   information-base user `psql` logs in as.
//!
//! No confirmed mapping from `v8users` to the public `StoredPasswordValue`
//! exists, so the gateway must refuse that login with `28P01`: these tests
//! check that it starts against a real base, reads its users, and fails
//! closed. They do not show a successful live 1C authentication.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn variable(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required for this live test"))
}

struct Gateway(Child);

impl Drop for Gateway {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(provider: &str, prefix: &str, port: u16) -> Gateway {
    let mut child = Command::new(env!("CARGO_BIN_EXE_open-sdbl"))
        .args([
            "serve",
            provider,
            "--host",
            &variable(&format!("{prefix}_HOST")),
            "--database",
            &variable(&format!("{prefix}_DATABASE")),
            "--user",
            &variable(&format!("{prefix}_USER")),
            "--listen",
            &format!("127.0.0.1:{port}"),
            "--allow-insecure-loopback",
        ])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let mut lines = BufReader::new(stderr).lines();
    let started = Instant::now();
    loop {
        let line = lines
            .next()
            .expect("the gateway reports that it listens")
            .unwrap();
        if line.contains("listening on") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(600), "{line}");
    }
    std::thread::spawn(move || for _ in lines {});
    Gateway(child)
}

fn psql(port: u16) -> std::process::Output {
    Command::new("psql")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            &port.to_string(),
            "-U",
            &variable("OPEN_SDBL_SERVE_IB_USER"),
            "-d",
            "base",
            "-c",
            "ВЫБРАТЬ 1",
        ])
        .env("PGPASSWORD", variable("OPEN_SDBL_SERVE_IB_PASSWORD"))
        .env("PGSSLMODE", "disable")
        .output()
        .expect("psql is installed")
}

fn check(provider: &str, prefix: &str, port: u16) {
    let _gateway = start(provider, prefix, port);
    let output = psql(port);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("password authentication failed"),
        "{stderr}"
    );
}

#[test]
#[ignore = "needs a live PostgreSQL 1C base and psql"]
fn psql_against_a_postgresql_base_fails_closed() {
    check("postgres", "OPEN_SDBL_SERVE_PG", 55433);
}

#[test]
#[ignore = "needs a live SQL Server 1C base and psql"]
fn psql_against_an_mssql_base_fails_closed() {
    check("mssql", "OPEN_SDBL_SERVE_MSSQL", 55434);
}

/// Catalog checks require a readable user with a confirmed password layout.
fn check_catalog(provider: &str, prefix: &str, port: u16) {
    let _gateway = start(provider, prefix, port);
    let output = Command::new("psql")
        .args([
            "-X",
            "-h",
            "127.0.0.1",
            "-p",
            &port.to_string(),
            "-U",
            &variable("OPEN_SDBL_SERVE_IB_USER"),
            "-d",
            "base",
            "-v",
            "ON_ERROR_STOP=1",
            "-P",
            "pager=off",
            "-c",
            "\\dn",
            "-c",
            "\\dt",
            "-c",
            "\\dv",
            "-c",
            &format!("\\d {}", variable("OPEN_SDBL_SERVE_CATALOG_RELATION")),
        ])
        .env("PGPASSWORD", variable("OPEN_SDBL_SERVE_IB_PASSWORD"))
        .env("PGSSLMODE", "disable")
        .output()
        .expect("psql is installed");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty());
}

#[test]
#[ignore = "needs a live PostgreSQL 1C base, a confirmed user, and psql"]
fn psql_catalog_postgres() {
    check_catalog("postgres", "OPEN_SDBL_SERVE_PG", 55435);
}

#[test]
#[ignore = "needs a live SQL Server 1C base, a confirmed user, and psql"]
fn psql_catalog_mssql() {
    check_catalog("mssql", "OPEN_SDBL_SERVE_MSSQL", 55436);
}
