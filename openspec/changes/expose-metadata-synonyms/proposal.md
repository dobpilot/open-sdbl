## Why

`Config` descriptors carry the localized synonyms and the comment an
object or an attribute was given in the configurator, and the decoder
already reads them: `ConfigDescriptor::synonyms` and
`ConfigDescriptor::comment` are public and populated.

`resolve_metadata` then drops both. `MetadataObject` and `MetadataField`
carry the metadata name and nothing else human-readable, so an application
holding a `MetadataSnapshot` — the only thing the compiler consumes —
cannot present `Корр. счет` for `КоррСчет`.

The console shows what that costs: to title a document it scans
`snapshot.descriptors()` linearly for a matching `object_guid`, picks the
`ru` synonym by hand, trims it, drops it when empty, and falls back to the
name. Every consumer that wants a presentation repeats that, and
`bsl-1c-orm` now needs it for objects and attributes both.

## What Changes

- `MetadataObject` and `MetadataField` SHALL carry the localized synonyms
  and the comment of the descriptor they were resolved from.
- Both SHALL answer a synonym by language, and a presentation that falls
  back to the metadata name, so the selection rule exists once rather
  than in every caller.
- The snapshot SHALL answer the same for an object addressed by its
  identifier.
- Nothing about name resolution, physical mapping, or query compilation
  changes; the additions are resolver output only.

## Capabilities

### Modified Capabilities

- `onec-metadata`: localized synonyms and comments on the resolved
  objects and fields, and how a presentation is chosen from them.

## Impact

- `src/metadata/resolve.rs` (two structs, their construction, accessors).
- `crates/open-sdbl-cli/src/repl/presentation.rs` drops its linear scan
  and uses the accessor; its output is unchanged.
- README and `docs/query-language-support.md`.
- No new production dependency; no change to generated SQL.
