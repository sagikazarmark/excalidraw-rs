use excaliplot::{
    AxisScale, LegendPosition, LineChart, NamedSeries, ScatterChart, Scene, TickFormat,
};
use serde_json::Value;

fn elements(scene: Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn log_domain_accepts_tiny_spans_and_rejects_invalid_bounds_before_drawing() {
    let points = [(1e-15, 1.), (1e-14, 2.), (1e-13, 3.)];
    let items = elements(
        LineChart::new(&points, 1e-15..1e-13, 0.0..4.0)
            .x_scale(AxisScale::Log10)
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .render()
            .unwrap(),
    );
    // Plotters may omit an exact endpoint decade due to logarithmic roundoff.
    for text in ["1.0e-14", "1.0e-13"] {
        assert!(items.iter().any(|e| e["text"] == text), "{text}");
    }
    for range in [
        0.0..10.0,
        -10.0..-1.0,
        10.0..1.0,
        1.0..1.0,
        f64::NAN..10.0,
        1.0..f64::INFINITY,
        1e-16..1e-14,
        1.0..1e10,
        1e-15..1e-15_f64.next_up(),
    ] {
        for x_axis in [true, false] {
            let chart = if x_axis {
                LineChart::new(&[(1., 1.), (2., 2.)], range.clone(), 0.0..3.0)
                    .x_scale(AxisScale::Log10)
            } else {
                LineChart::new(&[(1., 1.), (2., 2.)], 0.0..3.0, range.clone())
                    .y_scale(AxisScale::Log10)
            };
            assert!(
                matches!(chart.render(), Err(excaliplot::Error::Invalid(_))),
                "{range:?}"
            );
        }
    }
    assert!(
        LineChart::auto(&[(1., 1.), (10., 10.)])
            .x_scale(AxisScale::Log10)
            .render()
            .is_err()
    );
    assert!(
        ScatterChart::auto(&[(1., 1.)])
            .y_scale(AxisScale::Log10)
            .render()
            .is_err()
    );
}

#[test]
fn powers_of_ten_are_equally_spaced_native_line_vertices_and_ticks() {
    let points = [(1., 1.), (10., 10.), (100., 100.), (1000., 1000.)];
    let items = elements(
        LineChart::new(&points, 1.0..1000.0, 1.0..1000.0)
            .x_scale(AxisScale::Log10)
            .y_scale(AxisScale::Log10)
            .render()
            .unwrap(),
    );
    let line = items
        .iter()
        .find(|e| e["strokeColor"] == "#1971c2")
        .unwrap();
    let vertices = line["points"].as_array().unwrap();
    assert_eq!(vertices.len(), 4);
    assert_eq!(line["groupIds"].as_array().unwrap().len(), 1);
    for axis in 0..2 {
        let gaps: Vec<_> = vertices
            .windows(2)
            .map(|p| p[1][axis].as_f64().unwrap() - p[0][axis].as_f64().unwrap())
            .collect();
        assert!(gaps[0].abs() > 50.);
        assert!(
            gaps.iter().all(|gap| (gap - gaps[0]).abs() <= 1.),
            "{gaps:?}"
        );
    }
    for label in ["1", "10", "100", "1000"] {
        assert_eq!(
            items.iter().filter(|e| e["text"] == label).count(),
            2,
            "{label}"
        );
    }
}

#[test]
fn log_ticks_must_exist_and_remain_distinct_and_measured() {
    let error = LineChart::new(&[(2., 1.), (3., 2.)], 2.0..3.0, 0.0..3.0)
        .x_scale(AxisScale::Log10)
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("no visible ticks"), "{error}");
    let error = LineChart::new(&[(1e-15, 1.), (1e-12, 2.)], 1e-15..1e-12, 0.0..3.0)
        .x_scale(AxisScale::Log10)
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("distinct"), "{error}");
    let error = LineChart::new(&[(1., 1.), (1e9, 1e9)], 1.0..1e9, 1.0..1e9)
        .x_scale(AxisScale::Log10)
        .y_scale(AxisScale::Log10)
        .tick_density(20, 20)
        .size((400, 300))
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("crowded"), "{error}");
}

#[test]
fn dense_single_decade_ticks_are_unique_on_either_axis() {
    for x_axis in [true, false] {
        let points = if x_axis {
            [(1., 21.), (10., 22.)]
        } else {
            [(21., 1.), (22., 10.)]
        };
        let chart = if x_axis {
            LineChart::new(&points, 1.0..10.0, 20.0..23.0)
                .x_scale(AxisScale::Log10)
                .x_tick_format(TickFormat::Decimal { places: 9 })
                .tick_density(10, 3)
                .size((16384, 400))
        } else {
            LineChart::new(&points, 20.0..23.0, 1.0..10.0)
                .y_scale(AxisScale::Log10)
                .y_tick_format(TickFormat::Decimal { places: 9 })
                .tick_density(3, 10)
                .size((640, 2000))
        };
        let items = elements(chart.render().unwrap());
        for value in 1..=10 {
            let label = format!("{value:.9}", value = f64::from(value));
            assert_eq!(
                items.iter().filter(|e| e["text"] == label).count(),
                1,
                "{label}"
            );
        }
        // Each major tick has one native stroke, even at the upper decade boundary.
        let ticks = items
            .iter()
            .filter(|e| {
                e["type"] == "line"
                    && if x_axis {
                        e["width"].as_f64() == Some(0.) && e["height"].as_f64() == Some(5.)
                    } else {
                        e["width"].as_f64() == Some(5.) && e["height"].as_f64() == Some(0.)
                    }
            })
            .count();
        assert_eq!(ticks, 9);
    }
}

#[test]
fn independent_scales_preserve_line_and_scatter_mapping_and_composed_groups() {
    for x_scale in [AxisScale::Linear, AxisScale::Log10] {
        for y_scale in [AxisScale::Linear, AxisScale::Log10] {
            let points = [(1., 1.), (10., 10.), (100., 100.), (1000., 1000.)];
            let names = [
                NamedSeries::new("Observed", &points, (25, 113, 194)),
                NamedSeries::new("Observed", &points, (25, 113, 194)),
            ];
            let line = LineChart::from_series(&names, 1.0..1000.0, 1.0..1000.0)
                .x_scale(x_scale)
                .y_scale(y_scale)
                .legend(LegendPosition::Off)
                .render()
                .unwrap();
            let marks = elements(line);
            let lines: Vec<_> = marks
                .iter()
                .filter(|e| e["strokeColor"] == "#1971c2")
                .collect();
            assert_eq!(lines.len(), 2);
            assert_ne!(lines[0]["groupIds"][0], lines[1]["groupIds"][0]);
            assert_eq!(lines[0]["groupIds"][1], lines[1]["groupIds"][1]);
            let vertices = lines[0]["points"].as_array().unwrap();
            for (axis, scale) in [x_scale, y_scale].into_iter().enumerate() {
                let delta = |i: usize| {
                    (vertices[i][axis].as_f64().unwrap() - vertices[0][axis].as_f64().unwrap())
                        .abs()
                };
                let expected_fraction = if scale == AxisScale::Log10 {
                    1. / 3.
                } else {
                    9. / 999.
                };
                assert!((delta(1) - delta(3) * expected_fraction).abs() <= 1.);
            }
            let mut scatter = ScatterChart::from_series(&names, 1.0..1000.0, 1.0..1000.0)
                .x_scale(x_scale)
                .y_scale(y_scale)
                .legend(LegendPosition::Off)
                .render()
                .unwrap();
            scatter.translate((50., 60.)).unwrap();
            let mut composed = Scene::new();
            composed.append(scatter).unwrap();
            let items = elements(composed);
            let circles: Vec<_> = items.iter().filter(|e| e["type"] == "ellipse").collect();
            assert_eq!(circles.len(), 8);
            for (circle, point) in circles.iter().take(4).zip(vertices) {
                assert_eq!(circle["width"].as_f64(), Some(10.));
                assert_eq!(circle["height"].as_f64(), Some(10.));
                assert_eq!(circle["groupIds"].as_array().unwrap().len(), 2);
                for (axis, origin, offset) in [(0, "x", 50.), (1, "y", 60.)] {
                    assert_eq!(
                        circle[origin].as_f64().unwrap() + 5.,
                        lines[0][origin].as_f64().unwrap() + point[axis].as_f64().unwrap() + offset
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_observations_collapse_and_marker_clearance_return_errors() {
    for value in [0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1001.] {
        for point in [(value, 10.), (10., value)] {
            let points = [(1., 1.), point];
            assert!(
                LineChart::new(&points, 1.0..1000.0, 1.0..1000.0)
                    .x_scale(AxisScale::Log10)
                    .y_scale(AxisScale::Log10)
                    .render()
                    .is_err()
            );
            assert!(
                ScatterChart::new(&points, 1.0..1000.0, 1.0..1000.0)
                    .x_scale(AxisScale::Log10)
                    .y_scale(AxisScale::Log10)
                    .render()
                    .is_err()
            );
        }
    }
    let error = LineChart::new(
        &[(10., 10.), (10.000001, 10.000001)],
        1.0..1000.0,
        1.0..1000.0,
    )
    .x_scale(AxisScale::Log10)
    .y_scale(AxisScale::Log10)
    .render()
    .err()
    .unwrap();
    assert!(error.to_string().contains("collapses"), "{error}");
    let error = ScatterChart::new(&[(1., 1.)], 1.0..1000.0, 1.0..1000.0)
        .x_scale(AxisScale::Log10)
        .y_scale(AxisScale::Log10)
        .marker(20, false)
        .render()
        .err()
        .unwrap();
    assert!(error.to_string().contains("marker crowds"), "{error}");
    assert!(
        ScatterChart::new(
            &[(100., 10.), (10., 100.), (10., 100.)],
            1.0..1000.0,
            1.0..1000.0
        )
        .x_scale(AxisScale::Log10)
        .y_scale(AxisScale::Log10)
        .marker(20, false)
        .render()
        .is_ok()
    );
    // A signed linear companion axis stays valid.
    assert!(
        LineChart::new(&[(-1., 1.), (1., 1000.)], -1.0..1.0, 1.0..1000.0)
            .y_scale(AxisScale::Log10)
            .render()
            .is_ok()
    );
    assert!(
        LineChart::new(&[(1., -1.), (1000., 1.)], 1.0..1000.0, -1.0..1.0)
            .x_scale(AxisScale::Log10)
            .render()
            .is_ok()
    );
}

#[test]
fn multi_decade_example_exports_native_panels_and_protects_editor_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log_axes.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--quiet", "--example", "log_axes", "--"])
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
    let items = doc["elements"].as_array().unwrap();
    for title in [
        "Log X / linear Y",
        "Linear X / log Y",
        "Log X / log Y",
        "Small positive log range",
    ] {
        assert!(items.iter().any(|e| e["text"] == title));
    }
    assert!(
        items.iter().all(
            |e| ["text", "line", "ellipse", "rectangle"].contains(&e["type"].as_str().unwrap())
        )
    );
    std::fs::write(&path, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"editor work");
}
