## ADDED Requirements

### Requirement: Test a composite reference in В ИЕРАРХИИ
`В ИЕРАРХИИ` MAY test a field whose value is a composite reference. The
identifier member SHALL be compared with the seeds and their descendants,
and the type member SHALL be required to equal the type of the catalog the
seeds belong to, because a value of another type is under no seed. When
the seeds' catalog has no parent column the predicate SHALL degenerate to
membership, exactly as it does for a single-target field.

#### Scenario: Composite value under a group
- **WHEN** a catalog attribute holds either a product or a client and the
  predicate names a product group as its seed
- **THEN** the rows holding a product under that group answer, and the
  rows holding a client do not

#### Scenario: Branch of unbound parameters beside a reference
- **WHEN** an alternative mixes a nested `ВЫБОР` whose branches are all
  `NULL` with a branch carrying a reference
- **THEN** the untyped branches are rendered as `NULL` of the reference
  type, so the server accepts the alternative

#### Scenario: Seeds of a catalog without hierarchy
- **WHEN** the seeds are clients, whose catalog has no parent column
- **THEN** the predicate answers the rows whose value is that client,
  as the platform answers
