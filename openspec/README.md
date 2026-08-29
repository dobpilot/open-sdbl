# OpenSpec workflow

Current contracts live in `openspec/specs/`. Proposed observable changes live
in `openspec/changes/<change-name>/` and contain a proposal, requirement delta,
design, and verifiable tasks.

Before implementation:

```console
openspec validate <change-name> --strict
```

After implementation, completed changes are archived with `openspec archive`,
which merges their requirements into the current specs.
