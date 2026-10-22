## Context
Native controls in the consumer requests establish owner Read for InformationRegister dimension/resource/Period and Document reference/own String. Input XML is not a native oracle. Retain native receipts and extract actual CF resources with existing container APIs.

## Decisions
Extend only the existing verified programmatic-field mapping. Confirm complete owner collections, known independent-register settings, resource ownership and nonempty declared types. Period requires the measured Day periodicity; other periodicities are not authorized by this change. Document reference requires recognized document settings; own attributes require complete own membership. Existing role iteration, explicit member Read overrides, principal selection, table-part refusal and RLS code remain unchanged. Do not infer UI or unsupported field semantics.

## Non-goals
Writes, document tabular sections, other standard fields, new RLS shapes, SQL bypasses, dependencies, publication and archival.
