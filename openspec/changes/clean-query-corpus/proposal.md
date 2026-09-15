## Why

The recorded corpus answers the question "what can the compiler not do?"
with a number that is wrong in both directions. Of its 397 entries, 15 are
not queries at all but interface captions that begin with the word
`Выбрать` ("Выбрать пользователя", "Выбрать версию для восстановления…"),
which the extraction mistook for `ВЫБРАТЬ`. Two more are refused only
because the harness binds every parameter to `ParameterValue::Null`, so a
`СрезПоследних(&ДатаОкончания,)` cannot be compiled even though the same
query compiles against a date. One reaches a register the pruned fixture
does not carry, which is a property of the fixture rather than of the
compiler.

Every later gap-closing change is measured against this corpus, so it has
to stop lying first.

## What Changes

- Drop the recorded entries that are not queries, and refuse them at
  recording time so the next re-record does not bring them back.
- Record an explicit parameter value per query, so a query that needs a
  date is compiled against a date instead of `NULL`.
- Mark the entries whose metadata the pruned fixture does not carry, and
  count the compiled share against the applicable entries only.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: the recorded corpus carries only queries, binds their
  parameters, and separates fixture limits from compiler gaps.

## Impact

No public API, syntax, or diagnostic changes. The change affects the test
fixture `tests/fixtures/demo/corpus.jsonl`, its recorded results, and the
corpus harness in `tests/query_corpus.rs`.
