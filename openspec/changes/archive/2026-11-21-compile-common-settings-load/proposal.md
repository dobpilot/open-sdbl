# Compile scoped common-settings reads

## Why
The original demo startup loads one common setting. Its host must read the exact user/separation scope without constructing SQL or treating missing scope as unrestricted access.

## What Changes
Add a pure QueryCompiler method for a bounded PostgreSQL common-settings read using acquired live columns, DBNames and explicit resolved separation states. Reuse existing dialect literal/identifier rendering. No new SDBL syntax or database driver path is introduced.

## Impact
Additive SDK Rust API; no writes, dependencies or BSL interpretation. Other backends and unmeasured non-string separators fail explicitly.

The unpublished scope prototype now represents three measured states explicitly: active typed value, inactive separator and disabled global condition. Add measured numeric separators alongside strings, reusing ParameterValue and the existing decimal renderer. The initial Option<String> could not represent numeric values or distinguish inactive from the canonical globally disabled scope.
