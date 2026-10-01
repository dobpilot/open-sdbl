## ADDED Requirements

### Requirement: Authorize measured native member mappings
The SDK SHALL use verified native standard-field and default-attribute permission mappings when available. Explicit denials, missing rights, unresolved fields, unsupported paths and unknown defaults SHALL remain fail closed. Object Read alone SHALL NOT be accepted as proof for unmeasured standard fields. RLS and separation checks SHALL remain applicable.

#### Scenario: Verified standard field
- **WHEN** a field has a measured native member mapping and a selected role grants its required permissions
- **THEN** the field passes without bypassing object or row authorization

#### Scenario: Unsupported mapping
- **WHEN** the field or default rule is not measured
- **THEN** authorization fails before SQL execution

### Requirement: Map verified catalog programmatic reads
For a recognized catalog Config owner with complete own attribute membership and standard settings, the SDK SHALL map existing Ref, Code, Description and DeletionMark fields, and own custom GUIDs with recognized declarations, to the role's object Read authorization when no explicit supported member Read overrides it. This rule SHALL NOT depend on attribute-default flags or UI View/Edit rights. Unverified snapshots, other standard fields and ambiguous member paths SHALL remain fail closed.

#### Scenario: Native catalog has a denied UI View
- **WHEN** the verified own attribute has an explicit UI View refusal but the role grants object Read and has no member Read refusal
- **THEN** its programmatic field read passes without renaming View to Read

#### Scenario: Native catalog has defaults off
- **WHEN** a role grants object Read of a verified catalog and attribute defaults are false
- **THEN** the supported standard and own fields pass the programmatic check

#### Scenario: Object Read is refused
- **WHEN** no selected role grants object Read
- **THEN** all field reads fail before SQL

#### Scenario: No verified Config owner
- **WHEN** a compact or unknown snapshot cannot prove the native catalog membership and settings
- **THEN** the existing unsupported mapping refusal remains

## MODIFIED Requirements

### Requirement: Check every field read for the selected user
The database layer SHALL check the complete `FieldUsageRequest` against the selected user's loaded role rights and the metadata snapshot. It SHALL return success only when every read is proven permitted; an absent current user, unread rights, unresolved field, explicit denial without another granting role, or unsupported member path SHALL fail the whole check. It SHALL NOT return a partial authorization result or treat table-level `Read` as sufficient for a custom attribute with an explicit member `Read` refusal. Measured native catalog mappings MAY authorize supported programmatic reads independently of UI rights.

#### Scenario: A denied attribute is projected
- **WHEN** a role grants object `Read` but explicitly refuses `Read` of a projected custom attribute, and no other role grants that attribute
- **THEN** the complete field check fails before SQL execution

#### Scenario: A second role grants the attribute
- **WHEN** one role refuses an attribute and another role explicitly grants `Read` on both the object and its exact attribute member GUID
- **THEN** that field passes the check

#### Scenario: A field is used only for filtering
- **WHEN** a denied attribute appears in `WHERE` but not the projection
- **THEN** the complete field check fails

#### Scenario: Rights are not loaded
- **WHEN** any role of the selected user has unread or unavailable rights
- **THEN** the complete field check fails rather than treating the missing role as a denial or grant

#### Scenario: An unsupported member path
- **WHEN** a field's rights cannot be mapped unambiguously from the decoded resource and snapshot
- **THEN** the complete field check fails naming the field and reason

#### Scenario: A default flag without an explicit member grant
- **WHEN** an object grants `Read` and its role sets `setForAttributesByDefault`, but an unverified custom attribute has no exact member `Read` grant
- **THEN** the field check fails rather than inferring platform field semantics from the flag

#### Scenario: A standard field without a verified member mapping
- **WHEN** a prepared query reads a standard field without a Config GUID or a measured native mapping, even if the object has `Read`
- **THEN** the field check fails closed and names the mapping limitation


### Requirement: Preserve unsupported custom member authorization
Until a native member rule is implemented with its measured mapping, custom attributes and standard fields SHALL retain their existing authorization boundary. Native `View` permissions SHALL NOT be renamed into member `Read` permissions.

#### Scenario: A custom attribute has no exact member Read grant
- **WHEN** an unmeasured custom attribute has no supported exact member `Read` grant
- **THEN** the field check retains its existing fail-closed result
