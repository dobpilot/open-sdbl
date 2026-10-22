# Legacy sales-book document sections

Native CF/XML evidence comes from the existing `../buh-sales-book-document` fixture (owner `642117be-dba4-4c6c-b5e7-c2821543a830`). No database acquisition or write was used. The retained XML projections contain exactly six own sections, 68 fields and 201 qualified type rows. `xml-reference-type-mapping.json` records the 42 native XML reference-type sources.

The measured section wrapper is 1/2, body 11/9 and identity 1/7; field wrapper is 7/5, body 27/23 and typed declaration 2/3. Existing strict identity constraints remain in force. All sections and declarations are staged before publication, with the owner and seven own-attribute UUIDs reserved against collisions.

`red.log` reproduces missing metadata on the frozen own-attribute review. `collision-red.log` reproduces a cross-collection UUID collision before the reserved-identity correction. `focused.log` records all four new tests passing. Existing test files remain unchanged; their former `document_tabular_sections.is_none()` assertion awaits explicit approval in a separate one-line proposal.

The batch compares all 455 native document XML files with the frozen own-attribute SDK and this review. Four rows change (10 sections / 103 fields); 451 remain identical. All changed section and field memberships, orders and names match XML; existing own-attribute metrics and all modern class-40 rows remain identical. This batch checks membership and declaration presence, not every qualifier of every document. The reached document has separate exact 201-row qualifier verification. Unknown profiles remain unknown.

Version remains 0.9.20; this isolated review is not a release.
