# Design

Reuse decode_data_separation for complete structural validation, then accept only lengths 31, 37 and 38 with zero separation bindings. Read the existing atom GUID at position 6. No SQL, dependencies, fallback or acquisition changes.

Evidence: user-language-legacy-native-1018, official 1C 8.3.27.2342. Own reconstructed records were read through the user manager and remained byte-identical. This is not an old-version dump or an authentication test. Form 32 and records with separation bindings remain unmeasured for language.
