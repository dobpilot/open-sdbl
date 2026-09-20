## MODIFIED Requirements

### Requirement: Restriction condition in the platform's full form
A restriction condition SHALL be accepted in the platform's full form:
an optional leading `ТекущаяТаблица`, an optional alias after `КАК`, the
join clauses of the form, an optional `ГДЕ`, then the condition.
`ТекущаяТаблица` and the alias SHALL qualify the fields of the restricted
table, in a nested query of the condition as well, while an unqualified
field keeps resolving against it.

A join clause SHALL name a metadata table, take an alias, and carry its
condition after `ПО`; a nested query, a parameter table, a temporary
table, a constants table or a criterion SHALL be refused with a
`Restriction` diagnostic. The joins SHALL compile as one correlated
predicate of the restricted row — the joined tables read under their
aliases with their data-separator predicates, the conditions of the
clauses in their `ON`, and the restriction's own condition after them —
so that a row of the restricted table passes when its joined rows carry
at least one row satisfying the condition, an outer join contributing its
unmatched row. The restricted table SHALL stay a plain derived table, so
no join multiplies its rows.

#### Scenario: Correlated key check
- **WHEN** the restriction is `ТекущаяТаблица ГДЕ ИСТИНА В (ВЫБРАТЬ ПЕРВЫЕ 1 ИСТИНА ИЗ Справочник.Y КАК К ГДЕ К.Объект = ТекущаяТаблица.Ссылка)`
- **THEN** the nested query compares with the restricted source's column

#### Scenario: Alias
- **WHEN** the restriction is `ТекущаяТаблица КАК Т ГДЕ Т.Организация = &Орг`
- **THEN** it compiles as `Организация = &Орг` would

#### Scenario: Join
- **WHEN** the restriction is
  `ТекущаяТаблица ЛЕВОЕ СОЕДИНЕНИЕ РегистрСведений.Y КАК К ПО ТекущаяТаблица.Ссылка = К.Объект ГДЕ К.Поле = &Значение`
- **THEN** the derived table of the restricted source carries a predicate
  reading the joined register under the alias `К`, correlated with the
  restricted row, and its rows are not multiplied

#### Scenario: A join reading something that is not a table
- **WHEN** a join clause names a temporary table or a nested query
- **THEN** compilation fails with a `Restriction` diagnostic
