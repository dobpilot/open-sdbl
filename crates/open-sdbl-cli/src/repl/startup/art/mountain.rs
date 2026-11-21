use super::{Canvas, Ink, Point, variation};

/// An isometric block below a triangulated height field. Draw the ground
/// mesh back to front so nearer slopes cover the grid and distant ridges.
struct Projection {
    width: f64,
    height: f64,
    offset: f64,
}
impl Projection {
    fn at(&self, u: f64, v: f64, z: f64) -> Point {
        (
            self.width * (0.5 + 0.44 * (u - v)),
            self.height * (0.31 + 0.18 * (u + v) - z + self.offset),
        )
    }
}

fn elevation(u: f64, v: f64, peaks: &[(f64, f64, f64, f64); 5], phase: f64) -> f64 {
    let mut h = 0_f64;
    for &(cx, cy, peak, spread) in peaks {
        let dx = (u - cx).abs();
        let dy = (v - cy).abs();
        let distance = dx.max(dy) + 0.45 * dx.min(dy);
        h = h.max(peak * (1. - distance / spread));
    }
    0.045 + h + (0.009 + h * 0.035) * ((26. * u + 7. * v + phase).sin() + (22. * v - 9. * u).cos())
}

fn edge(a: Point, b: Point, p: Point) -> f64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

fn facet(c: &mut Canvas, mut pts: [Point; 3], light: f64, offset: i32, sheen: f64) {
    if edge(pts[0], pts[1], pts[2]) < 0. {
        pts.swap(1, 2);
    }
    let [a, b, d] = pts;
    let x0 = a.0.min(b.0).min(d.0).floor().max(0.) as i32;
    let x1 = a.0.max(b.0).max(d.0).ceil().min(c.width as f64 - 1.) as i32;
    let y0 = a.1.min(b.1).min(d.1).floor().max(0.) as i32;
    let y1 = a.1.max(b.1).max(d.1).ceil().min(c.height as f64 - 1.) as i32;
    let tone = if light > 0.63 {
        Ink::Lit
    } else if light > 0.22 {
        Ink::Surface
    } else if light > -0.15 {
        Ink::Midtone
    } else {
        Ink::Shadow
    };
    for y in y0..=y1 {
        for x in x0..=x1 {
            let p = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            if edge(a, b, p) < 0. || edge(b, d, p) < 0. || edge(d, a, p) < 0. {
                continue;
            }
            let local_y = y - offset;
            let diagonal = if light > 0.22 { x } else { -x };
            let hatch = (diagonal + 2 * local_y).rem_euclid(8);
            let dense = (4. - 2.5 * light) as i32;
            let ink = if hatch < dense || (light < 0. && (x - local_y) % 7 == 0) {
                if (p.0 - sheen).abs() < 1.6 && light > 0.25 {
                    Ink::Glint
                } else {
                    tone
                }
            } else {
                Ink::Paper
            };
            c.dot(x, y, ink);
        }
    }
}

pub(super) fn draw(c: &mut Canvas, seed: u32, progress: f64, seconds: f64) {
    let view = Projection {
        width: (c.width - 1) as f64,
        height: (c.height - 1) as f64,
        offset: 0.44 * (1. - progress),
    };
    let sheen = seconds.rem_euclid(3.6) * 0.6 * view.width;
    for side in [false, true] {
        let at = |t, z| {
            if side {
                view.at(1., t, z)
            } else {
                view.at(t, 1., z)
            }
        };
        let tone = if side { Ink::Midtone } else { Ink::Surface };
        for z in [0., -0.07, -0.15, -0.17, -0.25, -0.29] {
            c.line(at(0., z), at(1., z), tone);
        }
        for i in 0..=9 {
            let t = f64::from(i) / 9.;
            c.line(
                at(t, 0.),
                at(t, -0.29),
                if i == 9 { Ink::Lit } else { tone },
            );
        }
        for i in 1..35 {
            let t = f64::from(i) / 35.;
            c.line(at(t, -0.17), at(t, -0.20), Ink::Shadow);
        }
        let t = if side {
            2. - 2. * sheen / view.width
        } else {
            2. * sheen / view.width
        };
        if (0.03..0.97).contains(&t) {
            c.line(at(t - 0.03, -0.15), at(t + 0.03, -0.15), Ink::Glint);
        }
    }
    let mut peaks = [
        (0.49, 0.41, 0.41, 0.44),
        (0.23, 0.68, 0.30, 0.37),
        (0.78, 0.59, 0.28, 0.32),
        (0.31, 0.17, 0.23, 0.34),
        (0.74, 0.21, 0.20, 0.31),
    ];
    for (i, (u, v, _, _)) in peaks.iter_mut().enumerate() {
        *u += (variation(seed, 2 * i as u32) - 0.5) * 0.09;
        *v += (variation(seed, 2 * i as u32 + 1) - 0.5) * 0.09;
    }
    let n = (c.size.cols / 5).clamp(14, 40);
    let phase = 4. * variation(seed, 31);
    let mut mesh = Vec::with_capacity((n + 1) * (n + 1));
    for v in 0..=n {
        for u in 0..=n {
            let (x, y) = (u as f64 / n as f64, v as f64 / n as f64);
            let z = elevation(x, y, &peaks, phase);
            mesh.push((view.at(x, y, z), z));
        }
    }
    for depth in 0..2 * n - 1 {
        for u in depth.saturating_sub(n - 1)..=depth.min(n - 1) {
            let i = (depth - u) * (n + 1) + u;
            let (a, b, e, d) = (mesh[i], mesh[i + 1], mesh[i + n + 1], mesh[i + n + 2]);
            for (points, du, dv) in [
                ([a.0, b.0, d.0], b.1 - a.1, d.1 - b.1),
                ([a.0, d.0, e.0], d.1 - e.1, e.1 - a.1),
            ] {
                let (du, dv) = (du * n as f64, dv * n as f64);
                let light = (0.35 + 0.9 * du - 0.55 * dv) / (1. + du * du + dv * dv).sqrt();
                facet(
                    c,
                    points,
                    light,
                    (view.offset * view.height).round() as i32,
                    sheen,
                );
            }
            if u == n - 1 {
                c.line(b.0, d.0, Ink::Midtone);
            }
            if depth - u == n - 1 {
                c.line(e.0, d.0, Ink::Surface);
            }
        }
    }
}
