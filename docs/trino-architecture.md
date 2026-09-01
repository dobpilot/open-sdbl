# Trino 476 integration architecture

## Decision

The MVP uses a thin Java Trino plugin backed by a separate Rust service:

```text
CedrusData / Trino 476
        |
        | trino-open-sdbl (io.trino:trino-spi:476)
        | HTTP metadata + streaming scans
        v
open-sdbl-trino (Rust)
        |
        | open-sdbl metadata adapter + parameterized SQL
        v
PostgreSQL used by 1C
```

The stock Trino Thrift connector was inspected first at upstream tag `476`
(commit `7f3746a7fa0b27ace2470340e848feaf3ee73f48`). Its published
`TrinoThriftService` can transport requested columns and a tuple domain, but it
has no limit field or method. The connector itself does not implement
`ConnectorMetadata.applyLimit`. `maxBytes` on `trinoGetRows` only bounds a
transport page; it does not communicate SQL `LIMIT`. Consequently a Rust
Thrift service cannot guarantee PostgreSQL limit pushdown, which is an
acceptance requirement. The protocol also has no decimal, UUID, or varbinary
data blocks. This is why the documented fallback architecture is used.

The fallback uses upstream Trino APIs only. It does not depend on
`cedrusdata-spi`, does not embed Rust in the JVM, and uses neither JNI nor JNA.
CedrusData reports `476-5`; the connector therefore targets exactly upstream
SPI 476 and treats `-5` as a distribution revision. Deployment must still run
the documented plugin smoke test because public CedrusData source proving
binary identity for that build was not found.

## Existing repository components

The workspace currently contains:

- `open-sdbl`, the dependency-free Rust 2024 core library;
- `crates/open-sdbl-cli`, the Tokio/tokio-postgres command-line application;
- OpenSpec contracts under `openspec/specs`.

The core library owns all deterministic 1C behavior. Runtime dependencies and
I/O remain outside it.

## Metadata loading flow

`open-sdbl-cli` opens an explicit read-only `READ COMMITTED` transaction and:

1. reads and raw-DEFLATE decodes `Params.DBNames`;
2. obtains exact Config resource/byte totals;
3. streams bare-GUID, part-zero Config resources;
4. decodes Config in bounded blocking batches while database delivery can
   continue;
5. reads `SchemaStorage.CurrentSchema` for `SchemaID = 0`;
6. reads live PostgreSQL tables, columns, and indexes;
7. calls `resolve_metadata` on a blocking worker.

The service reuses this sequence and its fixed SELECT-only
`PostgresMetadataQueries`. Acquisition and query transactions are verified
read-only. CPU-heavy metadata parsing stays off Tokio runtime workers.

## `MetadataSnapshot`

`MetadataSnapshot` is the immutable source for one service cache generation.
It contains:

- `db_names`: authoritative GUID/alias/numeric physical-name entries;
- `descriptors`: Config names, synonyms, comments, and field purposes;
- `schema`: authoritative SchemaStorage tables, columns, type tags, reference
  targets, and indexes;
- `live_tables`: PostgreSQL catalog tables, SQL column types, and indexes;
- `objects`: resolved GUID, `MetadataKind`, Config name, physical table,
  declaration/live status, and length semantics;
- `fields`: resolved custom attribute GUID/name/purpose/owner information;
- `indexes`: declared-versus-live index comparisons;
- private lookup indexes for object/field/reference identity.

An `Arc`-owned snapshot and a derived adapter catalog are swapped as one cache
generation. Queries never observe a partially refreshed generation.

## Logical and physical model

The Trino catalog name (`onec` in examples) is selected by Trino configuration,
not returned by the service.

Schemas are derived from every `MetadataKind`, using canonical Russian names:
`Справочник`, `Документ`, `Перечисление`, `РегистрСведений`,
`РегистрНакопления`, `РегистрБухгалтерии`, `РегистрРасчета`,
`ПланВидовХарактеристик`, `ПланВидовРасчета`, `ПланСчетов`, `Константа`,
`ПланОбмена`, `БизнесПроцесс`, `Задача`, and `Последовательность`.

A table is a live resolved metadata object. Its normal table name is the Config
object name. Case-folded duplicates are disambiguated with a stable GUID
suffix and emitted as metadata issues. Objects without a usable name or live
table are also reported as issues instead of disappearing silently.

Trino 476 normalizes schema names returned by connectors to lowercase for its
information schema. The connector therefore publishes lowercase discovery
names, but resolves incoming schema and table handles Unicode-case-
insensitively. Quoted source-form queries such as `"Справочник"."Контрагенты"`
remain valid; metadata listings may display them in lowercase.

Columns come from `queryable_field_catalog(snapshot)`. A `QueryableField`
retains its logical 1C name and aliases, SchemaStorage name, one or more exact
physical PostgreSQL columns and their live SQL types, plus fixed or possible
reference targets. This is the adapter boundary; the Java plugin never sees
physical identifiers.

For platform standard fields, the adapter chooses the Russian logical alias
(`Ссылка`, `Код`, `Наименование`, `ПометкаУдаления`, and so on)
instead of leaking SchemaStorage names such as `ID` or `Description`.

## Query path

The existing SDBL compiler tokenizes and resolves user SDBL text and emits
bounded SELECT SQL. Ordinary Trino tables do not synthesize SDBL: the adapter
uses the same resolved `MetadataSnapshot` and `QueryableField` primitives to
compile a small structured scan request with typed domains and bind parameters.
It accepts no raw PostgreSQL SQL from the Java process.

The Java connector stores accepted filters and the minimum limit in its table
handle. A worker sends schema, table, requested columns, typed domains, and
limit to Rust. Rust resolves all names again, selects only required physical
members, quotes identifiers internally, emits placeholders, binds values, and
streams rows. Unsupported filters remain above the scan in Trino.

SDBL-only semantics are exposed separately as the standard Trino 476
polymorphic table function `system.sdbl(QUERY varchar)`. During analysis Java
sends the SDBL source to Rust. Rust parses and compiles the bounded SELECT,
applies the shared structured reference-presentation policy, and asks
PostgreSQL to prepare the generated SELECT without reading rows. The resulting
column descriptor becomes the Trino table-function return type.

`ConnectorMetadata.applyTableFunction` replaces the invocation with the same
one-split streaming scan path. Workers carry only the original SDBL source and
analyzed result descriptor, never PostgreSQL SQL. Rust recompiles and verifies
the descriptor at execution, then wraps the compiled relation with ordinal
private aliases so projection and LIMIT can be applied safely. Predicates above
the table function remain in Trino until their placement relative to slice
ranking and balance aggregation can be proven semantics-preserving.

## Types

The adapter maps exact scalar storage to Trino boolean, integer, bigint,
decimal, varchar, date, timestamp(3), UUID, and varbinary. Fixed references
with one RRef payload use UUID. Multi-target references use a documented
`<type>:<uuid>` varchar because the type discriminator is part of the value.
Other compound values use JSON containing every physical member. Encoded
compound columns are visible but do not claim scalar predicate pushdown.

## Runtime and deployment constraints

- The Java plugin must be installed on the coordinator and every worker.
- Every node must reach the Rust service; every service pod must reach the 1C
  PostgreSQL endpoint.
- The first version returns one split and makes no parallelism claim.
- Metadata is cached with a TTL and refreshed as immutable generations.
- Passwords are supplied through secrets/environment and are never logged.
- All catalog mutations remain unsupported.
