# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`open-sdbl` translates 1C SDBL queries (Russian or English keywords) into SQL for PostgreSQL and Microsoft SQL Server. It decodes 1C platform metadata (`DBNames`, `Config`, `SchemaStorage`), resolves logical object/attribute names to the physical schema, and compiles SELECT queries. Rust 2024, rust-version 1.85.

## Commands

```console
cargo build --release                 # builds CLI at target/release/open-sdbl
cargo build --release --package open-sdbl   # library only

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

All five checks above are required before committing (CI runs exactly these).

Single test: `cargo test <test_name>` for library tests, or target a specific integration test file, e.g. `cargo test --test query_compile <test_name>` or `cargo test -p open-sdbl-cli --test repl_cli <test_name>`.

Optional MSSQL integration test (needs a live server): see README, runs with `-- --ignored`.

Fuzzing lives in `fuzz/` as a **separate workspace** (not built by workspace checks): `cargo fuzz run inflate` / `cargo fuzz run compile_query`.

## OpenSpec workflow (required for behavior changes)

Changes to public API, CLI, syntax, diagnostics, or compatibility require a validated OpenSpec change **before** writing code. Current contracts: `openspec/specs/`. Active proposals: `openspec/changes/<change-name>/` (proposal, requirement delta, design, tasks).

```console
openspec validate <change-name> --strict   # before implementation
openspec archive <change-name>             # after completion; merges into specs
```

## Architecture

Two-crate split with a hard boundary:

- **Root crate `open-sdbl`** (`src/`) — zero production dependencies, `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, and **no I/O of any kind** (no process, filesystem, env, terminal, network). It only decodes caller-provided bytes and generates SQL text. Keep it that way; runtime/database dependencies belong only to application crates.
- **`crates/open-sdbl-cli`** (binary `open-sdbl`) — all I/O: tokio, tokio-postgres, tiberius (MSSQL), rustyline REPL, TLS, SOCKS5, secrets handling (passwords come only from env vars, moved into zeroized memory at startup). Reads only: PG uses a `READ COMMITTED READ ONLY` transaction; MSSQL asks for read-only application intent, wraps each read in a transaction and rolls it back, poisoning the session when that rollback fails. It sends no verification statement of its own — role membership, isolation level and transaction state are the operator's business, and the compiler generates SELECT statements only.

### Core library flow

1. `src/lexer.rs` — dependency-free SDBL lexer (bilingual keywords).
2. `src/metadata/` — decodes `DBNames`, `Config` (deflate.rs handles raw-DEFLATE), `SchemaStorage`; `queries.rs` holds the fixed SELECT-only acquisition statements in two layout variants plus a `LAYOUT` probe that yields `StorageLayout` (platform 8.2 bases have no `PartNo` column and no extension tables; modern bases split resources into parts that adapters concatenate before decoding); `resolve.rs` cross-checks them and produces `ResolvedMetadata` with an immutable `snapshot` (what the compiler consumes) plus a `report` of `ResolutionFinding`s for recoverable mismatches. `extension.rs` handles 1C configuration extensions: `parse_extension_restructure` decodes `_ExtensionsRestruct._restructData` and `resolve_metadata_with_extensions` merges extension attributes so fields living only in `X`/`X1` suffix tables become queryable.
3. `src/query/` — `QueryCompiler` is generic over the **sealed** `Backend` trait; `PostgresBackend` and `MsSqlBackend` are the only implementations (do not expose a dialect extension point). `core/` holds parser → AST → name resolution → `codegen/` (SELECT, sources, expressions, virtual tables for register slices/balances/turnovers). `MsSqlBackend::new` is fallible and takes a 1C year offset in `0..=10_000`; `with_dialect_level(MsSqlDialectLevel::Sql2008 | Sql2012)` selects the T-SQL capability level (2008 emulates `DATETIME2FROMPARTS` with `DATEADD`/`DATEDIFF`), and generated PostgreSQL targets PostgreSQL 13 or newer without backend state.

### Diagnostics contract

Errors are machine-readable: `QueryDiagnosticKind` (with byte offset/line/column) and `MetadataErrorKind`. These public enums are `#[non_exhaustive]` — callers must keep a fallback match arm; preserve wrapped sources via `Error::source`.

## Conventions

- Public behavior, rustdoc, and specs are documented in English (README is Russian).
- Every new production dependency must be justified in the OpenSpec design.
- CI also runs `cargo check` on Windows — avoid Unix-only assumptions outside `cfg(unix)` blocks.
