//! Tests of the `progress` module.

use super::render_metadata_progress;

#[test]
fn renders_exact_resource_and_byte_totals() {
    assert_eq!(
        render_metadata_progress("Config", 1, 2, 512, 1024, 4),
        "metadata [##--]  50.0% Config 1/2 512 B/1.0 KiB"
    );
    assert_eq!(
        render_metadata_progress("Config", 25, 100, 512 * 1024, 1024 * 1024, 10),
        "metadata [#####-----]  50.0% Config 25/100 512.0 KiB/1.0 MiB"
    );
}
