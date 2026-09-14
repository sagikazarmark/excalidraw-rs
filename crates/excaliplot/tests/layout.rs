use excaliplot::{
    AreaChart, BarChart, LegendPosition, LineChart, NamedBarSeries, NamedSeries, ScatterChart,
    Scene, TickFormat,
};
use serde_json::Value;

fn elements(scene: Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn density_and_percent_units_are_explicit_and_rounding_cannot_hide_distinct_ticks() {
    let items = elements(
        LineChart::new(&[(0., 0.), (1., 100.)], 0.0..1.0, 0.0..100.0)
            .tick_density(3, 3)
            .x_tick_format(TickFormat::FractionPercent { places: 0 })
            .y_tick_format(TickFormat::Percent { places: 0 })
            .render()
            .unwrap(),
    );
    let labels: Vec<_> = items.iter().filter_map(|e| e["text"].as_str()).collect();
    for expected in ["0%", "50%", "100%"] {
        assert_eq!(
            labels.iter().filter(|&&s| s == expected).count(),
            2,
            "{labels:?}"
        );
    }
    assert_eq!(labels.len(), 9); // two axes and three descriptions
    let error = LineChart::new(&[(0., 0.), (1., 1.)], 0.0..1.0, 0.0..1.0)
        .x_tick_format(TickFormat::Decimal { places: 0 })
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("distinct"), "{error}");
    let labels = elements(
        LineChart::new(&[(0., 0.), (1., 1.)], 0.0..1.0, 0.0..1.0)
            .x_tick_format(TickFormat::Decimal { places: 1 })
            .render()
            .unwrap(),
    );
    assert!(labels.iter().any(|e| e["text"] == "0.2"));
    for count in [0, 1, 21, usize::MAX] {
        assert!(
            LineChart::new(&[(0., 0.), (1., 1.)], 0.0..1.0, 0.0..1.0)
                .tick_density(count, 6)
                .render()
                .is_err()
        );
    }
    assert!(
        LineChart::new(&[(0., 0.), (1., 1.)], 0.0..1.0, 0.0..1.0)
            .y_tick_format(TickFormat::Decimal { places: 10 })
            .render()
            .is_err()
    );
}

#[test]
fn compact_scientific_ticks_keep_explicit_numeric_geometry() {
    let items = elements(
        LineChart::new(&[(0., 0.), (1e9, 1e9)], 0.0..1e9, 0.0..1e9)
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .y_tick_format(TickFormat::Scientific { places: 1 })
            .render()
            .unwrap(),
    );
    let labels: Vec<_> = items.iter().filter_map(|e| e["text"].as_str()).collect();
    assert!(labels.contains(&"1.0e9"), "{labels:?}");
    assert!(labels.contains(&"2.0e8"));
    assert!(!labels.contains(&"1000000000"));
    let line = items
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    assert_eq!(line["points"].as_array().unwrap().len(), 2);
}

#[test]
fn measured_margins_fit_extreme_labels_and_reject_real_collisions() {
    for (points, range, format) in [
        ([(0., 0.), (1e9, 1e9)], 0.0..1e9, TickFormat::Auto),
        (
            [(0., 0.), (1e-6, 1e-6)],
            0.0..1e-6,
            TickFormat::Scientific { places: 1 },
        ),
    ] {
        let items = elements(
            LineChart::new(&points, range.clone(), range)
                .x_tick_format(format)
                .y_tick_format(format)
                .size((1000, 400))
                .render()
                .unwrap(),
        );
        for e in items
            .iter()
            .filter(|e| e["type"] == "text" && e["angle"] == 0)
        {
            assert!(e["x"].as_f64().unwrap() >= 0.);
            assert!(e["x"].as_f64().unwrap() + e["width"].as_f64().unwrap() <= 1000.);
        }
        let ticks: Vec<_> = items
            .iter()
            .filter(|e| {
                e["type"] == "text" && e["fontSize"] == 16 && e["y"].as_f64().unwrap() > 311.
            })
            .collect();
        for pair in ticks.windows(2) {
            assert!(
                pair[0]["x"].as_f64().unwrap() + pair[0]["width"].as_f64().unwrap() + 8.
                    <= pair[1]["x"].as_f64().unwrap()
            );
        }
    }
    let error = LineChart::new(&[(0., 0.), (10., 10.)], 0.0..10.0, 0.0..10.0)
        .tick_density(20, 20)
        .size((400, 300))
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("crowded"), "{error}");
}

#[test]
fn right_legends_are_measured_and_hidden_legends_keep_named_series_identity() {
    let a = [(0., 1.), (1., 2.)];
    let b = [(0., 2.), (1., 3.)];
    let names = [
        NamedSeries::new("A long descriptive series", &a, (25, 113, 194)),
        NamedSeries::new("A long descriptive series", &b, (25, 113, 194)),
    ];
    let right = elements(
        LineChart::from_series(&names, 0.0..1.0, 0.0..4.0)
            .legend(LegendPosition::Right)
            .size((1000, 400))
            .render()
            .unwrap(),
    );
    assert_eq!(
        right
            .iter()
            .filter(|e| e["text"] == "A long descriptive series")
            .count(),
        2
    );
    for scene in [
        LineChart::from_series(&names, 0.0..1.0, 0.0..4.0)
            .legend(LegendPosition::Off)
            .render()
            .unwrap(),
        AreaChart::from_series(&names, 0.0..1.0, 0.0..4.0)
            .legend(LegendPosition::Off)
            .render()
            .unwrap(),
        ScatterChart::from_series(&names, 0.0..1.0, 0.0..4.0)
            .legend(LegendPosition::Off)
            .render()
            .unwrap(),
        BarChart::from_series(
            &["A", "B"],
            &[
                NamedBarSeries::new("Same", &[1., 2.], (25, 113, 194)),
                NamedBarSeries::new("Same", &[2., 3.], (25, 113, 194)),
            ],
            0.0..4.0,
        )
        .legend(LegendPosition::Off)
        .render()
        .unwrap(),
    ] {
        let items = elements(scene);
        assert!(
            !items
                .iter()
                .any(|e| e["text"] == "A long descriptive series" || e["text"] == "Same")
        );
        let marks: Vec<_> = items
            .iter()
            .filter(|e| e["strokeColor"] == "#1971c2" || e["backgroundColor"] == "#1971c2")
            .collect();
        assert!(
            marks
                .iter()
                .all(|e| e["groupIds"].as_array().unwrap().len() == 2)
        );
        let groups: std::collections::HashSet<_> = marks
            .iter()
            .map(|e| e["groupIds"][0].as_str().unwrap())
            .collect();
        assert_eq!(groups.len(), 2);
    }
    let long_name = "W".repeat(40);
    let names = [NamedSeries::new(&long_name, &a, (25, 113, 194))];
    let error = LineChart::from_series(&names, 0.0..1.0, 0.0..4.0)
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("plot width"), "{error}");
    assert!(
        LineChart::from_series(&names, 0.0..1.0, 0.0..4.0)
            .legend(LegendPosition::Off)
            .render()
            .is_ok()
    );
}

#[test]
fn formatting_handles_signs_small_values_and_explicit_precision_limits() {
    let items = elements(
        LineChart::new(&[(-1e-6, -1e-6), (1e-6, 1e-6)], -1e-6..1e-6, -1e-6..1e-6)
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .y_tick_format(TickFormat::Decimal { places: 7 })
            .render()
            .unwrap(),
    );
    assert!(items.iter().any(|e| e["text"] == "-1.0e-6"));
    assert!(items.iter().any(|e| e["text"] == "-0.0000005"));
    assert!(!items.iter().any(|e| e["text"] == "-0.0000000"));
    let error = LineChart::new(&[(1e8, 0.), (1e8 + 10., 1.)], 1e8..1e8 + 10., 0.0..1.0)
        .x_tick_format(TickFormat::Scientific { places: 1 })
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("distinct"));
    let bars = elements(
        BarChart::new(&[("Every", 25.), ("category", 50.)], 0.0..100.0)
            .tick_density(2, 3)
            .y_tick_format(TickFormat::Percent { places: 0 })
            .render()
            .unwrap(),
    );
    for text in ["Every", "category", "0%", "50%", "100%"] {
        assert!(bars.iter().any(|e| e["text"] == text));
    }
    assert!(
        BarChart::new(
            &[("A category too long for a slot", 25.), ("B", 50.)],
            0.0..100.0
        )
        .tick_density(2, 3)
        .size((400, 300))
        .render()
        .is_err()
    );
}

#[test]
fn comparison_example_protects_editor_work_and_reports_impossible_fit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layout.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--quiet", "--example", "layout", "--"])
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
    assert!(String::from_utf8_lossy(&first.stderr).contains("Expected impossible fit:"));
    let bytes = std::fs::read(&path).unwrap();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();
    for title in [
        "Decimal comparison",
        "Compact right legend",
        "Compact legend off",
    ] {
        assert!(
            doc["elements"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["text"] == title)
        );
    }
    std::fs::write(&path, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"editor work");
}

#[test]
fn title_and_top_tick_need_clearance_and_categories_keep_typed_glyph_errors() {
    let title = "W".repeat(25);
    let items = elements(
        LineChart::new(&[(0., 0.), (1e9, 1e9)], 0.0..1e9, 0.0..1e9)
            .labels(&title, "X", "Y")
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .render()
            .unwrap(),
    );
    let caption = items.iter().find(|e| e["text"] == title).unwrap();
    let tick = items.iter().find(|e| e["text"] == "1000000000").unwrap();
    assert_eq!(items.iter().filter(|e| e["text"] == "X").count(), 1);
    assert_eq!(items.iter().filter(|e| e["text"] == "1.0e9").count(), 1);
    assert!(
        caption["y"].as_f64().unwrap() + caption["height"].as_f64().unwrap() + 8.
            <= tick["y"].as_f64().unwrap()
    );
    assert!(matches!(
        BarChart::new(&[("µ", 1.)], 0.0..2.0).render(),
        Err(excaliplot::Error::UnsupportedGlyph('µ'))
    ));
}
