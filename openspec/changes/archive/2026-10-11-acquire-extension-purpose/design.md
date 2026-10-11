# Design

Purpose is independent of name, active, version and scope. Native disposable PostgreSQL infobases will provide Patch, Customization and AddOn extension records, CFE and XML exports and native property receipts before decoding is implemented. ConfigurationExtensionPurpose lives in the metadata boundary and is exposed through its existing public path. The acquisition layer carries Option of that same enum. Unknown or malformed shapes remain None. No inferred default, extra per-session connection or dependency is introduced.

The exact measured source and structural checks will be recorded after measurement, before implementation. Existing SQL and error assertions remain unchanged.

## Measured source

Native 8.3.27.2342 installed extensions, CFE/XML and ConfigCAS agree: purpose is configuration root version 2, class group 9cd510cd-abfc-11d4-9434-004095e12fc7, wrapper version 1, header version 68 with 61 fields, compatibility at index 43 equal to 80327 and purpose at index 44. Codes 0/1/2 map to Patch/Customization/AddOn. The root is selected by the existing extension index, not by name or a scanned integer. SQL rows carry identical compressed configuration resources to CFE. Evidence with SHA-256 lives in tests/fixtures/extension/purpose-0915.

ExtensionZippedInfo contains no purpose, so ExtensionInfo gains no field. ExtensionCatalog carries optional purpose decoded while its existing complete resource is parsed once; AcquiredExtension copies that value. A missing, duplicate or unsupported configuration group/header or unknown purpose yields None without rejecting an otherwise complete catalog. Both providers use this same acquisition path, unchanged SQL.

## Measured older compatibility modes

Read-only current demo roots match header68/61/80327. Current buh roots have the same header68/61 layout with compatibility80324 and80317. Disposable native XML import/apply reproduced all three purposes for each of those two CompatibilityMode values. CFE/XML/native-info/ConfigCAS pairs confirm index44 remains 0/1/2. The supported compatibility guard is exactly80327,80324,80317; other versions remain unknown. Source fixtures retain all nine native variants.
