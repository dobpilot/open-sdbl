## Context

Each branch is compiled by `compile_branch` into a `CompiledBranch` with
`sql`, `columns` (label + `ColumnKind`), and deferred presentations; the
source layer wraps virtual tables in parenthesized statements already
(`CompiledSourceRelation`). A nested query therefore fits the same slot as a
virtual-table relation: a parenthesized statement with an alias and a known
column list. Correlated subqueries are not part of this change: the 1C
Syntax Assistant describes a nested query as an independent source, and
non-correlated `IN` subqueries cover the everyday filter idiom.

## Decisions

### AST

```rust
enum SourceAst { Metadata(MetadataSourceAst), Nested { token, query: Box<QueryAst>, alias } }
Expression::InList  { value, items, negated }
Expression::InQuery { value, token, query: Box<QueryAst>, negated }
```

A nested source requires an alias (1C requires one too). Both forms are
parsed by recursing into `parse` with the shared depth counter, so
`MAX_DEPTH` bounds nesting; each nested statement charges one work unit plus
its own content.

### Derived sources

The nested query is compiled with `compile` (unions, grouping, joins, `TOP`,
`DISTINCT` allowed) into a statement `S` with columns `c1..cn`. The outer
branch registers a `SourceScope` whose fields are synthesized from those
columns: one `QueryableField` per column with `output_label` = emitted
label, one physical member named by the label, `data_type` derived from the
kind, and `kind` copied. Field lookup by name uses the column's label
(alias or generated label) case-insensitively. Rendering is
`(S) AS alias` with columns addressed as `alias.label`.

Restrictions inside a nested query, each an `UnsupportedFeature`
diagnostic:

- final `УПОРЯДОЧИТЬ ПО` without `ПЕРВЫЕ n` (SQL Server forbids `ORDER BY`
  in a derived table unless `TOP` is present; with `ПЕРВЫЕ` the ordering is
  emitted as `TOP n … ORDER BY` / `ORDER BY … LIMIT n`, which is the
  "first N by …" idiom);
- deferred reference presentations (universal or payload references), whose
  raw payload would have to be threaded through the outer projection;
  inline presentations (string columns, catalog descriptions joined by
  `_Description`) are ordinary text columns and are allowed;
- `*`, whose labels would be positional.

The outer query may present or dereference the derived columns instead.

Dereference through a derived column (`Т.Товар.Наименование`) is allowed
when the column kind is a reference with `runtime_typed == false` and one
target: the join cache is asked for a `LEFT JOIN` of the target on
`alias.label = target._IDRRef`, the same key shape as a direct field. A
runtime-typed payload column (20 bytes) cannot be joined without splitting
the payload, so dereferencing it is a diagnostic; users narrow it inside the
nested query with `ВЫРАЗИТЬ` first. Deferred presentations of derived
reference columns work unchanged because they are keyed by the outer output
column.

### `В (подзапрос)` and `НЕ В`

The subquery must project exactly one column (`UnsupportedFeature`
otherwise) whose kind is compatible with the left operand. Rendering:

| Left operand | Subquery column | SQL |
|---|---|---|
| scalar | scalar | `left [NOT] IN (S)` |
| fixed reference | fixed reference | `left._RRRef [NOT] IN (S)` where `S` projects the inner `_RRRef` |
| runtime-typed | fixed reference to `T` | `(left._RTRef = <T> AND left._RRRef [NOT] IN (S))` (negated form: `left._RTRef <> <T> OR …`) |
| fixed reference to `T` | runtime-typed | `left._RRRef [NOT] IN (S')` where `S'` projects the inner `_RRRef` and adds `inner._RTRef = <T>` to its filter |
| runtime-typed | runtime-typed | `left.payload [NOT] IN (S)` over the 20-byte payloads |

For reference columns the inner statement projects the physical member
rather than the payload, which is why the nested query is compiled with a
"member projection" flag when it feeds `IN`. `НЕ В (список)` reuses the
existing list path with `NOT IN`.

### Correlation

Name resolution inside a nested query sees only its own scopes. An
identifier that fails to resolve there but would resolve in an enclosing
scope produces `correlated subqueries are not supported` positioned at the
identifier; other failures keep their usual `UnknownField` diagnostic.

### Labels

Output labels stay unique per statement; a nested statement allocates its
own label set, and the outer statement re-allocates labels for what it
projects, so the same alias may appear in both without collision.

## Risks / Trade-offs

- Synthesized `data_type` strings for derived columns are approximations
  used only for literal rendering (`literal_for_type`); kinds carry the
  truth. Dates in a derived column are already offset-corrected by the
  nested projection on MSSQL, so the outer projection must not correct them
  again: derived date columns are flagged as logical, not physical.
- MSSQL requires derived tables to have named columns; every nested
  projection already carries a label.
