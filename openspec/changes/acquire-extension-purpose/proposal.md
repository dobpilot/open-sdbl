# Acquire extension purpose

## Why
Original configuration startup reads the extension purpose. Acquisition currently omits this independent native property.

## What Changes
- Add typed ConfigurationExtensionPurpose and optional acquired purpose.
- Decode only native-measured complete records; preserve unknown values.
- Release version 0.9.15 without new dependencies or provider-specific decoding.

## Impact
- Public metadata and acquisition API gains one enum and optional field.
- PostgreSQL and SQL Server retain the shared decoder and read-only acquisition.
