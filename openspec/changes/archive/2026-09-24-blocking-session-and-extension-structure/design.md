# Design — a blocking session owner and the structure of an extension

## Context

The consumer is `open-bsl/bsl-config-db`, a synchronous BSL host. It must
own one live connection per BSL session, it cannot add `tokio` or `zeroize`
to its own tree, and it applies configuration extensions from the result
of `DatabaseSession::configuration`. Two independent pieces of work follow:
a synchronous owner of one session (A) and the structural facts of an
extension (B). Neither touches the whole-configuration contract — the
`READ COMMITTED` wording and the `Config`/`ConfigCAS` retention limits
passed their own review and stay as they are.

## A. `BlockingDatabaseSession`

### A1. One value, one runtime, one connection

```rust
pub struct BlockingDatabaseSession {
    driver: BlockingDriver<DatabaseSession>,
}
```

`BlockingDriver<S>` is private. It owns a `tokio::runtime::Runtime` built
with `Builder::new_current_thread().enable_all()` and an
`Option<S>`: `Some` from a successful `connect` until `close`. Every
blocking method is `runtime.block_on(<the asynchronous method of S>)`.

The runtime is single-threaded because one session is one connection and
never runs two statements at once. It still has its own blocking pool, so
the `spawn_blocking` decoding of the acquisition pipeline runs off the
runtime thread exactly as it does under an asynchronous caller. The
PostgreSQL connection driver, spawned onto the runtime at connect, makes
progress whenever a call is being driven, which is the only time the
client waits on it.

Each value builds its own runtime, so two values share nothing: not a
connection, not a runtime, not a thread. `BlockingDatabaseSession` is
`Send`, so a host may move it to the thread that runs its BSL session.

No new dependency: `tokio`'s current-thread runtime is behind its `rt`
feature, which `rt-multi-thread` — already enabled — implies.

### A2. One SQL path

`BlockingDriver` is generic over a crate-private trait whose methods are the
six operations; the one production implementation is `DatabaseSession`,
and each method of it calls the `DatabaseSession` method of the same name.
The blocking module contains no statement, no provider type and no
decoding. Limits, timeouts, diagnostics, the SQL Server rule that a
timed-out or abandoned read poisons the session, and every provider
difference are therefore the asynchronous session's, not restated.

The trait exists so that the blocking driver can be exercised over an
artificial provider in tests; it is not exported and is not a dialect
extension point.

### A3. Closing, and calls after it

`close(&mut self)` takes the session out of the value and drives its
asynchronous `close`. From then on every operation — including a second
`close` — answers `DbError::SessionClosed`, whether the first `close`
succeeded or failed: a close that failed has still consumed the session.
`close` takes `&mut self` rather than `self` precisely so that "a call
after close" is an answerable question rather than a compile error.

Dropping the value without `close` drops the session inside the runtime's
context and then the runtime, which is what dropping an asynchronous
session does too: the socket is closed without the graceful wind-down.

### A4. Called from an asynchronous context

`Runtime::block_on` panics when called on a thread already driving a Tokio
runtime. Every blocking method checks `Handle::try_current()` first and
answers the typed `DbError::AsyncContext` instead, so a host that mixes the
two models gets an error, never a panic. `connect` performs the same check
before building its runtime.

### A5. Credentials from owned strings

```rust
impl Credentials {
    pub fn new(postgres: Option<String>, mssql: Option<String>, socks5: Option<String>) -> Self;
}
impl EnvironmentSecret {
    pub fn from_option(secret: Option<String>) -> Self;
}
```

The `String` is moved, not copied, into `Zeroizing<String>` inside the
crate, so the only heap copy of the secret is the one wiped on drop. The
existing `EnvironmentSecret::Present(Zeroizing<String>)` variant stays for
the CLI, which already depends on `zeroize`.

`Credentials` and `EnvironmentSecret` gain hand-written `Debug`
implementations that print `Present(<redacted>)`, `Missing`,
`InvalidUnicode` and nothing else. `DatabaseConnection` carries no secret
at all, and the error texts of the crate name a secret by its variable
name only; tests pin both.

## B. The structure of an extension

Everything below was measured on the reference bases: `demo`
(PostgreSQL, two extensions, one applied and one not) and `buh`
(PostgreSQL, two extensions, one of them an add-on with 105 resources).

### B1. What `_ExtensionsInfo` itself records

The table carries, beside the four columns read today, `_ExtensionScope`
(numeric), `_ExtensionUsePurpose` (numeric), `_UsedInDistributedInfoBase`
(boolean), `_Version` (integer), `_MasterNode` and `_UpdateTime`.

- `used_in_distributed_infobase: Option<bool>` is the
  `_UsedInDistributedInfoBase` column. The column is named for exactly
  that property; it answers `None` when the base has no such column.
- `scope: Option<u32>` is the `_ExtensionScope` code **as stored**. All
  four measured extensions — every one attached to a whole, unseparated
  base — store `1`. No base with an extension attached to a data area was
  available, so the code is not mapped to the platform's enumeration: a
  mapping of one observed value would be a guess about the other.
- `_Version` is not the extension's version: it is `0` and `3` for the two
  `demo` extensions, whose records carry no version string, and `0` for
  the two `buh` extensions, whose records carry `1.19` and `2.4`. It is
  not read.

Older platforms have `_ExtensionsInfo` without the two columns. The
statement is built from fixed fragments after a probe of the two columns,
and answers `NULL` for a column the base lacks, so no statement ever names
a column that is not there.

### B2. What `_ExtensionZippedInfo` records

- `version: Option<String>` is the ASCII string the record carries
  (measured `1.19`, `2.4`; absent on the two `demo` extensions, which have
  no version). The decoder already reads it.
- `content_hash: String` is the lower-case hexadecimal of the root key.
  The store names each row by the SHA-1 of its stored bytes (verified on
  the fixtures: the SHA-1 of the stored root equals its row name), and the
  root lists the key of every other resource, so the root key changes
  whenever any resource of the extension changes and two extensions with
  the same key have the same content. It is *not* the platform's
  `ConfigurationExtension.HashSum`, which was not measured, and the rustdoc
  says so.
- `safe_mode: Option<bool>` is `None` on every base today. The four
  measured records differ in three standalone tag bytes besides the
  activity flag (`a1`/`a2` after the key, two tags after the eight-byte
  run, one after the flag), but the differences line up with the base and
  with the purpose column as well as with anything else, and none of the
  bases lets safe mode be toggled for a measurement. Reading one of those
  bytes as safe mode would be the same guess the activity flag's design
  refused. The field exists so that the day it is measured is not an API
  change; until then a consumer must treat it as unknown.

### B3. The catalog of the extension

The root index of an extension carries, before the name/key pairs, the
record `{2,<guid>,…}` naming the resource that holds the extension's
configuration object. That resource has the same shape the main
configuration's root object has, which `bsl-config-db` already walks:

```
{2, {<self>}, <n>, <group>×n}
<group>    = {<class>, <versioned>}
<versioned>= {6, <include>} | <include>
<include>  = {<k>, <header>, <m>, <entry>×m}
<entry>    = {<class>, <c>, <object>×c}
```

`open_sdbl::metadata::parse_extension_catalog` decodes it into
`ExtensionCatalog { configuration, classes: Vec<ExtensionClass { class,
objects }> }`, class and object identifiers as `Guid`s, every entry kept in
stored order. Any entry of another shape is an error, never a skipped
entry; in the acquisition that error becomes `catalog: None`, so a
consumer that needs the catalog refuses the extension instead of seeing a
partial one. The core does not name classes — a consumer that already
maps class GUIDs (as `bsl-config-db` does) keeps doing it.

### B4. Explicit links to the main configuration

Every object description — the object itself and each nested one, such as
an attribute — starts with a header:

```
{3, {1,0,<id>}, "<name>", {<synonym>}, "<comment>",
    <belonging>, <n>, (<guid>, <tag>)×n, <extends>, <x>}
```

Measured across all 127 object headers of the four extensions (69 adopted,
33 of them with a non-nil link), every one in this shape:

- `<belonging>` is `0` for every object the extension creates and `1` for
  every object it adopts.
- `<extends>` is, for an adopted object, the identifier of the
  main-configuration object it adopts. `СтавкиНДС` of `Расширение1`
  carries `6a386de9-…`, which is `Reference14574` in the base's `DBNames`
  and a resource of its `Config`; its adopted attribute `Ставка` carries
  `9a871782-…`, `Fld14578` of the same base. Own objects carry the nil
  GUID. So do adopted objects of an extension that maps by name rather than
  by identifier (`БанковскиеСчета` of the `buh` add-on).
- The `(<guid>, <tag>)` pairs are not interpreted.

`open_sdbl::metadata::parse_extension_objects` walks a descriptor and
answers every header as `ExtensionObject { id, name, belonging:
Option<ObjectBelonging>, extends: Option<Guid> }`. `belonging` is `None` for
a value other than `0`/`1` or a tail of another length; `extends` is
`Some` only for an adopted object whose `<extends>` is a non-nil GUID. An
adopted object with a nil `<extends>` answers `extends: None`: the link is
by name, and a name is not the explicit link this field promises.

`AcquiredExtension::objects` carries the headers of every bare-GUID
resource of the extension as `AcquiredExtensionObject { resource, object }`.
The catalog lists top-level objects only; forms and templates are nested
objects of their owner and appear among the headers, not in the catalog. A resource that is not brace text, or does not inflate within the
per-resource decoding limit, contributes nothing; `ConfigCAS` names never
do.

### B5. Provider neutrality

Both providers answer the same `ExtensionCatalogRow`, now with `scope:
Option<i32>` and `used_in_distributed_infobase: Option<i32>` (`0`/`1` as
the statement answers it), and everything after the row — the mapping to
`Option<bool>`, where any other integer is `None`, the grouping, the
record, the catalog, the objects — is the one function in `pipeline.rs`.
Both statements answer the two new columns as integers (`CONVERT(int, …)` of SQL Server's `binary(1)` boolean,
`::int` of PostgreSQL's `boolean`), so the row decoders differ only in
their driver's cell API.

## Risks

- `safe_mode` is always `None`. That is honest and it is still a hole; the
  measurement needs a base where safe mode can be switched.
- The catalog shape was measured on configurations of compatibility mode
  8.3.27; an older compatibility mode may differ. It answers `None`
  rather than a wrong catalog.
