## Context

`collect_descriptors` needs only overlapping immediate-list windows of three
values, an optional fourth comment, the first two values for collection
purpose, and recursive child descriptors. Retaining every unrelated subtree is
unnecessary.

## Decisions

### Retain a four-candidate sliding window

While parsing each list, retain at most the last four immediate candidates.
Candidates are a scalar, a simple scalar-only child list, or an opaque complex
child. A self-reference is recognized from an exact three-scalar child list;
synonyms are projected from the following simple list only after the preceding
self-reference and quoted name match.

### Preserve inherited field purpose

Inspect the first two immediate candidates with the existing bilingual-agnostic
collection GUID mapping. Future child lists inherit the discovered purpose. If
a child appeared before the purpose candidate, fill only its descriptors whose
own nested list did not establish a purpose, matching the existing recursive
semantics.

### Validate and recurse through opaque branches

Every child is fully parsed and may emit nested descriptors before its
candidate becomes opaque to the parent. Thus irrelevant or complex branches do
not consume retained memory but malformed content remains fatal.

## Verification

Compare streaming and generic projections on owner, nested field-purpose,
synonym, comment, and malformed fixtures. Then rebuild release and measure the
reported live console startup.
