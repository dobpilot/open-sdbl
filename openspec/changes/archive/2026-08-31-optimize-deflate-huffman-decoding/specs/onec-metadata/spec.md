## ADDED Requirements

### Requirement: Decode DEFLATE Huffman symbols with bounded lookup cost
The metadata decoder SHALL resolve each fixed or dynamic DEFLATE Huffman symbol
with lookup work bounded by the RFC 1951 maximum code width rather than by the
number of entries in the Huffman tree. Lookup storage SHALL remain bounded by
the 15-bit DEFLATE code limit. The optimization SHALL preserve decoded bytes,
output-size limits, and rejection of empty, oversubscribed, incomplete, or
truncated invalid streams.

#### Scenario: Large compressed Config resource
- **WHEN** a Config resource contains many symbols encoded with a populated
  dynamic Huffman tree
- **THEN** decoding does not scan the tree entries for every output symbol

#### Scenario: Short code at physical input end
- **WHEN** the final valid symbol needs fewer bits than the tree maximum and is
  followed only by DEFLATE byte padding
- **THEN** lookup consumes only the symbol's actual code length and succeeds

#### Scenario: Invalid incomplete-tree prefix
- **WHEN** input bits address an unassigned prefix in an incomplete Huffman tree
- **THEN** decoding returns the existing invalid-Huffman-code diagnostic
