## ADDED Requirements

### Requirement: Read a document journal
`ЖурналДокументов.<Имя>` / `DocumentJournal.<Name>` SHALL be a source. Its
standard fields SHALL be `Ссылка` — the reference of the registered
document, stored as one column for a journal of a single document kind and
as the `RTRef ‖ RRRef` pair otherwise — together with `Тип`, which answers
the type value of that reference, and `Дата`, `Номер`, `ПометкаУдаления`
and `Проведен`. The journal's own columns SHALL answer under their
metadata names.

#### Scenario: Projection of a journal
- **WHEN** `ВЫБРАТЬ Ж.Ссылка, Ж.Дата, Ж.Номер, Ж.Клиент ИЗ
  ЖурналДокументов.ЖурналПродаж КАК Ж` is compiled
- **THEN** each field reads its column of the journal table

#### Scenario: Type of the registered document
- **WHEN** `Ж.Тип` is read
- **THEN** it answers the type value of the journal's reference, which
  compares with `ТИП(Документ.X)`

#### Scenario: Dereference through the journal reference
- **WHEN** `Ж.Ссылка.Дата` is read from a journal of a single document kind
- **THEN** the document is joined and its field answers
