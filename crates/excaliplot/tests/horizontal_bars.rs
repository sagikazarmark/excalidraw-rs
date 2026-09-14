use excaliplot::{BarChart, LegendPosition, NamedBarSeries, Scene};
use serde_json::Value;

fn scene_elements(scene: Scene) -> Vec<Value> {
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    doc["elements"].as_array().unwrap().clone()
}

#[test]
fn grouped_long_duplicate_categories_keep_slots_colors_and_series_identity() {
    let categories = [
        "International customer support",
        "Domestic customer support",
        "International customer support",
    ];
    let data = [
        NamedBarSeries::new("Before", &[4., -2., 0.], (25, 113, 194)),
        NamedBarSeries::new("After", &[2., -1., 3.], (224, 49, 49)),
    ];
    let elements = scene_elements(
        BarChart::from_series(&categories, &data, -5.0..5.0)
            .horizontal()
            .size((1000, 400))
            .render()
            .unwrap(),
    );
    let before = elements.iter().find(|e| e["text"] == "Before").unwrap();
    let after = elements.iter().find(|e| e["text"] == "After").unwrap();
    assert_eq!(before["groupIds"].as_array().unwrap().len(), 2);
    assert_ne!(before["groupIds"][0], after["groupIds"][0]);
    assert_eq!(before["groupIds"][1], after["groupIds"][1]);
    let marks = |label: &Value| {
        elements
            .iter()
            .filter(|e| e["type"] == "rectangle" && e["groupIds"] == label["groupIds"])
            .collect::<Vec<_>>()
    };
    let blue = marks(before);
    let red = marks(after);
    assert_eq!(blue.len(), 3, "two bars plus legend swatch");
    assert_eq!(red.len(), 4, "three bars plus legend swatch");
    assert_eq!(blue[2]["height"], 14.0);
    assert_eq!(red[3]["height"], 14.0);
    assert!(blue.iter().all(|e| e["backgroundColor"] == "#1971c2"));
    assert!(red.iter().all(|e| e["backgroundColor"] == "#e03131"));
    let ordered = [blue[0], red[0], blue[1], red[1], red[2]];
    assert!(
        ordered
            .windows(2)
            .all(|p| number(p[0], "y") + number(p[0], "height") < number(p[1], "y"))
    );
    let zero = number(blue[0], "x");
    assert_eq!(number(red[0], "x"), zero);
    assert_eq!(number(blue[1], "x") + number(blue[1], "width"), zero);
    assert_eq!(number(red[1], "x") + number(red[1], "width"), zero);
    let labels: Vec<_> = elements
        .iter()
        .filter(|e| e["text"].as_str().is_some_and(|s| categories.contains(&s)))
        .collect();
    assert_eq!(labels.len(), 3);
    assert_eq!(
        labels
            .iter()
            .map(|e| e["text"].as_str().unwrap())
            .collect::<Vec<_>>(),
        categories
    );
    assert!(
        labels
            .windows(2)
            .all(|p| number(p[0], "y") + number(p[0], "height") < number(p[1], "y"))
    );
    let axis_left = elements
        .iter()
        .filter(|e| e["type"] == "line" && e["width"] == 0.0 && number(e, "height") > 200.)
        .map(|e| number(e, "x"))
        .fold(f64::INFINITY, f64::min);
    assert!(labels.iter().all(|e| e["angle"] == 0.0
        && number(e, "x") >= 24.
        && number(e, "x") + number(e, "width") <= axis_left - 10.));
    // Hiding the legend preserves the independent series groups.
    let hidden = scene_elements(
        BarChart::from_series(&categories, &data, -5.0..5.0)
            .horizontal()
            .size((1000, 400))
            .legend(LegendPosition::Off)
            .render()
            .unwrap(),
    );
    assert!(!hidden.iter().any(|e| e["text"] == "Before"));
    assert_eq!(
        hidden
            .iter()
            .filter(|e| e["type"] == "rectangle" && e["groupIds"].as_array().unwrap().len() == 2)
            .count(),
        5
    );
}

#[test]
fn grouped_slots_must_retain_visible_separation_at_final_resolution() {
    let series = vec![NamedBarSeries::new("S", &[1.], (25, 113, 194)); 80];
    let result = BarChart::from_series(&["A"], &series, 0.0..2.0)
        .horizontal()
        .legend(LegendPosition::Off)
        .render();
    assert!(
        result.is_err(),
        "crowded grouped slots cannot merge into a solid band"
    );
}

#[test]
fn horizontal_layouts_reject_invalid_data_labels_bounds_and_unsupported_stacks() {
    for data in [
        vec![],
        vec![("", 1.)],
        vec![("  ", 1.)],
        vec![("Bad", f64::NAN)],
        vec![("Bad", f64::INFINITY)],
        vec![("Outside", 6.)],
        vec![("Tiny", 1e-10)],
        vec![("Crowded", 1.); 20],
    ] {
        assert!(
            BarChart::new(&data, -5.0..5.0)
                .horizontal()
                .render()
                .is_err(),
            "{data:?}"
        );
    }
    for bounds in [1.0..5.0, -5.0..-1.0, 0.0..0.0, 5.0..-5.0, f64::NAN..5.0] {
        assert!(
            BarChart::new(&[("A", 2.)], bounds)
                .horizontal()
                .render()
                .is_err()
        );
    }
    assert!(
        BarChart::new(&[(&"W".repeat(100), 1.)], 0.0..5.0)
            .horizontal()
            .render()
            .is_err()
    );
    assert!(
        BarChart::new(&[("A", 1.)], 0.0..5.0)
            .horizontal()
            .size((400, 300))
            .labels("Title", "Category", &"W".repeat(100))
            .render()
            .is_err()
    );
    let unsupported = BarChart::new(&[("A", 1.)], 0.0..5.0)
        .horizontal()
        .stacked()
        .render()
        .err()
        .unwrap();
    assert!(unsupported.to_string().contains("horizontal stacked"));
    assert!(
        BarChart::new(&[("A", 1.)], 0.0..100.0)
            .percent_stacked()
            .horizontal()
            .render()
            .err()
            .unwrap()
            .to_string()
            .contains("horizontal stacked")
    );
    assert!(
        BarChart::auto(&[("A", 1.)])
            .stacked()
            .horizontal()
            .render()
            .err()
            .unwrap()
            .to_string()
            .contains("horizontal stacked")
    );
    assert!(
        BarChart::from_series(
            &["A", "B"],
            &[NamedBarSeries::new("S", &[1.], (0, 0, 0))],
            0.0..5.0
        )
        .horizontal()
        .render()
        .is_err()
    );
    assert!(
        BarChart::from_series(&["A"], &[], 0.0..5.0)
            .horizontal()
            .render()
            .is_err()
    );
    assert!(matches!(
        BarChart::new(&[("\u{03bc}", 1.)], 0.0..5.0)
            .horizontal()
            .render(),
        Err(excaliplot::Error::UnsupportedGlyph('\u{03bc}'))
    ));
}

fn number(element: &Value, key: &str) -> f64 {
    element[key].as_f64().unwrap()
}

#[test]
fn horizontal_signed_bars_have_known_widths_and_a_vertical_zero_baseline() {
    let elements = scene_elements(
        BarChart::new(&[("Gain", 4.), ("Loss", -4.), ("Zero", 0.)], -5.0..5.0)
            .horizontal()
            .render()
            .unwrap(),
    );
    let bars: Vec<_> = elements
        .iter()
        .filter(|e| e["backgroundColor"] == "#1971c2")
        .collect();
    assert_eq!(bars.len(), 2, "zero retains its slot but has no rectangle");
    assert_eq!(bars[0]["x"], 363.0);
    assert_eq!(bars[0]["width"], 201.0);
    assert_eq!(bars[1]["x"], 162.0);
    assert_eq!(bars[1]["width"], 201.0);
    assert!(number(bars[0], "y") + number(bars[0], "height") < number(bars[1], "y"));
    assert!(
        bars.iter()
            .all(|e| e["type"] == "rectangle" && e["groupIds"].as_array().unwrap().len() == 1)
    );
    assert!(elements.iter().any(|e| e["type"] == "line"
        && e["x"] == 363.0
        && e["width"] == 0.0
        && number(e, "height") > 200.));
    for name in ["Gain", "Loss", "Zero"] {
        let label = elements.iter().find(|e| e["text"] == name).unwrap();
        assert_eq!(label["angle"], 0.0);
        assert!(number(label, "x") + number(label, "width") < 112.);
    }
}

#[test]
fn horizontal_demo_exports_three_categories_and_protects_editor_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("horizontal_bars.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args([
                "run",
                "--locked",
                "--quiet",
                "--example",
                "horizontal_bars",
                "--",
            ])
            .arg(&path)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    assert_eq!(
        elements.iter().filter(|e| e["type"] == "rectangle").count(),
        8
    );
    for text in [
        "International customer support",
        "Domestic customer support",
        "Enterprise account management",
        "Before",
        "After",
    ] {
        assert_eq!(elements.iter().filter(|e| e["text"] == text).count(), 1);
    }
    std::fs::write(&path, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"editor work");
}

#[test]
fn horizontal_options_keep_logical_axis_roles_and_auto_ranges_include_zero() {
    use excaliplot::TickFormat;
    let data = [("A", 2.), ("A", 6.)];
    let explicit = scene_elements(
        BarChart::new(&data, -2.0..8.0)
            .labels("Comparison", "Category", "Hours")
            .y_tick_format(TickFormat::Decimal { places: 1 })
            .tick_density(2, 3)
            .horizontal()
            .render()
            .unwrap(),
    );
    let bars: Vec<_> = explicit
        .iter()
        .filter(|e| e["backgroundColor"] == "#1971c2")
        .collect();
    assert_eq!(bars[0]["x"], 212.);
    assert_eq!(bars[0]["width"], 101.);
    assert_eq!(bars[1]["x"], 212.);
    assert_eq!(bars[1]["width"], 302.);
    assert_eq!(
        explicit.iter().find(|e| e["text"] == "Hours").unwrap()["angle"],
        0.
    );
    assert_ne!(
        explicit.iter().find(|e| e["text"] == "Category").unwrap()["angle"],
        0.
    );
    assert!(explicit.iter().any(|e| e["text"] == "5.0"));
    for data in [
        vec![("A", 2.), ("A", 6.)],
        vec![("A", -2.), ("A", -6.)],
        vec![("A", 0.), ("A", 0.)],
    ] {
        let automatic = scene_elements(BarChart::auto(&data).horizontal().render().unwrap());
        let marks: Vec<_> = automatic
            .iter()
            .filter(|e| e["backgroundColor"] == "#1971c2")
            .collect();
        assert_eq!(marks.len(), if data[0].1 == 0. { 0 } else { 2 });
        assert_eq!(automatic.iter().filter(|e| e["text"] == "A").count(), 2);
        if !marks.is_empty() {
            assert!(number(marks[1], "width") > number(marks[0], "width") * 2.9);
        }
    }
}
