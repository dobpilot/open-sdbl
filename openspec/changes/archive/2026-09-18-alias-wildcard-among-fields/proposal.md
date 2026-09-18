## Why

`Взаиморасчеты.*` written after named fields and next to a join is
refused as an unknown tabular section; the platform projects every
field of the source there. One query, shared by the УНФ and the
accounting corpora, writes it so.

## What Changes

- `<Псевдоним>.*` naming a source SHALL stand for every field of that
  source wherever it is written in the projection list, alongside
  other fields and in joined statements; `*` alone keeps its rules.

## Capabilities

### Modified Capabilities

- `query-repl`: alias wildcard among fields and joins.

## Impact

`source_wildcard_scope` in `src/query/core/codegen/select.rs`; the
section positions after such a wildcard count its fields.
