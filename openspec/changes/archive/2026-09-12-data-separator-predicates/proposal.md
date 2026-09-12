## Why

Every separated 1C base (BSP data areas, any configuration with a common
attribute in `Разделять` mode) prepends the separator column `_Fld<N>` to
every index of every separated table, including the clustered primary key
`(_Fld<N>, _IDRRef)`. The compiler knows which fields are separators
(`MetadataField::data_separator`, from DBNames `DataSeparationUse`
entries) but never emits a predicate on them, so a join `ON a._Fld1RRef =
b._IDRRef` or a filter `ГДЕ Ссылка = &Ссылка` cannot seek the index and
scans the table. The platform itself adds `_Fld<N> = <area>` to every table
it reads once the session parameter bound to the separator is set. The
library also ignores what Config says about a separator: its separation
mode and the session parameters it is bound to.

## What Changes

- Metadata decoding SHALL read the separation settings of every common
  attribute marked as a data separator from its Config resource: the
  separated-data-use mode (`Independent` or `IndependentAndShared`), the
  session parameter carrying the separator value, and the session
  parameter carrying the use flag. The snapshot exposes them next to the
  existing `data_separator` flag; unknown or undecodable settings fall back
  to `IndependentAndShared` with no bindings and produce a
  `ResolutionFinding`.
- The compiler SHALL conjoin `<alias>._Fld<N> = <value>` to every physical
  table it reads whose SchemaStorage declaration contains the separator
  column: main tables, tabular sections, extension `X`/`X1` branches,
  change-registration and calculation-kind tables, dereference and
  presentation joins, virtual-table base reads, constants, and tables
  inside restrictions and nested queries. Temporary tables and derived
  sources are untouched.
- The value comes from `SessionParameters`: the parameter bound in Config
  as the separator value, or the common attribute's own name when Config
  binds nothing. A session parameter bound as the use flag and set to
  `ЛОЖЬ` disables the predicate for that separator, as the platform does
  for `ИспользованиеРазделителя = НеИспользовать`.
- A missing value SHALL default to the empty value of the separator's
  type (`0`, `""`, `ЛОЖЬ`, the empty date) in `IndependentAndShared` mode,
  so a session without parameters reads shared data only; in `Independent`
  mode a missing value is a `Parameter` diagnostic naming the separator
  and the parameter it expects.
- Placement follows the join shape so the predicate filters the table
  before null extension: `WHERE` for the first source and the preserved
  side of a one-sided outer join, `ON` for the non-preserved side, the
  derived-table wrapper already used by restrictions for both sides of a
  `ПОЛНОЕ СОЕДИНЕНИЕ`.
- A base without separators, and a table without the separator column,
  generate SQL byte-identical to today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `onec-metadata`: common-attribute separation settings decoded from
  Config and exposed on the snapshot.
- `query-compilation`: separator predicates on every separated table read,
  session-parameter binding, default and disabled values, `Parameter`
  diagnostic for `Independent` separators without a value.

## Impact

- `src/metadata/config.rs` (common-attribute resource projection,
  `DataSeparation` settings), `resolve.rs` (`MetadataField::separation`,
  finding for undecodable settings), `snapshot.rs`.
- `src/query/core/codegen/sources.rs`, `select.rs`, `virtual_tables.rs`,
  `context.rs` (separator predicate per scope), `params.rs` (empty-value
  literals per type), `diag.rs` messages.
- CLI: no new command; `\session` already sets the bound parameters. `\d`
  keeps showing separator rows. README and
  `docs/query-language-support.md`.
- Tests: a fixture with two separators (DBNames `DataSeparationUse`,
  Config common-attribute resources, session parameter descriptors,
  SchemaStorage tables with and without the column) and goldens on both
  dialects.
