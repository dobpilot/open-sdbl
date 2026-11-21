# Design

The gateway handles these two fixed SQL-shaped functions itself after client
authentication. They must never be passed through as SQL to the backing DB.
Results use PostgreSQL text columns and the simple and extended protocols,
including parameters and portal row limits.

Build a logical metadata path index from the configuration catalog and confirmed
object and child resource formats. Keep resource identifiers separate from paths,
read resources using existing bounded, parameterized database APIs, and decode
only positively identified textual BSL or XML. Unknown formats must produce an
explicit error instead of guessed content. Respect existing state refresh,
connection lifetime, cancellation, and resource limits. The root library stays
independent of runtime dependencies and I/O.

The index follows the active base `Config` catalog. A form path resolves to
its XML when stored as XML, or to its textual BSL module when stored in a
supported managed-form format. Processing and report paths select their object
modules. A length-delimited data composition template returns the original
schema XML, which references settings variants held in separate documents.

These functions are deliberately available to every authenticated user; data
rights are not a substitute for controlling source access. Operators must be
told this access policy in the gateway documentation.
