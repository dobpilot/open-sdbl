## Context

DBNames marks separator fields (`DataSeparationUse`/`DataSeparationHolder`
entries sharing the common attribute's GUID), and `MetadataField` carries
`data_separator: bool`. The compiler uses the flag only to partition
register slices and balances. Session parameters exist since 0.3.5
(`SessionParameters`, `\session`), and restrictions already produce a
derived-table wrapper per source (`wrap_restricted_relation`).

A common attribute's Config resource (demo base and probe base, platform
8.3.27) is a bare-GUID resource whose class list starts with `5`:

```text
{1,
 {5,
  {27,{2,{3,{1,0,<guid>},"ОбластьДанныхОсновныеДанные",{synonyms},"",0,0,<zero>,0},{"Pattern",{"N",7,0,1}}},
      0,{0},{0},0,"",0,{"U"},{"U"},0,<zero>,2,0,{5006,0},{3,0,0},{0,0},0,{0},{"N",0},0,0,0},
  {3,<count>,<object guid>,{2,<use>,<zero>},…},
  0,1,0,<auto-use>,
  {1,<value session parameter>},{1,<use session parameter>},{1,<conditional constant>},
  <users separation>,<authentication separation>,<separated-data-use>,<extensions separation>,1},
 0}
```

The three `{1,<guid>}` references after the content list are, in order, the
session parameter carrying the value (`ОбластьДанныхЗначение`, Number 7),
the session parameter carrying the use flag (`ОбластьДанныхИспользование`,
Boolean), and the conditional-separation constant (nil when unset). The
third scalar after them is the separated-data-use mode: `0` is
`Независимо`, `1` is `Независимо и совместно`. The probe configuration
(`ibcmd` on 8.3.27, one attribute per mode, both bound to the same session
parameters) confirmed the position, and the BSP demo base agrees: the main
data area is `Независимо`, the auxiliary one `Независимо и совместно`. The
platform refuses to load a separator without both bindings, so the
references are always present in a base that loads. SchemaStorage repeats
the same facts per table: after the kind letter each table lists all its
separator columns and then the `Независимо` ones alone, which are the
columns leading its primary key.

## Decisions

### Config projection

`ConfigDescriptor` gains `separation: Option<DataSeparationSettings>`,
filled only for resources whose class id is `5` and whose owner descriptor
GUID has a DBNames separator entry (the decoder does not know DBNames, so
it fills the value for every common attribute and resolution keeps it for
separators only):

```rust
pub enum SeparatedDataUse { Independent, IndependentAndShared }
pub struct DataSeparationSettings {
    pub mode: SeparatedDataUse,
    pub value_parameter: Option<Guid>,
    pub use_parameter: Option<Guid>,
}
```

The streaming projector records the scalar window and the three references
that follow the content list; a resource that does not match the layout
(older platform, truncated tail) yields `None`, and resolution reports
`ResolutionFinding::SeparatorSettingsMissing { guid, name }` while treating
the separator as `IndependentAndShared` with no bindings. Session parameter
names come from their own bare-GUID descriptors, which the existing
acquisition already loads (class id `1`), so resolution turns the GUIDs into
names without new queries.

Snapshot: `MetadataField` keeps `data_separator` and gains
`separation: Option<DataSeparation>` with `mode`, `value_parameter:
Option<String>`, `use_parameter: Option<String>`. `MetadataSnapshot`
gains `separators()` listing the separator fields for the CLI and for the
compiler.

### Value resolution

For each separator, at statement compilation:

1. If `use_parameter` names a session parameter present in
   `SessionParameters` with the boolean value `false`, the separator is
   disabled: no predicate anywhere in the statement.
2. Otherwise the value is the session parameter named by
   `value_parameter`, else the one named like the common attribute.
3. Missing value: `IndependentAndShared` uses the empty value of the
   separator's SchemaStorage kind (number `0`, string `""`, boolean
   `ЛОЖЬ`, date `0001-01-01`, rendered with the dialect's typed literal,
   including the MSSQL year offset); `Independent` raises
   `QueryDiagnosticKind::Parameter` at the first source token reading a
   table with the column: `data separator "<name>" requires session
   parameter "<parameter>"`. A separator of any other kind without a
   value is a `Metadata` diagnostic. Preparation runs with unbound
   parameters and only collects requests, so separators stay silent there
   and act in the bound compilation.

The resolved values live on `CompilationCatalog` next to the restriction
state, so nested queries, `В (ВЫБРАТЬ …)`, and restriction bodies see the
same values. Query parameters do not participate; separators are session
state, as in the platform.

### Placement

`compile_live_relation` returns, in addition to the relation string, the
list of separator predicates `<alias>._Fld<N> = <literal>` for the columns
the live table declares (a `UNION ALL` of extension tables filters each
branch inside, so an `X1` table without the column gets none and the
outer list is empty). The branch renderer places them
(`place_separator_predicates`):

- a source introduced by `ВНУТРЕННЕЕ`/`ЛЕВОЕ`: appended to its own `ON`
  after the user's condition, which filters its rows before any null
  extension;
- the first source and a source introduced by `ПРАВОЕ`: appended to the
  `ON` of the next `ПРАВОЕ` join that null-extends them, or conjoined to
  the statement `WHERE` (created when absent, before the query's own
  filter) when no later `ПРАВОЕ` follows;
- `ПОЛНОЕ`: the compiler already emulates it as two `LEFT JOIN`
  directions, so each direction filters its joined side in `ON` and its
  base side in `WHERE`; no wrapper is needed;
- dereference and presentation joins (`append_reference_join`): appended
  to `ON` after the type guard (`JoinPlan::target_predicates`);
- virtual tables: conjoined to the base read predicate list of slices,
  balances, and turnovers, next to the restriction conjunction;
- restricted sources: inside the `__restricted` wrapper, before the
  restriction condition;
- `Константа.X` single reads and the constants table: `WHERE` of their
  reads.

The literal is repeated per table rather than compared between tables
(`a._Fld1 = b._Fld1`), so every table gets an index seek on a constant.

### Diagnostics and reporting

No new diagnostic kind. `\d <object>` keeps the `DataSeparator` row kind;
`\dt` is unchanged. A new `ResolutionFinding` variant covers undecodable
settings; the enum is `#[non_exhaustive]`.

## Risks / Trade-offs

- The Config tail layout is inferred from one platform version; the probe
  configuration (task 1.1) fixes the mode encoding, and the fallback keeps
  old or unexpected bases working with the shared-data default.
- Defaulting to the empty value changes the result set of every query on a
  separated base that ran before without a session parameter: rows of
  areas other than `0` disappear. This is the behavior the maintainer
  chose; the disable flag restores the old, unfiltered reads.
- The deferred presentation batch (`SELECT … WHERE "_IDRRef" IN (…)`)
  reads by primary reference and is not filtered by separators; the
  values are unique across areas, so only the seek shape differs.
