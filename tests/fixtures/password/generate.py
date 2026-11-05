#!/usr/bin/env python3
"""Generates the synthetic stored-password fixtures in `vectors.tsv`.

Provenance: every value here is synthetic. The stored values are computed
with Python's `hashlib` from the documented contract of the Python
`verify_password` v0.2 algorithm the maintainer supplied on 2026-10-26
(see openspec change add-pgwire-gateway, design.md "Password
verification"). The expected outcomes come from `verify` below, a second
implementation of that contract independent of the Rust code. No probe
base, no dump, no `v8users.Data` record and no reverse engineering was
used, and these vectors say nothing about where a real record keeps its
password fields.

Run: python3 -I tests/fixtures/password/generate.py > tests/fixtures/password/vectors.tsv
"""

import base64
import hashlib

PBKDF2_LIMIT = 1_048_576
SOURCES = {
    "public-stored-value-8.3.27",
    "synthetic-public-stored-value-8.3.27",
    "public-stored-value-legacy",
    "synthetic-public-stored-value-legacy",
}
PROFILES = {"1": "SHA256", "2": "SHA512", "3": "PBKDF2_HMAC_SHA256"}
LENGTHS = {"LEGACY_SHA1": 20, "SHA256": 32, "SHA512": 64, "PBKDF2_HMAC_SHA256": 64}


def digest(profile, data):
    if profile == "LEGACY_SHA1":
        return hashlib.sha1(data).digest()
    if profile == "SHA256":
        return hashlib.sha256(data).digest()
    if profile == "SHA512":
        return hashlib.sha512(data).digest()
    return hashlib.pbkdf2_hmac("sha256", data, b"", 16000, dklen=64)


def b64(data):
    return base64.b64encode(data).decode("ascii")


def canonical(field):
    try:
        decoded = base64.b64decode(field, validate=True)
    except Exception:
        return None
    return decoded if b64(decoded) == field else None


def verify(candidate, stored, source, state, mode):
    """Returns (status, reason, profile, primary, upper, consistency)."""

    def early(status, reason, profile="-"):
        return (status, reason, profile, "-", "-", "not_checked")

    if source not in SOURCES:
        return early("INDETERMINATE", "SOURCE_PROFILE_UNCONFIRMED")
    if state == "false":
        return early("NO_PASSWORD_CONFIGURED", "EXPLICIT_PASSWORD_NOT_SET")
    if stored == "":
        return early("INDETERMINATE", "EMPTY_STORED_VALUE")
    fields = stored.split(",")
    if len(fields) == 2:
        profile, salt = "LEGACY_SHA1", b""
    elif len(fields) == 4:
        ident = fields[2]
        if not ident or any(c not in "0123456789" for c in ident):
            return early("INVALID_RECORD", "INVALID_ALGORITHM_ID")
        ident = ident.lstrip("0") or "0"
        if ident == "0":
            return early("UNSUPPORTED_PROFILE", "NONCANONICAL_LEGACY_RECORD")
        if ident not in PROFILES:
            return early("UNSUPPORTED_PROFILE", "UNKNOWN_ALGORITHM_ID")
        profile, salt = PROFILES[ident], fields[3].encode("utf-8")
    else:
        return early("INVALID_RECORD", "INVALID_FIELD_COUNT")
    if profile == "PBKDF2_HMAC_SHA256" and len(candidate) + len(salt) > PBKDF2_LIMIT:
        return early("LIMIT_EXCEEDED", "PBKDF2_INPUT_TOO_LARGE", profile)
    stored_digests = []
    for field in fields[:2]:
        decoded = canonical(field)
        if decoded is None:
            return early("INVALID_RECORD", "INVALID_BASE64", profile)
        if len(decoded) != LENGTHS[profile]:
            return early("INVALID_RECORD", "INVALID_DIGEST_LENGTH", profile)
        stored_digests.append(decoded)
    primary = digest(profile, candidate + salt) == stored_digests[0]
    upper, consistency = "-", "not_checked"
    if mode == "ASCII_UPPER_DIAGNOSTIC":
        consistency = "not_established"
        if all(b < 0x80 for b in candidate):
            matched = digest(profile, candidate.upper() + salt) == stored_digests[1]
            upper = "true" if matched else "false"
            if primary:
                consistency = "consistent" if matched else "inconsistent"
    status, reason = (
        ("MATCH", "PRIMARY_DIGEST_MATCH") if primary else ("NO_MATCH", "PRIMARY_DIGEST_MISMATCH")
    )
    return (status, reason, profile, "true" if primary else "false", upper, consistency)


def record(profile, h1_input, h2_input, salt, ident=None):
    h1 = b64(digest(profile, h1_input + salt.encode("utf-8")))
    h2 = b64(digest(profile, h2_input + salt.encode("utf-8")))
    if profile == "LEGACY_SHA1":
        return f"{h1},{h2}"
    return f"{h1},{h2},{ident},{salt}"


def ascii_upper(data):
    return bytes(b - 32 if 0x61 <= b <= 0x7A else b for b in data)


rows = []


def case(name, candidate, stored, source="public-stored-value-8.3.27", state="unknown", mode="NONE"):
    outcome = verify(candidate, stored, source, state, mode)
    rows.append((name, source, state, mode, candidate.hex() or "-", stored or "-") + outcome)


PASSWORD = "PaSs123!".encode("utf-8")
UTF8 = "пароль€".encode("utf-8")
NUL = b"a\x00b"
DEMO = "IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk="
EMPTY_SHA1 = b64(hashlib.sha1(b"").digest())

# The supplied synthetic demo and its documented variants.
case("demo-match", PASSWORD, DEMO, "synthetic-public-stored-value-legacy")
case("demo-match-diagnostic", PASSWORD, DEMO, "synthetic-public-stored-value-legacy", mode="ASCII_UPPER_DIAGNOSTIC")
case("demo-upper-only", b"PASS123!", DEMO, "synthetic-public-stored-value-legacy", mode="ASCII_UPPER_DIAGNOSTIC")
case("demo-inconsistent-h2", PASSWORD, DEMO.split(",")[0] + "," + EMPTY_SHA1, "synthetic-public-stored-value-legacy", mode="ASCII_UPPER_DIAGNOSTIC")
case("empty-digest-empty-candidate", b"", f"{EMPTY_SHA1},{EMPTY_SHA1}", state="true")
case("empty-digest-nonempty-candidate", b"x", f"{EMPTY_SHA1},{EMPTY_SHA1}")

for profile, ident in [("LEGACY_SHA1", None), ("SHA256", "1"), ("SHA512", "2"), ("PBKDF2_HMAC_SHA256", "3")]:
    salt = "" if ident is None else "abc123"
    tag = profile.lower()
    standard = record(profile, PASSWORD, ascii_upper(PASSWORD), salt, ident)
    for source in sorted(SOURCES):
        case(f"{tag}-match-{source}", PASSWORD, standard, source)
    case(f"{tag}-match-diagnostic", PASSWORD, standard, mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-wrong-case", b"PASS123!", standard, mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-wrong-case-none", b"pass123!", standard)
    case(f"{tag}-empty-password", b"", record(profile, b"", b"", salt, ident), state="true")
    case(f"{tag}-empty-password-mismatch", b"x", record(profile, b"", b"", salt, ident))
    case(f"{tag}-utf8", UTF8, record(profile, UTF8, UTF8, salt, ident), mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-nfc-candidate-nfd-record", "\u00e9".encode("utf-8"), record(profile, "e\u0301".encode("utf-8"), b"", salt, ident))
    case(f"{tag}-embedded-nul", NUL, record(profile, NUL, ascii_upper(NUL), salt, ident), mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-nul-truncation-rejected", b"a", record(profile, NUL, NUL, salt, ident))
    case(f"{tag}-trailing-newline", PASSWORD + b"\n", standard)
    case(f"{tag}-leading-space", b" " + PASSWORD, standard)
    case(f"{tag}-inconsistent-h2", PASSWORD, record(profile, PASSWORD, b"other", salt, ident), mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-upper-only-match", b"PASS123!", record(profile, b"nope", b"PASS123!", salt, ident), mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-non-ascii-diagnostic-skipped", UTF8, record(profile, UTF8, b"", salt, ident), mode="ASCII_UPPER_DIAGNOSTIC")
    case(f"{tag}-password-not-set", PASSWORD, standard, state="false")
    case(f"{tag}-unconfirmed-raw-blob", PASSWORD, standard, "raw-blob")
    h1, h2 = standard.split(",")[:2]
    tail = "" if ident is None else f",{ident},{salt}"
    case(f"{tag}-malformed-h2", PASSWORD, f"{h1},!{tail}")
    case(f"{tag}-h2-whitespace", PASSWORD, f"{h1}, {h2}{tail}")
    case(f"{tag}-wrong-length", PASSWORD, f"{EMPTY_SHA1 if ident else b64(bytes(32))},{EMPTY_SHA1 if ident else b64(bytes(32))}{tail}")

for ident in ["1", "2", "3"]:
    profile = PROFILES[ident]
    tag = profile.lower()
    for salt in ["", "YWJjMTIz", "AbC123", "ёabc123", " abc123 "]:
        label = {"": "empty", "YWJjMTIz": "base64-looking", "AbC123": "mixed-case", "ёabc123": "non-ascii", " abc123 ": "spaces"}[salt]
        stored = record(profile, PASSWORD, ascii_upper(PASSWORD), salt, ident)
        case(f"{tag}-salt-{label}", PASSWORD, stored, mode="ASCII_UPPER_DIAGNOSTIC")
    # The salt is literal: a record salted with `YWJjMTIz` does not verify
    # as if salted with its Base64 decoding `abc123`, and vice versa.
    case(f"{tag}-salt-not-decoded", PASSWORD, record(profile, PASSWORD, PASSWORD, "abc123", ident).rsplit(",", 1)[0] + ",YWJjMTIz")
    for zeros in ["000" + ident, "0" * 4000 + ident]:
        stored = record(profile, PASSWORD, PASSWORD, "abc123", ident).split(",")
        stored[2] = zeros
        case(f"{tag}-leading-zero-id-{len(zeros)}", PASSWORD, ",".join(stored))

legacy = record("LEGACY_SHA1", PASSWORD, PASSWORD, "")
for ident, label in [("0", "zero"), ("000", "zeros"), ("4", "four"), ("9" * 3000, "long"), ("0" * 50 + "17", "padded-unknown"), ("+1", "plus"), ("", "empty"), ("١", "arabic-digit"), ("1 ", "space")]:
    case(f"id-{label}", PASSWORD, f"{legacy},{ident},abc123")
case("field-count-trailing-comma", PASSWORD, legacy + ",")
case("field-count-three", PASSWORD, legacy + ",1")
case("field-count-one", PASSWORD, legacy.split(",")[0])
case("field-count-five", PASSWORD, legacy + ",1,abc,def")
case("empty-stored-unknown", PASSWORD, "")
case("empty-stored-set", PASSWORD, "", state="true")
case("empty-stored-not-set", b"", "", state="false")
case("not-set-beats-valid-h1", b"", f"{EMPTY_SHA1},{EMPTY_SHA1}", state="false")
case("unconfirmed-beats-not-set", PASSWORD, legacy, "raw-blob", state="false")
case("noncanonical-pad-bits", b"", f"2jmj7l5rSw0yVb/vlWAYkK/YBWl=,{EMPTY_SHA1}")

print("# Synthetic stored-password vectors; see generate.py for provenance.")
print("# name\tsource\tpassword_state\tsecondary_mode\tcandidate_hex\tstored_value\tstatus\treason\tprofile\tprimary\tupper\tconsistency")
for row in rows:
    print("\t".join(row))
