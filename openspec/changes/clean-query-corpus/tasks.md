## 1. Recording

- [x] 1.1 Accept a text into the corpus only when the lexer and parser
  read a query from it, so interface captions are refused at recording
  time.
- [x] 1.2 Remove the recorded entries that are not queries.

## 2. Parameters

- [x] 2.1 Carry an explicit parameter binding per corpus entry and use it
  when compiling, replacing the blanket `ParameterValue::Null`.
- [x] 2.2 Record the bindings the existing queries need, starting with the
  register slices that require a date.

## 3. Fixture limits

- [x] 3.1 Mark an entry whose metadata the pruned fixture does not carry,
  and exclude it from the asserted count.
- [x] 3.2 Re-record the corpus, update the asserted count, and update
  `docs/query-language-support.md`.

## 4. Verification

- [x] 4.1 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
