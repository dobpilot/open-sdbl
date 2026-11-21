use super::*;

#[test]
fn keeps_counter_columns_stable_and_shortens_the_bar_first() {
    let phase = Frame {
        phase: "Config",
        totals: Some((100, 100 * 1024)),
        resources: 9,
        bytes: 9 * 1024,
        started: Instant::now(),
        glint: false,
    };
    let first = phase.render(100, Palette::Plain).unwrap();
    let tenth = Frame {
        resources: 10,
        bytes: 10 * 1024,
        ..phase
    }
    .render(100, Palette::Plain)
    .unwrap();
    let complete = Frame {
        resources: 100,
        bytes: 100 * 1024,
        ..phase
    }
    .render(100, Palette::Plain)
    .unwrap();
    assert_eq!(first.find(']'), tenth.find(']'));
    assert_eq!(tenth.find(']'), complete.find(']'));
    assert_eq!(first.width(), complete.width());
    assert!(first.contains("  9%") && tenth.contains(" 10%") && complete.contains("100%"));
    let narrower = phase.render(first.width() - 2, Palette::Plain).unwrap();
    assert!(narrower.contains("resources") && narrower.find(']') < first.find(']'));
    assert!(phase.render(12, Palette::Plain).is_none());

    let unknown = Frame {
        totals: None,
        ..phase
    };
    assert!(unknown.measurements().0.ends_with("/?"));
    let empty = Frame {
        totals: Some((0, 0)),
        resources: 0,
        bytes: 0,
        ..phase
    };
    assert_eq!(empty.measurements().2, "100%");
}

#[test]
fn permanent_warning_clears_the_current_row() {
    let mut display = Display {
        frame: None,
        line: Some("14.7 MiB/16.0 MiB".into()),
        last_draw: None,
    };
    let mut output = b"metadata  Config  14.7 MiB/16.0 MiB".to_vec();
    display
        .permanent(
            &mut output,
            "WARN  skipped 3 unreadable Config resources\n      A  invalid UTF-8",
        )
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("MiBwarning:"));
    assert!(output.contains(
        "\r\x1b[2KWARN  skipped 3 unreadable Config resources\n      A  invalid UTF-8\n"
    ));
}
