# Design

The unchanged BUH resource has root1/3 with trailing0, class2/5, header1/7 with reference0/0/non-nil owner UUID, String name, Russian synonym, String comment and zero flags. Location is a300ecc3-8031-4a50-a17d-b08cf3a72139; populated Content has six valid owner wrappers, and the last class flag1 matches native PrivilegedGetMode=true.

A separate existing native CF/XML pair in functional-option-content-native independently measures false and true using the same class2/5 final0/1 flag with modern headers. This is sufficient evidence for existing flag parsing; no mutation is called a native false control and no extra live database operation is needed.

Add a local old-header functional-option branch. Reuse legacy_data_processor::identity_with_text_comment unchanged, functional_option_content and flag; do not expand object_header globally or alter existing modern branch behavior. Require the identity to equal the resource, exactly one matching descriptor, complete valid root/class/content/location/mode, and stage both settings before publication. Malformed/unknown forms leave fields unknown.

Public resource parsing returns locationGUID; resolved Constant/CatalogAttribute/RegisterResource location belongs to the existing global resolution phase and is not a separate raw-parser defect. New tests preserve owner identity, exact native locationGUID and mode, both existing native false/true controls and fail-closed negatives. ROOT independently compares the whole native corpus, including unchanged modern and non-option descriptors. No version, commit, archive or old-test edits.

The native corpus independently confirms 121 old identity1/7 headers: 115 empty and six nonempty String comments. Both are recognized; no tag0/5 or tag2/8 profile is added. Reuse the existing text-comment identity entry point, and update its explanatory comment only; strict identity callers retain their old behavior. The other 64 previously unknown option profiles remain unknown.
