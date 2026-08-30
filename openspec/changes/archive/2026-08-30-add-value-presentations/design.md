# Design: Application-defined reference presentations

## Decisions

### Stable identities at the application boundary

`ObjectId` and `AttributeId` contain the real 16-byte 1C metadata GUID.
`FieldId` is either `Metadata(AttributeId)` or `Standard(StandardFieldId)`.
`StandardFieldId` is a stable numeric enum because platform standard fields do
not have Config attribute GUIDs. Strings occur only as literal template text.

The metadata snapshot builds immutable hash indexes for GUID, normalized
kind/name, owner/name, and DBNames database type number. Ambiguous normalized
names produce typed errors rather than an arbitrary match.

### Two-phase compile-time callback

Compilation proceeds as follows:

1. Parse and resolve the query against the metadata snapshot.
2. Determine every possible target object of every requested reference
   presentation.
3. Return one deduplicated `PresentationRequest` batch to the application.
4. The application returns one `PresentationPlan` per requested object ID.
5. Validate every field ID against its owner and validate the structured
   template AST.
6. Generate SQL, reusing source columns and joins already needed by the query.

The core never executes a callback while reading rows and never accepts raw SQL
or a source-language expression from the application.

### SQL shape

- A presentation of the source object's own `Ссылка` uses source columns.
- A fixed reference target uses one reusable `LEFT JOIN` by reference ID.
- Multiple possible reference targets use one guarded `LEFT JOIN` per target
  and a `CASE` selected by the RTRef database type discriminator.
- A scalar `ПРЕДСТАВЛЕНИЕССЫЛКИ` is passed through; a scalar
  `ПРЕДСТАВЛЕНИЕ` is converted to text.
- A compound value mixing reference and scalar alternatives is rejected until
  its complete platform type semantics are implemented.

### CLI caching

The CLI provider owns a bounded `moka::future::Cache`. Its key includes a
metadata generation, object ID, language, and presentation-policy version.
The value is a validated application presentation plan. Refreshing metadata
increments the generation, so an old plan cannot be used for a new snapshot.
The authoritative metadata indexes remain ordinary snapshot-owned hash maps;
Moka is not used in the dependency-free core.

The default CLI policy chooses available standard `Наименование` and `Код`
fields and creates the safe structured equivalent of
`Наименование + "(" + Код + ")"`, with deterministic fallbacks for object
kinds that do not expose both fields.

### Rust compatibility

Both workspace packages remain Rust Edition 2024 with MSRV 1.85. The core
retains zero production dependencies; Moka and Tokio integration are CLI-only.
