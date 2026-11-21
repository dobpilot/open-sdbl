# Design

Existing accepted contract: onec-metadata Requirement Read a value a column stores. Extract one private decoded-content helper returning Cow<[u8]>, preserving the existing ten-byte envelope and body detection/raw-DEFLATE path. The tree API parses those bytes with the existing parser; the text API validates UTF-8 and strips the optional serialization BOM, borrowing uncompressed content and transferring the inflated allocation into String for compressed content. No tree writer or second codec.

The limit counts decoded payload bytes including an optional decoded UTF-8 BOM, excluding the ten-byte envelope. Input bytes are caller-owned; there is no separate input-size bound or database read. External STORHDR is not content and remains an error for decode APIs; parse_stored_value_ref is the separate discovery API. Existing envelope fields are not newly interpreted or declared-length/version promises added.

The text API validates framing/inflation/UTF-8, not the full brace grammar or supported value kinds. Syntax/semantics remain the consumer parser's responsibility; decode_stored_value retains that validation. Successful tree decoding must equal parse_serialized(text.as_bytes()). The uncompressed path previously ignored limit; this draft explicitly fixes that bound under the new delta requirement without altering existing valid fixture expectations.

Native own controls will be attempted through existing owned filebase/staticform runner; any external serialization wrapper versus inline envelope distinction will be recorded rather than guessed. Original databases are not queried or changed.

Own file and PostgreSQL controls completed on platform 8.3.27.2342 / CompatibilityMode Version8_3_27. The actual Structure ValueStorage column uses a distinct 0201/GUID envelope. It remains unsupported and supplies negative fixtures only; no native positive coverage for the supported ten-byte envelope is claimed. Exact bytes and receipts are retained in tests/fixtures/stored-value-text-native-1013.
