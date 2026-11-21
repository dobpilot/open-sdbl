## ADDED Requirements

### Requirement: Compile an independently scoped system-settings load
The SDK SHALL compile a read-only PostgreSQL SystemSettings load for exact user, object and settings keys and complete explicit string or numeric separator states. It SHALL validate the acquired SystemSettings table, required columns and scalar types and derive all separator mappings from DBNames. Missing, duplicate, extra, unsupported or ambiguous scope SHALL fail before execution. Inactive states SHALL use false and a typed empty value; globally disabled conditions SHALL use true and a typed empty value. Reads SHALL return at most two payloads. Unsupported backends SHALL fail explicitly. CommonSettings compilation SHALL remain unchanged. Authorization and user selection SHALL remain the caller's responsibility.

#### Scenario: Independent system storage
- **WHEN** system and common tables contain identical keys with different payloads
- **THEN** system load targets only the acquired system table and its complete scope

#### Scenario: Missing or unsupported schema
- **WHEN** a required system table, column, scalar type or scope mapping is missing or unsupported
- **THEN** compilation fails without borrowing CommonSettings schema

#### Scenario: Numeric inactive and disabled condition
- **WHEN** a numeric separator is inactive or globally disabled
- **THEN** the corresponding SQL uses false/zero or true/zero respectively
- **AND** the namespaces remain distinct

#### Scenario: Quoted keys and unsupported backend
- **WHEN** a key contains quotes
- **THEN** the existing literal renderer escapes it
- **WHEN** the backend is not supported
- **THEN** compilation fails explicitly
