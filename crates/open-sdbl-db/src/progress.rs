//! Progress of a metadata read, reported to whoever asked for it.

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

    /// Reports that the read finished.
    fn finish(&mut self) {}
}

/// A reporter that says nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl MetadataProgress for NoProgress {}
