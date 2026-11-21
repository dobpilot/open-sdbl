use super::{Canvas, Ink, variation};
use std::f64::consts::TAU;

pub(super) fn draw(c: &mut Canvas, seed: u32, fill: f64, seconds: f64) {
    let yaw = seconds.rem_euclid(4.) * 90.;
    let radius = (c.height.min(c.width) as f64 * 0.47).max(1.);
    let center = ((c.width - 1) as f64 / 2., (c.height - 1) as f64 / 2.);
    let (sa, ca) = (0.12 + 0.14 * variation(seed, 3)).sin_cos();
    let (sk, ck) = (0.10 + 0.18 * variation(seed, 5)).sin_cos();
    let project = |lat: f64, lon: f64| {
        let (sp, cp) = lat.to_radians().sin_cos();
        let (sl, cl) = (lon + yaw).to_radians().sin_cos();
        let (x, z) = (cp * sl, cp * cl);
        let v = sp * ca - z * sa;
        (
            (
                center.0 + radius * (x * ck - v * sk),
                center.1 - radius * (v * ck + x * sk),
            ),
            z * ca + sp * sa,
        )
    };
    let segments = (radius * TAU).ceil() as usize;
    for lon in (0..360).step_by(36) {
        for i in 0..=segments / 2 {
            let (p, z) = project(
                -89. + 178. * i as f64 / (segments / 2) as f64,
                f64::from(lon),
            );
            if z >= 0. || i % 9 == 0 {
                c.dot(
                    p.0.round() as i32,
                    p.1.round() as i32,
                    if z < 0.25 { Ink::Shadow } else { Ink::Midtone },
                );
            }
        }
    }
    for lat in [-56, -34, -14, 8, 30, 56] {
        for i in 0..segments {
            let (p, z) = project(f64::from(lat), 360. * i as f64 / segments as f64);
            if z >= 0. {
                c.dot(
                    p.0.round() as i32,
                    p.1.round() as i32,
                    if z > 0.65 { Ink::Surface } else { Ink::Midtone },
                );
            }
        }
    }
    let lanes = (radius / 5.).round().max(3.) as usize;
    let samples = (radius * 2.8).ceil() as usize;
    for (band, lat) in [-42., -4., 23.].into_iter().enumerate() {
        let load = (3. * fill - band as f64).clamp(0., 1.);
        let spacing = 4 - (3. * load) as usize;
        for j in 0..lanes {
            for i in 0..samples {
                if (i + j) % spacing != 0 {
                    continue;
                }
                let (p, z) = project(
                    lat + 12. * j as f64 / (lanes - 1) as f64,
                    360. * (i as f64 + 0.5 * (j % 2) as f64) / samples as f64,
                );
                if z > 0. {
                    let ink = if z > 0.96 && load > 0. && j == lanes - 1 {
                        Ink::Glint
                    } else if z > 0.70 && load > 0. {
                        Ink::Lit
                    } else if z > 0.45 {
                        Ink::Surface
                    } else {
                        Ink::Shadow
                    };
                    c.dot(p.0.round() as i32, p.1.round() as i32, ink);
                }
            }
        }
    }
    for i in 0..segments * 2 {
        let (s, k) = (TAU * i as f64 / (segments * 2) as f64).sin_cos();
        c.dot(
            (center.0 + radius * k).round() as i32,
            (center.1 + radius * s).round() as i32,
            if s + k < 0. { Ink::Lit } else { Ink::Midtone },
        );
    }
    for (lat, lon, name) in [
        (-3., 0., "SDBL"),
        (-3., -130., "serve"),
        (28., -23., "lex"),
        (-36., 23., "Config"),
        (-36., -132., "DBNames"),
    ] {
        let (p, z) = project(lat, lon);
        if z < 0.34 {
            continue;
        }
        let text = if z > 0.84 && radius >= 29. {
            name.chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            name.into()
        };
        c.labels
            .push(((p.0 / 2.).round() as usize, p.1 as usize / 4, text));
    }
}
