# Design

## Initialization
`GatewayState` owns an `Arc<Catalog>` containing every supported relation and field. Candidate compilation is independent and runs in scoped worker threads, bounded by `available_parallelism`; workers return ordered chunks so OIDs and catalog row order remain deterministic. Preparation and final compilation retain their existing failure and empty-relation rules, but use unrestricted decisions because this phase describes the complete metadata inventory rather than a user's view.

Each compiled candidate retains its restriction request and the complete field-usage request associated with every visible result column. These requests are the authorization metadata needed later; no user rights are evaluated during the complete build.

## Sharing and personalization
Catalog tables are reference-counted. Cloning the complete catalog for a connection clones table handles rather than all rows. Only small session-specific tables (`pg_roles`, `pg_user`, `information_schema.schemata` ownership and `pg_settings`) are copied on write. OIDs and the snapshot revision remain those of the prebuilt state.

With `unrestricted_access`, the connection receives this shared complete catalog without role or field checks.

## Restricted filtering
Without unrestricted access, the connection evaluates each candidate's saved object/section restriction request with its derived session parameters and existing decision cache. It applies the existing complete field-rights check to each saved column-usage request. A visibility index of relation OIDs, schema OIDs and allowed attribute numbers filters the prebuilt catalog tables, views and schema list. Hidden objects and fields remain absent from all catalog relations and function lookups. A new connection repeats only authorization and filtering, never relation parsing or code generation.

Metadata refresh acquires a new complete catalog concurrently. Statements retaining an old `GatewayState` also retain its old catalog and revision.
