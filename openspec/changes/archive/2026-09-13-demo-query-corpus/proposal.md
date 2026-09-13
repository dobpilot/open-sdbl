## Why

Every test so far states what the compiler should do with a query the
maintainer wrote. Nothing states what it does with the queries a real
configuration contains, so a gap is only discovered when someone runs
into it. The demo base holds hundreds of such queries.

## What Changes

- A corpus of the query texts written in the demo configuration SHALL be
  committed together with the result the compiler produces for each: the
  generated PostgreSQL text, or the diagnostic it reports.
- A metadata fixture of that base, pruned to the objects the corpus
  reaches, SHALL be committed so the corpus compiles offline.
- A test SHALL recompile the corpus and compare every result with the
  recorded one, and SHALL state how many queries compile, so an
  improvement or a regression is a diff.

## Capabilities

### Modified Capabilities

- `query-repl`: a recorded corpus of real queries.

## Impact

- `tests/fixtures/demo/` (about 3.5 MB), `tests/query_corpus.rs`,
  `tests/support/mod.rs`.
