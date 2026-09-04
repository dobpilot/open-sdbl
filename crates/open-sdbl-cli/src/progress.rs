use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

const PROGRESS_REDRAW_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_BAR_WIDTH: usize = 24;

pub(crate) struct MetadataProgress {
    enabled: bool,
    active: bool,
    phase: &'static str,
    pub(crate) completed_resources: u64,
    total_resources: u64,
    pub(crate) completed_bytes: u64,
    total_bytes: u64,
    started: Instant,
    last_draw: Option<Instant>,
}

impl MetadataProgress {
    pub(crate) fn new() -> Self {
        Self {
            enabled: io::stderr().is_terminal(),
            active: false,
            phase: "starting",
            completed_resources: 0,
            total_resources: 0,
            completed_bytes: 0,
            total_bytes: 0,
            started: Instant::now(),
            last_draw: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        let mut progress = Self::new();
        progress.enabled = false;
        progress
    }

    pub(crate) fn phase(&mut self, phase: &'static str) {
        self.phase = phase;
        self.draw(true);
    }

    pub(crate) fn config_totals(&mut self, resources: u64, bytes: u64) {
        self.total_resources = resources;
        self.total_bytes = bytes;
        self.phase("Config");
    }

    pub(crate) fn advance_config(&mut self, resources: usize, bytes: usize) {
        self.completed_resources = self.completed_resources.saturating_add(resources as u64);
        self.completed_bytes = self.completed_bytes.saturating_add(bytes as u64);
        self.draw(false);
    }

    pub(crate) fn finish(mut self) {
        if !self.enabled {
            return;
        }
        self.phase = "complete";
        self.completed_resources = self.total_resources;
        self.completed_bytes = self.total_bytes;
        let line = render_metadata_progress(
            self.phase,
            self.completed_resources,
            self.total_resources,
            self.completed_bytes,
            self.total_bytes,
            PROGRESS_BAR_WIDTH,
        );
        let mut stderr = io::stderr().lock();
        let _ = writeln!(
            stderr,
            "\r\x1b[2K{line} in {}",
            format_elapsed(self.started.elapsed())
        );
        let _ = stderr.flush();
        self.active = false;
    }

    fn draw(&mut self, force: bool) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if !force
            && self
                .last_draw
                .is_some_and(|last| now.duration_since(last) < PROGRESS_REDRAW_INTERVAL)
        {
            return;
        }
        self.last_draw = Some(now);
        self.active = true;
        let line = render_metadata_progress(
            self.phase,
            self.completed_resources,
            self.total_resources,
            self.completed_bytes,
            self.total_bytes,
            PROGRESS_BAR_WIDTH,
        );
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "\r\x1b[2K{line}");
        let _ = stderr.flush();
    }
}

impl Drop for MetadataProgress {
    fn drop(&mut self) {
        if self.enabled && self.active {
            let mut stderr = io::stderr().lock();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        }
    }
}

pub(crate) fn render_metadata_progress(
    phase: &str,
    completed_resources: u64,
    total_resources: u64,
    completed_bytes: u64,
    total_bytes: u64,
    width: usize,
) -> String {
    let ratio = if total_bytes != 0 {
        completed_bytes as f64 / total_bytes as f64
    } else if total_resources != 0 {
        completed_resources as f64 / total_resources as f64
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    let filled = ((ratio * width as f64).floor() as usize).min(width);
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(width - filled));
    let resources = if total_resources == 0 {
        format!("{completed_resources}/?")
    } else {
        format!("{completed_resources}/{total_resources}")
    };
    let bytes = if total_bytes == 0 {
        format!("{}/?", format_bytes(completed_bytes))
    } else {
        format!(
            "{}/{}",
            format_bytes(completed_bytes),
            format_bytes(total_bytes)
        )
    };
    format!(
        "metadata [{bar}] {:>5.1}% {phase} {resources} {bytes}",
        ratio * 100.0
    )
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn format_elapsed(elapsed: Duration) -> String {
    if elapsed.as_secs() != 0 {
        format!("{:.1} s", elapsed.as_secs_f64())
    } else {
        format!("{} ms", elapsed.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::render_metadata_progress;

    #[test]
    fn renders_exact_resource_and_byte_totals() {
        assert_eq!(
            render_metadata_progress("Config", 1, 2, 512, 1024, 4),
            "metadata [##--]  50.0% Config 1/2 512 B/1.0 KiB"
        );
    }
}
