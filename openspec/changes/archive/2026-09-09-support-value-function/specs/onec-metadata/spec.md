## ADDED Requirements

### Requirement: Acquire predefined-value metadata

The application adapters SHALL load part-zero bare-GUID Config resources and
part-zero `<guid>.1c` resources. The core SHALL decode only verified `.1c`
predefined-value rows and associate every value with the catalog GUID encoded
in its file name. Other Config suffixes SHALL remain excluded.

#### Scenario: Catalog predefined values
- **WHEN** `<catalog-guid>.1c` contains verified predefined rows
- **THEN** each exact symbolic name and stable GUID is associated with that
  catalog without reading catalog business rows

#### Scenario: Unrelated suffix resource
- **WHEN** a Config file has a suffix other than `.1c`
- **THEN** predefined-value decoding returns no values and does not interpret
  the payload as metadata

### Requirement: Resolve predefined values deterministically

The metadata snapshot SHALL index exact normalized predefined names by owning
object. Enumeration child descriptors and catalog `.1c` rows SHALL share the
same lookup contract. Missing and ambiguous values SHALL remain distinct typed
outcomes.

#### Scenario: Enumeration value
- **WHEN** a child descriptor belongs to a resolved enumeration resource
- **THEN** its metadata GUID and name are indexed under that enumeration

#### Scenario: Ambiguous value
- **WHEN** two values normalize to the same name under one owner
- **THEN** lookup reports ambiguity instead of selecting one by source order

### Requirement: Convert metadata GUIDs to 1C storage bytes

The core SHALL convert canonical UUID fields `a-b-c-d-e` to physical 1C byte
order `d + e + c + b + a` without changing byte order inside a field.

#### Scenario: Known enumeration GUID
- **WHEN** GUID `d2f8bde9-fadd-4be8-9022-249e3a1ac4b9` is converted
- **THEN** the bytes are `9022249e3a1ac4b94be8faddd2f8bde9`
