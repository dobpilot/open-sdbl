## Why

`ГДЕ Д.Товары.Количество > 2` names a column of a tabular section inside a
predicate. The compiler reports `field "Товары" was not found`, and a
corpus query of a real configuration stops there — it asks
`Задача.ЗадачаИсполнителя.Предметы.Предмет = Файлы.Ссылка` inside a
correlated subquery.

The platform accepts it and answers with existence, not with a join.
Measured on the probe base against 8.3.27 over five documents carrying one
row each plus one carrying two rows that both satisfy the condition: the
query answers four documents and `КОЛИЧЕСТВО(*)` answers four. A join
would have answered five, because the two-row document would come twice.

## What Changes

- Compile a comparison whose operand is `<источник>.<Состав>.<Поле>` into
  an `EXISTS` over the section, correlated to the owner row.
- Keep refusing the path anywhere else — in a projection, a grouping key,
  an ordering key — where its meaning is not existence.

## Capabilities

### Modified Capabilities

- `query-repl`: a predicate may test a column of a tabular section.

## Impact

One corpus query compiles. Generated SQL gains a correlated `EXISTS` for
such a predicate; nothing else changes.
