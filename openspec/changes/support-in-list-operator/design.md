## Context

The lexer already maps Russian `В` and English `IN` to one keyword. The parser
currently stops comparison parsing after arithmetic expressions, `IS NULL`,
or binary comparison operators. `ЗНАЧЕНИЕ` expressions are scalar expressions,
so an IN-list can reuse their existing metadata resolution and SQL generation.

## Goals / Non-Goals

**Goals:**

- support a non-empty list of two or more heterogeneous scalar expressions;
- also accept the useful single-item form without special semantics;
- retain database-aware conversion of literals relative to the left operand;
- produce positional diagnostics instead of partially generated SQL.

**Non-Goals:**

- subqueries inside `В`;
- tuple/row-value comparisons;
- `НЕ В`/`NOT IN` in this increment;
- application-side evaluation or rewriting of metadata values.

## Decisions

### Represent the list explicitly in the AST

An `InList` expression stores the left expression and an ordered, non-empty
vector of member expressions. This keeps the operator distinct from binary
comparisons and prevents accidental comma handling elsewhere in the grammar.

### Parse members at scalar arithmetic precedence

Each member is parsed with the same scalar-expression entry point used for a
binary comparison operand. Commas and the closing parenthesis remain owned by
the IN-list parser. This supports literals, fields, arithmetic, date functions,
and `ЗНАЧЕНИЕ`, while excluding predicates and subqueries.

### Reuse left-operand type context

During SQL generation, each member is compiled as the right operand of the
left expression. This preserves the existing PostgreSQL/MSSQL handling for
dates, binary values, booleans, and strings. Metadata values retain their own
resolved binary or scalar-subquery representation.

## Risks / Trade-offs

- Large literal lists can produce large SQL strings; list-size limits are left
  to the target DBMS because the compiler currently has no analogous query
  complexity limit.
- Type compatibility between non-literal expressions remains a DBMS concern,
  matching existing binary comparison behavior.
