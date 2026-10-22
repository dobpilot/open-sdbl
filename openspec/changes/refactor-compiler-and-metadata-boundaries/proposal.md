# Refactor compiler and metadata boundaries

## Why
Source-free and sourced compilation duplicate semantic decisions, which already
caused equality and logical-projection regressions. The database crate decodes a
pure native empty-role format and constructs a result through hard-coded
compressed bytes. Runtime metadata projection combines format recognition,
validation and publication in a large function over option-heavy descriptors.
These boundaries make behavior difficult to audit and extend safely.

## What Changes
- Extract shared internal scalar comparison and projection-availability policies;
  keep name resolution and dialect rendering explicit at their existing boundaries.
- Move empty-role format recognition into the dependency-free core decoder with
  a typed internal construction path; keep resource acquisition in open-sdbl-db.
- Split runtime metadata format handling into focused modules and introduce
  validated internal records before mutating published runtime facts.
- Preserve public API, accepted measured formats, diagnostics, ordering and SQL.

## Impact
Affected capabilities: crate-architecture, onec-metadata, query-compilation.
This change is a proposal for subsequent implementation. It does not introduce
new format support, a public ConfigDescriptor redesign, incremental resolution,
new dependencies, or changes to SQL/runtime semantics. The four concrete review
defects are repaired separately by `fix-reviewed-runtime-regressions`.
