## ADDED Requirements

### Requirement: Compile grouped branches
The compiler SHALL accept `СГРУППИРОВАТЬ ПО <keys>` / `GROUP BY` after the
filter of a branch and an optional `ИМЕЮЩИЕ <predicate>` / `HAVING` after
it. A key SHALL be a one-hop field path, a projection alias, or an
expression textually equal to a projected expression. Every non-aggregate
projection SHALL match a key, otherwise compilation SHALL fail with a
positional diagnostic. Generated SQL SHALL group by every physical member of
a reference key and by the physical columns of inline presentations of
keys, SHALL compile `ИМЕЮЩИЕ` as a predicate that may contain aggregates, SHALL
accept aggregates inside `ВЫБОР` branches of grouped projections,
SHALL reject aggregates in `ГДЕ`, `ПО`, and keys, SHALL restrict grouped
ordering to keys and projection aliases, and SHALL reject grouping combined
with `ПОЛНОЕ СОЕДИНЕНИЕ`.

#### Scenario: Grouped sum over a reference key
- **WHEN** a query projects `Номенклатура, СУММА(Количество)` and groups by
  `Номенклатура`
- **THEN** both dialects emit `GROUP BY` over the `_RTRef` and `_RRRef`
  members of the field and project the one-column reference payload

#### Scenario: Ungrouped projection
- **WHEN** a grouped query projects a field that is neither aggregated nor a
  key
- **THEN** compilation fails with a positional diagnostic at that field

#### Scenario: Having predicate
- **WHEN** a query groups by `Склад` and filters with
  `ИМЕЮЩИЕ СУММА(Количество) > 100`
- **THEN** generated SQL contains `HAVING SUM(…) > 100` and no aggregate in
  `WHERE`

#### Scenario: Aggregate inside a conditional projection
- **WHEN** a grouped query projects
  `ВЫБОР КОГДА СУММА(Количество) > 0 ТОГДА "Есть" ИНАЧЕ "Нет" КОНЕЦ`
- **THEN** generated SQL contains the `CASE` with `SUM(…)` in its condition

#### Scenario: Dereferenced key with a presentation
- **WHEN** a query groups by `Номенклатура.Родитель` and projects
  `ПРЕДСТАВЛЕНИЕ(Номенклатура.Родитель)`
- **THEN** generated SQL joins the parent through the shared reference join
  and groups by the joined key and presentation columns
