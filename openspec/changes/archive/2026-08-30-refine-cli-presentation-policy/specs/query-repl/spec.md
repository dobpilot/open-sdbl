## ADDED Requirements

### Requirement: Provide kind-specific default CLI presentations
The console's default presentation provider SHALL select a structured template
by the requested target object's resolved metadata kind. A catalog with live
Description and Code fields SHALL use `Наименование (Код)`. A document with
live Number and Date fields SHALL use `<Тип> <Номер> от <Период>`, where
`<Тип>` is the Russian Config synonym falling back to the metadata name, and
`<Период>` is the standard document `Дата`/`Date` field. Missing optional
fields SHALL use deterministic non-failing fallbacks.

#### Scenario: Catalog reference
- **WHEN** the CLI resolves a catalog target exposing Description and Code
- **THEN** its structured plan concatenates Description, `" ("`, Code, and
  `")"`

#### Scenario: Document reference
- **WHEN** the CLI resolves a document target exposing Number and Date
- **THEN** its structured plan concatenates localized document type, `" "`,
  Number, `" от "`, and Date

#### Scenario: Internal callback identities
- **WHEN** either default template is returned to the core
- **THEN** every field remains a numeric standard-field ID and only separator
  and type presentation text is represented as a literal
