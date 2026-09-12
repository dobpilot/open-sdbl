## Why

Reports group and filter by pieces of a date: `ГОД(Документ.Дата) КАК
Год`, `ГДЕ МЕСЯЦ(Дата) = &Месяц`, `ДЕНЬНЕДЕЛИ(Дата) В (6, 7)`. The
compiler has no date-part functions at all, so every such query fails at
the parser although both providers expose the parts natively.

## What Changes

- The lexer SHALL recognize `ГОД`/`YEAR`, `КВАРТАЛ`/`QUARTER`,
  `МЕСЯЦ`/`MONTH`, `ДЕНЬГОДА`/`DAYOFYEAR`, `ДЕНЬ`/`DAY`, `НЕДЕЛЯ`/`WEEK`,
  `ДЕНЬНЕДЕЛИ`/`WEEKDAY`, `ЧАС`/`HOUR`, `МИНУТА`/`MINUTE`, and
  `СЕКУНДА`/`SECOND` as keywords that remain usable as identifiers, period
  names, and aliases (`ГОД(Дата) КАК Год`).
- The compiler SHALL accept each of them with one date argument and return
  a number: `НЕДЕЛЯ` numbers weeks the platform way (the week containing
  1 January is week 1, weeks start on Monday, numbering restarts on
  1 January), `ДЕНЬНЕДЕЛИ` is 1 for Monday through 7 for Sunday.
- On MSSQL with a non-zero year offset the parts SHALL be taken from the
  logical date.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: ten new bilingual keywords.
- `query-repl`: date-part functions.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/expression.rs`, `codegen/sources.rs`, `codegen/select.rs`,
  `dialect.rs`.
- CLI completion list; README and `docs/query-language-support.md`.
- Builds on `date-period-functions` (shared period parsing helper).
