## Why

The string and arithmetic functions the platform added in 8.3.20 are the
last large family of the query language the compiler does not know:
`ПОДСТРОКА`, `ДлинаСтроки`, `СокрЛП`/`СокрЛ`/`СокрП`, `ВРег`/`НРег`,
`Лев`/`Прав`, `СтрНайти`, `СтрЗаменить`, `Окр`, `Цел`, and the eleven
arithmetic functions. A query using any of them fails on the name.

## What Changes

- The lexer SHALL recognize the twenty-two new bilingual names as
  keywords that stay contextual identifiers, so fields named `Окр` or
  `Лог` keep parsing.
- The parser SHALL read them as one `ScalarFunction` node with a checked
  arity. `Лев` and `Прав` share their English spelling with the join
  keywords, so their names SHALL be recognized in expression position
  only, the Russian ones by lexeme.
- The compiler SHALL render each function per dialect and SHALL check
  that string positions receive strings and numeric positions numbers,
  `NULL` and unclassified values passing as elsewhere. PostgreSQL
  renderings SHALL stay portable to 9.0, so `Лев`/`Прав` use `substring`
  rather than `left`/`right`, and a character column of the 1C extension
  types SHALL be cast to `text` before the call.
- Measured against the platform: positions are one-based, `СтрНайти`
  answers `0` when the substring is absent, `ДлинаСтроки` counts trailing
  spaces, the trims remove spaces, `Окр` rounds half away from zero and
  accepts a negative scale, `Цел` truncates toward zero, `Log` is the
  natural logarithm, and every function propagates `NULL`.
- `СТРОКА(x)` stays unsupported: the platform returns a locale-formatted
  presentation (`10,00`, `15.01.2024 00:00:00`, `Да`), which SQL cannot
  reproduce faithfully; `ВЫРАЗИТЬ(… КАК СТРОКА)` remains the cast.

## Capabilities

### Modified Capabilities

- `sdbl-lexer`: twenty-two new bilingual keywords.
- `query-repl`: the scalar string and arithmetic library.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`, `dialect.rs`,
  `codegen/expression.rs`, `codegen/select.rs`, `codegen/sources.rs`;
  CLI completion; README and `docs/query-language-support.md`.
