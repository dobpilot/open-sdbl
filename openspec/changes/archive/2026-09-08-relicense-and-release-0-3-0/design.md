## Context

Both workspace packages are at 0.1.0 under GPL-3.0-only. The 0.3.0 release
carries breaking API changes already merged into the specs.

## Decisions

- Library and CLI share one version number so that a CLI build always names
  the library contract it was built with.
- The MIT text names the copyright holder as `Korolev Alexander`, 2026, the
  sole author in the repository history.
- The fuzz workspace, although unpublished, declares the same license so that
  every `Cargo.toml` in the repository agrees.
- No CHANGELOG file is introduced; `openspec/changes/archive` remains the
  change log.

## Risks / Trade-offs

- Forks created under GPL-3.0-only keep that license for their copies; the
  relicense applies from this commit forward.
