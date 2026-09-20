# Design — localized synonyms on the resolved metadata

## Context

Three layers already exist and only the last is missing:

1. `parse_config_descriptors` reads `{2,"ru","Корр. счет"}` into
   `ConfigDescriptor::synonyms: Vec<Synonym>` and the trailing comment
   into `ConfigDescriptor::comment`. Both are public.
2. `resolve_metadata` matches descriptors to DBNames entries and the
   physical schema, and builds `MetadataObject`/`MetadataField` — copying
   `name` and `marker`, dropping the rest.
3. `MetadataSnapshot` hands the resolver output to the compiler and to
   applications. It also exposes `descriptors()`, which is why the console
   can work around the gap at all.

## Decisions

### 1. The synonyms travel with the resolved item, not beside it

`MetadataObject` and `MetadataField` gain `synonyms: Vec<Synonym>` and
`comment: Option<String>`, filled from **the descriptor that already
supplies `name`** — the one `descriptor_by_guid` selects by `object_guid`.
No new matching rule is introduced, and name, synonym and comment are
guaranteed to come from one descriptor.

That last property is the reason not to keep the console's rule. The
console scanned the descriptor list and took the *first* match, while
`name` has always come from the map, which keeps the *last* of duplicate
`object_guid`s. Where a configuration carries two descriptors for one
GUID, the console could print the synonym of one beside the name of
another; after this change both come from the same descriptor. Nothing
else changes, because a duplicated `object_guid` is the only case in which
the two rules disagree.

Both structs are resolver output: nothing outside `resolve.rs` constructs
them, so the added fields break no caller in practice.

### 2. The selection rule lives once

A raw `Vec<Synonym>` would leave every caller to repeat the console's four
steps: find the language case-insensitively, trim, discard an empty
string, fall back to the name. So both types answer

```rust
fn synonym(&self, language: &str) -> Option<&str>
fn presentation(&self, language: &str) -> Option<&str>
```

`synonym` applies the trimming and emptiness rule; `presentation` is the
synonym or the metadata name. `MetadataSnapshot::object_synonym` and
`object_presentation` answer the same for an `ObjectId`, so a caller
holding only the identifier does not have to look the object up first.

The language stays a caller-supplied string rather than an enum: 1C writes
whatever language codes the configuration declares, and the library has no
list of them.

### 3. What this does not change

Name resolution, physical mapping, separators, and every byte of generated
SQL are untouched. The console keeps printing exactly what it printed; its
scan over `descriptors()` is replaced by the accessor, which is the same
rule with the same fallbacks.

`ConfigDescriptor` keeps its fields and `snapshot.descriptors()` keeps
working, so an application already doing the scan is not forced to move.

## Risks

- Memory: the synonyms are cloned onto every object and field instead of
  living once in the descriptor list. A configuration has tens of
  thousands of descriptors with one or two short synonyms each, which is
  small next to the schema the snapshot already holds, and the descriptors
  stay where they are because the snapshot exposes them.

## No new dependencies

The change is confined to the core crate and the console.
