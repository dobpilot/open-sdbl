//! The motif and its seed are launch state. All three shapes are rasterized
//! directly onto the current terminal's 2-by-4 dot cells.

mod branch;
mod globe;
mod mountain;

use std::fmt::Write;
use std::time::Duration;

use crate::terminal::Palette;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Motif {
    Branch,
    Sphere,
    Mountain,
}

impl Motif {
    pub(super) fn sample(byte: u8) -> Option<Self> {
        (byte != 255).then(|| [Self::Branch, Self::Sphere, Self::Mountain][usize::from(byte % 3)])
    }
}

#[derive(Clone, Copy)]
pub(super) struct Scene {
    pub(super) motif: Motif,
    pub(super) seed: u32,
}

impl Scene {
    pub(super) fn random() -> Option<Self> {
        let rng = rustls::crypto::ring::default_provider().secure_random;
        let mut bytes = [0; 5];
        loop {
            rng.fill(&mut bytes).ok()?;
            if let Some(motif) = Motif::sample(bytes[0]) {
                return Some(Self {
                    motif,
                    seed: u32::from_le_bytes(bytes[1..].try_into().unwrap()),
                });
            }
        }
    }

    pub(super) fn draw(
        self,
        progress: f64,
        time: Duration,
        size: Size,
        unicode: bool,
        colors: Colors,
    ) -> String {
        let mut canvas = Canvas::new(size);
        let seconds = time.as_secs_f64();
        match self.motif {
            Motif::Branch => branch::draw(&mut canvas, self.seed, progress, seconds),
            Motif::Mountain => mountain::draw(&mut canvas, self.seed, progress, seconds),
            Motif::Sphere => globe::draw(&mut canvas, self.seed, progress, seconds),
        }
        canvas.render(unicode, colors)
    }
}

// Stable, independently addressed variation. A resize can change the number of
// samples without changing the geometry that those samples describe.
fn variation(seed: u32, index: u32) -> f64 {
    let mut x = seed ^ index.wrapping_mul(0x9e3779b9);
    x = (x ^ (x >> 16)).wrapping_mul(0x21f0aaad);
    x = (x ^ (x >> 15)).wrapping_mul(0x735a2d97);
    f64::from(x ^ (x >> 15)) / f64::from(u32::MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Size {
    pub(super) cols: usize,
    pub(super) rows: usize,
}

impl Size {
    pub(super) fn fit(height: u16, width: u16) -> Option<Self> {
        if width < 52 || height < 14 {
            return None;
        }
        let cols = usize::from(width - 1).min(512);
        let rows = (cols / 4).min(usize::from(height - 4)).min(96);
        Some(Self { cols, rows })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Ink {
    #[default]
    Paper,
    Shadow,
    Midtone,
    Surface,
    Lit,
    Glint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Colors {
    Plain,
    Basic,
    Indexed,
    Dark,
    Light,
    Neutral,
}

impl Colors {
    pub(super) fn detect(palette: Palette) -> Self {
        match palette {
            Palette::Plain => Self::Plain,
            Palette::Dark => Self::Dark,
            Palette::Light => Self::Light,
            Palette::Ansi => {
                if matches!(
                    std::env::var("COLORTERM").as_deref(),
                    Ok("truecolor" | "24bit")
                ) {
                    Self::Neutral
                } else if std::env::var("TERM").is_ok_and(|term| term.contains("256color")) {
                    Self::Indexed
                } else {
                    Self::Basic
                }
            }
        }
    }

    fn style(self, ink: Ink) -> &'static str {
        if ink == Ink::Paper || self == Self::Plain {
            return "";
        }
        let ramp = match self {
            Self::Plain => unreachable!(),
            // Shadow, midtone, surface, lit edge, then the single green accent.
            Self::Basic => ["2", "90", "", "1", "1;32"],
            Self::Indexed => ["38;5;241", "38;5;244", "38;5;247", "38;5;250", "38;5;78"],
            Self::Dark => [
                "38;2;112;112;112",
                "38;2;150;150;150",
                "38;2;195;195;195",
                "38;2;241;241;241",
                "38;2;52;213;154",
            ],
            Self::Light => [
                "38;2;140;140;140",
                "38;2;108;108;108",
                "38;2;75;75;75",
                "38;2;41;41;41",
                "38;2;8;117;78",
            ],
            Self::Neutral => [
                "38;2;106;106;106",
                "38;2;124;124;124",
                "38;2;144;144;144",
                "38;2;164;164;164",
                "38;2;46;151;110",
            ],
        };
        ramp[ink as usize - 1]
    }
}

type Point = (f64, f64);
struct Canvas {
    size: Size,
    width: usize,
    height: usize,
    dots: Vec<Ink>,
    labels: Vec<(usize, usize, String)>,
}

impl Canvas {
    fn new(size: Size) -> Self {
        let (width, height) = (size.cols * 2, size.rows * 4);
        Self {
            size,
            width,
            height,
            dots: vec![Ink::Paper; width * height],
            labels: Vec::new(),
        }
    }

    fn dot(&mut self, x: i32, y: i32, ink: Ink) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.dots[y as usize * self.width + x as usize] = ink;
        }
    }

    fn line(&mut self, a: Point, b: Point, ink: Ink) {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            self.dot(
                (a.0 + t * dx).round() as i32,
                (a.1 + t * dy).round() as i32,
                ink,
            );
        }
    }

    fn render(&self, unicode: bool, colors: Colors) -> String {
        const BITS: [[u8; 2]; 4] = [[1, 8], [2, 16], [4, 32], [64, 128]];
        let mut out = String::new();
        let mut row = vec![(' ', Ink::Paper); self.size.cols];
        for y in 0..self.size.rows {
            if y > 0 {
                out.push_str("\r\n");
            }
            for (x, cell) in row.iter_mut().enumerate() {
                let mut mask = 0u8;
                let mut ink = Ink::Paper;
                for (dy, bits) in BITS.iter().enumerate() {
                    let i = (y * 4 + dy) * self.width + 2 * x;
                    for (&weight, &dot) in bits.iter().zip(&self.dots[i..i + 2]) {
                        if dot != Ink::Paper {
                            mask |= weight;
                            ink = ink.max(dot);
                        }
                    }
                }
                let glyph = if mask == 0 {
                    ' '
                } else if colors == Colors::Plain && ink == Ink::Glint {
                    '*'
                } else if unicode {
                    char::from_u32(0x2800 + u32::from(mask)).unwrap()
                } else {
                    match mask.count_ones() {
                        1..=2 => '.',
                        3..=4 if mask & 0x2a != 0 => '/',
                        3..=4 => '\\',
                        5..=6 => '+',
                        _ => '#',
                    }
                };
                *cell = (glyph, ink);
            }
            for (cx, cy, label) in &self.labels {
                if *cy == y {
                    let start = cx.saturating_sub(label.len() / 2);
                    for (cell, c) in row[start..].iter_mut().zip(label.chars()) {
                        *cell = (c, Ink::Paper);
                    }
                }
            }
            let used = row.iter().rposition(|c| c.0 != ' ').map_or(0, |i| i + 1);
            let mut last = "";
            for &(glyph, ink) in &row[..used] {
                let style = colors.style(ink);
                if style != last {
                    if !last.is_empty() {
                        out.push_str("\x1b[0m");
                    }
                    if !style.is_empty() {
                        let _ = write!(out, "\x1b[{style}m");
                    }
                    last = style;
                }
                out.push(glyph);
            }
            if !last.is_empty() {
                out.push_str("\x1b[0m");
            }
        }
        out
    }
}
