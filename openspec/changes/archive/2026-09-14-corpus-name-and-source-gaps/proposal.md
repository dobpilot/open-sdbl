## Why

Two small gaps the demo corpus shows. Nine queries read a tabular section
of a business process or a task, which the compiler refused as «supported
only for catalogs and documents», although every object kind that has
tabular sections stores them the same way. Four more name an enumeration
value `НеОпределено`, which the lexer reads as the keyword, so
`ЗНАЧЕНИЕ(Перечисление.ВероятностиКТ.НеОпределено)` was rejected.

## What Changes

- A tabular-section source SHALL be accepted for every object kind that
  has tabular sections: catalogs, documents, charts of characteristic
  types, charts of accounts, charts of calculation types, business
  processes, tasks and exchange plans.
- The kind, object and value names inside `ЗНАЧЕНИЕ(…)` SHALL accept a
  name spelled like a keyword, as the grammar there admits nothing else.

## Capabilities

### Modified Capabilities

- `query-compilation`: tabular-section sources and metadata names.

## Impact

- `src/query/core/resolve.rs`, `parser.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
