## ADDED Requirements

### Requirement: The name of the movement-type standard field
The `RecordKind` standard field of an accumulation register SHALL also be
named `ВидДвижения` and `RecordType`, the spellings the platform accepts.

#### Scenario: Reading the movement type by its query name
- **WHEN** `ВЫБРАТЬ Р.ВидДвижения ИЗ РегистрНакопления.X КАК Р` is
  compiled
- **THEN** the `RecordKind` column is projected
