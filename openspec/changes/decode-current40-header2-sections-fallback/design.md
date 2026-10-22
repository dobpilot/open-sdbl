# Design

Native CF/XML establishes class40/53, ten own attributes, two parts/ten fields and twenty qualified type rows. All23 owner/own/part/field headers are2/8 with reference1/0/UUID, nonempty String name, synonym1/3 with String ru/nonempty text, empty String comment, atom flags0/0 and nil-GUID tail. Sections use doc collection21c53e09-8950-4b5e-a6a0-1054f1bbc274, row3, wrapper1/2, body11/9 and identity wrapper0/2. Fields use collection888744e1-b616-11d4-9436-004095e12fc7, row2, wrapper8/5 ending0/1/1, body27/23, typed2/3 and the same header2/8.

Keep the original part lookup first. Only its None result, known complete own attributes, class40/53 and a locally fully validated owner2/8 enable the new fallback. In particular owner2/8 plus previously recognized part3 must continue through the old Some route unchanged. Local strict version2 recognition reuses object_header for ID only after shape/synonym/comment/flag/tail checks; neither shared header function changes.

Parameterize the existing complete-set section helper with the measured local header and field profiles. Reuse existing declaration parsing. Reserve owner+own identities and verify all parts/fields/descriptors before publishing any NEW membership or type. Invalid late entries leave the new fallback unknown and preserve own metadata and old settings. Do not fix old partial modern3 behavior or change numbering/registers.

New external tests retain the whole native pair, malformed owner/part/field controls, collisions and a marked synthetic control with only section tags changed to3. The latter proves old Some(parts) behavior stays Some2 with unknown field metadata. Native synthetic tag controls are not claimed as additional native formats. Existing tests are untouched.
