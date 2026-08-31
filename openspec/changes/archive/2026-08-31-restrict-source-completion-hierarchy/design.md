## Context

`ConsoleHelper` stores every candidate in one vector and performs only a
case-insensitive prefix match. Since fields and physical tables share that
vector with metadata objects, source positions cannot exclude invalid layers.

## Decisions

### Maintain a dedicated source catalog

Build `source_candidates` once from every live named metadata object. Include
only `RussianKind.Name` and `EnglishKind.Name`; do not include bare names,
fields, or physical PostgreSQL identifiers. Add virtual-table suffixes only for
the register kinds that support them.

### Detect the immediately preceding source keyword

Use the already computed replacement boundary and inspect the last token before
it. Select the source catalog only when that token is `ИЗ`, `FROM`,
`СОЕДИНЕНИЕ`, or `JOIN`, case-insensitively. Other positions continue using
the general catalog.

## Verification

Cover empty and partial source prefixes, exclusion of field/physical/bare
candidates, bilingual source keywords, virtual register sources, and unchanged
field completion outside source context.
