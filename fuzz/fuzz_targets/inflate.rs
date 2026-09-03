#![no_main]

use libfuzzer_sys::fuzz_target;
use open_sdbl::metadata::inflate_raw_deflate_bounded;

const FUZZ_OUTPUT_LIMIT: usize = 1024 * 1024;

fuzz_target!(|input: &[u8]| {
    let _ = inflate_raw_deflate_bounded(input, FUZZ_OUTPUT_LIMIT);
});
