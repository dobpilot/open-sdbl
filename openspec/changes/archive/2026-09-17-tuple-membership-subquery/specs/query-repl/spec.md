## ADDED Requirements

### Requirement: Compile tuple membership tests
`(<expression>, <expression>, …) [НЕ] В (<query>)` SHALL compile when
the subquery projects one column per tuple item of a compatible kind:
the test SHALL render as `EXISTS` over the subquery with an equality per
column, `NOT EXISTS` when negated, on both dialects, and SHALL be
accepted wherever a predicate is, including the condition of a virtual
table. A tuple anywhere else, a column count that differs from the
tuple, an incompatible column, or a reference of several types on either
side SHALL be an `UnsupportedFeature` diagnostic.

#### Scenario: Pair in a slice condition
- **WHEN** `РегистрСведений.Цены.СрезПоследних(&Дата, (Номенклатура, Характеристика) В (ВЫБРАТЬ С.Номенклатура, С.Характеристика ИЗ Документ.Заказ.Товары КАК С ГДЕ С.Ссылка = &Заказ))` is compiled
- **THEN** the slice's condition holds `EXISTS (SELECT 1 FROM (…) AS "__in" WHERE "__in"."Номенклатура" = … AND "__in"."Характеристика" = …)`

#### Scenario: Column count mismatch
- **WHEN** a two-item tuple is tested against a one-column subquery
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic
