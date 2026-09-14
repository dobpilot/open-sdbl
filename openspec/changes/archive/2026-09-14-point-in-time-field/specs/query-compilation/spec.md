## ADDED Requirements

### Requirement: The point-in-time standard field
`МоментВремени` / `PointInTime` SHALL resolve on a document and on a
register record that carries a recorder, as the pair of that row's date
(`Дата` of a document, `Период` of a register record) and its reference
(`Ссылка` of a document, `Регистратор` of a register record). The pair
SHALL occupy two output columns, the date first under `<имя>_T` and the
reference second under `<имя>`, and SHALL expand into both members
wherever the field is used as a whole. A source that carries no such pair
SHALL keep reporting the field as unknown, as the platform does.

#### Scenario: Point in time of a document
- **WHEN** `ВЫБРАТЬ Д.МоментВремени КАК М ИЗ Документ.X КАК Д` is
  compiled
- **THEN** the date column and the reference column of the document are
  both projected, labelled `М_T` and `М`

#### Scenario: Point in time of a register record
- **WHEN** `ВЫБРАТЬ Р.МоментВремени ИЗ РегистрНакопления.X КАК Р` is
  compiled
- **THEN** the period column and the recorder of the record are both
  projected

#### Scenario: Ordering by a point in time
- **WHEN** `УПОРЯДОЧИТЬ ПО Д.МоментВремени` is compiled
- **THEN** the ordering lists the date term before the reference term

#### Scenario: A source without a point in time
- **WHEN** `ВЫБРАТЬ С.МоментВремени ИЗ Справочник.X КАК С` is compiled
- **THEN** the field is reported as unknown
