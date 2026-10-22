# Native legacy report-event catalog sections

This paired native CF/XML resource belongs to СобытияОтправкиОтчетности, owner 4132dc03-17a4-4287-b1fc-a67d57fa8ea2. Original BUH module line 25 references ПричиныОтказа, UUID bc1b8607-1759-46c9-8ca9-d4b870ab0e96. The baseline SDK preserves its seven own attributes but not that section or its four fields. `verification.json` records this pre-fix snapshot, not the post-fix state.

Native owner class 53/58 and strict identity 1/9 are already supported. Section collection 932159f9-95b2-4e76-a8dd-8849fe5c5ded has one row 3, wrapper 1/3 ending 0, body 11/9 and identity 1/7. Fields collection 888744e1-b616-11d4-9436-004095e12fc7 has four row 2 entries with wrapper 6/4, body 27/23, typed 2/3 and identity 1/7. Identity comments are empty and flags 0,0; wrapper trailing flags are separately retained in native-field-wrapper-flags.tsv (0/1 for the strings, 1/1 for Boolean). No meaning is inferred for those wrapper flags.

Independent native XML checks cover every own and section UUID/name/order/type/qualifier: 12 type rows, including the compound own Отправка field. Four section fields in exact order are ИсходнаяПричина (String unlimited Variable), НеПоказывать (Boolean), Причина (String unlimited Variable), ПояснениеАудитора (String unlimited Variable). Two external native XML generated-reference sources and all 14 generated TypeId/ValueId locations are retained.

New tests reproduce baseline failure, confirm all four ordered declarations and preserved seven own attributes, verify the earlier report-deliveries native pair (one section/two String fields), and test 21 malformed count/wrapper/header/type/descriptor/identity/collision cases. All sections and fields stage before publication; malformed late entries cannot publish a prefix or overwrite own metadata. Existing tests are unchanged.

Only the existing staged helper is parameterized. Modern catalog/document paths, shared headers, catalog settings, public API and runtime object writes are unchanged. No live database read or write was needed. Version remains 0.9.20; no release, commit or archive is included.

Independent root review also compared all 752 native catalogs: 12 legacy53/58 rows change, 740 remain unchanged, including 727 modern non53 rows with identical full descriptor projection hashes. The changed rows restore 3 parts/7 fields, and all changed membership/order/names/type presence match XML. Nine changed rows have explicitly verified empty native section collections. The full qualifier proof stays scoped to the reached owner (12 rows); no claim is made for all qualifiers of all 752 catalogs. The independent batch source, native manifest and full before/after tables are retained.

The independent complete455-document regression is byte-identical to the earlier retained document-section batch-after.tsv; its final table and receipt are retained here.
