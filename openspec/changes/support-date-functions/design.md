## Context

The expression AST currently contains fields, literals, unary/binary operators,
and null predicates. Date strings are interpreted only when compared directly
with a typed physical column. This is insufficient for the 1C query forms
`ДАТАВРЕМЯ(2026, 9, 2)` and
`НАЧАЛОПЕРИОДА(Источник.Период, МЕСЯЦ)`.

MSSQL information bases may store logical 1C dates with `_YearOffset` added to
the physical year. A date expression used in a predicate must therefore use
the storage year, while a projected date expression must subtract the offset
before conversion to output text.

## Decisions

### Represent date expressions explicitly

Add typed AST variants for a validated date constructor and beginning-of-period
operation. `ДАТАВРЕМЯ` accepts three through six integer literals in the order
year, month, day, hour, minute, second. Missing time components are zero.
Calendar dates and component ranges are validated before SQL generation.

`НАЧАЛОПЕРИОДА` accepts any supported scalar expression as its first argument
and one unquoted bilingual period identifier as its second. Unknown or missing
periods fail positionally rather than being emitted as SQL identifiers.

### Render dates in the correct storage domain

PostgreSQL constructors use a typed timestamp literal. MSSQL constructors use
an ISO-8601 `datetime2` conversion and add `_YearOffset` when compiled in a
source-backed expression. When a known date expression is projected, the MSSQL
renderer subtracts `_YearOffset` before converting it to text.

Constant virtual-table periods use the storage-domain renderer. Source-free
SELECT expressions remain in the logical date domain because no physical 1C
column participates.

### Use deterministic native period expressions

PostgreSQL uses `date_trunc` for standard boundaries and bounded arithmetic for
ten-day and half-year boundaries. MSSQL uses `DATEADD`/`DATEDIFF` and date-part
arithmetic without session-dependent SQL settings.

Week starts on Monday in both renderers. The 1C platform can derive the first
weekday from infobase regional settings, but those settings are absent from the
current compiler input. A deterministic documented boundary is preferable to
depending on MSSQL `DATEFIRST`; configurable regional-week support remains a
future metadata/API change.

## Risks / Trade-offs

- A non-Monday regional first weekday cannot yet be reproduced.
- `ДАТАВРЕМЯ` intentionally accepts literal numeric components only, matching
  the bounded literal constructor and avoiding dialect-dependent dynamic date
  normalization.
- The change does not add `КОНЕЦПЕРИОДА`, `ДОБАВИТЬКДАТЕ`, or date-part
  extraction functions.
