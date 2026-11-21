use super::{Canvas, Ink, Point, variation};
use std::f64::consts::{PI, TAU};

/// Curves use a normalized canvas. The grain is cut into the wood before
/// converting to terminal dots, so new columns reveal finer parallel cuts.
#[derive(Clone, Copy)]
struct Limb {
    root: Point,
    tip: Point,
    bend: f64,
    radius: f64,
    depth: u8,
    id: u32,
}

impl Limb {
    fn point(self, t: f64) -> Point {
        (
            self.root.0 + (self.tip.0 - self.root.0) * t,
            self.root.1 + (self.tip.1 - self.root.1) * t + self.bend * (PI * t).sin(),
        )
    }

    fn paint(self, c: &mut Canvas, seed: u32, reach: f64, pulse: f64) {
        let dx = (self.tip.0 - self.root.0) * c.width as f64;
        let steps = (dx.abs()
            + (self.tip.1 - self.root.1).abs() * c.height as f64
            + self.bend.abs() * PI * c.height as f64)
            .ceil()
            .max(1.) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let (x, y) = self.point(t);
            let remaining = reach - x - 0.22 * (y - 0.5).abs();
            if remaining < 0. {
                continue;
            }
            let dy =
                ((self.tip.1 - self.root.1) + PI * self.bend * (PI * t).cos()) * c.height as f64;
            let len = dx.hypot(dy).max(0.01);
            let radius = (self.radius
                * (remaining / 0.08).clamp(0., 1.)
                * (1. - t).powf(0.75)
                * c.height as f64)
                .max(0.45);
            let lit = (x - pulse).abs() < 0.025 && ((self.id == 1 && t < 0.18) || self.id == 9);
            for offset in -(radius.ceil() as i32)..=radius.ceil() as i32 {
                let across = f64::from(offset) / radius;
                if across.abs() > 1. {
                    continue;
                }
                let grain = across * 11. + 1.2 * (t * TAU * 1.7).sin();
                let ink = if lit && across < 0.25 {
                    Ink::Glint
                } else if radius > 2. && grain.cos() > 0.78 && across.abs() < 0.84 {
                    Ink::Paper
                } else if across < -0.5 {
                    Ink::Lit
                } else if across < 0. {
                    Ink::Surface
                } else if across < 0.58 {
                    Ink::Midtone
                } else {
                    Ink::Shadow
                };
                c.dot(
                    (x * c.width as f64 - f64::from(offset) * dy / len).round() as i32,
                    (y * c.height as f64 + f64::from(offset) * dx / len).round() as i32,
                    ink,
                );
            }
        }
        if self.depth == 0 {
            return;
        }
        let trunk = self.id == 1;
        let count = if trunk { 6 } else { 2 };
        for j in 0..count {
            let id = self.id * 8 + j + 1;
            let noise = variation(seed, id);
            let t = if trunk {
                0.16 + f64::from(j) * 0.14
            } else {
                0.48 + f64::from(j) * 0.30
            };
            let root = self.point(t + 0.045 * (noise - 0.5));
            let sign = if j % 2 == 0 { -1. } else { 1. };
            let (length, rise) = if trunk {
                (0.14 + 0.06 * noise, sign * (0.29 + 0.10 * noise))
            } else {
                (
                    (self.tip.0 - self.root.0) * (0.40 + 0.14 * noise),
                    (self.tip.1 - self.root.1) * 0.27 + sign * (0.045 + 0.065 * noise),
                )
            };
            Limb {
                root,
                tip: (root.0 + length, (root.1 + rise).clamp(0.02, 0.96)),
                bend: sign * (noise - 0.35) * 0.09,
                radius: self.radius * (1. - t * 0.65) * 0.46,
                depth: self.depth - 1,
                id,
            }
            .paint(c, seed, reach, pulse);
        }
    }
}

pub(super) fn draw(c: &mut Canvas, seed: u32, progress: f64, seconds: f64) {
    Limb {
        root: (-0.04, 0.43),
        tip: (0.99, 0.32 + variation(seed, 0) * 0.10),
        bend: 0.14,
        radius: 0.081,
        depth: 3,
        id: 1,
    }
    .paint(
        c,
        seed,
        0.44 + 0.69 * progress,
        seconds.rem_euclid(2.5) * 0.5,
    );
}
