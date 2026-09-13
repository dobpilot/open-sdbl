## MODIFIED Requirements

### Requirement: Expose structured output column kinds
Every compiled query SHALL describe each output column with its emitted label
and a structured `ColumnKind`: a reference with resolved target object IDs and
a runtime-typed flag, binary with optional length, string with optional length,
number with optional precision and scale, boolean, date-time, UUID, the `NULL`
literal, the `НЕОПРЕДЕЛЕНО` literal, a type value, or an unknown catalog type
carrying its raw type name. Kinds SHALL be derived from the resolved live
catalog and SchemaStorage without database round trips, and every physical
member of a queryable field SHALL expose the same kind. A type value SHALL be
encoded as five bytes, the platform's `_TYPE` tag followed by the big-endian
`RTRef` table number, and the public `TypeValue` codec SHALL decode and encode
that representation and name the type through a snapshot.

#### Scenario: Numeric catalog column
- **WHEN** a projected column is declared as `numeric(10,2)` on PostgreSQL or
  MSSQL
- **THEN** the compiled column kind is a number with precision 10 and scale 2

#### Scenario: Reference field
- **WHEN** a projected field is a SchemaStorage reference to one catalog
- **THEN** the compiled column kind is a reference whose targets contain that
  catalog's object ID and whose runtime-typed flag is false

#### Scenario: Unknown catalog type
- **WHEN** a projected column has a catalog type the compiler does not
  classify
- **THEN** the compiled column kind is unknown and carries the raw type name

#### Scenario: Type value column
- **WHEN** a projected column is `ТИПЗНАЧЕНИЯ(Т.Объект)`
- **THEN** the compiled column kind is `Type`, and `TypeValue::decode` turns
  the five bytes `0x08` + `RTRef` into the referenced object
