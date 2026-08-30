## Context

After parsing `<kind>.<object>`, the parser currently looks only for
`КАК`/`AS`. An ordinary identifier at that position is later reported as
unsupported trailing syntax even though it is a valid implicit source alias.

## Decisions

### Parse an optional identifier after the source

If `КАК`/`AS` is present, require the following identifier. Otherwise consume
the next token only when it is an identifier. Clause keywords such as
`ГДЕ`/`WHERE` and `УПОРЯДОЧИТЬ`/`ORDER` have keyword token kinds and
therefore remain clause delimiters.

### Reuse existing qualifier resolution

Both spellings populate the same AST alias field. Existing field-path
resolution, SQL alias quoting, generated joins, filtering, and ordering then
remain unchanged.

## Risks / Trade-offs

- A misspelled clause represented as an identifier can be interpreted as an
  alias and diagnosed at the following token. This matches the grammar's
  unavoidable ambiguity and still fails before SQL execution.
