## ADDED Requirements

### Requirement: Expose receipts and expenses on turnovers
`РегистрНакопления.<Имя>.Обороты` of a balance register SHALL expose,
per resource, `<Ресурс>Приход` (`<Resource>Receipt`) — the sum of the
resource over the receipt records of the interval — and `<Ресурс>Расход`
(`<Resource>Expense`) — the sum over the expense records — beside
`<Ресурс>Оборот`, and SHALL sum them over the dimensions the statement
never reads like every resource column. A turnover-only register SHALL
keep exposing the turnover only.

#### Scenario: Receipts of an order
- **WHEN** `ВЫБРАТЬ О.КоличествоПриход, О.КоличествоРасход ИЗ РегистрНакопления.Заказы.Обороты(, , , Заказ = &Заказ) КАК О` is compiled
- **THEN** the SQL sums the resource over records with `_RecordKind = 0`
  for the receipt and `_RecordKind = 1` for the expense

#### Scenario: Turnover-only register
- **WHEN** `О.КоличествоПриход` is read from a turnover-only register
- **THEN** compilation fails with an `UnknownField` diagnostic
