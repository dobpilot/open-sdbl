## Why

Every real 1C query is parameterized: `ГДЕ Дата >= &НачалоПериода`,
`Склад = &Склад`, `СрезПоследних(&Период, …)`. The compiler rejects any
`&Имя` token, so callers must splice literals into the source text
themselves, which is both error-prone and impossible for references. The
companion literal `ЗНАЧЕНИЕ(Справочник.X.ПустаяСсылка)` is likewise rejected
although it is a constant of 16 zero bytes.

## What Changes

- Add a public `ParameterValue` model (`Null`, `Boolean`, `Number` as an
  unscaled `i128` plus scale, `String`, `Date`, `Reference` with its target
  object, `Binary`, `List`) and a `CompileOptions` builder carrying
  presentation plans and named parameters.
- Add `QueryCompiler::compile_with(source, &options)` and
  `Prepared::compile_with(snapshot, &options)`; existing methods stay and are
  equivalent to default options. `prepare` keeps working on parameterized
  sources without values.
- Parse `&Имя` as an expression; at compilation the value is inlined as a
  typed literal using the same rendering as `ЗНАЧЕНИЕ` and typed literals,
  including MSSQL year offsets for dates and `RTRef` guards for references.
  A `List` value is accepted only inside `В (&Список)`.
- Report a missing or unused parameter with a new
  `QueryDiagnosticKind::Parameter`.
- Compile `ЗНАЧЕНИЕ(<Вид>.<Объект>.ПустаяСсылка)` / `EmptyRef` for every
  tabular reference kind to the 16-byte zero literal with a reference kind.
- Compare any reference field with a binary literal or `Binary` parameter
  written in the console output format: 16 bytes for a single-member
  reference, 20 bytes `RTRef ‖ RRRef` for a runtime-typed field, split into
  member comparisons; other lengths are diagnostics.
- REPL: `\set Имя <литерал>`, `\params`, `\unset Имя`; the console passes
  only the parameters a query references.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `crate-architecture`: compilation options on the public compiler API.
- `query-compilation`: parameter diagnostics.
- `query-repl`: parameter expressions, empty references, console parameter
  commands.

## Impact

- New public types `ParameterValue`, `ParameterDate`, `QueryParameter`,
  `CompileOptions`; new non-exhaustive diagnostic variant.
- No new dependencies: decimal parameters are unscaled integer + scale.
- Existing behavior is unchanged when no parameter appears in the source.
