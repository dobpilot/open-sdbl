## Why

Real 1C queries commonly filter a field against several constants with the
`В (...)` operator. Although `В`/`IN` is already recognized as a keyword, the
query parser currently cannot represent or compile an IN-list, including a
list of predefined values returned by `ЗНАЧЕНИЕ`.

## What Changes

- Parse bilingual `В`/`IN` as a comparison operator followed by a non-empty
  parenthesized list of scalar expressions.
- Compile every list member using the type context of the left operand.
- Generate equivalent PostgreSQL and MSSQL `IN (...)` predicates.
- Allow metadata value expressions in the list without evaluating them in the
  application.
- Return positional diagnostics for empty or malformed lists.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: compile bounded `В`/`IN` list predicates.

## Impact

- Extends the query AST and SQL compiler without adding dependencies or I/O.
- Existing comparison precedence and database-specific literal conversion are
  preserved.
