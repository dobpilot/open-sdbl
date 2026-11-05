#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|text: &str| { open_sdbl_pgwire::fuzz_catalog_sql(text); });
