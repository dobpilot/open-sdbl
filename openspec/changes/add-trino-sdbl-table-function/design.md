## Context

The core query compiler already supports `Presentation`, `RefPresentation`,
reference-property presentation, information-register slices, and accumulation
register balance/turnover virtual tables. The Trino adapter currently builds a
physical scan directly and therefore cannot reach those compiler paths.

Trino SPI 476 supports connector-provided polymorphic table functions. Their
analysis can determine a dynamic result descriptor, and
`ConnectorMetadata.applyTableFunction` can replace the invocation with a
normal connector table scan. This preserves the existing split and streaming
record path.

## Decisions

### Provide one bounded SDBL table function first

The function is named `system.sdbl` and accepts one required VARCHAR `QUERY`
argument. Its value is SDBL source, not PostgreSQL SQL. Rust parses it with the
bounded core parser, which accepts SELECT only, resolves it against the current
metadata generation, and produces PostgreSQL SQL internally.

This exposes all compiler-supported SDBL operations without duplicating their
semantics as Java scalar functions. A Java presentation UDF would require a
database lookup per row and is deliberately not introduced.

### Determine the polymorphic result using PostgreSQL prepare

The core compiled query contains stable output labels but intentionally has no
Trino dependency. During table-function analysis the Rust service prepares the
generated SELECT in PostgreSQL without executing it and maps the returned
PostgreSQL column types to Trino signatures. Duplicate or empty labels receive
stable disambiguated names in the transport descriptor.

### Revalidate at execution

The connector handle carries the original SDBL source and its described
columns, never generated PostgreSQL SQL. A worker sends the source back to the
Rust service, which recompiles and verifies the output shape against the
current immutable metadata generation before execution. A changed shape fails
explicitly instead of decoding rows under stale types.

### Wrap the compiled relation for pushdown

Rust assigns ordinal private aliases to the compiled result and builds a safe
outer SELECT for the requested output ordinals and pushed limit. Identifiers
are generated internally. The initial slice leaves outer Trino predicates for
Trino evaluation because moving them inside `SliceLast` or `Balance` can change
1C semantics.

### Reuse one presentation policy

The default presentation-plan builder moves from the CLI application into the
dependency-free core query module. Both CLI and Trino call this safe structured
policy; neither application duplicates reference-target logic or raw SQL.

## Error handling

Invalid SDBL produces a compiler error with line and column. PostgreSQL prepare
or execution errors remain distinct. Empty result schemas, stale output shapes,
unsupported result types, and invalid projection ordinals are protocol errors.

## Compatibility

The Java implementation uses only table-function APIs present in the locally
verified `io.trino:trino-spi:476` artifact. Ordinary catalog tables and their
existing handles remain unchanged.
