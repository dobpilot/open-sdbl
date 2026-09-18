## ADDED Requirements

### Requirement: Correspondence of accounting turnovers
`РегистрБухгалтерии.<Имя>.Обороты` SHALL expose `КорСчет`
(`BalancedAccount`), `<Измерение>Кор` (`<Dimension>Balanced`) for each
non-balance dimension and `КорСубконто<k>`/`ВидКорСубконто<k>`
(`BalancedExtDimension<k>`/`BalancedExtDimensionType<k>`): for a debit
row the credit side's values, for a credit row the debit side's. The
`КорСубконто` argument SHALL list the kinds of the correspondence the
way `Субконто` lists the account's, excluding the records whose other
side lacks the kind. `УсловиеКорСчета` SHALL be compiled with the
account condition and the condition, over the same names. An unread
correspondence SHALL be summed away.

#### Scenario: Turnovers with the correspondent account
- **WHEN** `ВЫБРАТЬ О.Счет, О.КорСчет, О.СуммаОборотДт ИЗ РегистрБухгалтерии.Хозрасчетный.Обороты(&Н, &К, , , , , НЕ КорСчет В (&Счета), ) КАК О`
  is compiled
- **THEN** the debit branch projects the credit account as the
  correspondence and tests it against the list, the credit branch the
  debit account, and the outer aggregation groups by both accounts

#### Scenario: Listed balanced kind
- **WHEN** the `КорСубконто` argument lists a kind
- **THEN** each branch picks the value by the opposite side's kinds
