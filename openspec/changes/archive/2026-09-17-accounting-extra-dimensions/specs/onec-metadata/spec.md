## ADDED Requirements

### Requirement: Decode predefined items of charts of characteristic types
The application adapters SHALL also load `<guid>.7` resources, and the
core SHALL decode their verified predefined rows as it decodes `.1c`
and `.9` rows, keeping the values for charts of characteristic types
only (measured on the demo Бухгалтерия предприятия base: the `.7`
suffix of other classes carries other content).

#### Scenario: Kinds of extra dimensions
- **WHEN** `<chart-guid>.7` of `ВидыСубконтоХозрасчетные` is decoded
- **THEN** `Контрагенты` and the other kinds are associated with the
  chart by name and GUID

### Requirement: Resolve the extra-dimension values table
The `AccRgED` DBNames entry of an accounting register SHALL resolve to a
service object owned by the register, with its physical table and
declared columns.

#### Scenario: Owned by the register
- **WHEN** the demo register `Хозрасчетный` is resolved
- **THEN** an `AccountingExtraDimensions` object owned by it names
  `_AccRgED<N>`
