## Why

Two-stage reports (aggregate first, then join or filter the aggregate) and
`Поле В (ВЫБРАТЬ …)` filters are everyday 1C idioms. Both need a query
nested inside another: as a derived source in `ИЗ`/`СОЕДИНЕНИЕ`, or as the
operand of `В`/`IN`. The parser currently rejects `(` after `ИЗ` and treats
`В (ВЫБРАТЬ …)` as a malformed list.

## What Changes

- Parse `(<query>) [КАК] <alias>` as a source in `ИЗ` and in any join; the
  nested query supports everything a top-level query does except `*`,
  deferred reference presentations, and final ordering without `ПЕРВЫЕ`.
- Expose the nested query's output columns as fields of the derived source
  with their `ColumnKind`; allow one-hop dereference through a fixed-target
  reference column.
- Parse `<expr> [НЕ] В (<query>)` and `<expr> НЕ В (<list>)`, compiling to
  `[NOT] IN (SELECT …)` / `NOT IN (…)`; the subquery must project one column
  of a compatible kind, and reference comparisons are performed on `RRRef`
  members with `RTRef` guards where needed.
- Reject correlated references (identifiers that resolve only in an
  enclosing query) with a positional diagnostic.
- Charge nested statements to the work budget and the parser depth limit.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: derived sources and subquery predicates, `НЕ В`.
- `query-compilation`: label uniqueness is per statement.

## Impact

- `SourceAst` becomes an enum of metadata source and nested query;
  `Expression::InList` gains a subquery form and a `negated` flag.
- Depends on `support-group-by` and `support-multiple-joins` for the full
  nested grammar; archive those first.
