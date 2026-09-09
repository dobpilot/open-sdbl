## ADDED Requirements

### Requirement: Resolve extension attributes from the restructure resource
Given the extension restructure resource
(`_ExtensionsRestruct._restructData`), resolution SHALL map each
extension-added attribute to its physical extension column, expose it as
a queryable field of the owning object with its logical name and
extension origin, and record its type. Malformed restructure records
SHALL be reported, not panicked on, and SHALL NOT drop the surrounding
object.

#### Scenario: Extension attribute becomes queryable
- **WHEN** a base is resolved together with an extension whose
  restructure declares an attribute on an existing object
- **THEN** the owning object exposes that attribute as a queryable field
  mapped to the extension table's physical column, and a query selecting
  it compiles on both dialects instead of failing with a missing-field
  diagnostic

#### Scenario: Wildcard includes extension attributes
- **WHEN** a query selects all fields of an extended object
- **THEN** the compiled projection includes the extension attribute
  columns from the extension table

#### Scenario: Malformed restructure record
- **WHEN** an extension restructure record cannot be interpreted
- **THEN** resolution records a typed finding and the object's remaining
  fields still resolve
