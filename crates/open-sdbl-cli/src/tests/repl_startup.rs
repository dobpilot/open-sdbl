use super::*;
use art::Motif;
const REVIEW_SEED: u32 = 17;

fn startup(motif: Motif) -> Startup {
    let startup = Startup::new();
    {
        let mut display = startup.display.borrow_mut();
        display.scene = Some(Scene {
            motif,
            seed: REVIEW_SEED,
        });
        display.unicode = true;
        display.palette = Palette::Plain;
        display.static_stages = false;
    }
    startup
}

const SCENES: [Motif; 3] = [Motif::Branch, Motif::Sphere, Motif::Mountain];

#[test]
fn one_unbiased_choice_with_one_rejection() {
    for motif in SCENES {
        assert_eq!(
            (0..=255)
                .filter(|&b| Motif::sample(b) == Some(motif))
                .count(),
            85
        );
    }
    assert_eq!(Motif::sample(255), None);
}

#[test]
fn work_events_and_elapsed_time_are_independent() {
    for motif in SCENES {
        let startup = startup(motif);
        let mut p = startup.reporter();
        let frame = |ms, size| {
            startup
                .display
                .borrow()
                .frame(FIRST_PAINT + Duration::from_millis(ms), size)
                .unwrap()
        };
        let size = (24, 80);
        assert!(
            startup
                .display
                .borrow()
                .frame(FIRST_PAINT - Duration::from_millis(1), size)
                .is_none()
        );
        let start = frame(0, size);
        assert!(start.ends_with("open-sdbl  connecting"));
        for phase in ["transaction", "layout", "DBNames (legacy layout)", "Config"] {
            p.phase(phase);
            assert!(frame(500, size).ends_with(phase));
            assert_eq!(startup.display.borrow().work.fraction(), 0.);
        }
        p.config_totals(48, 24 * 1024 * 1024);
        p.advance_config(31, 12 * 1024 * 1024);
        let middle = frame(500, size);
        let (drawing, status) = middle.rsplit_once("\r\n").unwrap();
        assert_eq!(status, "open-sdbl  Config  31/48 res  12.0/24.0 MiB");
        assert_eq!(startup.display.borrow().work.fraction(), 0.5);
        let later = frame(900, size);
        assert_eq!(later.lines().last(), Some(status));
        assert_ne!(drawing, later.rsplit_once("\r\n").unwrap().0);
        assert_eq!(frame(500, (22, 76)).lines().count(), 19);
        assert_eq!(frame(500, (14, 52)).lines().count(), 11);
        for small in [(13, 80), (24, 51)] {
            assert_eq!(frame(500, small), status);
        }
        assert_eq!(frame(500, (6, 14)), "Config 31/48");
        assert_eq!(frame(500, (6, 5)), "Conf");
        assert_eq!(frame(500, size), middle);
        p.advance_config(17, 12 * 1024 * 1024);
        assert_ne!(drawing, frame(500, size).rsplit_once("\r\n").unwrap().0);
        for phase in [
            "extensions",
            "SchemaStorage",
            "catalog",
            "resolve",
            "commit",
            "line editor",
        ] {
            p.phase(phase);
            if phase == "commit" {
                p.finish();
            }
            let final_scene = frame(500, size);
            assert!(final_scene.ends_with(phase));
            assert!(!final_scene.contains("=>"));
            assert_eq!(startup.display.borrow().work.fraction(), 1.);
        }
    }
}

#[test]
fn zero_bytes_falls_back_to_resources_and_empty_config_finishes_on_exit() {
    for (resources, expected) in [(4, 0.5), (0, 0.)] {
        let startup = startup(Motif::Branch);
        let mut p = startup.reporter();
        p.phase("Config");
        p.config_totals(resources, 0);
        if resources > 0 {
            p.advance_config(2, 0);
        }
        assert_eq!(startup.display.borrow().work.fraction(), expected);
        p.phase("extensions");
        assert_eq!(startup.display.borrow().work.fraction(), 1.);
    }
}

#[test]
fn nine_monochrome_poses_follow_the_review_art() {
    let mut panels = Vec::new();
    for motif in SCENES {
        for (index, angle) in [0., 57., 134.].into_iter().enumerate() {
            let scene = Scene {
                motif,
                seed: REVIEW_SEED,
            };
            let frame = scene.draw(
                index as f64 / 2.,
                Duration::from_secs_f64(angle / 90.),
                Size { cols: 72, rows: 18 },
                true,
                Colors::Plain,
            );
            panels.push(frame.replace("\r\n", "\n"));
        }
    }
    assert_eq!(
        panels.join("\n---\n") + "\n",
        include_str!("../../tests/fixtures/terminal-ux/procedural-scenes.txt"),
    );
}

#[test]
fn sphere_label_compresses_then_crosses_the_limb() {
    let frame = |angle: f64| {
        Scene {
            motif: Motif::Sphere,
            seed: REVIEW_SEED,
        }
        .draw(
            0.5,
            Duration::from_secs_f64(angle / 90.),
            Size { cols: 72, rows: 18 },
            true,
            Colors::Plain,
        )
    };
    assert!(frame(0.).contains("S D B L"));
    assert!(frame(57.).contains("SDBL"));
    assert!(!frame(80.).contains("SDBL"));
}

#[test]
fn warning_and_resource_lines_survive_the_next_paint() {
    for motif in SCENES {
        let startup = startup(motif);
        let mut d = startup.display.borrow_mut();
        d.started = Instant::now() - Duration::from_secs(1);
        let frame = d.frame(FIRST_PAINT, (24, 80)).unwrap();
        let mut out = Vec::new();
        d.region.draw(&mut out, &frame, 24).unwrap();
        d.skips.record("bad.7", MetadataErrorKind::Utf8);
        let warning = d.skips.take_warning(Palette::Plain).unwrap();
        d.permanent(&mut out, &warning).unwrap();
        d.region.draw(&mut out, &frame, 24).unwrap();
        d.region.clear(&mut out, 24).unwrap();
        let transcript = String::from_utf8(out).unwrap();
        let (before, after) = transcript
            .split_once("WARN  skipped 1 unreadable Config resources\n      bad.7  invalid UTF-8\n")
            .unwrap();
        assert!(before.contains("connecting"));
        assert!(after.contains("connecting"));
        assert!(transcript.ends_with("\x1b[?25h"));
    }
}

/// The child uses the production signal, termios, renderer, and line editor.
/// Its control pipe belongs to the test driver and never consumes terminal input.
#[cfg(target_os = "linux")]
#[test]
fn startup_boundaries_in_a_pty() {
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;
    use std::process::Command;
    let Ok(control) = std::env::var("SDBL_TEST_STARTUP_PIPE") else {
        let status = Command::new("python3")
            .arg("-B")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/repl_startup_pty.py"
            ))
            .arg(std::env::current_exe().unwrap())
            .status()
            .unwrap();
        assert!(status.success());
        return;
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(async {
        let startup = Startup::new();
        let scene = std::env::var("SDBL_TEST_STARTUP_MOTIF")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        if startup.display.borrow().scene.is_some() {
            startup.display.borrow_mut().scene = Some(Scene {
                motif: SCENES[scene],
                seed: REVIEW_SEED,
            });
        }
        let _utf8 = startup.utf8_guard()?;
        let (tx, mut commands) = tokio::sync::mpsc::unbounded_channel();
        // SAFETY: the parent passes ownership of this dedicated read descriptor
        // to this child through pass_fds, once per subprocess.
        let pipe = unsafe { std::fs::File::from_raw_fd(control.parse().unwrap()) };
        std::thread::spawn(move || {
            for cmd in BufReader::new(pipe).lines().map_while(Result::ok) {
                if tx.send(cmd).is_err() {
                    break;
                }
            }
        });
        let (mut prepared, _input) = startup
            .watch(async {
                let mut reporter = startup.reporter();
                while let Some(cmd) = commands.recv().await {
                    match cmd.as_str() {
                        "Config" => {
                            reporter.phase("Config");
                            reporter.config_totals(48, 24 << 20);
                            reporter.advance_config(31, 12 << 20);
                        }
                        "warn" => {
                            reporter.skipped_config("resource.7", MetadataErrorKind::Utf8);
                            reporter.phase("extensions");
                        }
                        "commit" => reporter.phase("commit"),
                        "editor" => {
                            reporter.finish();
                            reporter.phase("line editor");
                        }
                        "fail" => {
                            return Err(CliError::Terminal(format!(
                                "{} failed: fixture cause",
                                startup.display.borrow().work.phase
                            )));
                        }
                        "ready" => break,
                        _ => panic!("unknown test gate"),
                    }
                }
                super::super::Prepared::new(
                    crate::enumeration_snapshot_support::enumeration_snapshot(),
                    true,
                )
                .await
            })
            .await?;
        drop(startup);
        let text = prepared
            .editor
            .as_mut()
            .unwrap()
            .readline("open-sdbl=> ")
            .unwrap();
        assert_eq!(text, "ВЫБРАТЬ 1;");
        Ok::<_, CliError>(())
    });
    if let Err(err) = result {
        crate::output::write_top_level_error(&mut io::stderr().lock(), &err).unwrap();
        std::process::exit(i32::from(err.exit_code()));
    }
}

#[test]
fn scene_reprojects_at_each_width_with_stable_geometry() {
    for motif in SCENES {
        let scene = Scene {
            motif,
            seed: REVIEW_SEED,
        };
        let render = |scene: Scene, width| {
            scene.draw(
                1.,
                Duration::from_millis(800),
                Size::fit(48, width).unwrap(),
                true,
                Colors::Plain,
            )
        };
        let first = render(scene, 80);
        let mut last_extent = 0;
        for width in [64, 80, 100, 120, 160] {
            let frame = render(scene, width);
            let size = Size::fit(48, width).unwrap();
            assert_eq!(size.cols, usize::from(width - 1));
            assert_eq!(frame.split("\r\n").count(), size.rows);
            let extent = frame.lines().map(UnicodeWidthStr::width).max().unwrap();
            assert!(extent > last_extent && extent <= size.cols);
            last_extent = extent;
        }
        assert_eq!(render(scene, 80), first);
        assert_ne!(render(Scene { seed: 18, ..scene }, 80), first);
        assert_eq!(Size::fit(14, 160).unwrap().rows, 10);
        assert_eq!(Size::fit(48, 51), None);
        assert_eq!(
            Size::fit(u16::MAX, u16::MAX),
            Some(Size {
                cols: 512,
                rows: 96
            })
        );
    }
}

#[test]
fn shading_uses_terminal_capabilities_and_preserves_ascii() {
    for motif in SCENES {
        let scene = Scene {
            motif,
            seed: REVIEW_SEED,
        };
        for (colors, accent) in [
            (Colors::Dark, "38;2;52;213;154"),
            (Colors::Light, "38;2;8;117;78"),
            (Colors::Neutral, "38;2;46;151;110"),
            (Colors::Indexed, "38;5;78"),
            (Colors::Basic, "1;32"),
            (Colors::Plain, ""),
        ] {
            let frame = scene.draw(
                0.75,
                Duration::from_millis(200),
                Size { cols: 88, rows: 22 },
                false,
                colors,
            );
            assert!(frame.is_ascii());
            let mut styles = frame
                .split("\x1b[")
                .skip(1)
                .map(|s| s.split_once('m').unwrap().0)
                .filter(|s| *s != "0")
                .collect::<std::collections::BTreeSet<_>>();
            if colors == Colors::Plain {
                assert!(styles.is_empty());
            } else {
                assert!(styles.len() >= 3, "{motif:?}: {styles:?}");
                assert!(styles.remove(accent), "missing green glint on {motif:?}");
                for style in styles {
                    let neutral = match colors {
                        Colors::Basic => matches!(style, "2" | "90" | "1"),
                        Colors::Indexed => style
                            .strip_prefix("38;5;")
                            .and_then(|entry| entry.parse::<u8>().ok())
                            .is_some_and(|entry| entry >= 232),
                        Colors::Dark | Colors::Light | Colors::Neutral => {
                            let rgb: Vec<_> = style.split(';').collect();
                            matches!(rgb.as_slice(), ["38","2",r,g,b] if r == g && g == b)
                        }
                        Colors::Plain => unreachable!(),
                    };
                    assert!(neutral, "unexpected scene color {style}");
                }
            }
        }
    }
}
