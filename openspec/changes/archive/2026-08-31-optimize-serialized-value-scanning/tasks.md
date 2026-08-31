## 1. Byte-oriented generic parser

- [x] 1.1 Match structural tokens and scan atoms using byte offsets.
- [x] 1.2 Copy quoted-string spans in blocks and preserve doubled quotes.
- [x] 1.3 Preserve Unicode whitespace and add regression coverage.

## 2. Verification

- [x] 2.1 Rebuild release and verify the reported live console reaches its
  prompt through SOCKS5.
- [x] 2.2 Compare live timing/profile with the character-scanning baseline.
- [x] 2.3 Run all repository quality gates and strict OpenSpec validation.
