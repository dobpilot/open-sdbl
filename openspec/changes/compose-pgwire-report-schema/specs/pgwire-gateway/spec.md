## ADDED Requirements

### Requirement: Follow an attribute path from a 1C object link
The gateway SHALL support `onec_get_attribute_text(link, path)`. Each
non-final path segment SHALL select a typed reference and the final segment
SHALL select a value rendered as text. Each read SHALL use the caller's
object, field, row, and data area rules. It SHALL project only the requested
attribute at each step.

#### Scenario: Resolve the name of a report
- **WHEN** a report variant's `Отчет` refers to a metadata identifier with
  `ПолноеИмя` equal to `Отчет.Запасы`
- **THEN** `onec_get_attribute_text(link, 'Отчет.ПолноеИмя')` returns
  `Отчет.Запасы`

#### Scenario: A linked row is excluded
- **WHEN** a link is empty, a row is absent, or a row filter excludes it
- **THEN** the function returns SQL NULL
- **WHEN** an object or field read is denied
- **THEN** the function returns the access error

### Requirement: Resolve a metadata reference property
The gateway SHALL support `onec_get_metadata_ref(name, property)`,
initially for `ОсновнаяСхемаКомпоновкиДанных` on reports. It SHALL return
the configured template's logical path or SQL NULL for a known unset
property. Unsupported or unresolvable properties SHALL return an error.

#### Scenario: The main schema has a custom name
- **WHEN** a report's configured main schema points to the template
  `ВыбраннаяСхема`
- **THEN** the function returns its path, independently of other template names

### Requirement: Compose text service calls
Service calls SHALL accept bounded nested scalar text service calls in
argument positions in both query protocols. Prepared statements SHALL
describe their positional text arguments and outer result type before
execution. A nested SQL NULL SHALL propagate through dependent calls.

#### Scenario: Load a report variant's main schema
- **WHEN** a client passes the result of the attribute lookup into metadata
  reference resolution and passes that result into `onec_show_metadata`
- **THEN** the gateway returns the configured schema XML in one query

#### Scenario: Null intermediate reference
- **WHEN** a nested lookup returns SQL NULL
- **THEN** its calling function returns SQL NULL without reading its resource
