## ADDED Requirements

### Requirement: Acquire complete document numbering settings
The metadata API SHALL expose an optional complete configured numbering-settings value for an own document descriptor. An acquired value SHALL contain the measured NumberPeriodicity and either the numerator configuration-object GUID or verified absence of a numerator. Unknown, malformed or unmeasured values SHALL leave the entire numbering-settings acquisition unknown; unknown absence SHALL NOT be represented as a verified empty numerator. Existing number type, length and allowed-length acquisition SHALL remain unchanged. Acquisition SHALL NOT promise number reservation or shared-numerator runtime behavior.

#### Scenario: Measured periodicity and no numerator
- **WHEN** native XML and Config establish a supported periodicity and a nil numerator reference
- **THEN** the descriptor exposes that periodicity and a verified absent numerator inside an acquired settings value

#### Scenario: Shared numerator identity
- **WHEN** two native documents refer to the same document numerator
- **THEN** both expose the exact numerator configuration-object UUID, rather than a generated type or document UUID

#### Scenario: Unknown or incomplete settings
- **WHEN** the class shape, periodicity code or numerator representation is unknown or malformed
- **THEN** complete numbering settings are unknown, without a fabricated default or absence

#### Scenario: Existing number settings remain independent
- **WHEN** a document has known number type, length and allowed length but an unknown periodicity
- **THEN** existing number settings remain acquired while complete numbering settings remain unknown
