//! The console owns this stderr region only between connection and the first
//! readline. The work future and the painter are polled on the same task.

mod art;
use art::{Colors, Scene, Size};
use std::cell::RefCell;
use std::future::Future;
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use open_sdbl::metadata::MetadataErrorKind;
use open_sdbl_db::MetadataProgress;
use unicode_width::UnicodeWidthStr;

use crate::error::CliError;
use crate::output::escape_field;
use crate::progress::{ConfigSkips, byte_text};
use crate::terminal::{Palette, Surface};

const FIRST_PAINT: Duration = Duration::from_millis(120);
const FRAME_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Default)]
struct Work {
    phase: &'static str,
    totals: Option<(u64, u64)>,
    resources: u64,
    bytes: u64,
    config_complete: bool,
}

impl Work {
    fn fraction(&self) -> f64 {
        if self.config_complete {
            return 1.;
        }
        match self.totals {
            Some((_, bytes)) if bytes > 0 => (self.bytes as f64 / bytes as f64).min(1.),
            Some((resources, _)) if resources > 0 => {
                (self.resources as f64 / resources as f64).min(1.)
            }
            _ => 0.,
        }
    }

    fn status(&self, width: usize) -> String {
        let phase = self.phase;
        let title = format!("open-sdbl  {phase}");
        if phase == "Config"
            && let Some((resources, bytes)) = self.totals
        {
            let count = format!(
                "{:>digits$}/{resources}",
                self.resources,
                digits = resources.to_string().len()
            );
            let total = byte_text(bytes, bytes);
            let (total, unit) = total.split_once(' ').expect("byte unit");
            let current = byte_text(self.bytes, bytes);
            let current = current.split_once(' ').expect("byte unit").0;
            let volume = format!("{current:>width$}/{total} {unit}", width = total.len());
            for status in [
                format!("{title}  {count} res  {volume}"),
                format!("{phase}  {count} res  {volume}"),
                format!("{phase}  {count} res"),
                format!("{phase} {count}"),
            ] {
                if status.len() <= width {
                    return status;
                }
            }
        }
        if title.len() <= width {
            return title;
        }
        let phase = if phase.starts_with("DBNames") {
            "DBNames"
        } else {
            phase
        };
        phase[..phase.len().min(width)].into()
    }
}

#[derive(Default)]
struct Region {
    rows: usize,
    last_frame: String,
}

impl Region {
    fn clear(&mut self, out: &mut impl Write, height: usize) -> io::Result<()> {
        self.last_frame.clear();
        let rows = std::mem::take(&mut self.rows).min(height);
        if rows > 0 {
            write!(out, "\r\x1b[2K")?;
            for _ in 1..rows {
                write!(out, "\x1b[A\r\x1b[2K")?;
            }
            write!(out, "\x1b[?25h")?;
        }
        Ok(())
    }

    fn draw(&mut self, out: &mut impl Write, frame: &str, height: usize) -> io::Result<()> {
        if self.rows > 0 && self.last_frame == frame {
            return Ok(());
        }
        // Send the erase and replacement together so the terminal can paint
        // them in one pass even when a scene changes size.
        let mut update = Vec::with_capacity(frame.len() + 256);
        self.clear(&mut update, height)?;
        write!(&mut update, "\x1b[?25l{frame}")?;
        self.rows = frame.lines().count();
        self.last_frame.push_str(frame);
        out.write_all(&update)?;
        out.flush()
    }
}

struct Display {
    scene: Option<Scene>,
    work: Work,
    skips: ConfigSkips,
    region: Region,
    static_stages: bool,
    palette: Palette,
    unicode: bool,
    started: Instant,
}

impl Display {
    fn frame(&self, elapsed: Duration, (height, width): (u16, u16)) -> Option<String> {
        let scene = self.scene?;
        if elapsed < FIRST_PAINT || width < 2 || height == 0 {
            return None;
        }
        let status = self.work.status(usize::from(width - 1));
        let status = match status.strip_prefix("open-sdbl") {
            Some(rest) => format!("{}{rest}", self.palette.mint("open-sdbl", false)),
            None => status,
        };
        let Some(size) = Size::fit(height, width) else {
            return Some(status);
        };
        let art = scene.draw(
            self.work.fraction(),
            elapsed - FIRST_PAINT,
            size,
            self.unicode,
            Colors::detect(self.palette),
        );
        Some(format!("{art}\r\n{status}"))
    }

    fn paint(&mut self, out: &mut impl Write) -> io::Result<()> {
        let size = dimensions();
        if let Some(frame) = self.frame(self.started.elapsed(), size) {
            self.region.draw(out, &frame, size.0.into())?;
        } else if self.region.rows != 0 {
            self.region.clear(out, size.0.into())?;
            out.flush()?;
        }
        Ok(())
    }

    fn permanent(&mut self, out: &mut impl Write, text: &str) -> io::Result<()> {
        self.region.clear(out, dimensions().0.into())?;
        writeln!(out, "{text}")?;
        out.flush()?;
        self.paint(out)
    }

    fn static_stage(&self) {
        if self.static_stages {
            eprintln!("{}", self.work.status(usize::MAX));
        }
    }

    fn flush_skips(&mut self) {
        if let Some(block) = self.skips.take_warning(self.palette) {
            let _ = self.permanent(&mut io::stderr().lock(), &block);
        }
    }
}

fn dimensions() -> (u16, u16) {
    #[cfg(unix)]
    if let Some(size) = crate::terminal::size(libc::STDERR_FILENO) {
        return size;
    }
    (1, 1)
}

fn dot_font() -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|s| !s.is_empty()))
        .unwrap_or_default();
    locale
        .to_ascii_lowercase()
        .replace('-', "")
        .contains("utf8")
        && "⠿".width() == 1
}

pub(crate) struct Startup {
    pub(crate) interactive: bool,
    display: RefCell<Display>,
}

impl Startup {
    pub(crate) fn new() -> Self {
        let interactive = crate::terminal::console_is_interactive();
        let surface = Surface::stderr();
        let live = interactive && surface.tty && cfg!(unix);
        let scene = if live { Scene::random() } else { None };
        Self {
            interactive,
            display: RefCell::new(Display {
                scene,
                work: Work {
                    phase: "connecting",
                    ..Work::default()
                },
                skips: ConfigSkips::default(),
                region: Region::default(),
                static_stages: interactive && io::stderr().is_terminal() && scene.is_none(),
                palette: if live {
                    surface.palette
                } else {
                    Palette::Plain
                },
                unicode: dot_font(),
                started: Instant::now(),
            }),
        }
    }

    pub(crate) fn utf8_guard(&self) -> Result<super::TerminalUtf8Guard, CliError> {
        super::TerminalUtf8Guard::enable(self.interactive)
    }

    pub(crate) fn reporter(&self) -> Reporter<'_> {
        Reporter(self)
    }

    pub(crate) fn warn(&self, message: &str) {
        let mut display = self.display.borrow_mut();
        let text = format!(
            "{}  {}",
            display.palette.label("WARN"),
            escape_field(message)
        );
        let _ = display.permanent(&mut io::stderr().lock(), &text);
    }

    pub(crate) async fn watch<T>(
        &self,
        work: impl Future<Output = Result<T, CliError>>,
    ) -> Result<(T, Option<super::StartupInput>), CliError> {
        if !self.interactive {
            return work.await.map(|result| (result, None));
        }
        #[cfg(unix)]
        let mut interrupt =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .map_err(|err| CliError::Io("cannot monitor console interrupt".into(), err))?;
        #[cfg(unix)]
        let cancelled = interrupt.recv();
        #[cfg(not(unix))]
        let cancelled = tokio::signal::ctrl_c();
        tokio::pin!(cancelled);

        #[cfg(unix)]
        let input = if Surface::stdout().tty {
            let input = super::StartupInput::enable().ok();
            if input.is_none() {
                let mut display = self.display.borrow_mut();
                display.scene = None;
                display.static_stages = io::stderr().is_terminal();
                display.palette = Palette::Plain;
            }
            input
        } else {
            None
        };
        #[cfg(not(unix))]
        let input = None;
        self.display.borrow().static_stage();
        let mut ticks =
            tokio::time::interval_at(tokio::time::Instant::now() + FIRST_PAINT, FRAME_INTERVAL);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let live = self.display.borrow().scene.is_some();
        tokio::pin!(work);
        loop {
            tokio::select! {
                biased;
                _ = &mut cancelled => return Err(CliError::Cancelled),
                result = &mut work => return result.map(|value|(value,input)),
                _ = ticks.tick(), if live => {
                    self.display.borrow_mut().paint(&mut io::stderr().lock()).map_err(|e|CliError::Io("cannot update console startup".into(),e))?;
                }
            }
        }
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        let display = self.display.get_mut();
        display.scene = None;
        display.flush_skips();
        let mut out = io::stderr().lock();
        let _ = display.region.clear(&mut out, dimensions().0.into());
        let _ = out.flush();
    }
}

pub(crate) struct Reporter<'a>(&'a Startup);

impl MetadataProgress for Reporter<'_> {
    fn phase(&mut self, phase: &'static str) {
        let mut d = self.0.display.borrow_mut();
        if phase == d.work.phase {
            return;
        }
        d.flush_skips();
        if d.work.phase == "Config" {
            d.work.config_complete = true;
            if d.work.totals.is_some() {
                d.static_stage();
            }
        }
        d.work.phase = phase;
        d.static_stage();
    }
    fn config_totals(&mut self, resources: u64, bytes: u64) {
        let mut d = self.0.display.borrow_mut();
        d.work.totals = Some((resources, bytes));
        d.static_stage();
    }
    fn advance_config(&mut self, resources: usize, bytes: usize) {
        let mut d = self.0.display.borrow_mut();
        d.work.resources = d.work.resources.saturating_add(resources as u64);
        d.work.bytes = d.work.bytes.saturating_add(bytes as u64);
    }
    fn skipped_config(&mut self, resource: &str, reason: MetadataErrorKind) {
        self.0.display.borrow_mut().skips.record(resource, reason);
    }
    fn warning(&mut self, message: &str) {
        self.0.warn(message);
    }
    fn finish(&mut self) {
        self.0.display.borrow_mut().flush_skips();
    }
}

#[cfg(test)]
#[path = "../../tests/repl_startup.rs"]
mod tests;
