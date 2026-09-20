## Why

Nothing in the crate answers "has this configuration changed?" cheaply.
`SnapshotFingerprint` is computed *after* a full load and is
`pub(crate)`, so it is useless for deciding whether to load at all and
invisible to the application besides.

A consumer holding metadata snapshots of two hundred bases must ask that
question often and cheaply. The alternative it has today is a full
acquisition: twelve round trips, a full scan of `config`, three scans of
the system catalogue over ten thousand tables, and inflating every Config
resource.

`CONFIG_TOTALS` is the nearest thing that exists, and it is too weak to
reuse: `count` plus `sum(octet_length)` does not change when a resource is
rewritten at the same size, which is exactly what editing an object does.

## What Changes

- A fingerprint statement SHALL join the acquisition statements of both
  providers, computed **on the server**, that changes when any `Config`
  resource of the configuration changes — its content included, not only
  its size — and stays equal across two reads of an unchanged base.
- It SHALL cost one round trip and SHALL NOT transfer resource content.
- It SHALL work on both storage layouts.
- `SnapshotFingerprint` and `MetadataSnapshot::fingerprint` SHALL become
  public, so that an application can confirm it is still working against
  the snapshot it compiled against. That value stays a property of a
  loaded snapshot and is not a substitute for the statement above.

## Capabilities

### Modified Capabilities

- `onec-metadata`: the configuration fingerprint statement and the public
  snapshot fingerprint.

## Impact

- `src/metadata/queries.rs` (two statements, both `all()` lists),
  `src/metadata/resolve.rs` (visibility).
- Tests over both providers' statement text and the coverage lists.
- No new production dependency; the core crate stays I/O-free — the
  statement is text, and running it belongs to the application.
