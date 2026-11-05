#![no_main]

use libfuzzer_sys::fuzz_target;
use open_sdbl_pgwire::codec::{decode_frontend, decode_startup};

// The first byte picks the message type; the rest is the body. Every
// message type, including the startup packet (type 0), must either decode
// or be rejected, never panic.
fuzz_target!(|data: &[u8]| {
    let Some((&code, body)) = data.split_first() else {
        return;
    };
    if code == 0 {
        let _ = decode_startup(body);
    } else {
        let _ = decode_frontend(code, body);
    }
});
