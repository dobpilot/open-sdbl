## ADDED Requirements

### Requirement: Project the roles collection while parsing
A bare-GUID Config resource SHALL report, in `ParsedConfigResource.roles`,
the role identifiers listed by the roles collection
`09736b02-9cac-4e3f-b4f7-d3e9576ab948` it carries, and an empty list
otherwise; a resource that is not a bare GUID SHALL report none.

#### Scenario: Configuration root
- **WHEN** the root resource carries `{09736b02-…, 2, <guid1>, <guid2>}`
- **THEN** the parsed resource lists exactly those two identifiers

### Requirement: Acquire the rights resources of named roles
The library SHALL provide, for both providers and both storage layouts,
a SELECT statement reading the `<guid>.0` Config resources of a given
list of role identifiers only — `(file name, part, data)` rows ordered by
file name and part — because a base stores tens of thousands of `.0`
resources of other objects. The identifiers are typed `Guid`s, so the
statement text carries no untrusted input.

#### Scenario: Two roles on PostgreSQL
- **WHEN** the statement is built for two identifiers on the modern layout
- **THEN** it reads `config` with `rtrim(filename::text) IN ('<g1>.0', '<g2>.0')`
  ordered by file name and part
