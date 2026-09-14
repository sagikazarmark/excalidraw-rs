use excaliplot::{HistogramChart, Scene};
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn unequal_signed_bins_keep_source_order_counts_and_numeric_baseline() {
    let scene = HistogramChart::new(
        &[-10., -8., -4., 2., 10.],
        &[4., 0., 8., 4.],
        -10.0..10.0,
        0.0..10.0,
    )
    .render()
    .unwrap();
    let es = elements(&scene);
    let bins: Vec<_> = es
        .iter()
        .filter(|e| e["backgroundColor"] == "#1971c2")
        .collect();
    assert_eq!(bins.len(), 3);
    // Default plot corners (112,311)..(615,64); intervals have widths 2,4,6,8.
    for (bin, (x, y, width, height)) in bins.iter().zip([
        (112., 213., 50., 98.),
        (262., 114., 151., 197.),
        (413., 213., 202., 98.),
    ]) {
        assert_eq!(bin["type"], "rectangle");
        assert_eq!(bin["x"], x);
        assert_eq!(bin["y"], y);
        assert_eq!(bin["width"], width);
        assert_eq!(bin["height"], height);
        assert_eq!(bin["strokeColor"], "transparent");
        assert_eq!(bin["opacity"], 100);
        assert_eq!(bin["groupIds"], bins[0]["groupIds"]);
    }
    assert_ne!(bins[0]["id"], bins[2]["id"]);
    assert!(
        es.iter()
            .all(|e| e["groupIds"].as_array().unwrap().len() == 1)
    );
    assert!(es.iter().any(|e| e["text"] == "-10"));
    assert!(es.iter().any(|e| e["text"] == "10"));
    assert_eq!(
        &es[es.len() - 3..],
        bins.into_iter().cloned().collect::<Vec<_>>()
    );
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}

#[test]
fn rejects_invalid_edges_counts_and_noncontaining_bounds() {
    for (edges, counts) in [
        (vec![], vec![]),
        (vec![0.], vec![]),
        (vec![0., 1., 2.], vec![1.]),
        (vec![0., 1.], vec![1., 2.]),
        (vec![0., 2., 1.], vec![1., 2.]),
        (vec![0., 1., 1.], vec![1., 2.]),
        (vec![-1., 1.], vec![1.]),
        (vec![0., 11.], vec![1.]),
        (vec![0., 1.], vec![-1.]),
        (vec![0., 1.], vec![11.]),
    ] {
        assert!(
            HistogramChart::new(&edges, &counts, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err(),
            "{edges:?} {counts:?}"
        );
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for edges in [[bad, 2., 3.], [1., bad, 3.], [1., 2., bad]] {
            assert!(
                HistogramChart::new(&edges, &[1., 2.], 0.0..10.0, 0.0..10.0)
                    .render()
                    .is_err()
            );
        }
        assert!(
            HistogramChart::new(&[0., 1.], &[bad], 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
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
            HistogramChart::new(&[0., 1.], &[1.], range.clone(), 0.0..10.0)
                .render()
                .is_err()
        );
        assert!(
            HistogramChart::new(&[0., 1.], &[1.], 0.0..10.0, range)
                .render()
                .is_err()
        );
    }
    assert!(
        HistogramChart::new(&[0., 1.], &[2.], 0.0..10.0, 1.0..10.0)
            .render()
            .is_err()
    );
    assert!(
        HistogramChart::new(&[0., 1.], &[0.], 0.0..10.0, -10.0..-1.0)
            .render()
            .is_err()
    );
}

#[test]
fn zero_bins_remain_in_axis_semantics_but_all_widths_and_nonzero_heights_must_resolve() {
    let empty = HistogramChart::new(&[-10., -8., 10.], &[0., 0.], -10.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    let es = elements(&empty);
    assert!(!es.iter().any(|e| e["backgroundColor"] == "#1971c2"));
    assert!(es.iter().any(|e| e["text"] == "-10"));
    assert!(es.iter().any(|e| e["text"] == "10"));
    for (edges, counts) in [
        (vec![0., 0.000001, 10.], vec![1., 2.]),
        (vec![0., 0.000001, 10.], vec![0., 2.]),
        (vec![0., 5., 10.], vec![1., 0.000001]),
    ] {
        assert!(
            HistogramChart::new(&edges, &counts, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
}

#[test]
fn signed_value_bounds_draw_an_interior_zero_baseline_and_fractional_counts() {
    let scene = HistogramChart::new(&[0., 10.], &[2.5], 0.0..10.0, -5.0..5.0)
        .render()
        .unwrap();
    let es = elements(&scene);
    let bin = es
        .iter()
        .find(|e| e["backgroundColor"] == "#1971c2")
        .unwrap();
    assert_eq!(bin["y"], 126.);
    assert_eq!(bin["height"], 62.);
    assert!(es.iter().any(|e| e["type"] == "line"
        && e["x"] == 112.
        && e["y"] == 188.
        && e["width"] == 503.
        && e["height"] == 0.));
}

#[test]
fn measured_labels_styles_and_composed_histograms_preserve_native_geometry_and_identity() {
    use excaliplot::{BorderStyle, FillStyle, SketchStyle, TickFormat};
    let chart = || {
        HistogramChart::new(&[0., 0.2, 0.6, 1.], &[0.4, 0., 0.4], 0.0..1.0, 0.0..1.0)
            .labels("Supplied frequencies", "Interval", "Count")
            .size((800, 440))
            .color((224, 49, 49))
            .opacity(0.255)
            .sketch(SketchStyle::new(1, FillStyle::Hachure).unwrap())
            .x_tick_format(TickFormat::Decimal { places: 1 })
            .y_tick_format(TickFormat::Scientific { places: 1 })
            .tick_density(3, 3)
    };
    let original = elements(&chart().render().unwrap());
    assert!(original.iter().any(|e| e["text"] == "1.0"));
    assert!(original.iter().any(|e| e["text"] == "1.0e0"));
    let bins: Vec<_> = original
        .iter()
        .filter(|e| e["backgroundColor"] == "#e03131")
        .collect();
    assert_eq!(bins.len(), 2);
    for bin in &bins {
        assert_eq!(bin["opacity"], 26);
        assert_eq!(bin["roughness"], 1);
        assert_eq!(bin["fillStyle"], "hachure");
    }
    let mut composed = Scene::new();
    for offset in [(20., 30.), (920., 30.)] {
        let mut panel = chart().render().unwrap();
        panel.add_border(12., BorderStyle::default()).unwrap();
        panel.add_frame(16., Some("Histogram")).unwrap();
        panel.place_at(offset).unwrap();
        composed.append(panel).unwrap();
    }
    let es = elements(&composed);
    assert_eq!(es.len(), 2 * (original.len() + 2));
    let ids: std::collections::HashSet<_> = es.iter().map(|e| &e["id"]).collect();
    assert_eq!(ids.len(), es.len());
    let copies: Vec<_> = es
        .iter()
        .filter(|e| e["backgroundColor"] == "#e03131")
        .collect();
    assert_eq!(copies.len(), 4);
    for (copy, source) in copies.iter().zip(bins.iter().cycle()) {
        for field in ["width", "height", "opacity", "fillStyle", "roughness"] {
            assert_eq!(copy[field], source[field]);
        }
        assert!(!copy["frameId"].is_null());
    }
    assert_ne!(copies[0]["frameId"], copies[2]["frameId"]);
    assert!(
        copies[0]["groupIds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| !copies[2]["groupIds"].as_array().unwrap().contains(g))
    );
    assert_eq!(
        copies[2]["x"].as_f64().unwrap() - copies[0]["x"].as_f64().unwrap(),
        900.
    );
    let library: Value = serde_json::from_slice(&composed.to_library_bytes().unwrap()).unwrap();
    assert_eq!(
        library["libraryItems"][0]["elements"],
        serde_json::json!(es)
    );
}

#[test]
fn invalid_styles_and_unresolved_measured_labels_return_errors() {
    use excaliplot::TickFormat;
    let chart = || HistogramChart::new(&[0., 0.5, 1.], &[0.4, 0.8], 0.0..1.0, 0.0..1.0);
    for opacity in [0., 0.004, -1., 1.1, f64::NAN, f64::INFINITY] {
        assert!(chart().opacity(opacity).render().is_err());
    }
    for size in [(399, 400), (640, 299), (16385, 400), (640, 16385)] {
        assert!(chart().size(size).render().is_err());
    }
    for (x, y) in [(1, 6), (6, 21)] {
        assert!(chart().tick_density(x, y).render().is_err());
    }
    for format in [
        TickFormat::Decimal { places: 10 },
        TickFormat::Decimal { places: 0 },
    ] {
        assert!(chart().x_tick_format(format).render().is_err());
        assert!(chart().y_tick_format(format).render().is_err());
    }
    for title in [
        "unsupported β".to_owned(),
        "two\nlines".to_owned(),
        "W".repeat(100),
    ] {
        assert!(chart().labels(&title, "X", "Count").render().is_err());
    }
    assert!(
        chart()
            .size((400, 300))
            .tick_density(20, 20)
            .render()
            .is_err()
    );
}

#[test]
fn frequency_demo_emits_all_nonzero_bins_and_protects_editor_work() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("histogram.excalidraw");
    let run = |overwrite| {
        let mut command = std::process::Command::new(env!("CARGO"));
        command.args(["run", "--locked", "--quiet", "--example", "histogram", "--"]);
        if overwrite {
            command.arg("--overwrite");
        }
        command.arg(&destination).output().unwrap()
    };
    let output = run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&std::fs::read(&destination).unwrap()).unwrap();
    let es = doc["elements"].as_array().unwrap();
    assert_eq!(
        es.iter()
            .filter(|e| e["backgroundColor"] == "#1971c2")
            .count(),
        5
    );
    assert!(es.iter().any(|e| e["text"] == "Unequal-bin frequencies"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("vertices:"));
    std::fs::write(&destination, b"editor work").unwrap();
    assert!(!run(false).status.success());
    assert_eq!(std::fs::read(&destination).unwrap(), b"editor work");
    assert!(run(true).status.success());
    assert!(serde_json::from_slice::<Value>(&std::fs::read(destination).unwrap()).is_ok());
}
