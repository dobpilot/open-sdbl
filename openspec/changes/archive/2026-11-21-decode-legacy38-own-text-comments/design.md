# Design

Whole native CF/XML proves class38/51, nine own attributes, one section with 18 fields and 32 qualified type rows. Two own identity1/7 comments are nonempty Strings. Emptying only these restores exact metadata on the frozen public parser.

Reuse one identity validation body. The existing strict entry point retains empty-comment semantics; a private text-comment entry point accepts any String and is called only by DocumentAttributeProfile::Legacy38 own rows. Owner37/38, section38, legacy processor and other profiles remain strict. Never clone or normalize native Values. Preserve reference, synonym, flags, descriptor and atomic publication checks. No settings, numbering, offsets, type grammar or runtime changes. Existing tests remain unchanged, including two inherited failures.
