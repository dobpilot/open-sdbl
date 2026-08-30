# Design: Kind-aware virtual completion

The completion catalog already creates three source spellings for each object:
bare Config name, English kind plus name, and Russian kind plus name. For an
information register, each spelling is extended with `SliceLast()`,
`SliceFirst()`, `СрезПоследних()`, and `СрезПервых()`. For an
accumulation register it is extended with `Balance()`, `Turnovers()`,
`Остатки()`, and `Обороты()`.

Other metadata kinds receive no virtual-table candidates. Candidate insertion
continues through the existing case-insensitive de-duplication helper and is
performed by `ConsoleHelper::from_snapshot`, so `\refresh` naturally rebuilds
the list from the new snapshot.
