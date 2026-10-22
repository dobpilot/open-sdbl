## ADDED Requirements

### Requirement: Resolve acquired Document rights targets
The SDK SHALL resolve acquired Document owner Read/View and Number/Date standard Read/View independently of grants and privileged mode. It SHALL expose an additive owner-bound ordinary-own-attribute resolver requiring complete acquired membership and a uniquely acquired child descriptor. Valid child Read SHALL be NotApplicable; owner Read/View and child View SHALL be Applicable. Standard aliases SHALL be bilingual and case-insensitive. Unknown owners/rights/paths, nonmember attributes, incomplete or ambiguous descriptions, unsupported shapes and unknown or zero-length Number settings SHALL produce explicit structural errors rather than guessed grants or applicability. Other metadata kinds and rights SHALL retain their existing behavior.

#### Scenario: Valid ordinary attribute Read
- **WHEN** a Document attribute belongs to its complete acquired own-attribute list and has a unique acquired descriptor
- **THEN** resolving Read succeeds with NotApplicable independently of role grants

#### Scenario: Validate before privilege policy
- **WHEN** a caller supplies a foreign or nonmember attribute UUID, unknown path or incomplete descriptor while planning a privileged bypass
- **THEN** structural resolution fails before bypass is possible

#### Scenario: Standard aliases
- **WHEN** an acquired Document with positive Number length is checked through Number/Номер or Date/Дата in a different case
- **THEN** the corresponding standard target is identical

### Requirement: Evaluate retained Document child role profiles
The existing cached role evaluator SHALL use exact compiled standard member pairs and ordinary-attribute rights object UUIDs, never fabricated owner-member paths. AccessRight child View SHALL require owner View within the same role before explicit child or attribute-default evaluation. Document VerifyAccessRights SHALL preserve retained independent-child profiles: with refused owner View it SHALL accept explicit child grants with defaults disabled and absent-child grants with defaults enabled when independent child rights are enabled. Explicit refusal SHALL override defaults in supported profiles; independent/default-enabled targets without parent View but with an explicit child record SHALL remain explicitly unsupported until measured. Whole-role results SHALL be combined only after evaluation, and every requested role SHALL have complete readable rights. Existing Catalog unsupported independent/default combinations and SQL/RLS authorization SHALL remain unchanged.

#### Scenario: Explicit refusal overrides enabled defaults
- **WHEN** owner View and attribute defaults are enabled but Number View is explicitly refused
- **THEN** that role refuses Number View

#### Scenario: Independent default verification
- **WHEN** Document owner View is refused, child rights are independent, defaults are enabled and the child has no explicit entry
- **THEN** AccessRight refuses and VerifyAccessRights accepts child View

#### Scenario: Ordinary attribute own identity
- **WHEN** an ordinary Document attribute is evaluated
- **THEN** its explicit right is obtained from its own acquired UUID rights object with an empty member path

#### Scenario: Missing role remains an error
- **WHEN** one requested role is missing despite another granting the target
- **THEN** cached evaluation fails explicitly
