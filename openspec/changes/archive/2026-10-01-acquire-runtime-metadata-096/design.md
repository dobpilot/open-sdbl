# Design

Use existing Config resources, parser, descriptor identities and type descriptions. Compare each supported shape to UUID-matched native XML; do not infer settings from SchemaStorage or SQL. Unknown and incomplete collections remain None, including unresolved named types and filling references. PostgreSQL and SQL Server share decoding. Standard permission mappings require native rights evidence; unavailable evidence remains fail closed. No writes to working infobases; native measurements use isolated disposable infobases.

## Measured catalog query rights

On a disposable file infobase, five explicitly authenticated users queried one actual row. Object Read allowed Ref, Code, Description, DeletionMark, Predefined, PredefinedDataName and a custom attribute with both attribute-default flag values and explicit View refusal. Object Read refusal denied all seven queries. This proves programmatic SQL semantics for these native catalog fields, not UI View/Edit or document/register fields.

Authorize only verified catalog Config owners with complete own membership and recognized standard settings. Map native Ref/Code/Description/DeletionMark to existing StandardFieldId; custom GUIDs must be present in the owner's own list and have a recognized declaration. Unverified compact snapshots retain the existing conservative refusal. Explicit member Read refusals and ambiguous paths remain conservative refusals; no role default flag is interpreted as field Read. RLS and separation remain enforced by existing query authorization.
