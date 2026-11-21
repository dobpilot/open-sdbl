//! Progress of a metadata read, reported to whoever asked for it.

use open_sdbl::metadata::MetadataErrorKind;

/// What a metadata read reports as it goes.
///
/// Every method does nothing by default, so an application that wants no
/// progress writes `impl MetadataProgress for MyType {}`, or passes
/// [`NoProgress`].
pub trait MetadataProgress {
    /// Names the stage the read has reached.
    fn phase(&mut self, phase: &'static str) {
        let _ = phase;
    }

    /// Announces how many Config resources, and how many compressed
    /// bytes, the read is about to decode.
    fn config_totals(&mut self, resources: u64, bytes: u64) {
        let _ = (resources, bytes);
    }

    /// Reports one decoded batch of Config resources.
    fn advance_config(&mut self, resources: usize, bytes: usize) {
        let _ = (resources, bytes);
    }

    /// A Config resource was skipped after validation failed.
    fn skipped_config(&mut self, resource: &str, reason: MetadataErrorKind) {
        let _ = (resource, reason);
    }

    /// A recoverable acquisition problem that does not belong to a Config ID.
    fn warning(&mut self, message: &str) {
        let _ = message;
    }

    /// Reports that the read finished.
    fn finish(&mut self) {}
}

/// A reporter that says nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl MetadataProgress for NoProgress {}
