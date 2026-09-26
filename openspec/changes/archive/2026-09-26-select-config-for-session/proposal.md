# Select Config resources for a session consumer

A configuration-backed interpreter needs the root and object/module resources, not every form, layout and binary asset. Whole-configuration acquisition currently retains both the full Config table and ConfigCAS, which exceeds a practical memory budget on large bases. It also needs synchronous access to the users and role rights already read by the asynchronous session.

Add a selected acquisition on the same database session and blocking wrappers for existing access reads. Preserve the existing whole and metadata-only APIs and their behavior.
