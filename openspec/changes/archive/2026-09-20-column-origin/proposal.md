## Why

`CompiledColumn` carries a label, a kind, and a private name. Nothing ties
it to the metadata it came from, so an application that must hide the
value of `Справочник.Контрагенты.ИНН` can only match on the label — and
that fails immediately:

- `ВЫБРАТЬ Т.ИНН КАК Х` labels the column `Х`;
- PostgreSQL truncates a label to 63 **bytes**, so a Russian name beyond
  about thirty-one characters is cut;
- duplicate labels take `_2`, `_3` suffixes;
- expressions, aggregates, `ВЫБОР` and `ПРЕДСТАВЛЕНИЕ()` are not the
  attribute at all.

The compiler knows the answer and throws it away: `ResolvedPath` carries
the owner and the field it resolved, `SourceScope` carries the object and
its fields, and the column assembly keeps only the SQL text, the label and
the kind.

## What Changes

- `CompiledColumn` and the columns of `NestedResult` SHALL carry where the
  column came from: the metadata object, the tabular section when the
  source is one, the field, and whether the column is one member of a
  field that spreads over several.
- A column that is not a field — an expression, an aggregate, a literal —
  SHALL carry no origin. That is a legitimate answer, not a failure.
- Truncating or de-duplicating a label SHALL NOT affect the origin.
- `QueryableField` SHALL carry the field identity it was projected from,
  which is where the origin comes from; no lookup by name is involved, so
  an ambiguous name cannot mislabel a column.

## Capabilities

### Modified Capabilities

- `query-compilation`: the origin of a result column.

## Impact

- `src/query/core/resolve.rs` (`QueryableField`, `CompiledColumn`),
  `src/query/core/codegen/` (source scopes, column assembly, nested
  sections).
- New tests; no change to any generated SQL.
- No new production dependency.
