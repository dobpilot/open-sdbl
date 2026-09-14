## Why

SchemaStorage names the target of a reference column only when the field
admits exactly one table; with several it writes an empty target and the
marker `4` — measured on 8.3.27 (`{"R",0,0,"",4}` against
`{"R",0,0,"Reference68",3}`). Lacking the list, the compiler scanned every
object carrying an attribute of the dereferenced name, which joined tables
the field can never hold and refused the query outright past thirty-two
candidates. Six demo-corpus queries stopped there.

The list is in Config. Each object's class list opens with the class id and
the identifiers of the object, the third of which is the reference type
other objects name; an attribute carries `{"Pattern", {"#", <reference
type>}, …}`. Both were measured against a probe configuration whose
attribute targets are known.

## What Changes

- The Config parser SHALL read the reference type of the object a resource
  describes and the reference types an attribute's type description names.
- A field whose SchemaStorage target is unnamed SHALL take its targets
  from that type description.
- A type description naming something that is not a stored object SHALL
  leave the list empty, so the field keeps the scan rather than being
  silently narrowed.

## Capabilities

### Modified Capabilities

- `onec-metadata`: reference targets from the Config type description.

## Impact

- `src/metadata/config.rs`; `src/metadata/resolve.rs`;
  `src/query/core/resolve.rs`; `tests/metadata_lookup.rs`;
  `tests/query_dereference.rs`; `tests/fixtures/demo/expected.jsonl`;
  `docs/query-language-support.md`.
