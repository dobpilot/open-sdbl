## ADDED Requirements

### Requirement: Discover extension and service sources
Metadata discovery and completion SHALL surface change-registration,
calculation-kind dependency, and extra-dimension sources under their
owning objects, and SHALL list extension-added attributes alongside base
attributes of the extended object.

#### Scenario: Completing a change-registration source
- **WHEN** the user completes a FROM clause for a registered object
- **THEN** the change-registration source spelling is offered

#### Scenario: Describing an extended object
- **WHEN** the user describes an object extended by a configuration
  extension
- **THEN** the output lists extension-added attributes with their
  extension origin
