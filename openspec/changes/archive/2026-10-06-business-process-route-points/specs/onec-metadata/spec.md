## ADDED Requirements

### Requirement: Complete business-process route points
The SDK SHALL expose the complete ordered native route-point membership of a recognized business process using existing GUID identities and explicitly retained ownership, including each member name and owner relationship. Verified empty membership SHALL remain distinct from unknown or incomplete membership. The generated route-point reference type identity SHALL be separate from owner and member identities. The acquisition order SHALL NOT imply BSL iteration order.

#### Scenario: Native complete control
- **WHEN** a recognized business process contains several named route points
- **THEN** every acquired member identity and name matches native XML dumped from the same configuration

#### Scenario: Known empty collection
- **WHEN** a recognized native business process contains no route points
- **THEN** its acquired collection is known empty rather than unknown

#### Scenario: Incomplete or unknown member
- **WHEN** any member or collection form is incomplete or unsupported
- **THEN** no partial collection is published as complete

#### Scenario: Separate owners and generated type
- **WHEN** two processes contain identically named points
- **THEN** ownership and member identities remain distinct even when copied members retain the same GUID, and neither replaces the generated reference type identity
