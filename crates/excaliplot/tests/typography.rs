use serde_json::Value;

#[test]
fn units_in_chart_labels_and_notes_keep_native_text_and_explicit_errors() {
    use excaliplot::{Error, LineChart};
    let points = [(0., 20.), (1., 22.)];
    let mut scene = LineChart::new(&points, 0.0..1.0, 0.0..30.0)
        .labels("Temperature ±2°C", "Time (s)", "Temperature (°C)")
        .render()
        .unwrap();
    scene
        .add_note("20 ± 2 °C\n ° ± ", (24., 424.), 20.)
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    for text in ["Temperature ±2°C", "Temperature (°C)", "20 ± 2 °C\n ° ± "] {
        let e = elements.iter().find(|e| e["text"] == text).unwrap();
        assert_eq!(e["originalText"], text);
        assert_eq!(e["fontFamily"], 5);
    }
    let note = elements.last().unwrap();
    assert!((note["width"].as_f64().unwrap() - 97.09991455078125).abs() < 0.01);
    assert_eq!(note["height"], 50.);
    let before = scene.to_bytes().unwrap();
    for ch in ['µ', 'μ', 'Ω', 'à', '\u{301}'] {
        let text = format!("20°C ±2 {ch}");
        assert!(
            matches!(scene.add_note(&text, (0., 0.), 20.), Err(Error::UnsupportedGlyph(c)) if c == ch)
        );
        assert_eq!(scene.to_bytes().unwrap(), before);
        assert!(matches!(LineChart::new(&points, 0.0..1.0, 0.0..30.0)
            .labels(&text, "Time", "Value").render(), Err(Error::UnsupportedGlyph(c)) if c == ch));
    }
}

#[test]
fn units_example_uses_chart_units_and_a_note_and_protects_manual_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("units.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--example", "units", "--"])
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
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    for text in [
        "Temperature ±2°C",
        "Temperature (°C)",
        "Method: synthetic samples\nUncertainty: ±2°C; interval: 1 s.",
    ] {
        let label = elements.iter().find(|e| e["text"] == text).unwrap();
        assert_eq!(label["fontFamily"], 5);
        assert_eq!(label["originalText"], text);
    }
    std::fs::write(&path, "manual edits").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
}
