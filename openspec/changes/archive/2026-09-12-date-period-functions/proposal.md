## Why

Period arithmetic is the second half of every 1C date filter:
`Обороты(НАЧАЛОПЕРИОДА(&П, МЕСЯЦ), КОНЕЦПЕРИОДА(&П, МЕСЯЦ))`,
`ГДЕ Дата < ДОБАВИТЬКДАТЕ(&Дата, ДЕНЬ, 1)`, or
`РАЗНОСТЬДАТ(Заказ.Дата, Отгрузка.Дата, ДЕНЬ)` in reports. The compiler
knows only `НАЧАЛОПЕРИОДА`, so such queries fail at the parser, and even
`НАЧАЛОПЕРИОДА(&Параметр, МЕСЯЦ)` is refused as a virtual-table period
because parameters are accepted there only at the top level.

## What Changes

- The lexer SHALL recognize `КОНЕЦПЕРИОДА`/`ENDOFPERIOD`,
  `ДОБАВИТЬКДАТЕ`/`DATEADD`, and `РАЗНОСТЬДАТ`/`DATEDIFF` as keywords that
  remain usable as identifiers.
- The compiler SHALL accept `КОНЕЦПЕРИОДА(дата, период)` with the nine
  periods of `НАЧАЛОПЕРИОДА` and render the last second of the period.
- The compiler SHALL accept `ДОБАВИТЬКДАТЕ(дата, период, число)` with ten
  periods (`СЕКУНДА` added) and any numeric expression as the count,
  reproducing the platform's rounding: the count is rounded for
  `СЕКУНДА`…`МЕСЯЦ` and truncated for `ДЕКАДА`, `КВАРТАЛ`, `ПОЛУГОДИЕ`,
  and `ГОД`.
- The compiler SHALL accept `РАЗНОСТЬДАТ(дата1, дата2, единица)` with
  `СЕКУНДА`, `МИНУТА`, `ЧАС`, `ДЕНЬ`, `МЕСЯЦ`, `КВАРТАЛ`, `ГОД` and count
  the unit boundaries crossed between the dates, as the platform does on
  both providers.
- Virtual-table period arguments SHALL accept `КОНЕЦПЕРИОДА`,
  `ДОБАВИТЬКДАТЕ`, and `&Параметр` nested in any of the date functions.
- A period the function does not accept (`НАЧАЛОПЕРИОДА(…, СЕКУНДА)`,
  `РАЗНОСТЬДАТ(…, НЕДЕЛЯ)`) SHALL be a `Syntax` diagnostic; an unknown
  period name stays `UnsupportedFeature`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: three new bilingual keywords.
- `query-repl`: end of period, date shift, date difference, and the widened
  virtual-table period argument.

## Impact

- `src/lexer.rs`, `src/query/core/ast.rs`, `parser.rs`,
  `codegen/expression.rs`, `codegen/sources.rs`, `codegen/select.rs`,
  `codegen/virtual_tables.rs`, `dialect.rs`.
- CLI completion list; README and `docs/query-language-support.md`.
- No new dependencies.
