# open-sdbl

`open-sdbl` is an open Rust foundation for tooling around the 1C query
language (SDBL). The first release provides a dependency-free lexer and a
small CLI for inspecting its output.

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
cargo run -- lex query.sdbl
```

Read from standard input:

```console
printf 'ВЫБРАТЬ * ИЗ Справочник.Номенклатура' | cargo run -- lex -
```

Each output line contains `line:column`, token kind, and escaped original text,
separated by tabs.

## Library

```rust
use open_sdbl::{Keyword, TokenKind, tokenize};

let tokens = tokenize("ВЫБРАТЬ Код ИЗ Справочник").unwrap();
assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Select));
```

## Development

Observable changes follow the [OpenSpec workflow](openspec/README.md). Run the
full local verification with:

```console
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
openspec validate --all --strict --no-interactive
```

The lexical subset is based on chapter 8, "Work with Queries", of the
[1C:Enterprise Developer Guide](https://1c-dn.com/download-trial/files/guides/developer_guide.pdf).

## License

[MIT](LICENSE)
