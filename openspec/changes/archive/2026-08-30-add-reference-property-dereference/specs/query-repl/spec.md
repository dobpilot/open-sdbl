## MODIFIED Requirements

### Requirement: Compile a bounded read-only 1C query subset
The `open-sdbl` library SHALL compile a single-source
`ВЫБРАТЬ`/`SELECT` query into a PostgreSQL SELECT using only authoritative
resolved metadata. The supported grammar SHALL include projection or `*`, one
metadata source, optional source alias, one-hop reference property paths,
`РАЗЛИЧНЫЕ`/`DISTINCT`, `ПЕРВЫЕ`/`TOP`, basic `ГДЕ`/`WHERE` expressions, and
`УПОРЯДОЧИТЬ ПО`/`ORDER BY` with `ВОЗР`/`ASC` and `УБЫВ`/`DESC` directions.
Unsupported syntax SHALL fail before execution.

#### Scenario: Logical catalog query
- **WHEN** a query selects `Код` and a custom attribute from
  `Справочник.<name>`
- **THEN** generated SQL uses the DBNames-resolved table and Config-resolved
  physical columns without inferring a numeric name

#### Scenario: Reference property projection
- **WHEN** a query selects `Организация.Код` from a source with a fixed
  `Организация` reference
- **THEN** generated SQL left-joins the SchemaStorage-declared target through
  its ID and projects the target Code column

#### Scenario: Reused reference join
- **WHEN** the same reference path is used in projection, filtering, or ordering
- **THEN** the generated SQL contains one shared join for that source reference

#### Scenario: Bounded syntax failure
- **WHEN** a query contains a join, mutation, temporary table, parameter,
  unsupported clause, ambiguous reference target, or path deeper than one hop
- **THEN** compilation returns a positional diagnostic and no SQL is produced
