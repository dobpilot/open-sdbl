# Design

Native CF/XML establishes class 53/58, owner identity 1/9 at [1,9,1]: reference 0/0/owner, nonempty string name, synonym 1/3 with language ru and nonempty string text, empty string comment, flags 0/1, additional nonnil GUID and final 2. Validate locally, including eight generated GUIDs, without widening shared headers.

Select class 53 for runtime preflight. Reuse staged typed-row parsing with wrapper 3/5 for collection cf4abea7-37b2-11d4-940f-008048da11f9. Its 16 rows have body 27/23, typed 2/3, identity 1/7. Reject invalid counts, wrappers, headers, types, duplicate identities/descriptors, missing descriptors and owner collisions before publishing anything.

Compare all 16 UUIDs/names/order/references/qualifiers against XML, including date-only D,D. No tabular support, catalog settings offsets, shared-header expansion or existing-test edits.
