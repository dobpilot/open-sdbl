//! The terminal itself: reading a bounded line, the pinned footer, the
//! UTF-8 guard and size detection.

use std::io::{self, IsTerminal, Write};
#[cfg(target_os = "linux")]
use std::mem::MaybeUninit;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

use crate::error::CliError;

use super::*;

#[cfg(test)]
#[path = "../tests/repl_terminal.rs"]
mod tests;

pub(super) fn statement_is_complete(source: &str) -> bool {
    let mut characters = source.chars().peekable();
    let mut string = false;
    let mut comment = false;
    let mut last_significant = None;
    while let Some(character) = characters.next() {
        if comment {
            if character == '\n' {
                comment = false;
            }
            continue;
        }
        if string {
            if character == '"' {
                if characters.peek() == Some(&'"') {
                    characters.next();
                } else {
                    string = false;
                }
            }
            continue;
        }
        match character {
            '"' => string = true,
            '/' if characters.peek() == Some(&'/') => {
                characters.next();
                comment = true;
            }
            value if !value.is_whitespace() => last_significant = Some(value),
            _ => {}
        }
    }
    !string && last_significant == Some(';')
}

pub(super) fn decode_input_line(line: &[u8]) -> Result<&str, usize> {
    std::str::from_utf8(line).map_err(|error| error.valid_up_to())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundedLine {
    Read(usize),
    TooLong,
}

pub(super) async fn read_bounded_line(
    input: &mut (impl AsyncBufRead + Unpin),
    output: &mut Vec<u8>,
    limit: usize,
) -> io::Result<BoundedLine> {
    let mut too_long = false;
    loop {
        let buffer = input.fill_buf().await?;
        if buffer.is_empty() {
            return Ok(if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Read(output.len())
            });
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |position| position + 1);
        if !too_long {
            let retained = consumed.min(limit.saturating_add(1).saturating_sub(output.len()));
            output.extend_from_slice(&buffer[..retained]);
            too_long = output.len() > limit;
        }
        input.consume(consumed);
        if newline.is_some() {
            return Ok(if too_long {
                BoundedLine::TooLong
            } else {
                BoundedLine::Read(output.len())
            });
        }
    }
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn footer_text(columns: u16) -> String {
    let available = usize::from(columns.saturating_sub(1));
    if COMMAND_HINT.len() <= available {
        return COMMAND_HINT.to_owned();
    }
    if available <= 3 {
        return ".".repeat(available);
    }
    format!("{}...", &COMMAND_HINT[..available - 3])
}

#[cfg(target_os = "linux")]
pub(super) struct PinnedFooter {
    enabled: bool,
    active: bool,
    rows: u16,
    columns: u16,
}

#[cfg(target_os = "linux")]
impl PinnedFooter {
    pub(super) fn enable(interactive: bool) -> Result<Self, CliError> {
        let mut footer = Self {
            enabled: interactive && io::stdout().is_terminal(),
            active: false,
            rows: 0,
            columns: 0,
        };
        if footer.enabled {
            footer.redraw()?;
        }
        Ok(footer)
    }

    pub(super) fn redraw(&mut self) -> Result<(), CliError> {
        if !self.enabled {
            return Ok(());
        }
        let Some((rows, columns)) = terminal_size() else {
            return Ok(());
        };
        if rows < 3 || columns < 4 {
            self.restore()?;
            return Ok(());
        }

        if !self.active || self.rows != rows || self.columns != columns {
            self.restore()?;
            self.rows = rows;
            self.columns = columns;
            self.active = true;
            let hint = footer_text(columns);
            write_terminal(format_args!(
                "\x1b[1;{}r\x1b[{};1H\x1b[2K\x1b[2m{}\x1b[0m\x1b[{};1H",
                rows - 1,
                rows,
                hint,
                rows - 1
            ))?;
        } else {
            let hint = footer_text(columns);
            write_terminal(format_args!(
                "\x1b7\x1b[{};1H\x1b[2K\x1b[2m{}\x1b[0m\x1b8",
                rows, hint
            ))?;
        }
        Ok(())
    }

    pub(super) fn restore(&mut self) -> Result<(), CliError> {
        if self.active {
            write_terminal(format_args!("\x1b[r\x1b[{};1H\x1b[2K", self.rows))?;
            self.active = false;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for PinnedFooter {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(target_os = "linux")]
pub(super) fn terminal_size() -> Option<(u16, u16)> {
    let mut size = MaybeUninit::<libc::winsize>::uninit();
    // SAFETY: `size` is writable storage for `winsize`, and stdout was
    // verified to be an interactive terminal before this function is used.
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, size.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: a successful TIOCGWINSZ call initialized the complete value.
    let size = unsafe { size.assume_init() };
    (size.ws_row > 0 && size.ws_col > 0).then_some((size.ws_row, size.ws_col))
}

#[cfg(target_os = "linux")]
pub(super) fn write_terminal(arguments: std::fmt::Arguments<'_>) -> Result<(), CliError> {
    let mut output = io::stdout().lock();
    output
        .write_fmt(arguments)
        .and_then(|()| output.flush())
        .map_err(|error| CliError::Io("cannot update terminal footer".to_owned(), error))
}

#[cfg(not(target_os = "linux"))]
pub(super) struct PinnedFooter;

#[cfg(not(target_os = "linux"))]
impl PinnedFooter {
    pub(super) fn enable(_interactive: bool) -> Result<Self, CliError> {
        Ok(Self)
    }

    pub(super) fn redraw(&mut self) -> Result<(), CliError> {
        Ok(())
    }

    pub(super) fn restore(&mut self) -> Result<(), CliError> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub(super) struct TerminalUtf8Guard {
    original: Option<libc::termios>,
}

#[cfg(target_os = "linux")]
impl TerminalUtf8Guard {
    pub(super) fn enable(interactive: bool) -> Result<Self, CliError> {
        if !interactive {
            return Ok(Self { original: None });
        }

        let mut original = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: `original` points to writable storage for a complete termios
        // value, and STDIN_FILENO is valid for this process.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, original.as_mut_ptr()) } != 0 {
            return Err(CliError::Io(
                "cannot inspect terminal input settings".to_owned(),
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: tcgetattr returned success and initialized the value.
        let original = unsafe { original.assume_init() };
        if original.c_iflag & libc::IUTF8 != 0 {
            return Ok(Self { original: None });
        }

        let mut updated = original;
        updated.c_iflag |= libc::IUTF8;
        // SAFETY: `updated` is a valid termios value obtained from this stdin
        // terminal with only the documented IUTF8 input bit changed.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &updated) } != 0 {
            return Err(CliError::Io(
                "cannot enable UTF-8 terminal input".to_owned(),
                io::Error::last_os_error(),
            ));
        }
        Ok(Self {
            original: Some(original),
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for TerminalUtf8Guard {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            // SAFETY: this is the complete termios value read from stdin by
            // `enable`; restoration is best-effort during scope cleanup.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, original);
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) struct TerminalUtf8Guard;

#[cfg(not(target_os = "linux"))]
impl TerminalUtf8Guard {
    pub(super) fn enable(_interactive: bool) -> Result<Self, CliError> {
        Ok(Self)
    }
}
