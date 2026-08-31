## Context

The input is validated as UTF-8 before `Parser` runs. Structural tokens `{`,
`}`, `,`, and `"` are single-byte ASCII and cannot occur inside a multibyte
codepoint, so structural scanning does not require decoding every character.

## Decisions

### Match grammar tokens as bytes

Replace `current() -> Option<char>` and repeated `advance()` calls with a byte
cursor. Lists and atoms compare ASCII bytes directly. Atom boundaries therefore
remain valid UTF-8 boundaries because an ASCII delimiter cannot be part of a
multibyte sequence.

### Copy string segments between quotes

Search for the next quote byte, append the preceding UTF-8 slice once, and
handle a doubled quote as one literal quote. This preserves current allocation
and unescaping semantics while avoiding one push and UTF-8 decode per
character.

### Retain Unicode whitespace compatibility

Skip ASCII whitespace one byte at a time. Only when the next byte is non-ASCII,
decode one character and apply `char::is_whitespace`, preserving existing
behavior and byte offsets.

## Verification

Retain all generic parser and metadata conformance tests, add Unicode and long
escaped-string parity coverage, then repeat the reported live startup timing
and profile.
