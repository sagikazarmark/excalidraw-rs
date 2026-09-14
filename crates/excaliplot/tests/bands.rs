use excaliplot::{BandChart, LegendPosition, Scene};
use serde_json::{Value, json};

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn rejects_missing_crossed_unordered_and_unresolvable_envelopes() {
    for (xs, lower, upper) in [
        (vec![], vec![], vec![]),
        (vec![1.], vec![2.], vec![4.]),
        (vec![1., 2.], vec![2.], vec![4., 5.]),
        (vec![1., 2.], vec![2., 3.], vec![]),
        (vec![1., 2.], vec![2., 6.], vec![4., 5.]),
        (vec![2., 1.], vec![2., 3.], vec![4., 5.]),
        (vec![1., 1.], vec![2., 3.], vec![4., 5.]),
        (vec![1., f64::NAN], vec![2., 3.], vec![4., 5.]),
        (vec![1., 2.], vec![f64::NEG_INFINITY, 3.], vec![4., 5.]),
        (vec![1., 2.], vec![2., 3.], vec![4., f64::INFINITY]),
        (vec![1., 11.], vec![2., 3.], vec![4., 5.]),
        (vec![1., 2.], vec![-1., 3.], vec![4., 5.]),
        (vec![1., 2.], vec![2., 3.], vec![4., 11.]),
        (vec![1., 2.], vec![2., 3.], vec![2., 3.]),
        (vec![1., 2.], vec![2., 3.], vec![2.000001, 3.]),
        (vec![1., 1.000001], vec![2., 3.], vec![4., 5.]),
        // Reject even partial resolution loss, rather than silently merging samples.
        (vec![1., 1.000001, 8.], vec![2., 3., 4.], vec![4., 5., 6.]),
        (vec![1., 2., 8.], vec![2., 3., 4.], vec![4., 3.000001, 6.]),
    ] {
        assert!(
            BandChart::new(&xs, &lower, &upper, "Range", 0.0..10.0, 0.0..10.0)
                .render()
                .is_err(),
            "{xs:?} {lower:?} {upper:?}"
        );
    }
    // A triangular band may close to zero width at either endpoint.
    assert!(
        BandChart::new(
            &[0., 5., 10.],
            &[2., 3., 4.],
            &[2., 8., 4.],
            "Range",
            0.0..10.0,
            0.0..10.0
        )
        .render()
        .is_ok()
    );
}

#[test]
fn varying_lower_limits_form_one_closed_native_fill_with_exact_mapped_widths() {
    let scene = BandChart::new(
        &[0., 5., 10.],
        &[2., 4., 3.],
        &[6., 8., 5.],
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
        .filter(|e| e["groupIds"].as_array().unwrap().len() == 2)
        .collect();
    assert_eq!(marks.len(), 1);
    let fill = marks[0];
    // Known default plot corners: (112,311)..(615,64).
    assert_eq!(fill["type"], "line");
    assert_eq!(fill["x"], 112.);
    assert_eq!(fill["y"], 163.);
    assert_eq!(fill["width"], 503.);
    assert_eq!(fill["height"], 148.);
    assert_eq!(
        fill["points"],
        json!([
            [0., 0.],
            [251., -49.],
            [503., 25.],
            [503., 74.],
            [251., 50.],
            [0., 99.],
            [0., 0.]
        ])
    );
    assert_eq!(fill["backgroundColor"], "#1971c2");
    assert_eq!(fill["strokeColor"], "transparent");
    assert_eq!(fill["opacity"], 35);
    assert_eq!(scene.diagnostics().calls["fill_polygon"], 1);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}

#[test]
fn optional_boundaries_and_center_are_separate_opaque_paths_over_translucent_fill() {
    let chart = || {
        BandChart::new(
            &[0., 5., 10.],
            &[2., 4., 3.],
            &[6., 8., 5.],
            "Range",
            0.0..10.0,
            0.0..10.0,
        )
        .legend(LegendPosition::Off)
    };
    let scene = chart()
        .center(&[3., 5., 4.])
        .boundary_lines(true)
        .line_width(6)
        .opacity(0.255)
        .color((224, 49, 49))
        .render()
        .unwrap();
    let es = elements(&scene);
    let marks: Vec<_> = es
        .iter()
        .filter(|e| e["groupIds"].as_array().unwrap().len() == 2)
        .collect();
    assert_eq!(marks.len(), 4);
    assert_eq!(marks[0]["opacity"], 26);
    assert_eq!(marks[0]["backgroundColor"], "#e03131");
    for (mark, y, points) in [
        (marks[1], 163., json!([[0., 0.], [251., -49.], [503., 25.]])),
        (
            marks[2],
            262.,
            json!([[0., 0.], [251., -49.], [503., -25.]]),
        ),
        (
            marks[3],
            237.,
            json!([[0., 0.], [251., -49.], [503., -24.]]),
        ),
    ] {
        assert_eq!(mark["x"], 112.);
        assert_eq!(mark["y"], y);
        assert_eq!(mark["points"], points);
        assert_eq!(mark["strokeWidth"], 6);
        assert_eq!(mark["opacity"], 100);
        assert_eq!(mark["strokeColor"], "#e03131");
        assert_eq!(mark["backgroundColor"], "transparent");
        assert_eq!(mark["groupIds"], marks[0]["groupIds"]);
    }
    for (boundaries, center, count) in [(false, false, 1), (true, false, 3), (false, true, 2)] {
        let mut c = chart().boundary_lines(boundaries);
        if center {
            c = c.center(&[2., 8., 3.]);
        }
        let es = elements(&c.render().unwrap());
        assert_eq!(
            es.iter()
                .filter(|e| e["groupIds"].as_array().unwrap().len() == 2)
                .count(),
            count
        );
    }
}

#[test]
fn centers_styles_bounds_and_measured_layout_are_validated_before_publishing() {
    let chart = || {
        BandChart::new(
            &[1., 5., 9.],
            &[2., 4., 3.],
            &[6., 8., 5.],
            "Range",
            0.0..10.0,
            0.0..10.0,
        )
    };
    for center in [
        vec![],
        vec![3.],
        vec![3., 5., 6.],
        vec![1., 5., 4.],
        vec![3., f64::NAN, 4.],
    ] {
        assert!(chart().center(&center).render().is_err());
    }
    for opacity in [0., 0.004, -1., 1.1, f64::NAN, f64::INFINITY] {
        assert!(chart().opacity(opacity).render().is_err());
    }
    for width in [0, 21, u32::MAX] {
        assert!(chart().line_width(width).render().is_err());
    }
    for range in [
        0.0..0.0,
        10.0..0.0,
        f64::NAN..10.,
        0.0..f64::INFINITY,
        -1e10..10.,
        0.0..1e-8,
    ] {
        assert!(
            BandChart::new(
                &[1., 2.],
                &[2., 3.],
                &[4., 5.],
                "Range",
                range.clone(),
                0.0..10.0
            )
            .render()
            .is_err()
        );
        assert!(
            BandChart::new(&[1., 2.], &[2., 3.], &[4., 5.], "Range", 0.0..10.0, range)
                .render()
                .is_err()
        );
    }
    for label in ["", "   ", "Range\n95%", "β"] {
        assert!(
            BandChart::new(&[1., 2.], &[2., 3.], &[4., 5.], label, 0.0..10.0, 0.0..10.0)
                .legend(LegendPosition::Off)
                .render()
                .is_err()
        );
    }
    assert!(
        chart()
            .size((400, 300))
            .labels(&"W".repeat(100), "X", "Y")
            .render()
            .is_err()
    );
    assert!(chart().tick_density(1, 6).render().is_err());
    assert!(
        chart()
            .x_tick_format(excaliplot::TickFormat::Decimal { places: 10 })
            .render()
            .is_err()
    );
    assert!(
        chart()
            .y_tick_format(excaliplot::TickFormat::Decimal { places: 10 })
            .render()
            .is_err()
    );
    // Complete path strokes must clear the existing five-unit plot halo.
    let edge = || {
        BandChart::new(
            &[0., 10.],
            &[0., 0.],
            &[10., 10.],
            "Range",
            0.0..10.0,
            0.0..10.0,
        )
    };
    assert!(edge().boundary_lines(true).line_width(10).render().is_ok());
    assert!(edge().boundary_lines(true).line_width(11).render().is_err());
    assert!(edge().center(&[5., 5.]).line_width(11).render().is_err());
    assert!(
        edge().line_width(20).render().is_ok(),
        "disabled paths need no clearance"
    );
}

#[test]
fn duplicate_diagrams_keep_independent_groups_and_matching_legends_through_composition() {
    use excaliplot::{BorderStyle, FillStyle, SketchStyle, TickFormat};
    let chart = || {
        BandChart::new(
            &[0.1, 0.5, 0.9],
            &[0.2, 0.4, 0.3],
            &[0.6, 0.8, 0.5],
            "Range",
            0.0..1.0,
            0.0..1.0,
        )
        .center(&[0.3, 0.5, 0.4])
        .boundary_lines(true)
        .opacity(0.65)
        .sketch(SketchStyle::new(1, FillStyle::Solid).unwrap())
        .x_tick_format(TickFormat::Decimal { places: 1 })
        .y_tick_format(TickFormat::FractionPercent { places: 0 })
        .tick_density(3, 3)
    };
    let original = elements(&chart().render().unwrap());
    assert!(original.iter().any(|e| e["text"] == "100%"));
    assert!(original.iter().any(|e| e["text"] == "1.0"));
    let grouped: Vec<_> = original
        .iter()
        .filter(|e| e["groupIds"].as_array().unwrap().len() == 2)
        .collect();
    assert_eq!(
        grouped.len(),
        6,
        "four data paths plus fill swatch and label"
    );
    assert_eq!(grouped[4]["type"], "rectangle");
    assert_eq!(grouped[4]["opacity"], grouped[0]["opacity"]);
    assert_eq!(grouped[4]["backgroundColor"], grouped[0]["backgroundColor"]);
    assert_eq!(grouped[5]["text"], "Range");
    assert_eq!(grouped[5]["opacity"], 100);
    assert!(
        grouped
            .iter()
            .all(|e| e["groupIds"] == grouped[0]["groupIds"] && e["roughness"] == 1)
    );
    let mut composed = Scene::new();
    for offset in [(20., 30.), (720., 30.)] {
        let mut panel = chart().render().unwrap();
        panel.add_border(12., BorderStyle::default()).unwrap();
        panel.add_frame(16., Some("Band")).unwrap();
        panel.place_at(offset).unwrap();
        composed.append(panel).unwrap();
    }
    let es = elements(&composed);
    assert_eq!(es.len(), 2 * (original.len() + 2));
    let ids: std::collections::HashSet<_> = es.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), es.len());
    let fills: Vec<_> = es
        .iter()
        .filter(|e| e["type"] == "line" && e["backgroundColor"] == "#1971c2")
        .collect();
    assert_eq!(fills.len(), 2);
    for f in &fills {
        assert_eq!(f["points"], grouped[0]["points"]);
        assert_eq!(f["opacity"], 65);
        assert!(!f["frameId"].is_null());
    }
    assert_ne!(fills[0]["frameId"], fills[1]["frameId"]);
    assert!(
        fills[0]["groupIds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| !fills[1]["groupIds"].as_array().unwrap().contains(g))
    );
    assert_eq!(
        fills[1]["x"].as_f64().unwrap() - fills[0]["x"].as_f64().unwrap(),
        700.
    );
    let library: Value = serde_json::from_slice(&composed.to_library_bytes().unwrap()).unwrap();
    assert_eq!(library["libraryItems"][0]["elements"], json!(es));
    assert_eq!(composed.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(composed.diagnostics().calls.get("blit_bitmap"), None);
}

#[test]
fn ordered_band_demo_exports_native_paths_and_protects_editor_work() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("bands.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--quiet", "--example", "bands", "--"])
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
    let paths: Vec<_> = es
        .iter()
        .filter(|e| e["type"] == "line" && e["groupIds"].as_array().unwrap().len() == 2)
        .collect();
    assert_eq!(paths.len(), 4);
    assert_eq!(paths[0]["points"].as_array().unwrap().len(), 13);
    assert!(es.iter().any(|e| e["text"] == "Supplied range"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("vertices:"));
    std::fs::write(&destination, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(destination).unwrap(), b"editor work");
}
