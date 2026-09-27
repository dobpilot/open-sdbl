## ADDED Requirements

### Requirement: Preserve missing restriction session parameter identity
When restriction expansion reads a session parameter without a value, the database layer SHALL return a structured error containing that parameter's name. Other restriction errors SHALL retain their existing diagnostic category and readable context.

#### Scenario: RLS parameter is absent
- **WHEN** a selected user's `Read` restriction references an unset session parameter
- **THEN** deriving its access fails with a structured missing-session-parameter error carrying the original name

### Requirement: Check every field read for the selected user
The database layer SHALL check the complete `FieldUsageRequest` against the selected user's loaded role rights and the metadata snapshot. It SHALL return success only when every read is proven permitted; an absent current user, unread rights, unresolved field, explicit denial without another granting role, or unsupported member path SHALL fail the whole check. It SHALL NOT return a partial authorization result or treat table-level `Read` as sufficient for a custom attribute with an explicit member right.

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
- **WHEN** an object grants `Read` and its role sets `setForAttributesByDefault`, but the custom attribute has no exact member `Read` grant
- **THEN** the field check fails rather than inferring platform field semantics from the flag

#### Scenario: A standard field without a verified member mapping
- **WHEN** a prepared query reads a standard field without a Config GUID, even if the object has `Read`
- **THEN** the field check fails closed and names the mapping limitation
