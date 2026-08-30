# open-sdbl

`open-sdbl` is an open Rust foundation for tooling around the 1C query
language (SDBL). The workspace separates a dependency-free core library from
the `open-sdbl-cli` application, which provides read-only access to PostgreSQL
information bases through `tokio-postgres`.

> [!IMPORTANT]
> This is an early, intentionally bounded lexical subset. It is not yet a
> complete parser and does not claim byte-for-byte compatibility with every
> 1C platform version.

## Supported input

- Unicode identifiers, including Cyrillic names;
- Russian and English aliases for common query keywords;
- parameters such as `&Дата`;
- integer and decimal numbers;
- double-quoted strings with doubled-quote escaping;
- `//` line comments;
- common operators and punctuation;
- byte spans and one-based line/column positions in diagnostics and tokens.

## CLI

Build and tokenize a file:

```console
cargo run -p open-sdbl-cli -- lex query.sdbl
```

Read from standard input:

```console
printf 'ВЫБРАТЬ * ИЗ Справочник.Номенклатура' | cargo run -p open-sdbl-cli -- lex -
```

Each output line contains `line:column`, token kind, and escaped original text,
separated by tabs.

Read and resolve 1C metadata from PostgreSQL:

```console
cargo run -p open-sdbl-cli -- metadata postgres \
  --host 192.168.166.15 \
  --database test \
  --user admin1c
```

The command reads `Params.DBNames`, bare-GUID `Config` resources,
`SchemaStorage`, and PostgreSQL catalogs. It prints tab-separated object,
field, and index records with their logical GUID/name, canonical physical name,
and declaration/live status. PostgreSQL authentication uses `PGPASSWORD`,
`PGPASSFILE`, or `$HOME/.pgpass`; the command has no password option. Every
acquisition runs in an explicitly verified read-only `READ COMMITTED`
transaction and does not require the `psql` executable.

Start the interactive 1C query console with the same connection options:

```console
cargo run -p open-sdbl-cli -- console postgres \
  --host 192.168.166.15 \
  --database test \
  --user admin1c
```

Queries may span several lines and are executed after a terminating semicolon.
Interactive input supports line editing and current-session history with the
Up/Down arrow keys. The final terminal row keeps the command reminder visible;
SDBL input is syntax-highlighted, and Tab completes console commands,
Russian/English keywords, metadata objects, fields, and reference properties.
The completion catalog is rebuilt by `\refresh`. Before executing a query, the
console prints the exact generated PostgreSQL SQL and the SDBL-to-SQL
generation duration. It reports the PostgreSQL execution duration separately
before rendering result rows.
The initial compiler supports projections, `ПЕРВЫЕ` / `TOP`, `РАЗЛИЧНЫЕ` /
`DISTINCT`, basic `ГДЕ` / `WHERE` expressions, and ordering. A SELECT branch
may omit `ИЗ` / `FROM` for source-independent scalar expressions such as
`SELECT 4`, `SELECT 2 + 2`, or `SELECT ПРЕДСТАВЛЕНИЕ(4)`. Scalar results are
transported as text; fields and wildcard projections still require a metadata
source. A SELECT branch
may join two resolved metadata sources with bilingual `INNER`, `LEFT`, `RIGHT`,
or `FULL [OUTER] JOIN` syntax. PostgreSQL receives native inner/left/right
joins; a full join is transposed to two left-join branches connected by
`UNION ALL` with an anti-match predicate so duplicate rows are preserved.
Join conditions currently accept direct scalar fields from opposing sources,
equality, and `AND`. Compatible SELECT branches can be combined with `ОБЪЕДИНИТЬ` /
`UNION` or `ОБЪЕДИНИТЬ ВСЕ` / `UNION ALL`; output labels come from the
first branch and final ordering applies to the combined result. A fixed
reference can be dereferenced by one property, for example `Организация.Код`; the
compiler generates a shared `LEFT JOIN` from the `R` target declared by
`SchemaStorage`. Reference `.Представление` / `.Presentation` and the bilingual
`ПРЕДСТАВЛЕНИЕССЫЛКИ` / `REFPRESENTATION` and `ПРЕДСТАВЛЕНИЕ` /
`PRESENTATION` functions use a compile-time application callback: the core
returns a batch of possible target metadata GUIDs, the application returns
numeric/GUID field IDs and a structured template, and only then does the core
generate SQL. The console's default policy uses `Наименование (Код)` for
catalogs and `<Тип> <Номер> от <Период>` for documents; document type prefers
the Russian Config synonym, and period resolves through the standard `Дата`
field. Plans are kept in a bounded Moka cache invalidated by metadata
generation. A source alias may follow the metadata source directly or
after `КАК` / `AS`, so both `... Остатки t` and `... Остатки КАК t` are
accepted. It resolves object and field names through `DBNames`, `Config`, and
`SchemaStorage`; unsupported constructs fail before SQL execution. Use `\dt`
to list metadata tables, `\di` for indexes, `\d <name>` to describe an object,
`\refresh` to reload metadata, `\help` for help, and `\q` to exit. Every query
runs in its own verified read-only `READ COMMITTED` transaction.

Bounded aggregation supports `COUNT` / `КОЛИЧЕСТВО`, `SUM` / `СУММА`, `MIN` /
`МИНИМУМ`, and `MAX` / `МАКСИМУМ`. COUNT accepts `*`, a resolved field, or
`DISTINCT field` / `РАЗЛИЧНЫЕ поле`; the other aggregates accept one resolved
field. Aggregates over the currently transposed FULL JOIN shape are rejected
before execution to avoid aggregating the two UNION ALL halves independently.

Periodic information registers support the bilingual virtual sources
`СрезПоследних` / `SliceLast` and `СрезПервых` / `SliceFirst`. An
optional scalar period literal and an optional direct-field condition can be
passed as `SliceLast(<period>, <condition>)` or
`SliceFirst(<period>, <condition>)`; either argument may be omitted. SliceLast
chooses the greatest Period not exceeding its boundary, while SliceFirst
chooses the least Period not preceding its boundary. Virtual parameters filter
candidates before the period is selected for each Config-declared dimension
and data separator; a following WHERE filters the completed slice. The compiler
uses the resolved `_InfoRgN` main table, so this works when optional
`_InfoRgSLN` and `_InfoRgSFN` totals are disabled.

Accumulation registers support the bilingual virtual sources `Остатки` /
`Balance` and `Обороты` / `Turnovers`. Balance groups active movements by
Config-declared dimensions and data separators, applies the receipt/expense
direction to resources, removes all-zero groups, and exposes resource fields
with `Остаток` / `Balance` suffixes. Its optional period is an exclusive upper
boundary. Turnovers exposes resources with `Оборот` / `Turnover` suffixes and
accepts optional begin and end bounds as a half-open interval. The fourth
Turnovers slot and the second Balance slot accept a direct dimension/separator
condition that is applied before aggregation; a following WHERE is applied to
the aggregated result. The periodicity slot is intentionally rejected for now.
Current Balance reads the latest stored period from the authoritative
`_AccumRgT*` table and merges totals-separation rows by Config dimensions. A
historical Balance starts from a stored totals anchor and applies only the
bounded signed delta from `_AccumRgN`; the totals table is resolved by the
register GUID's `DBNames` `AccumRgT` entry rather than a physical-name scan.
Turnovers continues to aggregate `_AccumRgN` movements. The resulting virtual
sources can be aliased, joined, or dereferenced like ordinary metadata sources.

Build the release executable with:

```console
cargo build --release -p open-sdbl-cli
./target/release/open-sdbl --help
```

## Library

```rust
use open_sdbl::{Keyword, TokenKind, tokenize};

let tokens = tokenize("ВЫБРАТЬ Код ИЗ Справочник").unwrap();
assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Select));
```

Metadata decoding and resolution are available through
`open_sdbl::metadata`. The library layer has no process or network I/O and can
be fed captured `DBNames`, `Config`, `SchemaStorage`, and live-catalog records
directly. Fixed SELECT-only acquisition statements are exposed by
`PostgresMetadataQueries`; executing them remains an application concern.
`MetadataSnapshot` owns immutable hash indexes for expected O(1) object,
attribute, standard-field, and RTRef type lookup. The core remains free of
production dependencies; the async Moka cache belongs only to the CLI crate.

## Development

Observable changes follow the [OpenSpec workflow](openspec/README.md). Run the
full local verification with:

```console
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
openspec validate --all --strict --no-interactive
```

Both workspace packages use Rust Edition 2024 and declare Rust 1.85 as their
minimum supported toolchain.

The lexical subset is based on chapter 8, "Work with Queries", of the
[1C:Enterprise Developer Guide](https://1c-dn.com/download-trial/files/guides/developer_guide.pdf).

## License

Copyright © 2026 open-sdbl contributors.

[GNU General Public License v3.0 only](LICENSE)
