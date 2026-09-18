## Why

The change-registration table `<Вид>.<Имя>.Изменения` is queryable, but
its own two fields — the exchange-plan node the change is registered
for and the message number — are reachable only by their SchemaStorage
names. Configurations read `Изменения.Узел = &Узел`; the UNF corpus stops
there with "field Узел was not found".

## What Changes

- The change-registration table SHALL expose `Узел`/`Node` (a reference
  to the exchange plan, a runtime-typed pair when several plans register
  the object) and `НомерСообщения`/`MessageNo` as standard fields.

## Capabilities

### Modified Capabilities

- `query-repl`: two standard fields of the change-registration table.

## Impact

`src/query/core/resolve.rs`, `docs/query-language-support.md`.
