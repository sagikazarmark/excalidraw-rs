use excaliplot::{AreaChart, LineChart, NamedSeries, ScatterChart, Scene};
use serde_json::Value;

const BLUE: (u8, u8, u8) = (25, 113, 194);

#[test]
fn wide_line_legend_caps_clear_labels_and_legacy_hollow_scatter_still_fits() {
    let scene = LineChart::from_series(
        &[NamedSeries::new("S", &[(2., 2.), (8., 8.)], BLUE).line_width(20)],
        0.0..10.0,
        0.0..10.0,
    )
    .render()
    .unwrap();
    let es = elements(scene);
    let label = es.iter().find(|e| e["text"] == "S").unwrap();
    let swatch = es
        .iter()
        .rfind(|e| e["type"] == "line" && e["strokeColor"] == "#1971c2")
        .unwrap();
    assert!(
        swatch["x"].as_f64().unwrap() + swatch["width"].as_f64().unwrap() + 10.
            < label["x"].as_f64().unwrap()
    );
    assert!(
        ScatterChart::new(&[(0., 5.)], 0.0..10.0, 0.0..10.0)
            .marker(5, false)
            .render()
            .is_ok()
    );
    assert!(
        ScatterChart::from_series(
            &[NamedSeries::new("S", &[(0., 5.)], BLUE).opacity(0.5)],
            0.0..10.0,
            0.0..10.0
        )
        .marker(5, false)
        .render()
        .is_ok()
    );
}

fn elements(scene: Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn invalid_styles_and_unsupported_area_overrides_fail_before_output() {
    let points = [(1., 2.), (9., 8.)];
    let base = NamedSeries::new("S", &points, BLUE);
    let mut invalid = vec![];
    for width in [0, 21, u32::MAX] {
        invalid.push(base.clone().line_width(width));
    }
    for radius in [0, 21, u32::MAX] {
        invalid.push(base.clone().marker(radius, false));
    }
    for alpha in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.,
        0.,
        0.004,
        1.001,
    ] {
        invalid.push(base.clone().opacity(alpha));
    }
    for series in invalid {
        assert!(
            LineChart::from_series(std::slice::from_ref(&series), 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
        assert!(
            ScatterChart::from_series(&[series], 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
    for series in [
        base.clone().line_width(3),
        base.clone().opacity(0.5),
        base.marker(4, true),
    ] {
        assert!(
            AreaChart::from_series(&[series], 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
}

#[test]
fn marker_clearance_includes_hollow_strokes_on_every_edge_and_final_log_mapping() {
    use excaliplot::{AxisScale, LegendPosition};
    for edge in [(0., 5.), (10., 5.), (5., 0.), (5., 10.)] {
        let mut points = [edge, (5., 5.)];
        points.sort_by(|a, b| f64::total_cmp(&a.0, &b.0));
        let base = NamedSeries::new("S", &points, BLUE);
        for (width, radius, fill, valid) in [
            (2, 4, false, true),
            (3, 4, false, false),
            (20, 5, true, true),
            (1, 6, true, false),
        ] {
            let series = [base.clone().line_width(width).marker(radius, fill)];
            assert_eq!(
                LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                    .legend(LegendPosition::Off)
                    .render()
                    .is_ok(),
                valid
            );
            assert_eq!(
                ScatterChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                    .legend(LegendPosition::Off)
                    .render()
                    .is_ok(),
                valid
            );
        }
    }
    let points = [(1.01, 5.), (50., 5.)];
    let series = [NamedSeries::new("S", &points, BLUE).marker(8, false)];
    assert!(
        LineChart::from_series(&series, 1.0..100.0, 0.0..10.0)
            .x_scale(AxisScale::Log10)
            .render()
            .is_err()
    );
    assert!(
        LineChart::auto_from_series(&[
            NamedSeries::new("S", &[(0., 0.), (10., 10.)], BLUE).marker(20, false)
        ])
        .size((400, 300))
        .render()
        .is_err()
    );
}

#[test]
fn scatter_overrides_inherit_independently_and_legends_fit_complete_large_markers() {
    let points = [(9., 8.), (1., 2.), (1., 2.)];
    let series = [
        NamedSeries::new("Same", &points, BLUE),
        NamedSeries::new("Same", &points, BLUE)
            .line_width(20)
            .opacity(0.255)
            .marker(20, false),
    ];
    let es = elements(
        ScatterChart::from_series(&series, -5.0..15.0, -5.0..15.0)
            .size((900, 500))
            .marker(7, true)
            .opacity(0.6)
            .render()
            .unwrap(),
    );
    let labels: Vec<_> = es.iter().filter(|e| e["text"] == "Same").collect();
    assert_ne!(labels[0]["groupIds"][0], labels[1]["groupIds"][0]);
    let mut previous_bottom = 0.;
    for (i, label) in labels.iter().enumerate() {
        let marks: Vec<_> = es
            .iter()
            .filter(|e| e["type"] == "ellipse" && e["groupIds"] == label["groupIds"])
            .collect();
        assert_eq!(marks.len(), 4);
        for mark in &marks {
            assert_eq!(mark["width"], if i == 0 { 14. } else { 40. });
            assert_eq!(mark["strokeWidth"], if i == 0 { 2 } else { 20 });
            assert_eq!(mark["opacity"], if i == 0 { 60 } else { 26 });
            assert_eq!(
                mark["backgroundColor"],
                if i == 0 { "#1971c2" } else { "transparent" }
            );
        }
        assert_eq!(marks[1]["x"], marks[2]["x"]);
        assert_eq!(marks[1]["y"], marks[2]["y"]);
        assert_ne!(marks[1]["id"], marks[2]["id"]);
        let swatch = marks[3];
        let stroke = if i == 0 { 0. } else { 10. };
        assert!(
            swatch["x"].as_f64().unwrap() + swatch["width"].as_f64().unwrap() + stroke + 8.
                <= label["x"].as_f64().unwrap()
        );
        assert!(swatch["y"].as_f64().unwrap() - stroke >= previous_bottom + 8.);
        previous_bottom =
            swatch["y"].as_f64().unwrap() + swatch["height"].as_f64().unwrap() + stroke;
    }
    let many = vec![series[1].clone(); 4];
    assert!(
        ScatterChart::from_series(&many, -5.0..15.0, -5.0..15.0)
            .size((900, 300))
            .render()
            .is_err()
    );
    assert!(
        LineChart::from_series(
            &[NamedSeries::new("W", &[(2., 2.), (8., 8.)], BLUE)
                .marker(20, false)
                .line_width(20)],
            0.0..10.0,
            0.0..10.0
        )
        .size((400, 300))
        .render()
        .is_err()
    );
}

#[test]
fn markers_do_not_relax_line_data_rules_and_visible_style_limits_are_accepted() {
    for points in [
        vec![],
        vec![(2., 2.)],
        vec![(2., 2.); 3],
        vec![(8., 2.), (2., 8.)],
        vec![(2., f64::NAN), (8., 8.)],
        vec![(2., 2.), (11., 8.)],
        vec![(2., 2.), (2.00000001, 2.)],
    ] {
        assert!(
            LineChart::from_series(
                &[NamedSeries::new("S", &points, BLUE).marker(4, true)],
                0.0..10.0,
                0.0..10.0
            )
            .render()
            .is_err()
        );
    }
    let scene = LineChart::from_series(
        &[NamedSeries::new("S", &[(2., 2.), (8., 8.)], BLUE)
            .line_width(20)
            .opacity(0.005)
            .marker(20, true)],
        0.0..10.0,
        0.0..10.0,
    )
    .size((900, 500))
    .render()
    .unwrap();
    let marks = elements(scene);
    assert!(
        marks
            .iter()
            .filter(|e| e["groupIds"].as_array().unwrap().len() == 2 && e["type"] != "text")
            .all(|e| e["opacity"] == 1 && e["strokeWidth"] == 20)
    );
}

#[test]
fn comparison_demo_emits_native_styles_and_protects_editor_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("comparison.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args([
                "run",
                "--locked",
                "--quiet",
                "--example",
                "series_styles",
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
    assert_eq!(es.iter().filter(|e| e["type"] == "ellipse").count(), 10);
    assert!(
        es.iter()
            .any(|e| e["strokeWidth"] == 6 && e["opacity"] == 65)
    );
    assert!(
        es.iter()
            .any(|e| e["strokeWidth"] == 2 && e["opacity"] == 100)
    );
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
fn line_styles_preserve_vertices_repeats_identity_and_painter_order() {
    let points = [(1., 2.), (1., 2.), (5., 8.), (9., 4.)];
    let defaults = [
        NamedSeries::new("Same", &points, BLUE),
        NamedSeries::new("Same", &points, BLUE),
    ];
    let plain = elements(
        LineChart::from_series(&defaults, 0.0..10.0, 0.0..10.0)
            .render()
            .unwrap(),
    );
    let series = [
        NamedSeries::new("Same", &points, BLUE)
            .line_width(6)
            .opacity(0.5)
            .marker(4, false),
        NamedSeries::new("Same", &points, BLUE)
            .line_width(2)
            .marker(4, true),
    ];
    let styled = elements(
        LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
            .render()
            .unwrap(),
    );
    let labels: Vec<_> = styled.iter().filter(|e| e["text"] == "Same").collect();
    assert_ne!(labels[0]["groupIds"][0], labels[1]["groupIds"][0]);
    assert_eq!(labels[0]["groupIds"][1], labels[1]["groupIds"][1]);
    let plain_paths: Vec<_> = plain
        .iter()
        .filter(|e| e["type"] == "line" && e["strokeColor"] == "#1971c2")
        .collect();
    for (i, label) in labels.iter().enumerate() {
        let members: Vec<_> = styled
            .iter()
            .filter(|e| e["groupIds"] == label["groupIds"])
            .collect();
        assert_eq!(
            members
                .iter()
                .map(|e| e["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "line", "ellipse", "ellipse", "ellipse", "ellipse", "line", "ellipse", "text"
            ]
        );
        let path = vertices(members[0]);
        assert_eq!(path, vertices(plain_paths[i]));
        assert_eq!(path.len(), 4);
        assert_eq!(path[0], path[1]);
        for (marker, center) in members[1..5].iter().zip(path) {
            assert_eq!(
                (
                    marker["x"].as_f64().unwrap() + 4.,
                    marker["y"].as_f64().unwrap() + 4.
                ),
                center
            );
        }
        assert_ne!(members[1]["id"], members[2]["id"]);
        for mark in &members[..7] {
            assert_eq!(mark["strokeWidth"], if i == 0 { 6 } else { 2 });
            assert_eq!(mark["opacity"], if i == 0 { 50 } else { 100 });
            if mark["type"] == "ellipse" {
                assert_eq!(mark["width"], 8.);
                assert_eq!(
                    mark["backgroundColor"],
                    if i == 0 { "transparent" } else { "#1971c2" }
                );
                assert_eq!(
                    mark["strokeColor"],
                    if i == 0 { "#1971c2" } else { "transparent" }
                );
            }
        }
    }
    let frame = |es: &[Value]| {
        es.iter()
            .filter(|e| e["groupIds"].as_array().unwrap().len() == 1)
            .map(|e| {
                [
                    "type",
                    "x",
                    "y",
                    "width",
                    "height",
                    "points",
                    "strokeWidth",
                    "opacity",
                    "backgroundColor",
                    "strokeColor",
                    "text",
                ]
                .map(|k| e[k].clone())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(frame(&plain), frame(&styled));
}

#[test]
fn an_invisible_opacity_is_reported_against_the_mark_that_carries_it() {
    use excaliplot::{BandChart, ErrorBarChart, HistogramChart, VerticalInterval};
    let series = [NamedSeries::new("S", &[(0., 5.)], BLUE).opacity(0.)];
    for (error, subject) in [
        (
            LineChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                .render()
                .err(),
            "series opacity",
        ),
        (
            ScatterChart::new(&[(0., 5.)], 0.0..10.0, 0.0..10.0)
                .opacity(0.)
                .render()
                .err(),
            "scatter opacity",
        ),
        (
            HistogramChart::new(&[0., 1.], &[1.], 0.0..1.0, 0.0..1.0)
                .opacity(0.)
                .render()
                .err(),
            "histogram opacity",
        ),
        (
            BandChart::new(
                &[1., 2.],
                &[2., 3.],
                &[4., 5.],
                "Range",
                0.0..10.0,
                0.0..10.0,
            )
            .opacity(0.)
            .render()
            .err(),
            "band opacity",
        ),
        (
            ErrorBarChart::new(
                &[VerticalInterval::new(2., 2., 3., 8.)],
                "Range",
                0.0..10.0,
                0.0..10.0,
            )
            .opacity(0.)
            .render()
            .err(),
            "error bar opacity",
        ),
    ] {
        let error = error.expect(subject).to_string();
        assert!(error.contains(subject), "{subject}: {error}");
    }
}
