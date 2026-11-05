//! Known-answer vectors from FIPS 180-4, RFC 4231 and RFC 7914, each
//! cross-checked against Python's `hashlib`/`hmac` (OpenSSL).

use super::*;
use crate::hex_test_support::hex;

const MESSAGES: [&[u8]; 3] = [
    b"",
    b"abc",
    b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
];

#[test]
fn sha1_vectors() {
    let expected = [
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
    ];
    for (message, expected) in MESSAGES.iter().zip(expected) {
        assert_eq!(sha1(message).to_vec(), hex(expected));
    }
    assert_eq!(
        sha1(&vec![b'a'; 1_000_000]).to_vec(),
        hex("34aa973cd4c4daa4f61eeb2bdbad27316534016f")
    );
}

#[test]
fn sha256_vectors() {
    let expected = [
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
    ];
    for (message, expected) in MESSAGES.iter().zip(expected) {
        assert_eq!(sha256(message).to_vec(), hex(expected));
    }
    assert_eq!(
        sha256(&vec![b'a'; 1_000_000]).to_vec(),
        hex("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0")
    );
}

#[test]
fn sha512_vectors() {
    let expected = [
        "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        "204a8fc6dda82f0a0ced7beb8e08a41657c16ef468b228a8279be331a703c33596fd15c13b1b07f9aa1d3bea57789ca031ad85c7a71dd70354ec631238ca3445",
    ];
    for (message, expected) in MESSAGES.iter().zip(expected) {
        assert_eq!(sha512(message).to_vec(), hex(expected));
    }
    assert_eq!(
        sha512(&vec![b'a'; 1_000_000]).to_vec(),
        hex(
            "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973ebde0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b"
        )
    );
}

#[test]
fn multi_block_input_crosses_every_buffer_boundary() {
    let message = (0..256_u16)
        .map(|byte| u8::try_from(byte).unwrap())
        .cycle()
        .take(1280)
        .collect::<Vec<_>>();
    assert_eq!(
        sha1(&message).to_vec(),
        hex("e37a04cb2353309f5cff4ee036cfb91a5e31cefd")
    );
    assert_eq!(
        sha256(&message).to_vec(),
        hex("d414b085826eb06778483ba35564dc849e643359f69ed9747878ba6e54985bed")
    );
    assert_eq!(
        sha512(&message).to_vec(),
        hex(
            "c93f55ccf2fa8c82699ff9b58afe3591242b135d908a6d865e17e38adb41c21d1d5359e51273036373d54d20b5659cc87e6e7b381ff027d33f971416cc590f90"
        )
    );
    // Feeding the incremental hasher in odd pieces gives the same answer.
    let mut hasher = Sha256::new();
    for piece in message.chunks(37) {
        hasher.update(piece);
    }
    assert_eq!(hasher.finalize(), sha256(&message));
}

#[test]
fn hmac_sha256_rfc4231() {
    assert_eq!(
        hmac_sha256(&[0x0b; 20], b"Hi There").to_vec(),
        hex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
    );
    assert_eq!(
        hmac_sha256(b"Jefe", b"what do ya want for nothing?").to_vec(),
        hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
    );
    assert_eq!(
        hmac_sha256(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        )
        .to_vec(),
        hex("60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54")
    );
}

#[test]
fn pbkdf2_hmac_sha256_vectors() {
    let mut output = [0_u8; 64];
    pbkdf2_hmac_sha256(b"passwd", b"salt", 1, &mut output);
    assert_eq!(
        output.to_vec(),
        hex(
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"
        )
    );
    pbkdf2_hmac_sha256(b"Password", b"NaCl", 80_000, &mut output);
    assert_eq!(
        output.to_vec(),
        hex(
            "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56a1d425a1225833549adb841b51c9b3176a272bdebba1d078478f62b397f33c8d"
        )
    );
    // The profile ID 3 call: the record salt is already in the password.
    pbkdf2_hmac_sha256(b"PaSs123!abc123", b"", 16_000, &mut output);
    assert_eq!(
        output.to_vec(),
        hex(
            "11c953967c8f47497443804c943e1b89daf4995e59d2d0bfb5139cb0ba0f7de7239aa708b7d003182928ed46e810f4343eff0990233e96ea6958f8921abd1aa2"
        )
    );
}

#[test]
fn base64_round_trips_every_digest_length() {
    for length in [20, 32, 64, 0, 1, 2, 3] {
        let bytes = (0..length)
            .map(|index| u8::try_from(index * 37 % 256).unwrap())
            .collect::<Vec<_>>();
        let text = base64_encode(&bytes);
        if length == 0 {
            assert_eq!(text, "");
            assert_eq!(base64_decode_canonical(&text), None);
        } else {
            assert_eq!(base64_decode_canonical(&text), Some(bytes));
        }
    }
    assert_eq!(
        base64_decode_canonical("2jmj7l5rSw0yVb/vlWAYkK/YBwk="),
        Some(sha1(b"").to_vec())
    );
}

#[test]
fn base64_rejects_noncanonical_text() {
    for text in [
        // Nonzero pad bits that a permissive decoder accepts.
        "2jmj7l5rSw0yVb/vlWAYkK/YBWl=",
        "2jmj7l5rSw0yVb/vlWAYkK/YBwl=",
        "QR==",
        // Missing, extra and misplaced padding.
        "2jmj7l5rSw0yVb/vlWAYkK/YBwk",
        "2jmj7l5rSw0yVb/vlWAYkK/YBwk==",
        "QQ=A",
        "QQ==QQ==",
        "Q===",
        "====",
        // Whitespace anywhere.
        " 2jmj7l5rSw0yVb/vlWAYkK/YBwk=",
        "2jmj7l5rSw0yVb/vlWAYkK/YBwk=\n",
        "2jmj7l5rSw0y Vb/vlWAYkK/YBwk=",
        // URL-safe substitutions and foreign characters.
        "2jmj7l5rSw0yVb_vlWAYkK_YBwk=",
        "2jmj7l5rSw0yVb-vlWAYkK-YBwk=",
        "!",
        "2jmj7l5rSw0yVb/vlWAYkK/YBwк=",
    ] {
        assert_eq!(base64_decode_canonical(text), None, "{text:?}");
    }
}

#[test]
fn constant_time_equal_compares_contents() {
    assert!(constant_time_equal(b"abc", b"abc"));
    assert!(!constant_time_equal(b"abc", b"abd"));
    assert!(!constant_time_equal(b"abc", b"ab"));
}
