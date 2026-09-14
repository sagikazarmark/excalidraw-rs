use excaliplot::BarChart;
use serde_json::Value;

#[test]
fn bars_validate_categories_bounds_and_visible_nonzero_values() {
    for data in [
        vec![],
        vec![("", 1.0)],
        vec![("Bad", f64::NAN)],
        vec![("High", 6.0)],
        vec![("Too small", 0.00001)],
        vec![("W", 1.0); 100],
    ] {
        assert!(
            BarChart::new(&data, -5.0..5.0).render().is_err(),
            "{data:?}"
        );
    }
    for bounds in [1.0..5.0, -5.0..-1.0, 0.0..0.0] {
        assert!(BarChart::new(&[("A", 2.0)], bounds).render().is_err());
    }
    for value in [-2.0, 0.0, 2.0] {
        assert!(
            BarChart::new(&[("A", value), ("A", value)], -5.0..5.0)
                .render()
                .is_ok()
        );
    }
    assert!(
        BarChart::new(&[("A", 1.0)], 0.0..5.0)
            .size((0, 0))
            .render()
            .is_err()
    );
    assert!(
        BarChart::new(&[("A", 1.0)], 0.0..5.0)
            .labels(&"W".repeat(100), "X", "Y")
            .render()
            .is_err()
    );
}

#[test]
fn signed_bars_are_individual_rectangles_meeting_the_zero_baseline() {
    let scene = BarChart::new(&[("Gain", 4.0), ("Loss", -4.0), ("Zero", 0.0)], -5.0..5.0)
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    let bars: Vec<_> = elements
        .iter()
        .filter(|e| e["backgroundColor"] == "#1971c2")
        .collect();
    assert_eq!(bars.len(), 2, "zero has a category but no fabricated area");
    assert_eq!(bars[0]["y"], 89.0);
    assert_eq!(bars[0]["height"], 99.0);
    assert_eq!(bars[1]["y"], 188.0);
    assert_eq!(bars[1]["height"], 99.0);
    assert!(
        bars.iter()
            .all(|e| e["type"] == "rectangle" && e["groupIds"].as_array().unwrap().len() == 1)
    );
    assert!(elements.iter().any(|e| e["type"] == "line"
        && e["y"] == 188.0
        && e["width"] == 503.0
        && e["height"] == 0.0));
    let labels: Vec<_> = elements.iter().filter_map(|e| e["text"].as_str()).collect();
    for label in ["Gain", "Loss", "Zero"] {
        assert!(labels.contains(&label));
    }
    assert_eq!(scene.diagnostics().elements["rectangle"], 3);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}
