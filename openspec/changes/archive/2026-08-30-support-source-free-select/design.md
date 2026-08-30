# Design: Source-free scalar branches

`SelectAst.source` becomes optional. Parsing first consumes the projection
list, then consumes `FROM` and its source only when present. A source-free
branch is compiled by a dedicated path that accepts scalar expressions and
literal arguments of `PRESENTATION`/`REFPRESENTATION`.

The compiler emits stable labels `column1`, `column2`, ... for unnamed scalar
expressions and preserves function names as labels for presentation calls.
Every expression is cast to PostgreSQL `text`, matching the console's existing
row decoder. Field references without a source fail before SQL generation.
