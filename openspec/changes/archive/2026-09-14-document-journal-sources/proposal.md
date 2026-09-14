## Why

A document journal is a physical table of its own (`_DocumentJournal<n>`),
but the compiler knew no such metadata kind, so `ИЗ ЖурналДокументов.X`
failed with «unknown metadata kind». Four demo queries read a journal, and
journals are a common source in real reports.

## What Changes

- `ЖурналДокументов.<Имя>` / `DocumentJournal.<Name>` SHALL resolve to the
  journal table, with the standard fields the platform exposes: `Ссылка`,
  `Тип`, `Дата`, `Номер`, `ПометкаУдаления`, `Проведен`, plus the journal's
  own columns under their metadata names.
- `Ссылка` SHALL be the reference of the registered document: one column
  where the journal registers a single document kind, and the
  `RTRef ‖ RRRef` pair where it registers several, so a dereference and a
  join work as for any reference.
- `Тип` SHALL answer the type value of that reference.

## Capabilities

### Modified Capabilities

- `onec-metadata`: the document-journal kind.
- `query-compilation`: journal sources.

## Impact

- `src/metadata/db_names.rs`, `src/query/core/resolve.rs`,
  `src/query/core/codegen/context.rs`; `tests/fixtures/demo/*`;
  README and `docs/query-language-support.md`.
