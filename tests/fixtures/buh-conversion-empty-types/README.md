# BUH conversion processor: native empty declarations and comments

Retained native Designer CF/XML pair from BUH, without modification. Owner: cae8a6f2-bbe9-4b73-b839-9bf8e5e2818d. XML declares 43 attributes; four have an empty Type element, and two carry nonempty textual comments. The former SDK 0.9.20 profile rejected both conditions and discarded the complete attribute list.

The external runtime_metadata regression compares all 43 ordered identities, names, declared types, raw reference identities and qualifiers against the native pair. It checks four known empty declarations, three ValueTable references, both textual comments and nine malformed variants. This establishes metadata representation only, not runtime assignment semantics. The original module is not modified.

The standalone verify.rs provides a smaller consumer check against xml-attributes.tsv. It checks membership, four empty declarations and three ValueTable references; full qualifier coverage belongs to the Rust regression. sdk20-red-tests.log records the unmodified decoder failing the new native test before implementation.
