## Why

Real 1C queries construct constant dates with `ДАТАВРЕМЯ` and truncate dates
to reporting boundaries with `НАЧАЛОПЕРИОДА`. The bounded compiler currently
parses both names as field paths, so these read-only expressions fail before
SQL generation.

## What Changes

- Recognize bilingual `ДАТАВРЕМЯ`/`DATETIME` date constructors with three to
  six integer components.
- Recognize bilingual `НАЧАЛОПЕРИОДА`/`BEGINOFPERIOD` expressions with the 1C
  period constants minute, hour, day, week, ten days, month, quarter, half year,
  and year.
- Generate native PostgreSQL and MSSQL date expressions, including the existing
  MSSQL `_YearOffset` storage convention.
- Allow the functions in projections, filters, JOIN-source filters, and virtual
  table period arguments while preserving read-only query guarantees.
- Return positional diagnostics for invalid arity, components, period names,
  and MSSQL dates outside the representable offset range.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: compile bounded date constructors and period-boundary
  expressions.

## Impact

- Extends the dependency-free lexer, query AST, parsers, and both SQL renderers.
- Adds no I/O or production dependency to the root crate.
- Week boundaries use Monday deterministically because `MetadataSnapshot` does
  not currently expose the infobase regional first-weekday setting.
