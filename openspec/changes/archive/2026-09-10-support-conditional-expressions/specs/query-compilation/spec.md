## MODIFIED Requirements

### Requirement: Diagnose UNION kind mismatches
When UNION branches project different column kinds at the same position, the
compiler SHALL fail with an unsupported-feature diagnostic positioned at the
union token before execution. The `NULL` literal and unknown catalog types
SHALL be compatible with every kind, and parameters such as length or
precision SHALL NOT participate in the comparison. Reference columns whose
branches differ in target or width SHALL be widened to one runtime-typed
payload column whose targets are the union of the branch targets, so every
branch emits the same byte width.

#### Scenario: Reference joined with string
- **WHEN** the first branch projects a reference and the second projects a
  string in the same position
- **THEN** compilation fails with an unsupported-feature diagnostic at the
  union keyword

#### Scenario: NULL branch
- **WHEN** one branch projects `NULL` where the other projects a number
- **THEN** compilation succeeds and the column kind is number

#### Scenario: Fixed and runtime-typed reference branches
- **WHEN** one branch projects a catalog `Ссылка` and the other projects a
  runtime-typed `Регистратор`
- **THEN** the catalog branch is rendered as `RTRef ‖ RRRef` with the
  catalog's type number and the merged column kind is a runtime-typed
  reference containing the catalog among its targets
