# Design

Use the measured root envelope `{1,{1,{2,<own header>,<Pattern>}},0}` only. Check each list length, tag, own header identity matching the resource and uniqueness of the target descriptor before publishing. Reuse the existing declaration decoder so unknown tags and incomplete patterns remain unknown and no prefix is returned. Preserve raw GUIDs and primitive qualifiers; do not scan arbitrary descendants for a Pattern. Root forms of constants and defined types already have separate projectors and remain untouched.

The native UUID GUID is an opaque reference declaration in the SDK; UUID runtime interpretation belongs to the existing consumer contract. No platform behavior is inferred.
