## ADDED Requirements

### Requirement: Default console to the shared data area
The console SHALL bind conditional separation constants to false by default, matching the gateway shared-area policy. It SHALL use the current metadata after refresh, preserve other session parameters. For false conditional separators it SHALL select the typed empty number, string or date value supported by the compiler.

#### Scenario: Query a conditional separator
- **WHEN** a console query reads a conditionally separated table without an explicitly bound condition
- **THEN** the console compiles the query for the shared data area

#### Scenario: Metadata refresh
- **WHEN** metadata is refreshed and conditional separator constants change
- **THEN** subsequent compilations use the refreshed constants

### Requirement: Name separation constants in console diagnostics
When a console diagnostic identifies a separation condition constant by GUID and the current snapshot has a name for it, the console SHALL display both the name and GUID. It SHALL preserve the original diagnostic when the name is unknown.

#### Scenario: Known condition constant
- **WHEN** a diagnostic identifies a condition whose metadata name is known
- **THEN** the console displays that name alongside its GUID
