# Checked field access and missing session parameters

## Problem

The database layer flattens a missing RLS session parameter into a string, and query diagnostics do not expose a missing parameter name. A host cannot initialize only the parameter that a restricted query needs without parsing diagnostic text. The query compiler reports every field a prepared query reads, but the database layer has no all-or-error check of the current user's field rights. Table-level decisions alone are insufficient for a host that executes queries under a selected 1C user.

## Change

- Preserve missing parameter names as structured errors through restriction expansion and query compilation.
- Add a read-only, all-or-error access check over `FieldUsageRequest` for the current user and loaded role rights.
- Keep unsupported or unresolved rights cases explicit and closed.
- Release `open-sdbl` and `open-sdbl-db` as 0.9.3.

## Scope

This change does not authenticate users, load rights implicitly, interpret arbitrary BSL, or change generated SQL. The host remains responsible for `ensure_users`, `ensure_rights`, current-user selection, and lazy session-parameter initialization.
