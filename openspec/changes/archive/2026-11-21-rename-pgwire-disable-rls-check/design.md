# Design

Map TOML `disable_rls_check = true` to `--disable-rls-check` using the existing
configuration conversion. The gateway-specific flag continues to populate the
existing internal unrestricted-access option and public `GatewayConfig` field;
no authorization code or Rust API changes. A false TOML value omits the flag.
Console, repl and metadata accept and type-check the gateway-only field while
ignoring it, consistent with shared configuration. Reject the old names instead
of silently accepting or ignoring them, including in non-gateway commands.

The new name might suggest that only row filtering is disabled. Help and
documentation must state that it also bypasses role-based object and field
checks and exposes the supported catalog to any successfully authenticated
user. Authentication and database-level permissions are unchanged. The
existing no-override default remains fail-closed. No dependencies are added.
