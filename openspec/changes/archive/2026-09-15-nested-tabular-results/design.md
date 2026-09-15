## Context

A tabular section projected as a column makes the result of one query
non-flat: every row of the main result carries a table of its own. The
platform answers it with two statements linked by the owner key, measured
on 8.3.27 (see the proposal). Three consumers have to work from what the
compiler returns: the CLI, which executes and prints; the `Запрос` object
of open-bsl, which must expose `Выборка.<Состав>.Выбрать()`; and the Trino
connector, which builds its own plan and must never parse our SQL.

## Decisions

### Return nested results beside the main statement

`CompiledQuery` carries `nested: Vec<NestedResult>`. Each entry holds the
label the section takes in the logical result, its position among the
columns, a SELECT-only statement, the columns of that statement, and the
link: `owner_column` (index into the main result) and `key_column` (index
into the nested result). A consumer runs the main statement, then the
nested one, and groups the nested rows by the linking value.

The alternative — one statement with `json_agg` or `ARRAY(ROW(…))` — was
rejected: SQL Server 2008 has no such construct, the library would gain a
decoder for nested payloads, and the shape would stop matching what the
platform does.

### Make the nested statement self-contained

The nested statement filters by the owner keys of the main statement
itself:

```sql
WHERE <section>.<owner key> IN (SELECT <key> FROM (<main statement>) AS m)
```

so a consumer can run it without inventing a temporary table. The inner
copy of the main statement is generated without `УПОРЯДОЧИТЬ ПО` and
without `ПЕРВЫЕ`, because a subquery may not carry them on SQL Server and
they do not change which owners the result names — except that `ПЕРВЫЕ N`
does: a limited main statement names fewer owners. A nested result of a
statement with `ПЕРВЫЕ` therefore keeps the limit inside the subquery,
which SQL Server accepts because `TOP` is allowed there.

A consumer that knows its own keys — open-bsl, which has the main rows in
memory — may ignore the subquery and filter by the values it holds; the
structural link says which column to use.

### Add the owner key as a service column

The nested statement needs the owner key, and the main result usually does
not select it. The main statement gains the key as a trailing column, and
`CompiledQuery.service_columns` lists the indices a consumer should not
print. This mirrors the platform, whose temporary table carries exactly
that key plus the row position.

### Order the nested rows by owner and line number

The platform orders the second statement by the position of the main row.
We order by the owner key and then by the section's line number, which
gives every owner its rows in line order and groups them together. A
consumer that needs the main-row order already has it from the main
result.

## Risks / Trade-offs

- The main statement is executed twice when the consumer uses the
  self-contained form. That is the price of not requiring a temporary
  table; a consumer that can bind keys avoids it.
- `CompiledQuery` becomes `#[non_exhaustive]`, which breaks external code
  that constructs it. The crate is pre-1.0 and the structure is meant to
  be read, not built.
