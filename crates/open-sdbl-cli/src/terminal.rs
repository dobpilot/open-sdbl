use std::env;
use std::io::{self, IsTerminal};

pub(crate) fn ci() -> bool {
    set("CI")
}

pub(crate) fn console_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal() && !ci()
}

pub(crate) fn interactive() -> bool {
    !ci() && !matches!(env::var("TERM").as_deref(), Ok("dumb" | "cons25" | "emacs"))
}

fn set(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

#[derive(Clone, Copy)]
pub(crate) struct Surface {
    pub(crate) tty: bool,
    pub(crate) palette: Palette,
}

impl Surface {
    pub(crate) fn stdout() -> Self {
        Self::new(io::stdout().is_terminal())
    }

    pub(crate) fn stderr() -> Self {
        let tty = io::stderr().is_terminal();
        let scripted_console = tty
            && !console_is_interactive()
            && std::env::args_os()
                .nth(1)
                .is_some_and(|name| name == "console" || name == "repl");
        Self::new(tty && !scripted_console)
    }

    fn new(is_terminal: bool) -> Self {
        let tty = is_terminal && interactive();
        Self {
            tty,
            palette: if tty && !set("NO_COLOR") {
                Palette::detect()
            } else {
                Palette::Plain
            },
        }
    }
}

#[cfg(unix)]
pub(crate) fn size(fd: std::os::fd::RawFd) -> Option<(u16, u16)> {
    let mut dimensions = std::mem::MaybeUninit::<libc::winsize>::uninit();
    // SAFETY: TIOCGWINSZ fills the supplied winsize for a terminal descriptor.
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, dimensions.as_mut_ptr()) } != 0 {
        return None;
    }
    let dimensions = unsafe { dimensions.assume_init() };
    (dimensions.ws_col > 0 && dimensions.ws_row > 0)
        .then_some((dimensions.ws_row, dimensions.ws_col))
}

pub(crate) fn columns(stderr: bool) -> Option<usize> {
    #[cfg(unix)]
    {
        size(if stderr {
            libc::STDERR_FILENO
        } else {
            libc::STDOUT_FILENO
        })
        .map(|(_, columns)| columns as usize)
    }
    #[cfg(not(unix))]
    {
        let _ = stderr;
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Palette {
    Plain,
    Ansi,
    Dark,
    Light,
}

impl Palette {
    fn detect() -> Self {
        if matches!(env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit")) {
            let background = env::var("COLORFGBG").unwrap_or_default();
            match background.rsplit(';').next() {
                Some("0") => return Self::Dark,
                Some("7" | "15") => return Self::Light,
                _ => {}
            }
        }
        Self::Ansi
    }

    pub(crate) fn mint(self, text: &str, edge: bool) -> String {
        let style = match (self, edge) {
            (Self::Plain, _) => "",
            (Self::Ansi, false) => "32",
            (Self::Ansi, true) => "1;32",
            (Self::Dark, false) => "38;2;52;213;154",
            (Self::Dark, true) => "38;2;181;245;220",
            (Self::Light, false) => "38;2;8;117;78",
            (Self::Light, true) => "38;2;19;128;90",
        };
        styled(style, text)
    }

    pub(crate) fn title(self, text: &str, glint: Option<usize>) -> String {
        // The only animated text is the fixed ASCII product or command name.
        if let Some(start) = glint.filter(|start| start + 2 <= text.len()) {
            let end = start + 2;
            format!(
                "{}{}{}",
                self.mint(&text[..start], false),
                self.mint(&text[start..end], true),
                self.mint(&text[end..], false)
            )
        } else {
            self.mint(text, false)
        }
    }

    pub(crate) fn label(self, label: &str) -> String {
        match (self, label) {
            (Self::Plain, _) => label.into(),
            (_, "WARN") => styled("1;33", label),
            (_, "ERROR") => styled("1;31", label),
            _ => label.into(),
        }
    }

    pub(crate) fn bold(self, text: &str) -> String {
        styled(if self == Self::Plain { "" } else { "1" }, text)
    }
}

fn styled(style: &str, text: &str) -> String {
    if style.is_empty() || text.is_empty() {
        text.into()
    } else {
        format!("\x1b[{style}m{text}\x1b[0m")
    }
}
