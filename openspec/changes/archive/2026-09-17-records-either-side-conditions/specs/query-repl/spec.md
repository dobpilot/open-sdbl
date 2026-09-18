## ADDED Requirements

### Requirement: Two-sided condition of the records table
The condition of `РегистрБухгалтерии.<Имя>.ДвиженияССубконто` SHALL
accept, besides the record's fields, the side-less names `Счет`,
`Субконто<k>`, `ВидСубконто<k>` and the names of the non-balance
dimensions and resources; a condition reading any of them SHALL select
the records for which it holds with the debit fields or with the credit
fields substituted. A condition reading none of them SHALL be compiled
once.

#### Scenario: Account of either side
- **WHEN** `ДвиженияССубконто(&Н, &К, Организация = &О И Счет = &С)` is
  compiled
- **THEN** the predicate is the organisation test with the debit account
  test, `OR` the organisation test with the credit account test

#### Scenario: Dereference through a two-sided name
- **WHEN** the condition reads `Счет.Код`
- **THEN** the debit and the credit account are each joined to the chart
  of accounts
