#![no_main]

use libfuzzer_sys::fuzz_target;
use open_sdbl::query::{PostgresBackend, QueryCompiler};
use open_sdbl_fuzz::snapshot;

fuzz_target!(|input: &[u8]| {
    if let Ok(source) = std::str::from_utf8(input) {
        let _ = QueryCompiler::new(snapshot(), PostgresBackend).compile(source);
    }
});
