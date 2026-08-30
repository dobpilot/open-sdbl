# Design: Bounded COUNT aggregation

`COUNT`/`КОЛИЧЕСТВО` becomes a keyword and a dedicated projection AST node.
Its argument is either `*` or one resolved logical field, with an optional
inner `DISTINCT`. A scalar field must have one physical value member; a pure
compound reference counts its RRef member.

The compiler emits `COUNT(...)::text` so result decoding remains uniform.
Ordinary sources and native INNER/LEFT/RIGHT JOIN branches can aggregate
directly. FULL JOIN is currently produced as two SELECT branches with
`UNION ALL`; aggregating each half would be incorrect, so COUNT with FULL JOIN
fails before SQL generation with an explicit diagnostic.
