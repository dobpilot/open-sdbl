## ADDED Requirements

### Requirement: An alias written without `КАК`
A projection SHALL accept an alias written without `КАК`, naming the
output column exactly as the `КАК` form names it, and one word only. A
word that opens the next clause SHALL never be read as such an alias,
neither in a projection nor after a source. A wildcard projection SHALL
keep refusing an alias in either form.

#### Scenario: A projection alias without `КАК`
- **WHEN** `ВЫБРАТЬ Т.Поле Имя ИЗ Справочник.X КАК Т` is compiled
- **THEN** the column is named `Имя`

#### Scenario: A clause keyword is not an alias
- **WHEN** `ВЫБРАТЬ 1 КАК Ч ИЗ Справочник.X КАК Т ИТОГИ СУММА(Ч) ПО ОБЩИЕ`
  is compiled
- **THEN** `ИТОГИ` opens the totals clause instead of naming the source

#### Scenario: A wildcard keeps refusing an alias
- **WHEN** `ВЫБРАТЬ * Имя ИЗ Справочник.X КАК Т` is compiled
- **THEN** the alias is refused
