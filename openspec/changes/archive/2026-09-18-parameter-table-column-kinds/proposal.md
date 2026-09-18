## Why

A value table parameter inferred its column kinds from the values, so an
empty table or a `NULL`-only column had no type and PostgreSQL could
refuse to compare it with a reference at run time. The owner asked for
typed columns.

## What Changes

- `ParameterValue::Table` columns become `ParameterColumn { name, kind }`
  with a declared `ColumnKind`; every value SHALL fit the kind, a
  reference SHALL point at one of the kind's targets (any object for a
  universal reference without targets), and `Null`, `Undefined`, `Type`,
  `Uuid` and `Unknown` kinds SHALL be refused.
- The first row of the CTE SHALL cast every value to the column's type;
  an empty table SHALL be a row of typed `NULL`s that never answers.
- The console literal SHALL name the kind of each column: `Имя КАК
  ЧИСЛО | СТРОКА | ДАТА | БУЛЕВО | ЛЮБАЯССЫЛКА | Вид.Объект | (Вид.Объект, …)`;
  the corpus tag becomes `T<col>:<kind>,…` and the binding tool infers a
  kind from the text.

## Capabilities

### Modified Capabilities

- `query-repl`: typed value table parameters.

## Impact

Public API: `ParameterColumn`, the `columns` field of
`ParameterValue::Table` (introduced in the same unreleased series).
