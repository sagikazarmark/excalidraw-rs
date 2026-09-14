use excaliplot::LineChart;
use serde_json::Value;

#[test]
fn caller_data_uses_numeric_spacing_and_axes_cross_beyond_the_corner() {
    let points = [
        (1.0, 2.0),
        (2.0, 5.0),
        (4.0, 3.0),
        (6.0, 8.0),
        (7.0, 6.0),
        (9.0, 9.0),
    ];
    let scene = LineChart::new(&points, 0.0..10.0, 0.0..10.0)
        .labels("Six-point line", "Time (s)", "Value")
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    let paths: Vec<Vec<(f64, f64)>> = elements
        .iter()
        .filter_map(|e| {
            e["points"].as_array().map(|points| {
                points
                    .iter()
                    .map(|p| {
                        (
                            e["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                            e["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
                        )
                    })
                    .collect()
            })
        })
        .collect();
    assert!(
        paths.contains(&vec![(102.0, 311.0), (615.0, 311.0)]),
        "X axis must be one full-length line: {paths:?}"
    );
    assert!(
        paths.contains(&vec![(112.0, 64.0), (112.0, 321.0)]),
        "Y axis must be one full-length line"
    );
    // Only the two continuous axes may touch the origin neighborhood. No
    // separate extension fragments or perpendicular zero-tick marks.
    let near_origin: Vec<_> = paths
        .iter()
        .filter(|p| {
            p.iter()
                .any(|&(x, y)| (102.0..=122.0).contains(&x) && (301.0..=321.0).contains(&y))
        })
        .collect();
    assert_eq!(
        near_origin.len(),
        2,
        "extra origin segments: {near_origin:?}"
    );
    assert!(paths.contains(&vec![(212.0, 311.0), (212.0, 316.0)]));
    assert!(paths.contains(&vec![(107.0, 262.0), (112.0, 262.0)]));
    assert!(paths.contains(&vec![
        (162.0, 262.0),
        (212.0, 188.0),
        (313.0, 237.0),
        (413.0, 114.0),
        (464.0, 163.0),
        (564.0, 89.0)
    ]));
    assert_eq!(scene.diagnostics().elements["line"], 13);
}

#[test]
fn signed_ranges_keep_interior_zero_ticks_and_extend_the_actual_plot_corner() {
    let scene = LineChart::new(&[(-4.0, -4.0), (4.0, 4.0)], -4.0..4.0, -4.0..4.0)
        .size((800, 500))
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let paths: Vec<Vec<(f64, f64)>> = doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| {
            e["points"].as_array().map(|points| {
                points
                    .iter()
                    .map(|p| {
                        (
                            e["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                            e["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
                        )
                    })
                    .collect()
            })
        })
        .collect();
    assert!(paths.contains(&vec![(102.0, 411.0), (775.0, 411.0)]));
    assert!(paths.contains(&vec![(112.0, 64.0), (112.0, 421.0)]));
    // Zero is inside these ranges; it still deserves a tick on each axis.
    assert!(paths.contains(&vec![(443.0, 411.0), (443.0, 416.0)]));
    assert!(paths.contains(&vec![(107.0, 238.0), (112.0, 238.0)]));
    assert!(!paths.contains(&vec![(112.0, 411.0), (112.0, 416.0)]));
    assert!(!paths.contains(&vec![(107.0, 411.0), (112.0, 411.0)]));
}

#[test]
fn invalid_data_bounds_and_sizes_return_errors_instead_of_partial_charts() {
    for points in [
        vec![],
        vec![(1.0, 2.0)],
        vec![(1.0, 2.0), (1.0, 2.0)],
        vec![(2.0, 2.0), (1.0, 3.0)],
        vec![(1.0, f64::NAN), (2.0, 3.0)],
        vec![(1.0, 2.0), (11.0, 3.0)],
    ] {
        assert!(
            LineChart::new(&points, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err(),
            "{points:?}"
        );
    }
    let points = [(1.0, 2.0), (2.0, 2.0)];
    for bounds in [2.0..2.0, 10.0..0.0, 0.0..f64::INFINITY, -f64::MAX..f64::MAX] {
        assert!(LineChart::new(&points, bounds, 0.0..10.0).render().is_err());
    }
    for size in [(0, 400), (100, 100), (u32::MAX, 400)] {
        assert!(
            LineChart::new(&points, 0.0..10.0, 0.0..10.0)
                .size(size)
                .render()
                .is_err()
        );
    }
    assert!(
        LineChart::new(&points, 0.0..10.0, 0.0..10.0)
            .labels("Missing Ω", "X", "Y")
            .render()
            .is_err()
    );
}

#[test]
fn repeated_points_and_constant_values_are_preserved_with_explicit_nonconstant_bounds() {
    let points = [(1.0, 2.0), (1.0, 2.0), (2.0, 2.0)];
    let scene = LineChart::new(&points, 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let series = doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    assert_eq!(
        series["points"],
        serde_json::json!([[0.0, 0.0], [0.0, 0.0], [50.0, 0.0]])
    );
}

#[test]
fn impractical_numeric_ranges_and_crowded_labels_are_rejected() {
    let points = [(0.0, 0.0), (1.0, 1.0)];
    assert!(
        LineChart::new(&points, 0.0..1e20, 0.0..2.0)
            .render()
            .is_err()
    );
    assert!(
        LineChart::new(&[(0.0, 0.0), (1e-300, 1.0)], 0.0..1e-300, 0.0..2.0)
            .render()
            .is_err()
    );
    assert!(
        LineChart::new(&points, 0.0..2.0, 0.0..2.0)
            .labels(&"W".repeat(100), "X", "Y")
            .render()
            .is_err()
    );
}

#[test]
fn endpoint_labels_must_fit_the_canvas_and_series_must_survive_pixel_mapping() {
    let scene = LineChart::new(&[(0.0, 0.0), (1e6, 1.0)], 0.0..1e6, 0.0..10.0)
        .render()
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let endpoint = doc["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["text"] == "1000000")
        .unwrap();
    assert!(endpoint["x"].as_f64().unwrap() + endpoint["width"].as_f64().unwrap() <= 640.);
    let error = LineChart::new(&[(1.0, 1.0), (1.00001, 1.00001)], 0.0..10.0, 0.0..10.0)
        .render()
        .err()
        .expect("collapsed series should fail");
    assert!(error.to_string().contains("resolution"));
}
