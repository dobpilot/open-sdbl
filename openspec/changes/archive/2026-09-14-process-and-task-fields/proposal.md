## Why

The standard fields of business processes and tasks had no names: the
compiler knew `ID`, `Date`, `Number` and `Marked`, but not `Завершен`,
`Стартован`, `ВедущаяЗадача`, `Наименование`, `Выполнена`,
`БизнесПроцесс` or `ТочкаМаршрута`. Six demo queries stop on them, and a
task list is a common report.

## What Changes

- The standard fields of a business process SHALL be named: `Completed` /
  `Завершен`, `Started` / `Стартован`, `HeadTask` / `ВедущаяЗадача`.
- The standard fields of a task SHALL be named: `Name` / `Наименование`,
  `Executed` / `Выполнена`, `BusinessProcess` / `БизнесПроцесс`,
  `Point` / `ТочкаМаршрута`.
- All seven names are accepted by the platform, checked on a probe
  configuration that declares a business process and its task.

## Capabilities

### Modified Capabilities

- `query-compilation`: the standard fields of business processes and
  tasks.

## Impact

- `src/query/core/resolve.rs`; `tests/query_compile.rs`;
  `tests/fixtures/demo/expected.jsonl`; `docs/query-language-support.md`.
