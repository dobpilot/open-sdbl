## Context

The expression compiler already has the two facilities these features need:
`compile_predicate` renders boolean positions per dialect, and
`expression_kind` computes a `ColumnKind` for any expression. `ВЫБОР` and
`ЕСТЬNULL` are pure functions of their operands' kinds; `ПОДОБНО` is a
predicate. Aggregates currently take a `FieldReference` because the kind of a
field was the only kind the compiler could name; with `expression_kind`
available the restriction is historical.

## Decisions

### AST

- `Expression::Case { token, branches: Vec<CaseBranch>, otherwise: Option<Box<Expression>> }`
  where `CaseBranch { when: Expression, then: Expression }`. Only the searched
  form exists in 1C, so no "simple CASE" operand is parsed.
- `Expression::IsNullFunction { token, value: Box<Expression>, fallback: Box<Expression> }`.
- `Expression::Like { token, value: Box<Expression>, pattern: Box<Expression>, escape: Option<Box<Expression>>, negated: bool }`.
- `AggregateArgument::Expression(Expression)` replaces `Field`; a bare field
  keeps its existing rendering because `Expression::Field` compiles to the
  same column reference.

`ЕСТЬNULL` is lexed as one keyword even though it mixes scripts; the lexer
identifier class already accepts any alphabetic character, so the keyword
table entry is enough. `ПОДОБНО`/`LIKE` and `СПЕЦСИМВОЛ`/`ESCAPE` are ordinary
keywords; `ESCAPE` is not used as a field name in 1C schemas.

### Kind rules

| Expression | Kind |
|---|---|
| `ВЫБОР` | first branch (in source order, `ИНАЧЕ` last) whose kind is not a wildcard; `Null` when all branches are `NULL` |
| `ЕСТЬNULL(x, y)` | kind of `x`, or of `y` when `x` is `NULL` |
| `ПОДОБНО` | `Boolean` |
| `СУММА(e)` | `Number`; `e` must be `Number` or a wildcard |
| `МИНИМУМ(e)`, `МАКСИМУМ(e)` | kind of `e`; a reference expression aggregates its payload (the PostgreSQL build shipped with 1C provides `max(bytea)`), while a reference field keeps aggregating its `RRRef` member alone |
| `КОЛИЧЕСТВО(e)` | `Number` |

Branch kinds are compared with `ColumnKind::is_compatible_with` (variant
only, `Null`/`Unknown` wildcards); a mismatch is an `UnsupportedFeature`
diagnostic at the offending branch token.

### Reference widening

Reference operands may differ in target and width: `ТОГДА Документ.Ссылка
ИНАЧЕ Справочник.Ссылка` yields two fixed 16-byte references to different
objects, and `ЕСТЬNULL(Регистратор, ЗНАЧЕНИЕ(…))` mixes a 20-byte payload
with a 16-byte constant. One shared helper (`widen_references`) computes the
common kind of a set of reference operands:

- all operands fixed with the same single target → fixed reference, 16 bytes,
  rendered as is;
- otherwise → runtime-typed reference whose targets are the union; every
  fixed operand is rewritten to `RTRef ‖ RRRef` with its own type number
  (`reference_payload` with a `binary_u32` constant), payload operands are
  kept.

The helper serves `ВЫБОР`, `ЕСТЬNULL`, and `ОБЪЕДИНИТЬ`: the union
orchestrator currently accepts a fixed branch next to a payload branch and
emits a column of mixed width, which this change closes by widening the
branch projections in place. `Unknown` operands cannot be widened and stay a
diagnostic.

### Rendering

- `CASE WHEN p1 THEN v1 … ELSE vn END` on both dialects; `ELSE` is omitted
  when `ИНАЧЕ` is absent (SQL and 1C both yield `NULL`). Conditions go
  through `compile_predicate`, values through `compile_expression`.
- `COALESCE(x, y)` on both dialects; `COALESCE` exists on SQL Server 2008 and
  PostgreSQL 9.0, unlike `ISNULL`, which is MSSQL-only.
- `(value LIKE pattern [ESCAPE escape])`, wrapped in `NOT (…)` when negated.
  The pattern is passed through verbatim on both providers: SQL Server `LIKE`
  supports `%`, `_`, `[…]`, `[^…]` natively, and on PostgreSQL the 1C `mchar`
  extension defines `LIKE` over `mchar`/`mvarchar` with the same character
  classes and case-insensitive comparison, so no `SIMILAR TO` rewrite is
  needed. The pattern and escape operands may be any string expression; the
  usual case is a literal (or, after parameters land, an inlined parameter).
  Case sensitivity follows the database column type.
- `ПОДОБНО` is accepted only in predicate positions (`ГДЕ`, `ПО`, `КОГДА`,
  and later `ИМЕЮЩИЕ`); using it as a projected value is an
  `UnsupportedFeature` diagnostic, because SQL Server cannot project a
  predicate. The left operand, the pattern, and the escape must have the
  `String` kind or a wildcard kind; anything else is a positional
  diagnostic, matching 1C.

### Predicate and date positions become kind-driven

`compile_predicate` currently recognizes boolean fields, literals, and casts
syntactically. It is generalized: any expression whose `expression_kind` is
`Boolean` is wrapped with `boolean_predicate` on MSSQL, so
`ГДЕ ВЫБОР КОГДА … ТОГДА ИСТИНА ИНАЧЕ ЛОЖЬ КОНЕЦ` compiles to
`(CASE … END = 0x01)`. Likewise the projection layer applies the MSSQL
year-offset correction exactly once to any expression whose kind is
`DateTime`, replacing `Expression::is_date`; `ЕСТЬNULL(Дата, ДАТАВРЕМЯ(1,1,1))`
therefore stays in physical (offset) space inside the expression and is
corrected at projection, exactly like a cast to `ДАТА` today.

### Aggregates over expressions

`compile_aggregate` compiles the argument with `compile_expression` and takes
the kind from `expression_kind`. `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ e)` renders
`COUNT(DISTINCT e)`. Aggregates nested inside another aggregate remain unsupported.
Aggregates inside a `ВЫБОР` branch (`ВЫБОР КОГДА СУММА(x) > 0 …`) and
mixing aggregate and non-aggregate projections stay diagnostics in this
change; `support-group-by` lifts both for grouped branches.

### Work budget

Each `ВЫБОР` branch, each `ЕСТЬNULL`, and each `ПОДОБНО` charges one unit,
matching binary operators, so pathological nesting still fails fast.

## Risks / Trade-offs

- On PostgreSQL a `LIKE` over a plain `text` expression (for example a
  `ВЫРАЗИТЬ(… КАК СТРОКА)` result) follows PostgreSQL semantics: no bracket
  classes and case-sensitive matching. Documented; not rewritten.
- Kind inference for `ВЫБОР` over `Unknown` catalog types yields `Unknown`,
  which stays UNION-compatible with anything; the database still type-checks.
