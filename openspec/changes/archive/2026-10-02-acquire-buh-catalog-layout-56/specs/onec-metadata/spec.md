## ADDED Requirements

### Requirement: Acquire measured buh catalog layout
The SDK SHALL acquire complete ordered own attribute identities and existing catalog runtime metadata from measured native Config class 56 with exactly 61 entries, subject to existing owner and collection validity checks. Unmeasured lengths SHALL remain unknown. A common module with the same name SHALL NOT acquire catalog membership.

#### Scenario: External users catalog
- **WHEN** the native buh ExternalUsers catalog has measured class 56/61
- **THEN** its eight own attribute identities are acquired in native XML order, including AuthorizationObject

#### Scenario: Unknown class length
- **WHEN** a class 56 catalog has an unmeasured class length
- **THEN** catalog membership and runtime metadata remain unknown

#### Scenario: Same-name common module
- **WHEN** a common module has the same name as a catalog
- **THEN** its descriptor does not acquire catalog membership
