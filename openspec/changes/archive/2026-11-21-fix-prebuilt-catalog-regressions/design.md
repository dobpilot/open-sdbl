# Design

Catalog compilation uses synthetic type-neutral separator values solely to determine
the relation's columns and access requests; it never executes the generated
SQL. Real queries and authorization keep their actual session parameters.
Gateway state acquisition offloads the completed snapshot's catalog build to
Tokio's blocking pool. Embedded gateway connections personalize database names
when their state was constructed for a different database, preserving the
existing public state constructor.

Resolution uses scoped workers only where threads are supported and available,
and otherwise preserves sequential processing and deterministic ordering.
