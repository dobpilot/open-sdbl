# BUH data-load sessions catalog reconnaissance

Owner `c4a91334-1238-4808-8511-77875d713c00`: `СеансыЗагрузкиДанныхВнешнихСистем`. Object module reads/writes bare ДатаЗавершения at lines 7–8. Sources are from existing native BUH CF/XML, without database access or SDK build.

Frozen public SDK returns three descriptors (owner and two attributes), but catalog_attributes/catalog_tabular_sections None and zero declared types. This is projection omission before ORM context, not evidence of lost object context.

Measured class54/59, owner wrapper0/2 and identity1/7: reference0/0, nonempty String name, synonym1/3 ru/nonempty String, empty String comment and flags0/0. Eight GUIDs at class[1..9] exactly match four XML generated TypeId/ValueId pairs (Object/Ref/Selection/List); the fifth Manager pair is independently matched at class[34]/[35], without interpreting any adjacent fields. Own collection cf4abea7... has two rows: wrapper4/6, tails[0,0,1,1] and[1,0,1,1], body27/23, typed2/3, identity1/7. Both own comments empty and flags0/0. ДатаНачала/ДатаЗавершения Pattern[D] match XML xs:dateTime with DateTime qualifier. Native section collection932159f9... is exactly empty, matching XML. All UUIDs/names/order/types/qualifiers checked in profile.json and xml-attributes.tsv.

Streaming runtime_class_seen excludes54. Runtime projector recognizes catalog53 or56/57, not54; existing53 own wrapper3/5 also differs from4/6. Adding only54 to preflight cannot restore projection. No synthetic cross-class slot substitution was made:54/59 differs from53/58, and removing an unexplained slot would not prove causality.

Minimal fix after separate strict OpenSpec: explicit54 runtime preflight, strict local owner54/59 using existing identity1/7 checks, measured own4/6 profile through existing fully staged typed rows. Publish complete own/types and the verified exact-known-empty section collection; do not claim any nonempty54section layout. No catalog settings, guessed offsets, new runtime APIs or shared-header relaxation.

No production/version/old-test change. Only a standalone consumer linked an existing frozen rlib.
