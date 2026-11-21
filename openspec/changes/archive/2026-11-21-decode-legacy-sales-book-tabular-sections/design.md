# Design

The same retained class 38/51 document owner uses section collection 21c53e09-8950-4b5e-a6a0-1054f1bbc274 at root[3]. Section rows have three entries: wrapper 1/2, flag 1, and field collection 888744e1-b616-11d4-9436-004095e12fc7. Section bodies are 11/9 with identity 1/7 at relative path 0/1/5/1. Body positions 1..4 match all 24 generated section and row type/value identities in XML.

Fields have row length 2, wrapper 7/5, body 27/23, typed declaration 2/3 and identity 1/7 at 0/1/1/1, with Pattern at 0/1/1/2. All 68 identity comments are empty and flags are 0,0. Native/XML order, all 201 type rows and qualifiers and 42 reference identities agree. Nine fields are compound. No modern offsets are guessed.

Split the existing document_attributes implementation into read-only staged typed-row parsing and publication. Keep its previous own-attribute callers behavior unchanged. Add an explicit legacy section-field profile selecting only collection 888744e1-b616-11d4-9436-004095e12fc7 and wrapper 7/5, reusing the measured legacy identity helper and common declaration parser.

Validate all section identities, uniqueness, descriptors, collection counts and all field declarations before committing any of the six section memberships or 68 declarations. A malformed last section or field leaves no published section prefix. The already acquired seven own document attributes remain intact. If own attributes are invalid, do not enable the additional legacy section projection, preserving earlier fail-closed tests. Do not alter shared header(), the modern tabular path, unrelated legacy processor section code, numbering or register metadata.

The preceding review is frozen. Versions stay 0.9.20; no old tests, lockfiles, commits, release or archives are changed. A separate inherited UUID SQL corpus mismatch remains pending. Compare all 455 native document section projections before/after and record unchanged own attributes and modern rows.

Reserve the owner resource identity and all seven own attribute identities before staging section/field identities. A section or field must not alias those descriptors: otherwise publication could repurpose an existing owner or own attribute even when descriptor lookup is unique. Four collision controls verify rejection and exact preservation of all own declarations.
