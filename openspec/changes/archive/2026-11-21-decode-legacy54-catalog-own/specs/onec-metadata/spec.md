## ADDED Requirements

### Requirement: Preserve measured legacy54 catalog own declarations
The decoder SHALL preserve complete ordered own membership and qualified declarations for the measured class54/59 catalog profile without extending other profile admission or inferring unknown settings.

#### Scenario: Native data-load sessions catalog
- **GIVEN** the whole native CF/XML profile with two DateTime own attributes and an empty section collection
- **WHEN** the resource is decoded
- **THEN** both ordered UUIDs and declarations match native XML and section membership is known empty

#### Scenario: Malformed own list
- **GIVEN** a malformed late row, identity, type, count, tail, descriptor or duplicate in the measured own list
- **WHEN** the resource is decoded
- **THEN** complete own membership and declarations remain unknown without a published prefix

#### Scenario: Unmeasured nonempty section layout
- **GIVEN** a nonempty section collection in a class54 resource
- **WHEN** only the measured empty-section profile is available
- **THEN** section membership remains unknown rather than fabricated empty
