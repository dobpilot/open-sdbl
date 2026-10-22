# Design

ParameterPointInTime carries ParameterDate and optional ParameterReference(ObjectId, physical RRRef). Boundary carries Date or PointInTime plus explicit Including/Excluding; a constructor defaults to Including. Object storage numbers come only from acquired metadata; unknown/non-document targets fail explicitly. Missing reference differs from typed-empty in the model.

For the measured Balance argument, absent and zero RRRef use date-wide boundaries: Including includes all same-second movements, Excluding excludes them. Nonempty points compare period, acquired recorder type number and RRRef. Raw point excludes its own recorder; raw date remains exclusive. These observations do not define standalone Compare or other query expressions. Other virtual tables and scalar positions explicitly diagnose unsupported temporal values.

Reuse historical totals+movement correction, date offset rendering, binary literals, metadata validation, rights/RLS and separator filtering. A structured temporal cut provides movement eligibility while the old date-only SQL path remains byte-for-byte unchanged. Totals at an anchor describe movements before the anchor; movement correction uses the measured cut on either side.

The existing logical result recipe has only Scalar and tagged Composite; a synthetic PointInTime has no active-type discriminator. Add PointInTime(date cell, typed-reference cell) to that same recipe. Keep the physical columns/labels unchanged, do not infer grouping from aliases. Unsupported transformations may retain None instead of manufacturing a recipe.

Evidence: docs/research/fixtures/temporal-balance-native-1019 (native source/XML/CF/DT, unmodified oracle, hashes; no production DB writes). Copy bounded durable evidence into tests/fixtures for delivery. PostgreSQL and MSSQL SQL must encode identical ordering; live MSSQL is not claimed.
