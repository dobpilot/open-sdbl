## 1. Public facade

- [x] 1.1 Add immutable generic `QueryCompiler<B>` bound to `MetadataSnapshot`
  and one backend value.
- [x] 1.2 Add separate PostgreSQL and MSSQL backend values with specialized,
  parallel APIs.
- [x] 1.3 Remove existing public free functions and migrate every workspace
  caller to `QueryCompiler<B>`.

## 2. Module boundaries

- [x] 2.1 Reduce `query.rs` to the public facade and provider-neutral exports.
- [x] 2.2 Move shared parsing and compilation into a private functional core.
- [x] 2.3 Place PostgreSQL and MSSQL backend implementations in separate
  modules without duplicated compilation logic.

## 3. Verification

- [x] 3.1 Cover both generic compiler backends and verify the legacy public API
  is absent.
- [x] 3.2 Update README examples for the generic backend API.
- [x] 3.3 Run formatting, Clippy with warnings denied, workspace tests, rustdoc
  with warnings denied, and strict OpenSpec validation.
