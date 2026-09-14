use excaliplot::{LineChart, NamedSeries, ReferenceRule, Scene, ShadedInterval, StrokeStyle};
use serde_json::Value;

fn elements(scene: Scene) -> Vec<Value> {
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn threshold_demo_exports_native_annotations_and_protects_editor_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("annotations.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args([
                "run",
                "--locked",
                "--quiet",
                "--example",
                "annotations",
                "--",
            ])
            .arg(&path)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let es = doc["elements"].as_array().unwrap();
    assert_eq!(
        es.iter().filter(|e| e["strokeColor"] == "#e03131").count(),
        2
    );
    assert_eq!(
        es.iter()
            .filter(|e| e["backgroundColor"] == "#ffec99")
            .count(),
        1
    );
    assert_eq!(
        es.iter()
            .filter(|e| e["backgroundColor"] == "#d0ebff")
            .count(),
        1
    );
    assert!(es.iter().all(|e| matches!(
        e["type"].as_str(),
        Some("text" | "line" | "rectangle" | "ellipse")
    )));
    std::fs::write(&path, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"editor work");
}

fn vertices(e: &Value) -> Vec<(f64, f64)> {
    e["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                e["x"].as_f64().unwrap() + p[0].as_f64().unwrap(),
                e["y"].as_f64().unwrap() + p[1].as_f64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn windows_use_log_mapping_and_paint_behind_axes_and_data_in_input_order() {
    use excaliplot::AxisScale;
    let es = elements(
        LineChart::new(
            &[(1., 1.), (10., 10.), (100., 100.)],
            1.0..100.0,
            1.0..100.0,
        )
        .x_scale(AxisScale::Log10)
        .y_scale(AxisScale::Log10)
        .reference_rule(ReferenceRule::vertical(10., (224, 49, 49)))
        .shaded_interval(ShadedInterval::x(1.0..10.0, (255, 236, 153)).opacity(0.255))
        .shaded_interval(ShadedInterval::y(10.0..100.0, (255, 236, 153)))
        .render()
        .unwrap(),
    );
    let data = es
        .iter()
        .find(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
        .unwrap();
    let p = vertices(data);
    let windows: Vec<_> = es
        .iter()
        .filter(|e| e["backgroundColor"] == "#ffec99")
        .collect();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["x"], p[0].0);
    assert_eq!(windows[0]["y"], p[2].1);
    assert_eq!(windows[0]["width"], p[1].0 - p[0].0);
    assert_eq!(windows[0]["height"], p[0].1 - p[2].1);
    assert_eq!(windows[1]["x"], p[0].0);
    assert_eq!(windows[1]["y"], p[2].1);
    assert_eq!(windows[1]["width"], p[2].0 - p[0].0);
    assert_eq!(windows[1]["height"], p[1].1 - p[2].1);
    assert_eq!(windows[0]["opacity"], 26);
    assert_eq!(windows[1]["opacity"], 20);
    for window in &windows {
        assert_eq!(window["type"], "rectangle");
        assert_eq!(window["strokeColor"], "transparent");
        assert_eq!(window["groupIds"][1], data["groupIds"][0]);
        let i = es.iter().position(|e| e["id"] == window["id"]).unwrap();
        assert!(es[..i].iter().all(|e| e["type"] != "line"));
    }
    assert_ne!(windows[0]["groupIds"][0], windows[1]["groupIds"][0]);
    let rule = es.last().unwrap();
    assert_eq!(vertices(rule), [(p[1].0, p[0].1), (p[1].0, p[2].1)]);
}

#[test]
fn calendar_rules_and_windows_use_typed_elapsed_coordinates() {
    use chrono::{NaiveDate, TimeZone, Utc};
    use excaliplot::{DateLineChart, DateScatterChart, UtcLineChart, UtcScatterChart};
    let date = |day| NaiveDate::from_ymd_opt(2024, 2, day).unwrap();
    let dates = [(date(1), 0.), (date(2), 5.), (date(5), 10.)];
    let es = elements(
        DateLineChart::new(&dates, date(1)..date(5), 0.0..10.0)
            .reference_rule(ReferenceRule::vertical(date(2), (224, 49, 49)))
            .shaded_interval(ShadedInterval::x(date(2)..date(5), (255, 236, 153)))
            .render()
            .unwrap(),
    );
    let p = vertices(
        es.iter()
            .find(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
            .unwrap(),
    );
    assert!((p[1].0 - p[0].0) * 3. <= p[2].0 - p[1].0 + 3.);
    assert_eq!(
        vertices(es.last().unwrap()),
        [(p[1].0, p[0].1), (p[1].0, p[2].1)]
    );
    let window = es
        .iter()
        .find(|e| e["backgroundColor"] == "#ffec99")
        .unwrap();
    assert_eq!(window["x"], p[1].0);
    assert_eq!(window["width"], p[2].0 - p[1].0);
    DateScatterChart::new(&dates, date(1)..date(5), 0.0..10.0)
        .reference_rule(ReferenceRule::horizontal(5., (224, 49, 49)))
        .shaded_interval(ShadedInterval::y(2.0..8.0, (255, 236, 153)))
        .render()
        .unwrap();
    let time = |second| Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, second).unwrap();
    let times = [(time(0), 0.), (time(10), 5.), (time(40), 10.)];
    let line = elements(
        UtcLineChart::new(&times, time(0)..time(40), 0.0..10.0)
            .size((1600, 400))
            .tick_density(2, 6)
            .reference_rule(ReferenceRule::vertical(time(10), (224, 49, 49)))
            .shaded_interval(ShadedInterval::x(time(10)..time(40), (255, 236, 153)))
            .render()
            .unwrap(),
    );
    let p = vertices(
        line.iter()
            .find(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
            .unwrap(),
    );
    assert_eq!(
        vertices(line.last().unwrap()),
        [(p[1].0, p[0].1), (p[1].0, p[2].1)]
    );
    UtcScatterChart::new(&times, time(0)..time(40), 0.0..10.0)
        .size((1600, 400))
        .tick_density(2, 6)
        .reference_rule(ReferenceRule::horizontal(5., (224, 49, 49)))
        .shaded_interval(ShadedInterval::y(2.0..8.0, (255, 236, 153)))
        .render()
        .unwrap();
}

#[test]
fn invalid_annotation_coordinates_styles_and_collapsed_windows_return_clear_errors() {
    let chart = || LineChart::new(&[(0., 0.), (10., 10.)], 0.0..10.0, 0.0..10.0);
    let error = |result: Result<Scene, excaliplot::Error>, expected: &str| {
        let message = match result {
            Ok(_) => panic!("invalid annotation accepted"),
            Err(e) => e.to_string(),
        };
        assert!(message.contains(expected), "{message}");
    };
    for coordinate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 11.] {
        for rule in [
            ReferenceRule::horizontal(coordinate, (0, 0, 0)),
            ReferenceRule::vertical(coordinate, (0, 0, 0)),
        ] {
            error(
                chart().reference_rule(rule).render(),
                "annotation coordinates",
            );
        }
        for interval in [
            ShadedInterval::x(coordinate..5., (0, 0, 0)),
            ShadedInterval::y(5.0..coordinate, (0, 0, 0)),
        ] {
            error(chart().shaded_interval(interval).render(), "annotation");
        }
    }
    for opacity in [f64::NAN, f64::INFINITY, -0.1, 0., 0.004, 1.001] {
        error(
            chart()
                .reference_rule(ReferenceRule::horizontal(5., (0, 0, 0)).opacity(opacity))
                .render(),
            "annotation opacity",
        );
        error(
            chart()
                .shaded_interval(ShadedInterval::x(2.0..8.0, (0, 0, 0)).opacity(opacity))
                .render(),
            "annotation opacity",
        );
    }
    for width in [0, 21, u32::MAX] {
        error(
            chart()
                .reference_rule(ReferenceRule::vertical(5., (0, 0, 0)).width(width))
                .render(),
            "annotation rule width",
        );
    }
    for (start, end, reason) in [
        (5., 5., "ordered"),
        (8., 2., "ordered"),
        (5., 5.00000001, "collapses"),
    ] {
        for interval in [
            ShadedInterval::x(start..end, (0, 0, 0)),
            ShadedInterval::y(start..end, (0, 0, 0)),
        ] {
            error(chart().shaded_interval(interval).render(), reason);
        }
    }
    chart()
        .reference_rule(
            ReferenceRule::vertical(0., (0, 0, 0))
                .width(20)
                .opacity(0.005),
        )
        .reference_rule(ReferenceRule::horizontal(10., (0, 0, 0)).width(1))
        .shaded_interval(ShadedInterval::x(0.0..10.0, (0, 0, 0)).opacity(1.))
        .render()
        .unwrap();
    error(
        LineChart::new(&[(1., 1.), (100., 100.)], 1.0..100.0, 1.0..100.0)
            .x_scale(excaliplot::AxisScale::Log10)
            .reference_rule(ReferenceRule::vertical(0., (0, 0, 0)))
            .render(),
        "annotation coordinates",
    );
    // Annotation bounds cannot rescue invalid data or silently widen automatic ranges.
    assert!(
        LineChart::new(&[(0., 0.), (11., 10.)], 0.0..10.0, 0.0..10.0)
            .reference_rule(ReferenceRule::horizontal(5., (0, 0, 0)))
            .render()
            .is_err()
    );
    error(
        LineChart::auto(&[(0., 0.), (10., 10.)])
            .reference_rule(ReferenceRule::vertical(11., (0, 0, 0)))
            .render(),
        "annotation coordinates",
    );
    LineChart::auto(&[(0., 0.), (10., 10.)])
        .shaded_interval(ShadedInterval::x(-0.5..10.5, (0, 0, 0)))
        .render()
        .unwrap();
}

#[test]
fn calendar_annotations_reject_out_of_bounds_leap_seconds_and_subpixel_windows() {
    use chrono::{NaiveDate, TimeZone, Timelike, Utc};
    use excaliplot::{DateLineChart, UtcLineChart};
    let date = |day| NaiveDate::from_ymd_opt(2024, 2, day).unwrap();
    assert!(
        DateLineChart::new(
            &[(date(2), 0.), (date(5), 10.)],
            date(2)..date(5),
            0.0..10.0
        )
        .shaded_interval(ShadedInterval::x(date(1)..date(3), (0, 0, 0)))
        .render()
        .is_err()
    );
    let time = |minute, second| Utc.with_ymd_and_hms(2024, 2, 1, 0, minute, second).unwrap();
    let points = [(time(0, 0), 0.), (time(2, 0), 10.)];
    let chart = || {
        UtcLineChart::new(&points, time(0, 0)..time(2, 0), 0.0..10.0)
            .size((1600, 400))
            .tick_density(2, 6)
    };
    let leap = time(0, 59).with_nanosecond(1_500_000_000).unwrap();
    assert!(
        chart()
            .reference_rule(ReferenceRule::vertical(leap, (0, 0, 0)))
            .render()
            .is_err()
    );
    assert!(
        chart()
            .shaded_interval(ShadedInterval::x(
                time(1, 0)..time(1, 0).with_nanosecond(1).unwrap(),
                (0, 0, 0)
            ))
            .render()
            .is_err()
    );
}

#[test]
fn area_scatter_and_composition_preserve_annotation_layers_geometry_and_identity() {
    use excaliplot::{AreaChart, ScatterChart};
    let points = [(1., 2.), (5., 8.), (9., 4.)];
    let rule = ReferenceRule::horizontal(5., (224, 49, 49));
    let window = ShadedInterval::x(2.0..8.0, (255, 236, 153));
    let area = AreaChart::new(&points, 0.0..10.0, 0.0..10.0)
        .reference_rule(rule.clone())
        .shaded_interval(window.clone())
        .render()
        .unwrap();
    let scatter = ScatterChart::new(&points, 0.0..10.0, 0.0..10.0)
        .reference_rule(rule)
        .shaded_interval(window)
        .render()
        .unwrap();
    for es in [elements(area), elements(scatter)] {
        let interval = es
            .iter()
            .position(|e| e["backgroundColor"] == "#ffec99")
            .unwrap();
        let data = es
            .iter()
            .position(|e| e["backgroundColor"] == "#1971c2")
            .unwrap();
        assert!(interval < data && data < es.len() - 1);
        assert_eq!(es.last().unwrap()["strokeColor"], "#e03131");
    }
    let chart = || {
        LineChart::new(&points, 0.0..10.0, 0.0..10.0)
            .reference_rule(ReferenceRule::vertical(5., (224, 49, 49)))
            .shaded_interval(ShadedInterval::y(2.0..8.0, (255, 236, 153)))
            .render()
            .unwrap()
    };
    let original = elements(chart());
    let mut composed = Scene::new();
    for offset in [(0., 0.), (700., 100.)] {
        let mut copy = chart();
        copy.translate(offset).unwrap();
        composed.append(copy).unwrap();
    }
    let es = elements(composed);
    assert_eq!(es.len(), original.len() * 2);
    let mut ids = std::collections::HashSet::new();
    let mut previous_groups = std::collections::HashSet::new();
    for (copy, (dx, dy)) in es.chunks(original.len()).zip([(0., 0.), (700., 100.)]) {
        let mut groups = std::collections::HashMap::new();
        for (a, b) in original.iter().zip(copy) {
            assert!(ids.insert(b["id"].as_str().unwrap()));
            assert!((b["x"].as_f64().unwrap() - a["x"].as_f64().unwrap() - dx).abs() < 1e-9);
            assert!((b["y"].as_f64().unwrap() - a["y"].as_f64().unwrap() - dy).abs() < 1e-9);
            for field in [
                "type",
                "points",
                "opacity",
                "strokeStyle",
                "strokeColor",
                "backgroundColor",
            ] {
                assert_eq!(a[field], b[field]);
            }
            for (ag, bg) in a["groupIds"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["groupIds"].as_array().unwrap())
            {
                let ag = ag.as_str().unwrap();
                let bg = bg.as_str().unwrap();
                assert!(!previous_groups.contains(bg));
                assert_eq!(*groups.entry(ag).or_insert(bg), bg);
            }
        }
        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        previous_groups.extend(groups.into_values());
    }
}

#[test]
fn rules_map_to_data_span_in_foreground_with_independent_groups_and_matching_paint() {
    let series = [NamedSeries::new(
        "Data",
        &[(0., 0.), (5., 5.), (10., 10.)],
        (25, 113, 194),
    )];
    let plain = elements(
        LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .render()
            .unwrap(),
    );
    let annotated = elements(
        LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .reference_rule(
                ReferenceRule::horizontal(5., (224, 49, 49))
                    .width(4)
                    .opacity(0.255)
                    .stroke_style(StrokeStyle::Dashed),
            )
            .reference_rule(
                ReferenceRule::vertical(5., (224, 49, 49)).stroke_style(StrokeStyle::Dotted),
            )
            .render()
            .unwrap(),
    );
    let data_index = annotated
        .iter()
        .position(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
        .unwrap();
    let data = &annotated[data_index];
    let points = vertices(data);
    let rules = &annotated[data_index + 1..data_index + 3];
    assert_eq!(
        vertices(&rules[0]),
        [(points[0].0, points[1].1), (points[2].0, points[1].1)]
    );
    assert_eq!(
        vertices(&rules[1]),
        [(points[1].0, points[0].1), (points[1].0, points[2].1)]
    );
    assert_eq!(rules[0]["strokeWidth"], 4);
    assert_eq!(rules[0]["opacity"], 26);
    assert_eq!(rules[0]["strokeStyle"], "dashed");
    assert_eq!(rules[1]["strokeStyle"], "dotted");
    for rule in rules {
        assert_eq!(rule["type"], "line");
        assert_eq!(rule["strokeColor"], "#e03131");
        assert_eq!(rule["groupIds"].as_array().unwrap().len(), 2);
        assert_eq!(rule["groupIds"][1], data["groupIds"][1]);
        assert_ne!(rule["groupIds"][0], data["groupIds"][0]);
    }
    assert_ne!(rules[0]["groupIds"][0], rules[1]["groupIds"][0]);
    let remaining: Vec<_> = annotated
        .iter()
        .filter(|e| e["strokeColor"] != "#e03131")
        .collect();
    assert_eq!(remaining.len(), plain.len());
    for (a, b) in plain.iter().zip(remaining) {
        for field in [
            "type",
            "x",
            "y",
            "width",
            "height",
            "points",
            "text",
            "strokeStyle",
            "opacity",
        ] {
            assert_eq!(a[field], b[field], "{field}");
        }
    }
    assert_eq!(annotated.last().unwrap()["text"], "Data");
}
