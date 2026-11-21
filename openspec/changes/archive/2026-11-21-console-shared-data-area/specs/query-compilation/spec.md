## MODIFIED Requirements

### Requirement: Compile using current global separation conditions
The compiler SHALL accept host-provided boolean conditions keyed by constant GUID, separately from named session parameters. An applicable separator lacking its condition SHALL report the condition GUID structurally without claiming a missing session parameter. False conditions SHALL select the empty String, Number or Date area without requiring use/value parameters. Unsupported conditional empty types and unsupported settings of a named separator SHALL fail explicitly instead of assuming an absent condition. True conditions and separators without conditions SHALL retain existing mode semantics and rights/RLS.

#### Scenario: Missing applicable condition
- **WHEN** a bound compilation reads a table with a conditional separator and no supplied condition
- **THEN** it reports the constant GUID structurally

#### Scenario: False condition
- **WHEN** a supplied condition is false for an Independent or Shared String/Number separator
- **THEN** the predicate selects the empty string or zero and ignores missing use/value parameters

#### Scenario: Condition changes
- **WHEN** the host changes a supplied condition before another compilation
- **THEN** the next compilation uses the new condition

#### Scenario: Preparation and unrelated tables
- **WHEN** the compiler prepares a query or reads a table without the separator
- **THEN** it does not require that condition value

#### Scenario: Unsupported named settings
- **WHEN** an applicable named separator has an unrecognized settings shape
- **THEN** bound compilation refuses the read instead of treating its condition as nil


#### Scenario: False date condition
- **WHEN** a conditional Date separator has a false condition
- **THEN** the predicate selects the empty date 0001-01-01 with the backend year offset applied
