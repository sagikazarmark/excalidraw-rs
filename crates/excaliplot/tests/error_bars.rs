use excaliplot::{ErrorBarChart, FillStyle, LegendPosition, Scene, SketchStyle, VerticalInterval};
use serde_json::{Value, json};

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn asymmetric_limits_map_independently_into_one_native_compound_observation() {
    let scene = ErrorBarChart::new(
        &[VerticalInterval::new(2., 2., 3., 8.)],
        "Supplied range",
        0.0..10.0,
        0.0..10.0,
    )
    .legend(LegendPosition::Off)
    .render()
    .unwrap();
    let es = elements(&scene);
    let marks: Vec<_> = es
        .iter()
        .filter(|e| e["groupIds"].as_array().unwrap().len() == 3)
        .collect();
    assert_eq!(
        marks
            .iter()
            .map(|e| e["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["line", "line", "line", "ellipse"]
    );
    for (mark, x, y, points) in [
        (marks[0], 204., 262., json!([[0., 0.], [16., 0.]])),
        (marks[1], 212., 262., json!([[0., 0.], [0., -148.]])),
        (marks[2], 204., 114., json!([[0., 0.], [16., 0.]])),
    ] {
        assert_eq!(mark["x"], x);
        assert_eq!(mark["y"], y);
        assert_eq!(mark["points"], points);
        assert_eq!(mark["strokeColor"], "#1971c2");
        assert_eq!(mark["strokeWidth"], 2);
    }
    assert_eq!(
        (marks[3]["x"].as_f64(), marks[3]["y"].as_f64()),
        (Some(208.), Some(233.))
    );
    assert_eq!(marks[3]["width"], 8.);
    assert_eq!(marks[3]["backgroundColor"], "#1971c2");
    assert!(marks.iter().all(|e| e["groupIds"] == marks[0]["groupIds"]));
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}

#[test]
fn invalid_limits_bounds_and_interpretations_return_errors() {
    for observation in [
        VerticalInterval::new(f64::NAN, 2., 3., 8.),
        VerticalInterval::new(2., f64::NEG_INFINITY, 3., 8.),
        VerticalInterval::new(2., 2., f64::NAN, 8.),
        VerticalInterval::new(2., 2., 3., f64::INFINITY),
        VerticalInterval::new(2., 4., 3., 8.),
        VerticalInterval::new(2., 2., 9., 8.),
        VerticalInterval::new(11., 2., 3., 8.),
        VerticalInterval::new(2., -1., 3., 8.),
        VerticalInterval::new(2., 2., 3., 11.),
    ] {
        assert!(
            ErrorBarChart::new(&[observation], "Range", 0.0..10.0, 0.0..10.0)
                .render()
                .is_err(),
            "{observation:?}"
        );
    }
    let observations = [VerticalInterval::new(2., 2., 3., 8.)];
    assert!(
        ErrorBarChart::new(&[], "Range", 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
    for range in [
        0.0..0.0,
        10.0..0.0,
        0.0..f64::INFINITY,
        f64::NAN..10.0,
        -1e10..10.,
        0.0..1e-8,
    ] {
        assert!(
            ErrorBarChart::new(&observations, "Range", range.clone(), 0.0..10.0)
                .render()
                .is_err()
        );
        assert!(
            ErrorBarChart::new(&observations, "Range", 0.0..10.0, range)
                .render()
                .is_err()
        );
    }
    for label in ["", "   ", "Range\n95%", "β"] {
        assert!(
            ErrorBarChart::new(&observations, label, 0.0..10.0, 0.0..10.0)
                .legend(LegendPosition::Off)
                .render()
                .is_err()
        );
    }
}

#[test]
fn repeated_and_degenerate_observations_keep_distinct_identities_and_input_order() {
    let observations = [
        VerticalInterval::new(8., 2., 4., 7.),
        VerticalInterval::new(2., 3., 3., 3.),
        VerticalInterval::new(2., 3., 3., 3.),
        VerticalInterval::new(5., 2., 2., 7.),
        VerticalInterval::new(6., 2., 7., 7.),
    ];
    let scene = ErrorBarChart::new(&observations, "Range", 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    let es = elements(&scene);
    let marks: Vec<_> = es
        .iter()
        .filter(|e| e["groupIds"].as_array().unwrap().len() == 3)
        .collect();
    assert_eq!(
        marks.len(),
        16,
        "three full compounds and two cap/center pairs"
    );
    let centers: Vec<_> = marks.iter().filter(|e| e["type"] == "ellipse").collect();
    assert_eq!(centers.len(), 5);
    assert!(centers[0]["x"].as_f64().unwrap() > centers[1]["x"].as_f64().unwrap());
    assert_eq!(centers[1]["x"], centers[2]["x"]);
    assert_eq!(centers[1]["y"], centers[2]["y"]);
    let groups: std::collections::HashSet<_> = centers
        .iter()
        .map(|e| e["groupIds"][0].as_str().unwrap())
        .collect();
    assert_eq!(groups.len(), 5);
    let label = es.iter().find(|e| e["text"] == "Range").unwrap();
    for mark in marks {
        assert_eq!(mark["groupIds"][1], label["groupIds"][0]);
        assert_eq!(mark["groupIds"][2], label["groupIds"][1]);
    }
    assert!(
        ErrorBarChart::new(
            &[VerticalInterval::new(2., 3., 3., 3.00001)],
            "Range",
            0.0..10.0,
            0.0..10.0
        )
        .render()
        .is_err()
    );
}

#[test]
fn complete_styling_and_independent_cap_radius_are_shared_by_the_measured_legend() {
    use excaliplot::{FillStyle, SketchStyle, StrokeStyle};
    for (stroke, serialized) in [
        (StrokeStyle::Solid, "solid"),
        (StrokeStyle::Dashed, "dashed"),
        (StrokeStyle::Dotted, "dotted"),
    ] {
        let scene = ErrorBarChart::new(
            &[VerticalInterval::new(5., 2., 4., 8.)],
            "Supplied 95% CI",
            0.0..10.0,
            0.0..10.0,
        )
        .color((224, 49, 49))
        .line_width(6)
        .opacity(0.65)
        .stroke_style(stroke)
        .cap_half_width(12)
        .marker(7, false)
        .sketch(SketchStyle::new(1, FillStyle::Solid).unwrap())
        .size((800, 440))
        .render()
        .unwrap();
        let es = elements(&scene);
        let marks: Vec<_> = es
            .iter()
            .filter(|e| e["type"] != "text" && e["groupIds"].as_array().unwrap().len() >= 2)
            .collect();
        assert_eq!(marks.len(), 8);
        for mark in &marks {
            assert_eq!(mark["strokeColor"], "#e03131");
            assert_eq!(mark["backgroundColor"], "transparent");
            assert_eq!(mark["strokeWidth"], 6);
            assert_eq!(mark["opacity"], 65);
            assert_eq!(mark["roughness"], 1);
            assert_eq!(
                mark["strokeStyle"],
                if mark["type"] == "line" {
                    serialized
                } else {
                    "solid"
                }
            );
            if mark["type"] == "ellipse" {
                assert_eq!(mark["width"], 14.);
            }
        }
        for cap in [marks[0], marks[2], marks[4], marks[6]] {
            assert_eq!(cap["width"], 24.);
        }
        let label = es.iter().find(|e| e["text"] == "Supplied 95% CI").unwrap();
        assert!(label["x"].as_f64().unwrap() >= marks[4]["x"].as_f64().unwrap() + 24. + 3. + 8.);
        assert!(label["x"].as_f64().unwrap() + label["width"].as_f64().unwrap() <= 776.);
    }
}

#[test]
fn full_cap_and_marker_strokes_must_clear_plot_edges_and_styles_must_be_visible() {
    let observations = [VerticalInterval::new(5., 2., 4., 8.)];
    let chart = || ErrorBarChart::new(&observations, "Range", 0.0..10.0, 0.0..10.0);
    for invalid in [0, 21, u32::MAX] {
        assert!(chart().line_width(invalid).render().is_err());
        assert!(chart().cap_half_width(invalid).render().is_err());
        assert!(chart().marker(invalid, false).render().is_err());
    }
    for invalid in [0., 0.001, -1., 1.1, f64::NAN] {
        assert!(chart().opacity(invalid).render().is_err());
    }
    for x in [0., 10.] {
        let observations = [VerticalInterval::new(x, 2., 4., 8.)];
        let edge = || ErrorBarChart::new(&observations, "Range", 0.0..10.0, 0.0..10.0);
        assert!(edge().cap_half_width(4).marker(4, true).render().is_ok());
        assert!(
            edge().cap_half_width(5).render().is_err(),
            "cap includes half stroke width"
        );
        assert!(
            edge().cap_half_width(1).marker(5, false).render().is_err(),
            "hollow center includes half stroke width"
        );
    }
    for o in [
        VerticalInterval::new(5., 0., 0., 8.),
        VerticalInterval::new(5., 2., 10., 10.),
    ] {
        assert!(
            ErrorBarChart::new(&[o], "Range", 0.0..10.0, 0.0..10.0)
                .marker(5, true)
                .render()
                .is_ok()
        );
        assert!(
            ErrorBarChart::new(&[o], "Range", 0.0..10.0, 0.0..10.0)
                .marker(5, false)
                .render()
                .is_err()
        );
    }
    for o in [
        VerticalInterval::new(5., 0., 5., 8.),
        VerticalInterval::new(5., 2., 5., 10.),
    ] {
        assert!(
            ErrorBarChart::new(&[o], "Range", 0.0..10.0, 0.0..10.0)
                .line_width(12)
                .render()
                .is_err(),
            "endpoint cap stroke extent"
        );
    }
    assert!(
        chart()
            .line_width(20)
            .marker(20, false)
            .cap_half_width(20)
            .render()
            .is_ok()
    );
    assert!(
        chart()
            .size((400, 300))
            .labels(&"W".repeat(100), "X", "Y")
            .render()
            .is_err()
    );
    assert!(
        ErrorBarChart::new(&observations, &"W".repeat(100), 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
}

#[test]
fn opaque_centers_cannot_hide_entire_nondegenerate_intervals() {
    let observations = [VerticalInterval::new(5., 4.8, 5., 5.2)];
    let chart = || {
        ErrorBarChart::new(&observations, "Range", 0.0..10.0, 0.0..10.0)
            .legend(LegendPosition::Off)
            .marker(20, true)
    };
    // The caps are five units above/below the center at the default size.
    for cap in [8, 18] {
        let error = chart()
            .cap_half_width(cap)
            .render()
            .err()
            .expect("hidden interval must fail");
        assert!(error.to_string().contains("completely hidden"), "{error}");
    }
    // Native opacity rounds to integer percent: this still becomes opaque.
    assert!(chart().opacity(0.999).render().is_err());
    // Each choice exposes some interval geometry without changing the data.
    for visible in [
        chart().marker(4, true),
        chart().marker(20, false),
        chart().opacity(0.99),
        chart().sketch(SketchStyle::new(0, FillStyle::Hachure).unwrap()),
        chart().sketch(SketchStyle::new(0, FillStyle::CrossHatch).unwrap()),
        chart().cap_half_width(19),
        chart().cap_half_width(18).line_width(4),
        chart().size((640, 1600)),
        ErrorBarChart::new(&observations, "Range", 0.0..10.0, 4.0..6.0)
            .legend(LegendPosition::Off)
            .marker(20, true),
    ] {
        assert!(visible.render().is_ok());
    }
    // Partial overlap, endpoint estimates, and degenerate intervals stay valid.
    for observation in [
        VerticalInterval::new(5., 4.8, 5., 6.),
        VerticalInterval::new(5., 5., 5., 6.),
        VerticalInterval::new(5., 4., 5., 5.),
        VerticalInterval::new(5., 5., 5., 5.),
    ] {
        assert!(
            ErrorBarChart::new(&[observation], "Range", 0.0..10.0, 0.0..10.0)
                .legend(LegendPosition::Off)
                .marker(20, true)
                .render()
                .is_ok()
        );
    }
}

#[test]
fn interval_axes_reuse_bounded_tick_formatting_and_density() {
    use excaliplot::TickFormat;
    let observations = [VerticalInterval::new(0.5, 0.2, 0.4, 0.8)];
    let chart = || ErrorBarChart::new(&observations, "Range", 0.0..1.0, 0.0..1.0);
    let es = elements(
        &chart()
            .x_tick_format(TickFormat::Decimal { places: 1 })
            .y_tick_format(TickFormat::FractionPercent { places: 0 })
            .tick_density(3, 3)
            .render()
            .unwrap(),
    );
    assert!(es.iter().any(|e| e["text"] == "100%"));
    assert!(es.iter().any(|e| e["text"] == "1.0"));
    assert!(chart().tick_density(1, 3).render().is_err());
    assert!(chart().tick_density(3, 21).render().is_err());
    assert!(
        chart()
            .x_tick_format(TickFormat::Decimal { places: 0 })
            .render()
            .is_err()
    );
}

#[test]
fn six_observation_example_exports_compounds_and_protects_editor_work() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("intervals.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args([
                "run",
                "--locked",
                "--quiet",
                "--example",
                "error_bars",
                "--",
            ])
            .arg(&destination)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
    let es = doc["elements"].as_array().unwrap();
    assert_eq!(
        es.iter()
            .filter(|e| e["type"] == "ellipse" && e["groupIds"].as_array().unwrap().len() == 3)
            .count(),
        6
    );
    assert_eq!(
        es.iter()
            .filter(|e| e["groupIds"].as_array().unwrap().len() == 3)
            .count(),
        22
    );
    assert!(es.iter().any(|e| e["text"] == "Supplied 95% CI"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("vertices:"));
    std::fs::write(&destination, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(destination).unwrap(), b"editor work");
}

#[test]
fn large_legend_centers_leave_both_caps_and_stem_visible() {
    for fill in [true, false] {
        let scene = ErrorBarChart::new(
            &[VerticalInterval::new(5., 2., 4., 8.)],
            "Range",
            0.0..10.0,
            0.0..10.0,
        )
        .marker(20, fill)
        .line_width(20)
        .render()
        .unwrap();
        let es = elements(&scene);
        let swatch: Vec<_> = es
            .iter()
            .filter(|e| e["type"] != "text" && e["groupIds"].as_array().unwrap().len() == 2)
            .collect();
        let center = swatch[3];
        let marker_stroke = if fill { 0. } else { 10. };
        let top = center["y"].as_f64().unwrap() - marker_stroke;
        let bottom = center["y"].as_f64().unwrap() + 40. + marker_stroke;
        assert!(swatch[0]["y"].as_f64().unwrap() - 10. >= bottom + 8.);
        assert!(swatch[2]["y"].as_f64().unwrap() + 10. <= top - 8.);
    }
}
