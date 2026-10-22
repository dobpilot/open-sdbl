# Request a bound shared separator value

## Why
The host supplies session values in response to structural compiler requests. With an explicitly enabled Shared separator, an absent CompileOptions value currently silently selects the empty area even when the real Session already contains B/2. The native-backed host controls establish the expected supplied-value result, not lazy initialization of an absent platform parameter.

## What Changes
Request an explicitly configured value parameter structurally when an explicitly configured use parameter is true. Preserve the no-use Shared default, the unbound-value fallback-name behavior, and false-condition/use handling. No dependencies, database writes, or new public types.
