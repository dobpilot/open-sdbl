# Design

## Error structure

`DbError::MissingSessionParameter { name, context }` carries the name emitted by `RestrictionError::MissingParameter`; only that variant is mapped specially. Other expansion failures retain their existing diagnostic text. Query compilation preserves a missing name in `QueryDiagnostic` without requiring callers to parse `Display`. A missing data-separator parameter is identified by `is_missing_session_parameter()` while retaining the existing `Parameter` category and message, so old diagnostics do not change. A bare `&Name` that is unbound is structurally named but has no provenance, because the compiler cannot know whether the host intended a query parameter or a session parameter.

## Field check

The database-layer check takes the complete `FieldUsageRequest`, `AccessStore`, and `MetadataSnapshot`. It first verifies a current user and that every role's rights are loaded. It resolves every object and field and checks each read. The only successful result means every reported read is authorized; no partial answer exists. For a custom attribute, an exact one-step member GUID entry permits reading only when it explicitly states `Read=true` and the same role grants object `Read`. An absent entry and a member entry without `Read` do not prove permission. A nested path mentioning the attribute GUID makes the result unknown and therefore fails closed. A standard field has no Config GUID by which to identify an independent member right and fails closed, even when the role grants object `Read`. Where the role format or the mapping to metadata does not support a definite decision, the operation fails closed with a diagnostic naming that field and reason. Rights are combined across roles: a role granting both the object and the field suffices; an explicit denial in one role does not cancel a separate role's grant. The access check does not replace the row restrictions of `user_decisions`.

The routine reads `AccessStore` only. Both PostgreSQL and SQL Server use the same decoded role rights, so no provider-specific SQL is added. The host checks the complete field request before sending compiled SQL.

## Limits

Member-kind markers and nested tabular-section member paths are not treated as platform semantics without evidence. A path not safely matched to a field is denied, rather than guessed. This deliberately may refuse a query that 1C permits; it cannot expose a field whose grant was not proven.

The cross-role union follows the already specified object-level `read_access` union but has not been measured as exact 1C field-access semantics. The decoded `setForAttributesByDefault` flag is deliberately not used to grant a field, because its effect on `Read` has not been measured. The API does not claim parity with the platform; a host requiring parity must keep refusing any case outside this proof boundary until a platform measurement is recorded.
