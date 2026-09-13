## Context

`SelectAst` holds one base source and a flat list of `JoinAst`; the
codegen resolves scopes in that order, renders `FROM base [INNER|LEFT|
RIGHT] JOIN … ON …`, places separator predicates by join kind, and
transposes a single `FULL JOIN` into a `UNION ALL`. Measured on the
platform (8.3.27, 2026-09-13): `A, B ЛЕВОЕ СОЕДИНЕНИЕ C ПО C.x = A.y`
fails with `Поле не найдено "A.y"`, so a join sees only its own comma
element; `A ЛЕВОЕ СОЕДИНЕНИЕ C ПО …, B` is accepted; `*` over two
sources projects both with `1`-suffixed duplicate names.

## Decisions

- A comma element after the first becomes a `JoinAst` of the new kind
  `JoinKind::Cross` with no condition (`condition: Option<Expression>`),
  positioned at the comma token. The flat join list therefore keeps the
  written order and every existing consumer of `joins` sees the extra
  sources as ordinary joined scopes.
- Rendering: `CROSS JOIN <source>` without `ON`. `CROSS JOIN` rather than
  a SQL comma, because the SQL comma binds looser than `JOIN` and would
  hide earlier elements from later `ON` clauses on both providers; with
  the chain the generated SQL is valid for any condition the compiler
  accepts.
- Visibility: every scope carries the index of its comma element; the
  join-scope check refuses a field of another element with `UnknownField`
  (`field "A.y" is not visible from this join; sources listed through
  commas are joined independently`).
- Separator predicates of a cross-joined source follow the base-source
  rule (the `ON` of the next `RIGHT JOIN`, else `WHERE`), because a
  `CROSS JOIN` never null-extends.
- `FULL JOIN` must still be the only join of the branch, so a comma list
  with a `FULL JOIN` is refused as today; `*` with several sources stays
  refused (the platform's `1`-suffixed duplicate columns are out of
  scope).

## Risks / Trade-offs

- `CROSS JOIN` lets a condition reference earlier comma elements at the
  SQL level; the compiler's own visibility check keeps the platform rule.
