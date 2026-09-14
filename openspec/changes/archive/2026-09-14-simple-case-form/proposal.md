## Why

1C accepts the simple form `ВЫБОР <выражение> КОГДА <значение> ТОГДА …`,
where every alternative is compared with one subject. Five real demo
queries use it, including `ВЫБОР ТИПЗНАЧЕНИЯ(поле) КОГДА ТИП(...)`, and
the parser reported «CASE requires at least one WHEN alternative».

## What Changes

- `ВЫБОР` SHALL accept a subject before the first `КОГДА`, and every
  alternative SHALL be rendered as the comparison of the subject with its
  value, which is how the platform answers: measured, a `NULL` subject
  matches no alternative, not even a `NULL` one.
- The comparison SHALL reuse the reference, composite and type-value
  rules, so a subject of any of those kinds answers as it does in `ГДЕ`.

## Capabilities

### Modified Capabilities

- `query-compilation`: the simple form of `ВЫБОР`.

## Impact

- `src/query/core/ast.rs`, `parser.rs`, `codegen/expression.rs`,
  `codegen/sources.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
