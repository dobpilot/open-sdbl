## 1. Constant table

- [x] 1.1 Name the value field of a `Константа.<Имя>` source `Значение`,
  accepting `Value`, and keep every other behavior of the field.
- [x] 1.2 Update the tests and goldens that read such a source by the
  constant's own name.

## 2. Full metadata names

- [x] 2.1 Resolve a field qualified by the full metadata name of an
  unaliased source, including inside subqueries.
- [x] 2.2 Keep refusing the full name when the source carries an alias.

## 3. Verification

- [x] 3.1 Compare the probe answers with the platform for the constant
  table, its dereference, and full-name qualification with and without an
  alias.
- [x] 3.2 Re-record the corpus and update the documentation.
- [x] 3.3 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
