# Compile scoped SystemSettings reads

Original UNF initialization reaches SystemSettingsStorage.Load. Expose a narrow PostgreSQL projection using the acquired system table, existing scoped settings compiler and existing bounded payload decoder. Preserve all CommonSettings SQL and host authorization boundaries.
