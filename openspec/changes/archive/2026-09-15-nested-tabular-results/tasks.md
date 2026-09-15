## 1. Contract

- [x] 1.1 Add `NestedResult` and extend `CompiledQuery` with `nested` and
  `service_columns`, making the structure `#[non_exhaustive]`.
- [x] 1.2 Document the link between the statements in the rustdoc of the
  new types.

## 2. Compilation

- [x] 2.1 Parse `Состав`, `Состав.(Поле, …)` and `Состав.*` in a
  projection, keeping every other position a diagnostic.
- [x] 2.2 Compile the nested statement of a section, selecting the owner
  key, the line number and the requested columns.
- [x] 2.3 Add the owner key to the main statement as a service column when
  it is not already selected.
- [x] 2.4 Filter the nested statement by the owner keys of the main
  statement, without its ordering and keeping its `ПЕРВЫЕ`.

## 3. Consumers

- [x] 3.1 Print a nested column in the CLI as the number of rows it holds,
  and hide service columns.

## 4. Verification

- [x] 4.1 Compare the probe answers with the platform for a whole section,
  a named list of columns, and a section of a filtered statement.
- [x] 4.2 Re-record the corpus, execute the nested statements on the live
  base, and update the documentation.
- [x] 4.3 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
