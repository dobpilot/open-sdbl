## ADDED Requirements

### Requirement: Compute hierarchy totals
The compiler SHALL accept `ИЕРАРХИЯ` and `ТОЛЬКО ИЕРАРХИЯ` on one control
point that is a fixed single-target reference to a hierarchical catalog.
With `ИЕРАРХИЯ` the result SHALL contain, before the rows of each value,
one hierarchy total row per ancestor folder of the values present,
aggregating every row beneath the folder; with `ТОЛЬКО ИЕРАРХИЯ` the rows
SHALL be grouped by the parent folder of the value and hierarchy rows
SHALL appear only for folders above those parents. A hierarchy row SHALL
aggregate the rows keyed by the folder itself and by every descendant. A
folder's level SHALL be its depth in the tree, a group total one deeper
than the row above it (its folder's hierarchy row, or its own hierarchy
row when the value is a folder with rows beneath), a detail row one
deeper than its group, all shifted by one under `ОБЩИЕ`. The parent
lookup SHALL read the catalog's extension tables as well as its base
table.
Sibling folders SHALL be ordered by the first appearance of any row
beneath them in the ordered result. A plain control point on the same
column right before the hierarchical one SHALL be ignored. A second
hierarchical control point SHALL be an `UnsupportedFeature` diagnostic;
a control point without a hierarchical catalog target SHALL be a
`Syntax` diagnostic. The standard fields `ParentID` and `OwnerID` SHALL
also answer to `Родитель`/`Parent` and `Владелец`/`Owner`.

#### Scenario: Hierarchy totals
- **WHEN** `… ПО Товар ИЕРАРХИЯ` is executed over items of folders
  `Мебель` ⊃ {`Стол`, `Кухня` ⊃ {`Табурет`}} ordered by name
- **THEN** the rows are the `Мебель` hierarchy total (level 0), the
  `Кухня` hierarchy total (1), the `Табурет` group total (2) and detail
  (3), then the `Стол` group total (1) and detail (2)

#### Scenario: Only hierarchy
- **WHEN** `… ПО Товар ТОЛЬКО ИЕРАРХИЯ` is executed over the same rows
- **THEN** the rows are the `Мебель` hierarchy total (level 0, all
  three items), the `Кухня` group total (1) with `Табурет` beneath it
  (2), and the `Мебель` group total (1) with `Стол` beneath it (2)

#### Scenario: Parent alias
- **WHEN** a query projects `Т.Родитель` from a hierarchical catalog
- **THEN** it resolves to the `_ParentIDRRef` column
