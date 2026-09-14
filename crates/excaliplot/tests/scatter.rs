use excaliplot::ScatterChart;
use serde_json::Value;

#[test]
fn large_boundary_markers_cannot_overpaint_tick_labels_or_the_title() {
    for point in [(0.0, 0.0), (10.0, 10.0), (5.0, 0.0), (0.0, 5.0)] {
        assert!(
            ScatterChart::new(&[point], 0.0..10.0, 0.0..10.0)
                .marker(20, true)
                .render()
                .is_err()
        );
        assert!(
            ScatterChart::new(&[point], 0.0..10.0, 0.0..10.0)
                .marker(5, true)
                .render()
                .is_ok()
        );
    }
    assert!(
        ScatterChart::new(&[(5.0, 5.0)], 0.0..10.0, 0.0..10.0)
            .marker(20, true)
            .render()
            .is_ok()
    );
}

#[test]
fn named_scatter_uses_native_legend_and_explicit_series_groups_for_hollow_marks() {
    use excaliplot::{FillStyle, NamedSeries, SketchStyle};
    let a = [(1.0, 2.0), (3.0, 4.0)];
    let b = [(8.0, 8.0)];
    let series = [
        NamedSeries::new("A", &a, (25, 113, 194)),
        NamedSeries::new("B", &b, (25, 113, 194)),
    ];
    let scene = ScatterChart::from_series(&series, 0.0..10.0, 0.0..10.0)
        .marker(7, false)
        .opacity(0.5)
        .sketch(SketchStyle::new(1, FillStyle::Solid).unwrap())
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    let a = elements.iter().find(|e| e["text"] == "A").unwrap();
    let b = elements.iter().find(|e| e["text"] == "B").unwrap();
    assert_ne!(a["groupIds"][0], b["groupIds"][0]);
    assert_eq!(a["groupIds"][1], b["groupIds"][1]);
    assert_eq!(
        elements
            .iter()
            .filter(|e| e["groupIds"] == a["groupIds"])
            .count(),
        4
    );
    assert_eq!(
        elements
            .iter()
            .filter(|e| e["groupIds"] == b["groupIds"])
            .count(),
        3
    );
    let marks: Vec<_> = elements.iter().filter(|e| e["type"] == "ellipse").collect();
    assert_eq!(marks.len(), 5, "three marks and two legend swatches");
    for e in marks {
        assert_eq!(e["backgroundColor"], "transparent");
        assert_eq!(e["strokeColor"], "#1971c2");
        assert_eq!(e["strokeWidth"], 2);
        assert_eq!(e["opacity"], 50);
        assert_eq!(e["roughness"], 1);
    }
    assert!(
        ScatterChart::from_series(&[], 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
}

#[test]
fn scatter_rejects_empty_invalid_or_invisible_data_and_crowded_labels() {
    for points in [vec![], vec![(f64::NAN, 1.0)], vec![(11.0, 1.0)]] {
        assert!(
            ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
    let points = [(0.0, 0.0), (10.0, 10.0)];
    for radius in [0, 21, u32::MAX] {
        assert!(
            ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
                .marker(radius, true)
                .render()
                .is_err()
        );
    }
    for alpha in [0.0, 0.001, -1.0, 1.1, f64::NAN] {
        assert!(
            ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
                .opacity(alpha)
                .render()
                .is_err()
        );
    }
    assert!(
        ScatterChart::new(&points, 0.0..0.0, 0.0..10.0)
            .render()
            .is_err()
    );
    assert!(
        ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
            .size((0, 0))
            .render()
            .is_err()
    );
    assert!(
        ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
            .labels(&"W".repeat(100), "X", "Y")
            .render()
            .is_err()
    );
    assert!(
        ScatterChart::new(&points, 0.0..1e6, 0.0..10.0)
            .tick_density(20, 6)
            .size((400, 300))
            .render()
            .is_err()
    );
    assert!(
        ScatterChart::new(&[(2.0, 2.0); 3], 0.0..10.0, 0.0..10.0)
            .render()
            .is_ok()
    );
}

#[test]
fn unordered_repeated_and_singleton_scatter_points_remain_individual_ellipses() {
    let scene = ScatterChart::new(&[(9.0, 9.0), (1.0, 2.0), (1.0, 2.0)], 0.0..10.0, 0.0..10.0)
        .marker(7, true)
        .opacity(0.5)
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let marks: Vec<_> = doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "ellipse")
        .collect();
    assert_eq!(marks.len(), 3);
    assert_eq!(
        (marks[0]["x"].as_f64(), marks[0]["y"].as_f64()),
        (Some(557.0), Some(82.0))
    );
    assert_eq!(
        (marks[1]["x"].as_f64(), marks[1]["y"].as_f64()),
        (Some(155.0), Some(255.0))
    );
    assert_eq!(marks[1]["x"], marks[2]["x"]);
    assert_ne!(marks[1]["id"], marks[2]["id"]);
    for e in marks {
        assert_eq!(e["width"], 14.0);
        assert_eq!(e["height"], 14.0);
        assert_eq!(e["opacity"], 50);
        assert_eq!(e["backgroundColor"], "#1971c2");
        assert_eq!(e["groupIds"].as_array().unwrap().len(), 1);
    }
    assert_eq!(scene.diagnostics().calls["draw_circle"], 3);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
    assert!(
        ScatterChart::new(&[(2.0, 2.0)], 0.0..10.0, 0.0..10.0)
            .render()
            .is_ok()
    );
}
