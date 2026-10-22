# Design

Reuse SeparatorState::Missing and the existing session-parameter diagnostic. Change only Shared with both explicit bindings and Boolean(true) use. Independent remains unchanged. A missing value in compiler options is not evidence that the real Session parameter is uninitialized. The protocol lets the host provide the already-set value; no new platform lazy-initialization promise is made.

A Shared separator without a use binding retains the typed empty default. With use=true but no value binding, preserve the existing common-attribute-name fallback and typed empty default when that name is absent; the request does not broaden this shape. Conditions are resolved before use/value and use=false disables Shared without requesting a value.

Tests exercise String/Number and PostgreSQL/SQL Server generation; live PostgreSQL replay belongs to the host integration. SQL Server live execution remains unverified. Existing conflicting assertions require a separately reviewed proposal rather than silent edits.
