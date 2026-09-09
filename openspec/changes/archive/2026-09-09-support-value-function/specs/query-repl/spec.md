## ADDED Requirements

### Requirement: Compile metadata value expressions

The compiler SHALL accept `ЗНАЧЕНИЕ`/`VALUE` with exactly one
`<kind>.<object>.<value>` metadata path for catalog and enumeration kinds. It
SHALL resolve the object and value through the metadata snapshot and permit the
expression in projections and predicates.

#### Scenario: Enumeration value
- **WHEN** a query uses
  `ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус)`
- **THEN** PostgreSQL and MSSQL SQL contain the enumeration GUID in physical 1C
  byte order as a typed binary expression

#### Scenario: Catalog predefined value
- **WHEN** a query uses
  `ЗНАЧЕНИЕ(Справочник.бит_СтатусыОбъектов.Утвержден)`
- **THEN** generated SQL returns `_IDRRef` from the resolved catalog table by
  equality on `_PredefinedID` and the stable metadata GUID

#### Scenario: Hierarchical symbolic name
- **WHEN** a catalog value name contains underscores such as
  `ДополнительныеУсловияПоДоговору_Проверен`
- **THEN** the complete symbolic name is resolved exactly as one path component
  without splitting or inspecting presentation data

#### Scenario: Invalid value expression
- **WHEN** the path shape, kind, object, value, live table, or required physical
  columns are unsupported, absent, or ambiguous
- **THEN** compilation returns a positional diagnostic and emits no SQL
