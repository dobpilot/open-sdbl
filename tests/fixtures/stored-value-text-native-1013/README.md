# Bounded native storage control

Platform 8.3.27.2342, configuration CompatibilityMode Version8_3_27, declared resource type v8:ValueStorage, own PostgreSQL database `sdbl_storage_native`, own information register `ОпцииПробы`, two rows containing a Structure with Text="native-storage-control", Flag=true and Number=17. Compression levels 0 and 9 were explicitly requested. Native create/apply and runtime completed with exit 0; runtime oracle confirms two written rows.

The exact bytea values begin with `0201` followed by a GUID. They are a different envelope from the existing ten-byte stored-content decoder and the reached UNF `0101` envelope. Neither slicing nor support for this distinct envelope is inferred here. These fixtures are negative coverage only; they do not establish native positive coverage for the new text API. Existing synthetic ten-byte tree fixtures establish preservation of the supported path. File ValueToFile also produces a different outer representation and is not used as inline content.

Full own XML, CF, native file roundtrip, logs and extraction are retained at `/tmp/sdbl-stored-value-text-native-1013`. Extraction SQL: `SELECT _fld55,encode(_fld59,'hex') FROM _inforg54 ORDER BY _fld55`, with `default_transaction_read_only=on`. No original database was queried for this control. Original user values are absent.

The API removes one optional serialization BOM, validates UTF-8, shares existing raw-DEFLATE framing, and does not validate brace grammar. The output limit includes BOM and excludes ten-byte framing; input-slice size has no separate bound. Header fields retain existing semantics. Unknown envelopes remain unsupported. This is an isolated draft, version 0.9.15 unchanged.
