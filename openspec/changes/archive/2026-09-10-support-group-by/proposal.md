## Why

Aggregates are usable today only when a branch projects nothing but
aggregates, which covers `КОЛИЧЕСТВО(*)` but not a single report line such
as `Номенклатура, СУММА(Количество)`. `СГРУППИРОВАТЬ ПО` and `ИМЕЮЩИЕ` are
already lexed as keywords and rejected by the parser.

## What Changes

- Parse `СГРУППИРОВАТЬ ПО <keys>` / `GROUP BY` after `ГДЕ` and
  `ИМЕЮЩИЕ <predicate>` / `HAVING` after it, per branch.
- Accept as keys one-hop field paths, projection aliases, and expressions
  textually equal to a projected expression; enforce the 1C rule that every
  non-aggregate projection is a key.
- Group reference fields by all their physical members; add the physical
  columns of inline presentations of grouped keys to the key list.
- Compile `ИМЕЮЩИЕ` as a predicate that may contain aggregates over
  expressions; allow aggregates inside `ВЫБОР` branches of grouped
  projections; reject aggregates in `ГДЕ` and `ПО`.
- Restrict `УПОРЯДОЧИТЬ ПО` in grouped branches to keys and projection
  aliases; reject grouping combined with `ПОЛНОЕ СОЕДИНЕНИЕ`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: grouped branches with `HAVING`.

## Impact

- `SelectAst` gains `group: Vec<GroupKey>` and `having: Option<Expression>`.
- Depends on `support-conditional-expressions` for aggregates over
  expressions in `ИМЕЮЩИЕ`; archive that change first.
- No public API change; diagnostics reuse `UnsupportedFeature`/`Syntax`.
