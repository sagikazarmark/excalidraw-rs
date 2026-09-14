use excaliplot::Scene;
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn unequal_lines_are_one_editable_note_at_the_explicit_position() {
    let mut scene = Scene::new();
    scene.add_note("AV\nCafé −2", (-12.5, 47.25), 20.).unwrap();
    let all = elements(&scene);
    assert_eq!(all.len(), 1);
    let note = &all[0];
    assert_eq!(note["type"], "text");
    assert_eq!(note["text"], "AV\nCafé −2");
    assert_eq!(note["originalText"], note["text"]);
    assert_eq!(note["x"], -12.5);
    assert_eq!(note["y"], 47.25);
    // Independent Chromium canvas oracle from the existing typography corpus.
    assert!((note["width"].as_f64().unwrap() - 78.05992126464844).abs() < 0.01);
    assert_eq!(note["height"], 50.);
    assert_eq!(note["fontSize"], 20.);
    assert_eq!(note["fontFamily"], 5);
    assert_eq!(note["lineHeight"], 1.25);
    assert_eq!(note["angle"], 0.);
    assert_eq!(note["textAlign"], "left");
    assert_eq!(note["verticalAlign"], "top");
    assert_eq!(note["autoResize"], true);
    assert!(note["containerId"].is_null());
    assert_eq!(note["boundElements"], serde_json::json!([]));
}

#[test]
fn crlf_and_blank_lines_keep_native_line_boxes_without_changing_text_content() {
    for (input, normalized, height) in [
        ("AV\r\nTo", "AV\nTo", 50.),
        ("\r\nAV\r\n\r\nTo\r\n", "\nAV\n\nTo\n", 125.),
        ("\nAV", "\nAV", 50.),
        ("AV\n", "AV\n", 50.),
        ("\n\n", "\n\n", 75.),
    ] {
        let mut scene = Scene::new();
        scene.add_note(input, (0., 0.), 20.).unwrap();
        let all = elements(&scene);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["text"], normalized);
        assert_eq!(all[0]["originalText"], normalized);
        assert_eq!(all[0]["height"], height);
        assert!(all[0]["width"].as_f64().unwrap() > 0.);
    }
}

#[test]
fn invalid_notes_are_atomic_and_do_not_relax_single_line_chart_labels() {
    use excaliplot::{Error, LineChart};
    let mut scene = Scene::new();
    scene.add_note("Keep\nthis", (0., 0.), 20.).unwrap();
    let before = scene.to_bytes().unwrap();
    assert!(scene.add_note("", (0., 0.), 20.).is_err());
    assert_eq!(scene.to_bytes().unwrap(), before);
    for ch in ['\r', '\t', '\0', '\u{7f}', 'Ω', '\u{301}', '😀'] {
        let text = format!("Valid first line\n{ch}");
        assert!(
            matches!(scene.add_note(&text, (0., 0.), 20.), Err(Error::UnsupportedGlyph(c)) if c == ch)
        );
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    for (position, size, text) in [
        ((f64::NAN, 0.), 20., "AV"),
        ((0., f64::INFINITY), 20., "AV"),
        ((0., 0.), 0., "AV"),
        ((0., 0.), -1., "AV"),
        ((0., 0.), f64::NAN, "AV"),
        ((0., 0.), f64::INFINITY, "AV"),
        ((0., 0.), 1e9, "\n\n\n\n"),   // Total height exceeds u32.
        ((0., 0.), 1e9, "WWWWWWWWWW"), // Advance width exceeds u32.
    ] {
        assert!(scene.add_note(text, position, size).is_err());
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    scene.translate((-f64::MAX, 0.)).unwrap();
    let extreme = scene.to_bytes().unwrap();
    assert!(scene.add_note("AV", (f64::MAX, 0.), 20.).is_err());
    assert_eq!(scene.to_bytes().unwrap(), extreme);
    for operation in ["translate", "place_at"] {
        let result = if operation == "translate" {
            scene.translate((-f64::MAX, 0.))
        } else {
            scene.place_at((f64::MAX, 0.))
        };
        assert!(result.is_err());
        assert_eq!(scene.to_bytes().unwrap(), extreme);
    }
    assert!(
        LineChart::new(&[(0., 1.), (1., 2.)], 0.0..1.0, 0.0..3.0)
            .labels("Two\nlines", "Time", "Value")
            .render()
            .is_err()
    );
}

#[test]
fn complete_note_geometry_survives_groups_padding_frames_and_composition() {
    use excaliplot::BorderStyle;
    let mut report = Scene::with_namespace("same");
    for x in [0., 200.] {
        let mut panel = Scene::with_namespace("same");
        panel.drawing_options().new_group().scope(|| {
            panel.add_note("AV\nTo\n", (-10., 40.), 20.).unwrap();
        });
        let original = elements(&panel).remove(0);
        // Native canvas: To is wider than AV, and the trailing blank is line 3.
        let bounds = panel.bounds().unwrap().unwrap();
        assert_eq!(bounds.x, -10.);
        assert_eq!(bounds.y, 40.);
        assert!((bounds.width - 27.3399658203125).abs() < 0.01);
        assert_eq!(bounds.height, 75.);
        panel
            .add_border(
                12.,
                BorderStyle {
                    width: 4,
                    ..BorderStyle::default()
                },
            )
            .unwrap();
        let bordered = elements(&panel);
        assert_eq!(bordered[1]["x"], -24.);
        assert_eq!(bordered[1]["y"], 26.);
        assert_eq!(bordered[1]["height"], 103.);
        assert_eq!(bordered[0]["groupIds"][0], original["groupIds"][0]);
        assert_eq!(bordered[0]["groupIds"][1], bordered[1]["groupIds"][0]);
        panel.add_frame(8., Some("Method")).unwrap();
        panel.place_at((x, 10.)).unwrap();
        let placed = panel.bounds().unwrap().unwrap();
        assert_eq!((placed.x, placed.y, placed.height), (x, 10., 123.));
        assert!((placed.width - 75.34).abs() < 1e-8);
        report.append(panel).unwrap();
    }
    let all = elements(&report);
    assert_eq!(all.len(), 6);
    for offset in [0, 3] {
        let note = &all[offset];
        assert_eq!(note["text"], "AV\nTo\n");
        assert_eq!(note["y"], 34.);
        assert_eq!(note["frameId"], all[offset + 2]["id"]);
        assert_eq!(note["groupIds"].as_array().unwrap().len(), 2);
    }
    assert_eq!(all[0]["x"], 24.);
    assert_eq!(all[3]["x"], 224.);
    assert_ne!(all[0]["id"], all[3]["id"]);
    assert_ne!(all[0]["groupIds"], all[3]["groupIds"]);
    let library: Value = serde_json::from_slice(&report.to_library_bytes().unwrap()).unwrap();
    assert_eq!(
        library["libraryItems"][0]["elements"],
        serde_json::json!(all)
    );
}

#[test]
fn failed_drawing_scenes_cannot_accept_notes_or_truncate_files() {
    use excaliplot::{ExcalidrawBackend, Overwrite};
    use plotters_backend::{BackendColor, DrawingBackend};
    let mut scene = Scene::new();
    scene.add_note("Keep\nthis", (0., 0.), 20.).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.excalidraw");
    std::fs::write(&path, "manual edits").unwrap();
    assert!(scene.write(&path, Overwrite::Refuse).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
    assert!(
        ExcalidrawBackend::new(&mut scene, (100, 100))
            .unwrap()
            .draw_pixel(
                (0, 0),
                BackendColor {
                    rgb: (0, 0, 0),
                    alpha: 1.
                }
            )
            .is_err()
    );
    assert!(scene.add_note("Another\nnote", (0., 0.), 20.).is_err());
    assert!(scene.write(&path, Overwrite::Allow).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
}

#[test]
fn method_note_demo_is_composed_and_protects_manual_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("method.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--example", "method_note", "--"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let all = document["elements"].as_array().unwrap();
    let notes: Vec<_> = all
        .iter()
        .filter(|e| e["text"].as_str().is_some_and(|s| s.contains('\n')))
        .collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(
        notes[0]["text"],
        "Method: synthetic samples\nOne-minute windows; no smoothing."
    );
    assert_eq!(notes[0]["height"], 50.);
    assert_eq!(all.last().unwrap()["type"], "frame");
    assert_eq!(notes[0]["frameId"], all.last().unwrap()["id"]);
    assert!(notes[0]["y"].as_f64().unwrap() >= 340.);
    std::fs::write(&path, "manually edited note").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"manually edited note");
}
