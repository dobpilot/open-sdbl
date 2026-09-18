## ADDED Requirements

### Requirement: The snapshot carries the roles of the configuration
`MetadataSnapshot` SHALL accept the role identifiers projected from the
configuration root through `attach_roles` and answer them through
`roles`; `RoleCatalog::from_snapshot` SHALL name them from the snapshot's
descriptors. `object_query_name` SHALL spell a metadata object as a
query names it — `Справочник.Номенклатура`, `Документ.Заказ.Товары` for a
tabular section — or answer nothing for a kind a query cannot name.

#### Scenario: Roles attached
- **WHEN** two role identifiers are attached to a snapshot whose
  descriptors name them
- **THEN** the catalog built from the snapshot lists both by name

#### Scenario: Query name of a tabular section
- **WHEN** the object of a tabular section `Товары` of `Документ.Заказ` is
  named
- **THEN** the name is `Документ.Заказ.Товары`
