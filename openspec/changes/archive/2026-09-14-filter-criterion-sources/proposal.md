## Why

`ИЗ КритерийОтбора.<Имя>(&Значение)` asks for every object whose listed
fields hold the value. Seven demo queries use it, and the platform's own
SQL, captured on the probe base, shows the rule plainly: one `SELECT` per
field of the criterion's content, united by `UNION ALL`, projecting the
found object as a `RTRef ‖ RRRef` payload.

## What Changes

- The Config resource of a filter criterion SHALL be decoded into its name
  and the fields it searches.
- `КритерийОтбора.<Имя>(<значение>)` / `FilterCriterion` SHALL be a source
  whose single field `Ссылка` carries the found object, so it dereferences
  and joins like any reference read through a derived source.
- A reference value SHALL be compared by its 16-byte identifier, the way
  the content columns store it.

## Capabilities

### Modified Capabilities

- `onec-metadata`: filter criteria decoded from Config.
- `query-compilation`: the filter-criterion source.

## Impact

- `src/metadata/config.rs`, `resolve.rs`; `src/query/core/parser.rs`,
  `codegen/select.rs`; `crates/open-sdbl-cli/src/pipeline.rs`;
  `tests/fixtures/demo/*`; README and `docs/query-language-support.md`.
