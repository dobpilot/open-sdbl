## ADDED Requirements

### Requirement: Resolve metadata right structure before grant evaluation
The SDK SHALL resolve known bilingual case-insensitive right names and acquired catalog owner identities independently of role grants or privileged mode. Supported owner Read/View and Code/Description Read/View targets SHALL distinguish Applicable and NotApplicable. Valid standard Read SHALL be NotApplicable. Empty or unknown standard paths, unknown owners, unsupported kinds/rights and missing acquired standard settings SHALL return explicit errors and SHALL NOT become granted or inapplicable.

#### Scenario: Valid standard Read
- **WHEN** a catalog Code or Description with acquired positive configured length is resolved for Read
- **THEN** resolution succeeds with NotApplicable without reading role grants

#### Scenario: Privileged caller validates structure
- **WHEN** a caller intends to bypass grants but supplies an unknown right or empty or unknown path
- **THEN** resolution fails before any bypass decision

#### Scenario: Unsupported metadata
- **WHEN** the owner kind or required standard declaration is unsupported or absent
- **THEN** resolution fails explicitly rather than guessing applicability

### Requirement: Evaluate cached metadata rights per complete role
For an applicable resolved target, AccessStore SHALL evaluate already acquired rights only and require complete readable rights for every requested role. Owner Read/View SHALL use the existing RoleRights object grant rule. For AccessRight, standard View SHALL gate the owner within each role, then apply the exact member pair and explicit right or the attribute default, and combine whole-role results with OR. A separate verification evaluator SHALL preserve the measured independent-child distinction: with independent child rights enabled and attribute defaults disabled, an explicit child View grant MAY pass VerifyAccessRights despite a refused owner View. Unmeasured independent/default combinations without a proven member result SHALL fail explicitly. The SDK SHALL NOT combine owner and child permissions across roles. It SHALL NOT reinterpret this metadata API as SQL/RLS field authorization.

#### Scenario: Split parent and child grants
- **WHEN** one role grants owner View but refuses Code View and another grants Code View but refuses owner View
- **THEN** Code View is refused

#### Scenario: Complete second role
- **WHEN** another role grants both owner View and Code View
- **THEN** Code View is granted

#### Scenario: Explicit refusal overrides defaults
- **WHEN** a role grants owner View and attribute defaults but explicitly refuses Description View
- **THEN** that role refuses Description View while a missing Code entry follows the attribute default

#### Scenario: Missing role data
- **WHEN** any requested role is missing or unreadable even if another role grants
- **THEN** evaluation fails explicitly

#### Scenario: Inapplicable target is not a denial
- **WHEN** role evaluation is requested for NotApplicable
- **THEN** evaluation fails explicitly and returns no boolean

#### Scenario: Independent child differs between operators
- **WHEN** a role refuses owner View, explicitly grants Code View, enables independent child rights and disables attribute defaults
- **THEN** metadata_right refuses Code View but metadata_right_for_verification grants it
