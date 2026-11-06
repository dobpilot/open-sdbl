## ADDED Requirements

### Requirement: Prebuild the complete catalog concurrently
Acquiring a gateway state SHALL build the complete supported virtual catalog before that state is served to clients. Independent relation compilations SHALL run concurrently on bounded scoped worker threads and SHALL preserve deterministic candidate order and stable OIDs. The complete build SHALL retain object/section restriction requests and per-column field-usage requests for later authorization. Catalog SQL requests SHALL NOT parse or compile the metadata relations to construct their model.

#### Scenario: First and subsequent connections share the built model
- **WHEN** several connections issue their first catalog query against one gateway state
- **THEN** each query uses the catalog built during state acquisition and none rebuilds or recompiles the relation inventory

#### Scenario: Refresh prebuilds the replacement model
- **WHEN** metadata refresh acquires a replacement state
- **THEN** the complete replacement catalog is built before the state is published and operations retaining the old state keep its catalog

### Requirement: Share immutable catalog storage between connections
Connections using one gateway state SHALL share the storage of unchanged catalog tables. Session personalization SHALL copy only the affected session-specific tables and SHALL NOT clone the complete relation and attribute inventories. Settings changes on one connection SHALL NOT modify another connection's catalog.

#### Scenario: Unrestricted connections do not copy the inventory
- **WHEN** two unrestricted connections personalize the catalog and issue catalog queries
- **THEN** they share unchanged table storage and observe their own user and settings

### Requirement: Filter the prebuilt catalog by user rights
A relation SHALL be listed only when the user's decision for reading it is not denied, and a column only when the complete field-rights check of the user proves it readable; a relation that cannot be decided SHALL be omitted. Hidden objects and fields SHALL be absent from every catalog relation and function result. Restricted connections SHALL apply these checks to the saved access requests of the prebuilt complete catalog without recompiling relations.

#### Scenario: Denied catalog is invisible
- **WHEN** no role of the user may read `Справочник.Контрагенты`
- **THEN** neither `\dt`, `pg_class`, `information_schema.tables` nor a lookup by its name returns it

#### Scenario: Restricted catalog is visible
- **WHEN** the user's roles restrict reading of `Справочник.Контрагенты` with a row-level condition
- **THEN** the table is listed; its rows stay filtered when queried

#### Scenario: Unreadable attribute is hidden
- **WHEN** the field check does not prove an attribute readable
- **THEN** the attribute is absent from `\d`, from `information_schema.columns`, from `pg_attribute` and from column descriptions

### Requirement: Reuse the metadata snapshot loaded at startup
Catalog introspection SHALL use the loaded in-memory metadata snapshot. Field lookup indexes SHALL be built once during snapshot finalization and reused across relation compilations and connections. The complete relation catalog SHALL be compiled once per gateway state and shared by connections. A refresh SHALL use the new snapshot's indexes and prebuilt catalog. Catalog requests SHALL perform no metadata or database reads and SHALL retain the complete user-rights checks in restricted mode.

#### Scenario: Many objects with custom attributes
- **WHEN** a user first runs `\dt` against a state with thousands of objects and tens of thousands of custom attributes
- **THEN** the request reuses the catalog compiled during state acquisition rather than compiling relations on the connection task
- **AND** the database receives no statement for the catalog request

#### Scenario: Refreshed attribute names
- **WHEN** metadata refresh replaces the snapshot with renamed attributes
- **THEN** subsequent introspection uses the replacement state's prebuilt catalog and the old snapshot remains valid for operations already using it
