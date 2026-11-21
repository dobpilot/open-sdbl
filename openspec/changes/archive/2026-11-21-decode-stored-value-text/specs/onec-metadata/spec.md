## ADDED Requirements

### Requirement: Decode stored value content as UTF-8 text
The library SHALL expose a pure bounded decoder for the serialized text of inline or already fetched stored-value content. It SHALL share framing and decompression with the existing tree decoder, SHALL return UTF-8 without its optional serialization BOM, and SHALL NOT reconstruct that text from a parsed tree. Uncompressed content SHALL be borrowable; compressed content MAY own the inflated text. The text decoder SHALL NOT promise complete serialized grammar or value-kind validation.

#### Scenario: Existing supported stored content
- **WHEN** valid content is decoded through the tree and text APIs
- **THEN** parsing the returned text yields the same tree, without changing the existing valid tree result

#### Scenario: Uncompressed content
- **WHEN** valid uncompressed content is supplied
- **THEN** its text is borrowed from the input after the optional BOM

#### Scenario: External stored-value reference
- **WHEN** STORHDR reference bytes are supplied instead of content
- **THEN** text and tree decoders refuse them without fetching any database data

### Requirement: Bound decoded stored-value payload
Both stored-value decoders SHALL reject decoded payloads exceeding the caller's byte limit on compressed and uncompressed paths. The limit SHALL include an optional decoded serialization BOM and exclude the ten-byte envelope. The input slice SHALL remain caller-owned, without a separate implied input-size limit. Truncated framing, invalid raw-DEFLATE and invalid UTF-8 SHALL return an error.

#### Scenario: Plain content exceeds limit
- **WHEN** uncompressed decoded payload is larger than the supplied limit
- **THEN** both decoding paths refuse it

#### Scenario: Compressed content exceeds limit
- **WHEN** inflation would exceed the supplied decoded-byte limit
- **THEN** both decoders refuse it without unbounded inflation

#### Scenario: Invalid content
- **WHEN** content framing, compressed bytes or UTF-8 are malformed
- **THEN** decoding returns an error instead of guessed text
