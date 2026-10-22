# Design
Move the existing private Common/System storage selector and physical names into a crate-private helper shared by projection and settings compiler. Seed selected physical table names only from observed supported tables, before the existing field discovery and DbNames projection. This reuses separator flags and strict missing-descriptor checks; no full metadata fallback is introduced. Existing whole resolver and settings SQL semantics remain unchanged.

Tests compare common/system SQL and scopes from native separator fixtures through whole, selected and cached resolution, reject missing/duplicate scopes, preserve prepared snapshot validation and exclude an unrelated ownerless table. No new 1C semantics are inferred.
