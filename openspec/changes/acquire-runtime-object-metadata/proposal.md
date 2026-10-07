# Change: Acquire runtime object metadata

## Why
The host requires complete business-process object members and accumulation-register dimensions and resources. Existing field-purpose classifications do not guarantee ordered complete membership.

## What Changes
- Expose measured business-process numbering, own attributes, sections and standard filling-check policies.
- Expose measured accumulation-register kind and ordered dimensions/resources.
- Preserve unknown and verified-empty states through snapshots and selective acquisition.
- Release all workspace packages as 0.9.9 without implementing ORM execution or SQL generation.

## Impact
Public ConfigDescriptor fields and metadata exports; existing decoding contracts remain unchanged.
