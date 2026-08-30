# Change: Refine default CLI presentation policy

## Why

The console currently applies one generic `Description(Code)` policy to every
reference target. Catalog and document references need different conventional
presentations.

## What Changes

- Catalog references use `Наименование (Код)` with a space before the opening
  parenthesis.
- Document references use `<Тип> <Номер> от <Период>`, where type is the
  Russian Config synonym (falling back to metadata name) and period is the
  document's standard `Дата`/`Date` field.
- Other metadata kinds retain deterministic field fallbacks.

## Impact

- Affected spec: `query-repl`.
- Affected code: CLI default presentation-plan provider and tests.
- The core callback ABI and SQL compiler do not change.
