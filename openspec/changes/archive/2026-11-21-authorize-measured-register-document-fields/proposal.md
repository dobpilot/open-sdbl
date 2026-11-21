# Change: Authorize measured register and document programmatic fields

## Why
Measured native reads succeed with owner Read regardless of UI attribute defaults, but the SDK rejects InformationRegister and Document fields.

## What Changes
- Recognize verified information-register dimensions, resources and periodic Period.
- Recognize verified document reference and own declared attributes.
- Preserve selected principal, complete role acquisition, role union, explicit member Read denial, unresolved-shape refusal and query/RLS enforcement.

## Impact
- Affected specs: access-rights
- Affected code: database field authorization and native fixture tests; release 0.9.23.
