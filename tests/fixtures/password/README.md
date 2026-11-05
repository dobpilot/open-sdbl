# Synthetic stored-password vectors

`vectors.tsv` holds synthetic cases for `open_sdbl::password`: legacy
SHA-1 (`H1,H2`) and the four-field profiles ID 1 (SHA-256), ID 2 (SHA-512)
and ID 3 (PBKDF2-HMAC-SHA256), with `PaSs123!`, wrong case, empty password,
UTF-8, embedded NUL, inconsistent H2, literal and Base64-looking salts,
leading-zero and very long IDs, unsupported IDs and malformed records.

## Provenance

- Every stored value is synthetic, computed with Python `hashlib` by
  `generate.py` from the documented contract of the Python
  `verify_password` v0.2 algorithm supplied by the maintainer on
  2026-10-26 (openspec change `add-pgwire-gateway`, `design.md`,
  "Password verification"). The supplied script itself is not part of
  the repository; `generate.py` re-implements its documented steps, and
  the Rust verifier is checked against that second implementation.
- The demo value `IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=`
  is the one supplied with the algorithm.
- No probe base, database dump, `v8users` row or reverse engineering was
  used. The vectors verify the algorithm only: they do not show where a
  real record keeps its password, and they do not show that a live 1C
  login succeeds.

Regenerate with:

```console
python3 -I tests/fixtures/password/generate.py > tests/fixtures/password/vectors.tsv
```

## Source mapping of `UserData`

The legacy H1/H2 positions and the extended algorithm/salt suffix were
measured on the authorized `unf` test base. The extended native shapes have
two or five counted optional strings followed by six suffix fields. Current
SHA-256 authentication was verified read-only on the live user's record;
SHA-512 record structure was also observed. The stored value remains private
on UserData, and malformed/unknown shapes fail closed. See the measurement
in `openspec/changes/authenticate-stored-password-from-v8users/design.md`.

The records in `tests/support/password_record.rs` use synthetic digests and
salt from these vectors with that structural layout. They contain no live
credential material. Core and gateway tests cover the UserData path for
SHA-256/SHA-512, distinct from the direct public StoredPasswordValue API.
