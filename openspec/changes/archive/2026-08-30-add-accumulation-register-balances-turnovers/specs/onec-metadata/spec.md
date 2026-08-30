## ADDED Requirements

### Requirement: Preserve accumulation-register field purpose
The metadata decoder SHALL classify custom accumulation-register fields as
dimensions, resources, or attributes from their enclosing Config collection
GUID and expose that purpose on resolved fields. It SHALL NOT infer these roles
from physical names or index layouts.

#### Scenario: Accumulation-register roles
- **WHEN** descriptors occur under the recognized accumulation-register
  dimension, resource, and attribute collection GUIDs
- **THEN** each resolved field carries the corresponding typed purpose

#### Scenario: Unknown accumulation collection
- **WHEN** no recognized collection encloses a descriptor
- **THEN** its purpose remains unknown instead of being guessed

