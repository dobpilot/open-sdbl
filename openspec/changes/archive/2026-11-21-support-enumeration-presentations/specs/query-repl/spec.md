## ADDED Requirements

### Requirement: Use enumeration metadata for default presentations
The default presentation policy SHALL map an enumeration's declared values to
their Russian nonblank synonyms, falling back to their metadata names. It SHALL
use the values belonging to that enumeration only and preserve reference
authorization for queries and deferred lookups.

#### Scenario: Synonym and fallback
- **WHEN** enumeration values have a Russian synonym or no nonblank synonym
- **THEN** their presentations use the synonym or the declared name respectively

#### Scenario: Gateway and console
- **WHEN** the gateway or console presents an enumeration reference
- **THEN** both use the same metadata-derived default presentation policy
