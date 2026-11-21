## 1. Contract and acquisition

- [x] Validate this OpenSpec change before implementing observable behavior.
- [x] Decode a bounded textual module container without accepting compiled
      images as source, with tests against a native module resource.
- [x] Decode measured managed-form modules against Designer exports.
- [x] Measure object, form, module, and layout paths and textual contents against
      native configuration fixtures and verify acquisition against PostgreSQL.
- [ ] Verify acquisition against a live MSSQL instance with native fixtures.
- [x] Acquire bounded configuration resources and maintain a coherent index on
      initial load and refresh, without making unknown data look complete.

## 2. Gateway

- [x] Implement SQL function routing, argument validation and result descriptions
      for simple and extended query protocols, including bound parameters.
- [x] Index base configuration objects and supported child collections, with
      Unicode substring filtering.
- [ ] Cover additional nested collections and configuration extensions.
- [x] Return textual BSL and XML for exact logical paths; handle missing,
      ambiguous, binary and unsupported resources explicitly.
- [x] Allow all authenticated users to inspect configuration regardless of RLS.
- [x] Document SQL examples and the source-access policy.

## 3. Validation

- [x] Test real BSL, forms and XML layout resources, SQL protocols and security.
- [x] Run the workspace tests and strict validation of this change.
