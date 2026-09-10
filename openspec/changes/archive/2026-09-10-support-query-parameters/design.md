## Context

The lexer already produces `TokenKind::Parameter`; the parser rejects it with
"query parameters are not supported by this REPL". Literal rendering per
dialect and per target column type exists (`literal_for_type`,
`binary_literal`, `datetime_literal`, `string_literal`), and reference
constants (`ЗНАЧЕНИЕ`) already flow through comparison compilation with
`RTRef` guards. Inlining parameter values as literals therefore reuses every
rendering path and keeps the generated SQL self-contained, which matters for
the Trino connector and for the REPL, neither of which wants to manage
placeholders per provider. This was the agreed choice over `$n`/`@pn`
placeholders.

## Decisions

### Public value model

```rust
#[non_exhaustive]
pub enum ParameterValue {
    Null,
    Boolean(bool),
    Number { unscaled: i128, scale: u8 },
    String(String),
    Date(ParameterDate),
    Reference { object: ObjectId, id: [u8; 16] },
    Binary(Vec<u8>),
    List(Vec<ParameterValue>),
}
pub struct ParameterDate { year, month, day, hour, minute, second } // validated by `ParameterDate::new`
pub struct QueryParameter { name: String, value: ParameterValue }    // `QueryParameter::new`
#[non_exhaustive] #[derive(Default)]
pub struct CompileOptions<'a> { presentations: &'a [PresentationPlan], parameters: &'a [QueryParameter] }
impl CompileOptions<'a> { fn new() -> Self; fn presentations(self, &'a [PresentationPlan]) -> Self; fn parameters(self, &'a [QueryParameter]) -> Self; }
```

`Number` as unscaled + scale renders exactly (`-12.345`) without a decimal
dependency and maps one-to-one onto JDBC/Trino `BigDecimal`. `Reference`
carries the target `ObjectId` so the compiler knows the kind and the `RTRef`
number for guarded comparisons; `Binary` is the escape hatch for raw bytes
and behaves like a `0x…` literal. `List` is valid only as the operand of
`В`/`IN`; elsewhere it is a `Parameter` diagnostic. Nested lists are
rejected; `Null` elements are allowed and render as `NULL` (never matching,
as in SQL and 1C).

Parameter names are compared case-insensitively, as 1C identifiers are.

### API surface

- `QueryCompiler::compile_with(&self, source, &CompileOptions) -> Result<CompiledQuery, QueryDiagnostic>`.
- `Prepared::compile_with(&self, snapshot, &CompileOptions)`.
- `compile(source)` ≡ `compile_with(source, &CompileOptions::new())`;
  `compile_with_presentations(source, plans)` ≡ options with plans only;
  `Prepared::compile(snapshot, plans)` likewise. None are deprecated.
- `prepare` resolves a parameterized source without values: a parameter
  expression has the `Unknown` (wildcard) kind during preparation, so the
  presentation request can be collected before values are known. Values are
  required at `compile_with`.

### Diagnostics

`QueryDiagnosticKind::Parameter` (the enum is `#[non_exhaustive]`) is raised
positioned at the `&Имя` token when a referenced parameter has no value, at
the offending token when a `List` appears outside `В (…)` or contains a
nested list, and unpositioned when a supplied parameter is never referenced.
The strict unused check keeps the core honest; the REPL filters its
persistent set to the names the query mentions before calling the core.

### Rendering

A parameter compiles as if a literal of its value had been written:

| Value | Rendering |
|---|---|
| `Null` | `NULL`; kind `Null` |
| `Boolean` | `TRUE`/`FALSE` or `0x01`/`0x00`; predicate rules apply |
| `Number` | decimal text; kind `Number { precision: None, scale: Some(scale) }` |
| `String` | `'…'` / `N'…'` with quote doubling; kind `String` |
| `Date` | dialect date literal, shifted by the MSSQL year offset like `ДАТАВРЕМЯ`; kind `DateTime` |
| `Reference` | 16-byte binary literal; kind `Reference { targets: [object], runtime_typed: false }` |
| `Binary` | binary literal; kind `Binary` |
| `List` | inside `В (…)`: `IN (v1, v2, …)` after rendering each element; an empty list renders the always-false predicate `(1 = 0)` (1C returns no rows) |

Scalar parameters follow literal behaviour: the literal is rendered for
the column type of the other operand (`literal_for_type`) without a kind
check, so a type mismatch surfaces from the database exactly as it does for
a written literal. Strictness applies to references only: comparing a
reference parameter with a runtime-typed field emits the same
`RTRef = <number> AND RRRef = <bytes>` form that `ЗНАЧЕНИЕ` comparisons use;
with a fixed-target field only `RRRef` is compared, and a target mismatch is
an `UnsupportedFeature` diagnostic.

### Output-format binary comparisons

The console prints a reference exactly as the column carries it: `0x` plus
upper-case hex of 16 bytes for a single-member reference or of the 20-byte
`RTRef ‖ RRRef` payload for a runtime-typed field. A user must be able to
paste that value back: `Ссылка = 0x9EBC4CED…` already works for
single-member fields, but a two-member field is rejected as a compound
field. Binary literals and `Binary` parameter values compared with a
reference field (`=`, `<>`, `В (…)`) therefore follow one rule:

| Field members | Accepted length | Rendering |
|---|---|---|
| `RRRef` only | 16 | `RRRef = 0x…` |
| `RTRef` + `RRRef` | 20 | `(RTRef = 0x<4> AND RRRef = 0x<16>)`; `<>` negates the conjunction; `В` expands to a disjunction of such pairs |
| `RTRef` + `RRRef` | 16 | diagnostic naming the expected 20-byte form |

Any other length is a positional diagnostic. Compound fields with value
members (`_TYPE`, `_S`, `_N`, …) stay unsupported in expressions. Parameters of `Date` kind are accepted as
virtual-table periods (`СрезПоследних(&Период)`, `Остатки(&Период)`,
`Обороты(&Начало, &Конец)`) and any parameter inside virtual-table
conditions.

### Empty references

`ЗНАЧЕНИЕ(<Вид>.<Объект>.ПустаяСсылка)` / `EmptyRef` is accepted for every
kind that has a reference table (catalogs, documents, enumerations, charts of
characteristic types, accounts, calculation types, exchange plans, business
processes, tasks) even though other `ЗНАЧЕНИЕ` forms stay limited to
catalogs and enumerations. It renders the 16-byte zero literal and has kind
`Reference { targets: [object], runtime_typed: false }`, so it composes with
`ЕСТЬNULL`, `ВЫБОР`, `В (…)`, and comparisons exactly like a reference
parameter.

### REPL

- `\set Имя <литерал>` stores a parameter for the session; the literal is
  lexed with the SDBL lexer and accepts a number, string, `ИСТИНА`/`ЛОЖЬ`,
  `NULL`, `ДАТАВРЕМЯ(…)`, `0x…` (16 bytes become `Binary`, compared like a
  hex literal), `ЗНАЧЕНИЕ(Перечисление.X.Y)` and
  `ЗНАЧЕНИЕ(<Вид>.<Объект>.ПустаяСсылка)` (constants resolved from the
  snapshot), and a parenthesized list of those. Catalog predefined values
  are not accepted in `\set` because they require a database lookup; they
  can be written inline in the query.
- `\params` lists the stored parameters in insertion order, one line per
  parameter: the name, the literal text exactly as entered in `\set`, and
  the value kind (`DateTime`, `Reference`, `List[2]`, …). `\set` without
  arguments prints the command syntax; `\unset Имя` removes one parameter;
  `\refresh` keeps them all.
- Before compiling, the console scans the query tokens for parameter names
  and passes only those, so a stale `\set` never triggers the unused
  diagnostic; a referenced name without a value surfaces the core
  `Parameter` diagnostic.
- Completion offers `\set`, `\params`, `\unset`, and stored parameter
  names after `&`.

## Risks / Trade-offs

- Inlined literals defeat plan caching on the server; acceptable for a
  debugging console and for a connector that already generates ad-hoc SQL.
- `Number` precision is bounded by `i128` (38 digits), matching SQL numeric
  limits.
