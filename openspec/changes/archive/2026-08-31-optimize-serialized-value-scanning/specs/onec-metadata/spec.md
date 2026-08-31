## ADDED Requirements

### Requirement: Scan brace serialization without repeated character decoding
After validating UTF-8, the generic metadata value parser SHALL recognize ASCII
structural delimiters and contiguous string spans without repeatedly decoding
each character. It SHALL preserve the public owned `Value` hierarchy, Unicode
text and whitespace, doubled-quote unescaping, byte-position diagnostics, and
malformed-input rejection.

#### Scenario: Long localized string
- **WHEN** a quoted Config value contains long multibyte text and doubled quotes
- **THEN** parsing returns the identical unescaped string without per-character
  structural checks

#### Scenario: Unicode whitespace
- **WHEN** valid non-ASCII whitespace surrounds a serialized value
- **THEN** the parser accepts it with the same semantics as ASCII whitespace

#### Scenario: Malformed UTF-8 or quoting
- **WHEN** input is byte-invalid UTF-8 or a quoted value is unterminated
- **THEN** parsing returns the same class of positional diagnostic
