# Native legacy BUH sales book document attributes

The unchanged native Config/Designer XML pair identifies document 642117be-dba4-4c6c-b5e7-c2821543a830 with class 38/51, owner identity 1/7 and seven own attributes. Member wrappers are 4/5, bodies 27/23, typed declarations 2/3 and identities 1/7. All seven GUIDs, names, raw declarations and qualifiers were independently verified directly in the native serialized tree against the XML-derived expectation table. Reference identities are verified in the paired catalog XML.

The original SDK already produced all seven descriptors but no declarations or owner membership. The new regression first reproduced that failure, then verified complete ordered membership and seventeen malformed variants without publishing prefix declarations. Nine unchanged existing credit-document tests verify modern 40/53 compatibility. Numbering, register membership and tabular-section metadata remain unknown for this legacy profile; XML properties are not inferred from modern field positions.

The full 455-document before/after batch retains both TSV files and uses the unchanged all-documents-sdk20.tsv from ../buh-credit-document-0920 as its baseline; the frozen news review had zero changes against it. Exactly five class 38/51 rows change, restoring 35 own attributes. All 450 other rows, including all modern documents, are identical. This checks acquisition shape/counts and presence of child declarations, not every qualifier of every document and not startup success.

| Changed document | Native own attributes |
|---|---:|
| УдалитьЗаявкаНаОткрытиеСчетов | 14 |
| УдалитьОтражениеЗарплатыВУчете | 5 |
| УдалитьРегистрацияОплатыНДСВБюджет | 4 |
| УстановкаЦенНоменклатуры | 5 |
| ФормированиеЗаписейКнигиПродаж | 7 |

All package versions remain 0.9.20. No existing test expectations are edited. The native compressed resource and XML files are retained byte-for-byte.

Native GUID order also matches XML for all five changed rows; native-order-after.tsv retains the order check for every document.

The batch still reports 46 unknown memberships: 32 class 38/51 documents and 14 class 37/50 documents. All remaining class 38/51 documents have at least one nonempty owner or child identity comment (10 owners, 31 documents with nonempty child comments; union 32). Their identity flags remain zero and no empty Pattern was observed. The reused measured identity helper intentionally still requires empty comments; this patch does not broaden it. These are diagnostic candidates only, not newly supported profiles or inferred startup blockers.
