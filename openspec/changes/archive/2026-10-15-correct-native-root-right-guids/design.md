# Design

Use the existing RIGHTS table and parser. Native 1C 8.3.27.2342 own-role measurements establish SaveUserData=d8682bbb-7800-4aa0-8590-d3cb11fe2a29, ConfigurationExtensionsAdministration=399d7390-8d83-4a57-b4d7-c902c15b701f, InteractiveOpenExtDataProcessors=7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6, and InteractiveOpenExtReports=eb29e198-c338-4a20-a253-be6fc3dd44d9. A role granting only each named right supplies independent positive and negative AccessRight results on the platform. Its CF resource contains the corresponding allowed GUID and explicit/default refusals of the other measured rights.

The previous 4df6d046-3bf8-4dda-991c-53ba664296a5 association has no evidence and is removed; it remains Other rather than a compatibility alias. No right implication or hierarchy is inferred. The adapter continues to request Right::SaveUserData by its honest name.

Source artifacts originate from the open-bsl own file-infobase control, retained here under tests/fixtures/native-root-right-guids-1015. The original databases are untouched. The workspace packages are released as 0.9.16. The existing unknown-identifier test uses a synthetic unassigned GUID because its former input is now independently measured as InteractiveOpenExtReports; its Other assertions remain unchanged.
