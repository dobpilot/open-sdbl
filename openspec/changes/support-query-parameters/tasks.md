## 1. Public API

- [ ] 1.1 Add `ParameterValue`, `ParameterDate`, `QueryParameter`,
  `CompileOptions`, and `QueryDiagnosticKind::Parameter` with rustdoc.
- [ ] 1.2 Add `QueryCompiler::compile_with` and `Prepared::compile_with`;
  route the existing methods through default options.

## 2. Compiler

- [ ] 2.1 Parse `&Имя` into `Expression::Parameter` and resolve values
  case-insensitively during compilation (wildcard kind during preparation).
- [ ] 2.2 Render every value variant as a typed literal, including reference
  guards, date offsets, and `IN` lists with the empty-list predicate.
- [ ] 2.3 Accept date parameters as virtual-table periods and parameters in
  virtual-table conditions.
- [ ] 2.4 Compile `ЗНАЧЕНИЕ(….ПустаяСсылка)` / `EmptyRef` for every reference
  kind.
- [ ] 2.5 Emit `Parameter` diagnostics for missing, unused, and misplaced
  list parameters.
- [ ] 2.6 Compare reference fields with 16-/20-byte binary literals and
  `Binary` values by physical member (`=`, `<>`, `В`), diagnosing other
  lengths.

## 3. CLI

- [ ] 3.1 Implement `\set`, `\params`, `\unset`, help text, and
  completion.
- [ ] 3.2 Pass only referenced parameters to the compiler; surface core
  diagnostics unchanged.

## 4. Verification and documentation

- [ ] 4.1 Add unit and golden tests for every value variant on both dialects,
  diagnostics, empty references, virtual-table periods, output-format binary comparisons on fixed and
  runtime-typed fields, and REPL commands.
- [ ] 4.2 Update README (API and console sections) and
  `docs/query-language-support.md`.
- [ ] 4.3 Run formatting, Clippy, workspace tests, rustdoc, and strict
  OpenSpec validation.
