# Correct constant value authorization

## Why
UNF initialization is rejected when querying `ОсновнойЯзык`: its value carries the constant's own GUID, while the current checker incorrectly expects a separate member Read grant. Native 1C measurements show that constant object Read authorizes its value independently of the default-attribute flag.

## What Changes
- Authorize only verified constant values by their owner's Read grants.
- Preserve conservative custom and standard-field member checks.
- Record native evidence, explicit object denial and cross-role regression coverage.
- Release as 0.9.5.

## Impact
Database field authorization, its contract, regression fixtures and release version.
