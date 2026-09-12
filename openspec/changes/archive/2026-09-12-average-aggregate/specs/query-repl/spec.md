## MODIFIED Requirements

### Requirement: Aggregate arbitrary scalar expressions
`СУММА`, `СРЕДНЕЕ`, `МИНИМУМ`, `МАКСИМУМ`, and `КОЛИЧЕСТВО([РАЗЛИЧНЫЕ] …)`
SHALL accept any scalar expression as their argument. `СУММА`, `СРЕДНЕЕ`,
and `КОЛИЧЕСТВО` SHALL report a number kind; `МИНИМУМ`/`МАКСИМУМ` SHALL
report the argument's kind and SHALL aggregate the payload of a reference
expression on both providers. `СРЕДНЕЕ` SHALL render as `AVG` and SHALL
refuse `РАЗЛИЧНЫЕ` and `*` as `СУММА` does. Aggregates nested in aggregates
SHALL fail with a positional diagnostic.

#### Scenario: Conditional sum
- **WHEN** a query projects `СУММА(ВЫБОР КОГДА Вид = ЗНАЧЕНИЕ(…) ТОГДА Сумма ИНАЧЕ 0 КОНЕЦ)`
- **THEN** generated SQL contains `SUM(CASE WHEN … END)` and the column kind
  is number

#### Scenario: Distinct count of an expression
- **WHEN** a query projects `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ))`
- **THEN** generated SQL contains `COUNT(DISTINCT …)` over the period
  expression

#### Scenario: Grouped average
- **WHEN** a query projects `СРЕДНЕЕ(Т.Цена * Т.Количество) КАК Среднее`
  in a grouped branch
- **THEN** generated SQL contains `AVG(…)` over the product and the column
  kind is number
