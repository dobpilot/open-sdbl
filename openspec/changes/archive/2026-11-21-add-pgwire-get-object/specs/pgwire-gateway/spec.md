## ADDED Requirements

### Requirement: Read a 1C object by its navigation link
The gateway SHALL implement `SELECT onec_get_object(link)` for an internal
1C object link of the form `e1cib/data/Kind.Name?ref=<32 hex digits>`.
It SHALL resolve the metadata name in its own information base and use
the reference bytes in their physical 1C order. It SHALL reject malformed
links, unknown objects and types without a queryable object reference.

#### Scenario: Look up a document using its own link
- **WHEN** a client supplies a document's internal navigation link
- **THEN** the gateway reads only the row with that physical reference
  in the resolved document table

#### Scenario: Malformed or external link
- **WHEN** the client supplies a link with an invalid reference, or supplies
  a URL for another information base
- **THEN** the gateway returns an argument error before reading any row

### Requirement: Return one JSON object of the object's attributes
The function SHALL return one `jsonb` column with the main row's queryable
standard and custom attributes keyed by their logical 1C names. Composite
attributes SHALL use their active value. The return value SHALL be SQL NULL
when the object is absent. It SHALL exclude tabular sections.

#### Scenario: Read native values
- **WHEN** the object has text, number, boolean, date and reference attributes
- **THEN** the JSON object contains text, numbers and booleans as their JSON
  types, dates as strings, and references as navigation links when their
  target metadata is known

#### Scenario: Missing object
- **WHEN** the metadata exists but no row matches the link
- **THEN** the result contains one SQL NULL value

### Requirement: Authorize an object lookup as a data query
An object lookup SHALL apply the same object, field, row and data area
checks as an equivalent SDBL query under the current gateway mode. It
SHALL NOT forward the link or SQL call to the backing database as SQL.

#### Scenario: Object or field read is denied
- **WHEN** object or field rights deny the corresponding SDBL read
- **THEN** the function returns the authorization error without attribute data

#### Scenario: Row is filtered by a restriction
- **WHEN** a row or data area restriction excludes the requested object
- **THEN** the result contains SQL NULL, as for a missing row

### Requirement: Bind object links in both query protocols
The function SHALL support string literals in simple queries and a text
argument bound to `$1` in extended queries.

#### Scenario: Prepare a lookup without an object link
- **WHEN** a client prepares `SELECT onec_get_object($1)`
- **THEN** the gateway describes a text argument and a `jsonb` result before
  the link is bound
