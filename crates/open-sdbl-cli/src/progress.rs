//! One stderr row for acquisition and permanent messages above it.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use open_sdbl::metadata::MetadataErrorKind;
use open_sdbl_db::MetadataProgress as ProgressReporter;
use unicode_width::UnicodeWidthStr;

use crate::output::escape_field;
use crate::terminal::{Palette, Surface, columns};

const REDRAW_INTERVAL: Duration = Duration::from_millis(50);
pub(crate) const GLINT_DURATION: Duration = Duration::from_millis(320);
const CLEAR_ROW: &str = "\r\x1b[2K";

static DISPLAY: Mutex<Display> = Mutex::new(Display {
    frame: None,
    line: None,
    last_draw: None,
});

struct Display {
    frame: Option<Frame>,
    line: Option<String>,
    last_draw: Option<Instant>,
}

impl Display {
    fn clear(&mut self, writer: &mut impl Write) -> io::Result<()> {
        if self.line.take().is_some() {
            write!(writer, "{CLEAR_ROW}")?;
        }
        Ok(())
    }

    fn paint(&mut self, writer: &mut impl Write) -> io::Result<()> {
        let surface = Surface::stderr();
        let rendered = self
            .frame
            .filter(|_| surface.tty)
            .and_then(|frame| frame.render(columns(true)?, surface.palette));
        if self.line == rendered {
            return Ok(());
        }
        self.clear(writer)?;
        if let Some(line) = rendered {
            write!(writer, "{line}")?;
            self.line = Some(line);
            self.last_draw = Some(Instant::now());
        }
        writer.flush()
    }

    fn permanent(&mut self, writer: &mut impl Write, line: &str) -> io::Result<()> {
        self.clear(writer)?;
        writeln!(writer, "{line}")?;
        writer.flush()?;
        self.paint(writer)
    }
}

fn permanent(line: &str) {
    let mut display = DISPLAY.lock().expect("stderr display");
    let _ = display.permanent(&mut io::stderr().lock(), line);
}

pub(crate) fn log(level: &str, message: impl std::fmt::Display) {
    let label = Surface::stderr().palette.label(level);
    permanent(&format!("{label}  {}", escape_field(&message.to_string())));
}

/// Checks the terminal for resizes and advances the startup glint while work
/// is in flight. A ready result always wins over a redraw.
pub(crate) async fn observe<F: Future>(work: F) -> F::Output {
    if !Surface::stderr().tty {
        return work.await;
    }
    tokio::pin!(work);
    let mut timer = tokio::time::interval(Duration::from_millis(40));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            result = &mut work => return result,
            _ = timer.tick() => {
                let mut display = DISPLAY.lock().expect("stderr display");
                if let Some(frame) = display.frame {
                    let in_glint = frame.glint && frame.started.elapsed() <= GLINT_DURATION;
                    if in_glint || display.last_draw.is_none_or(|last| last.elapsed() >= REDRAW_INTERVAL) {
                        let _ = display.paint(&mut io::stderr().lock());
                    }
                }
            }
        }
    }
}

pub(crate) struct MetadataProgress {
    frame: Frame,
    static_summary: bool,
    finished: bool,
    skips: ConfigSkips,
}

impl MetadataProgress {
    pub(crate) fn new() -> Self {
        Self::start(false, false)
    }

    pub(crate) fn start(static_summary: bool, glint: bool) -> Self {
        let progress = Self {
            frame: Frame {
                phase: "connecting",
                totals: None,
                resources: 0,
                bytes: 0,
                started: Instant::now(),
                glint,
            },
            static_summary,
            finished: false,
            skips: ConfigSkips::default(),
        };
        progress.draw(true);
        progress
    }

    fn draw(&self, force: bool) {
        let mut display = DISPLAY.lock().expect("stderr display");
        display.frame = Some(self.frame);
        if force
            || display
                .last_draw
                .is_none_or(|last| last.elapsed() >= REDRAW_INTERVAL)
        {
            let _ = display.paint(&mut io::stderr().lock());
        }
    }

    fn flush_skips(&mut self) {
        if let Some(block) = self.skips.take_warning(Surface::stderr().palette) {
            permanent(&block);
        }
    }

    pub(crate) fn outcome<T, E>(&mut self, result: &Result<T, E>) {
        self.end(if result.is_ok() { "complete" } else { "failed" });
    }

    pub(crate) fn end(&mut self, outcome: &str) {
        if self.finished {
            return;
        }
        self.finished = true;
        DISPLAY.lock().expect("stderr display").frame = None;
        self.flush_skips();
        let mut display = DISPLAY.lock().expect("stderr display");
        display.frame = None;
        display.last_draw = None;
        let mut stderr = io::stderr().lock();
        let _ = display.clear(&mut stderr);
        if self.static_summary || Surface::stderr().tty {
            let elapsed = self.frame.started.elapsed();
            let time = if elapsed.as_secs() > 0 {
                format!("{:.1} s", elapsed.as_secs_f64())
            } else {
                format!("{} ms", elapsed.as_millis())
            };
            let counts = if self.frame.totals.is_some() || self.frame.resources != 0 {
                format!(
                    "  {} resources  {}",
                    self.frame.resources,
                    byte_text(self.frame.bytes, self.frame.bytes)
                )
            } else {
                String::new()
            };
            let _ = writeln!(stderr, "metadata  {outcome}{counts}  {time}");
        }
        let _ = stderr.flush();
    }
}

impl ProgressReporter for MetadataProgress {
    fn phase(&mut self, phase: &'static str) {
        self.flush_skips();
        self.frame.phase = phase;
        self.draw(true);
    }

    fn config_totals(&mut self, resources: u64, bytes: u64) {
        self.frame.totals = Some((resources, bytes));
        self.phase("Config");
    }

    fn advance_config(&mut self, resources: usize, bytes: usize) {
        self.frame.resources = self.frame.resources.saturating_add(resources as u64);
        self.frame.bytes = self.frame.bytes.saturating_add(bytes as u64);
        self.draw(false);
    }

    fn skipped_config(&mut self, resource: &str, reason: MetadataErrorKind) {
        self.skips.record(resource, reason);
    }

    fn warning(&mut self, message: &str) {
        log("WARN", message);
    }

    fn finish(&mut self) {
        self.flush_skips();
    }
}

#[derive(Default)]
pub(crate) struct ConfigSkips {
    skips: BTreeMap<String, &'static str>,
    reported: BTreeSet<String>,
}

impl ConfigSkips {
    pub(crate) fn record(&mut self, resource: &str, reason: MetadataErrorKind) {
        if !self.reported.contains(resource) {
            self.skips.insert(
                resource.into(),
                match reason {
                    MetadataErrorKind::Utf8 => "invalid UTF-8",
                    MetadataErrorKind::Serialization => "invalid brace serialization",
                    _ => "unreadable resource",
                },
            );
        }
    }

    pub(crate) fn take_warning(&mut self, palette: Palette) -> Option<String> {
        if self.skips.is_empty() {
            return None;
        }
        let label = palette.label("WARN");
        let mut block = format!(
            "{label}  skipped {} unreadable Config resources",
            self.skips.len()
        );
        for (id, reason) in std::mem::take(&mut self.skips) {
            block.push_str(&format!("\n      {}  {reason}", escape_field(&id)));
            self.reported.insert(id);
        }
        Some(block)
    }
}

impl Drop for MetadataProgress {
    fn drop(&mut self) {
        self.end("interrupted");
    }
}

#[derive(Clone, Copy)]
struct Frame {
    phase: &'static str,
    totals: Option<(u64, u64)>,
    resources: u64,
    bytes: u64,
    started: Instant,
    glint: bool,
}

impl Frame {
    fn render(self, columns: usize, palette: Palette) -> Option<String> {
        let elapsed = self.started.elapsed();
        let highlight = (self.glint && elapsed < GLINT_DURATION)
            .then(|| (elapsed.as_millis() as usize / 40).min(6));
        let title = if self.glint && self.phase == "connecting" && palette != Palette::Plain {
            "open-sdbl  metadata"
        } else {
            "metadata"
        };
        let (resources, bytes, percent) = self.measurements();
        for label in ["resources", "res"] {
            let suffix = format!("  {percent:>4}  {resources} {label}  {bytes}");
            let prefix = format!("{title}  {}  [", self.phase);
            let width = columns
                .saturating_sub(prefix.width() + suffix.width() + 2)
                .min(16);
            if width < 6 {
                continue;
            }
            let filled = self.totals.map_or(0, |(resources, bytes)| {
                let (read, total) = if bytes == 0 {
                    (self.resources, resources)
                } else {
                    (self.bytes, bytes)
                };
                proportion(read, total, width)
            });
            let bar = if filled > 0 {
                format!(
                    "{}{}{}",
                    palette.mint(&"=".repeat(filled - 1), false),
                    palette.mint(">", true),
                    "-".repeat(width - filled)
                )
            } else {
                "-".repeat(width)
            };
            let title = if self.glint && self.phase == "connecting" && palette != Palette::Plain {
                format!(
                    "{}  {}",
                    palette.mint("open-sdbl", false),
                    palette.title("metadata", highlight)
                )
            } else {
                palette.title("metadata", highlight)
            };
            return Some(format!("{title}  {}  [{bar}]{suffix}", self.phase));
        }
        None
    }

    fn measurements(self) -> (String, String, String) {
        let Some((resources, bytes)) = self.totals else {
            return (
                format!("{}/?", self.resources),
                format!("{}/?", byte_text(self.bytes, self.bytes)),
                "?".into(),
            );
        };
        let range = resources.to_string().len();
        let total_bytes = byte_text(bytes, bytes);
        let read_bytes = byte_text(self.bytes, bytes);
        let percent = if bytes == 0 {
            proportion(self.resources, resources, 100)
        } else {
            proportion(self.bytes, bytes, 100)
        };
        (
            format!("{:>range$}/{resources}", self.resources),
            format!(
                "{read_bytes:>width$}/{total_bytes}",
                width = total_bytes.len()
            ),
            format!("{percent}%"),
        )
    }
}

fn proportion(read: u64, total: u64, width: usize) -> usize {
    if read >= total {
        width
    } else {
        (u128::from(read) * width as u128 / u128::from(total)) as usize
    }
}

pub(crate) fn byte_text(bytes: u64, reference: u64) -> String {
    match reference {
        0..=1023 => format!("{bytes} B"),
        1024..=1048575 => format!("{:.1} KiB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MiB", bytes as f64 / 1048576.0),
    }
}

#[cfg(test)]
#[path = "tests/progress.rs"]
mod tests;
