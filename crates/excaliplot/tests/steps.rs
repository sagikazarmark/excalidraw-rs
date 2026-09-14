use excaliplot::{Scene, StepChart};
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

fn path(es: &[Value]) -> &Value {
    es.iter()
        .find(|e| e["type"] == "line" && e["strokeColor"] == "#1971c2")
        .unwrap()
}

fn points(e: &Value) -> Vec<(f64, f64)> {
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
fn post_steps_hold_previous_value_then_jump_and_extend_only_to_right_bound() {
    let scene = StepChart::new(&[(2., 2.), (4., 8.), (6., 4.)], 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    let es = elements(&scene);
    let line = path(&es);
    // Default plot is (112,311)..(615,64): no left extrapolation, unequal holds.
    assert_eq!(
        points(line),
        [
            (212., 262.),
            (313., 262.),
            (313., 114.),
            (413., 114.),
            (413., 213.),
            (615., 213.)
        ]
    );
    assert!(line["roundness"].is_null());
    assert_eq!(line["strokeWidth"], 3);
    assert_eq!(es.last().unwrap(), line);
    assert!(es.iter().all(|e| e["groupIds"] == line["groupIds"]));
    assert_eq!(line["groupIds"].as_array().unwrap().len(), 1);
    assert!(
        es.iter()
            .any(|e| e["text"] == "Step plot" && e["fontFamily"] == 5)
    );
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}

#[test]
fn ecdf_sorts_samples_aggregates_ties_and_draws_cumulative_quarters_with_both_tails() {
    let samples = [6., 2., 4., 2.];
    let scene = StepChart::ecdf(&samples, 0.0..10.0)
        .unwrap()
        .render()
        .unwrap();
    // F(2)=2/4, F(4)=3/4, F(6)=4/4; no intermediate jump for the tie at 2.
    assert_eq!(
        points(path(&elements(&scene))),
        [
            (112., 311.),
            (212., 311.),
            (212., 188.),
            (313., 188.),
            (313., 126.),
            (413., 126.),
            (413., 64.),
            (615., 64.),
        ]
    );
    assert_eq!(samples, [6., 2., 4., 2.]);
    let sorted = StepChart::ecdf(&[2., 2., 4., 6.], 0.0..10.0)
        .unwrap()
        .render()
        .unwrap();
    assert_eq!(
        points(path(&elements(&scene))),
        points(path(&elements(&sorted))),
        "input order must not change duplicate jumps or staircase geometry"
    );
    let thirds = StepChart::ecdf(&[6., 2., 4.], 0.0..10.0)
        .unwrap()
        .render()
        .unwrap();
    // 1/3 and 2/3 map to 82 and 164 scene units above the baseline.
    assert_eq!(
        points(path(&elements(&thirds))),
        [
            (112., 311.),
            (212., 311.),
            (212., 229.),
            (313., 229.),
            (313., 147.),
            (413., 147.),
            (413., 64.),
            (615., 64.),
        ]
    );
}

#[test]
fn singleton_ecdf_all_equal_and_signed_zero_each_make_one_jump() {
    for samples in [vec![0.], vec![0., 0., 0.], vec![0., -0., 0., -0.]] {
        let scene = StepChart::ecdf(&samples, -5.0..5.0)
            .unwrap()
            .render()
            .unwrap();
        assert_eq!(
            points(path(&elements(&scene))),
            [(112., 311.), (363., 311.), (363., 64.), (615., 64.),]
        );
    }
    let constant = StepChart::new(&[(0., 0.)], 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    assert_eq!(
        points(path(&elements(&constant))),
        [(112., 311.), (615., 311.)]
    );
    // Equal Y retains original intermediate X vertices, with no zero-length jump.
    let flat = StepChart::new(&[(0., 10.), (2., 10.), (6., 10.)], 0.0..10.0, 0.0..10.0)
        .render()
        .unwrap();
    assert_eq!(
        points(path(&elements(&flat))),
        [(112., 64.), (212., 64.), (413., 64.), (615., 64.)]
    );
}

#[test]
fn steps_reject_invalid_observations_order_bounds_and_unresolved_holds_or_jumps() {
    for ps in [
        vec![],
        vec![(2., 2.), (1., 3.)],
        vec![(2., 2.), (2., 3.)],
        vec![(2., 2.), (2., 2.)],
        vec![(-1., 2.)],
        vec![(10., 2.)],
        vec![(11., 2.)],
        vec![(1., -1.)],
        vec![(1., 11.)],
        vec![(0., 2.), (0.000001, 3.)],
        vec![(9.999999, 2.)],
        vec![(1., 2.), (2., 2.000001)],
    ] {
        assert!(
            StepChart::new(&ps, 0.0..10.0, 0.0..10.0).render().is_err(),
            "{ps:?}"
        );
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for ps in [
            [(bad, 2.), (2., 3.)],
            [(1., 2.), (bad, 3.)],
            [(1., bad), (2., 3.)],
            [(1., 2.), (2., bad)],
        ] {
            assert!(StepChart::new(&ps, 0.0..10.0, 0.0..10.0).render().is_err());
        }
    }
    for bounds in [
        0.0..0.0,
        10.0..0.0,
        f64::NAN..10.,
        0.0..f64::INFINITY,
        -1e10..10.,
        0.0..1e-8,
    ] {
        assert!(
            StepChart::new(&[(1., 1.)], bounds.clone(), 0.0..10.0)
                .render()
                .is_err()
        );
        assert!(
            StepChart::new(&[(1., 1.)], 0.0..10.0, bounds.clone())
                .render()
                .is_err()
        );
        assert!(StepChart::ecdf(&[1.], bounds).is_err());
    }
    // Ordinary lines still allow duplicate X in caller order; they never sort.
    assert!(
        excaliplot::LineChart::new(&[(2., 2.), (2., 3.)], 0.0..10.0, 0.0..10.0)
            .render()
            .is_ok()
    );
    assert!(
        excaliplot::LineChart::new(&[(2., 2.), (1., 3.)], 0.0..10.0, 0.0..10.0)
            .render()
            .is_err()
    );
}

#[test]
fn ecdf_requires_finite_nonempty_samples_and_visible_zero_one_tails() {
    for samples in [
        vec![],
        vec![f64::NAN],
        vec![2., f64::INFINITY],
        vec![f64::NEG_INFINITY, 2.],
        vec![0., 2.],
        vec![2., 10.],
        vec![-1.],
        vec![11.],
    ] {
        assert!(StepChart::ecdf(&samples, 0.0..10.0).is_err(), "{samples:?}");
    }
    for samples in [vec![0.000001], vec![9.999999], vec![2., 2.000001]] {
        assert!(
            StepChart::ecdf(&samples, 0.0..10.0)
                .unwrap()
                .render()
                .is_err()
        );
    }
    // A 1/1000 jump cannot survive this canvas, even though all three Xs can.
    let mut samples = vec![6.; 999];
    samples.push(2.);
    assert!(
        StepChart::ecdf(&samples, 0.0..10.0)
            .unwrap()
            .render()
            .is_err()
    );
}

#[test]
fn measured_styles_and_scene_composition_preserve_sharp_step_vertices() {
    use excaliplot::{BorderStyle, FillStyle, SketchStyle, TickFormat};
    let chart = || {
        StepChart::ecdf(&[6., 2., 4., 2.], 0.0..10.0)
            .unwrap()
            .labels("Measured ECDF", "Measurement", "Probability")
            .size((800, 440))
            .color((224, 49, 49))
            .line_width(5)
            .sketch(SketchStyle::new(1, FillStyle::Solid).unwrap())
            .tick_density(3, 3)
            .x_tick_format(TickFormat::Decimal { places: 1 })
            .y_tick_format(TickFormat::FractionPercent { places: 0 })
    };
    let original = elements(&chart().render().unwrap());
    let source = original
        .iter()
        .find(|e| e["strokeColor"] == "#e03131")
        .unwrap();
    assert_eq!(source["roughness"], 1);
    assert_eq!(source["strokeWidth"], 5);
    assert!(source["roundness"].is_null());
    assert!(original.iter().any(|e| e["text"] == "100%"));
    assert!(original.iter().any(|e| e["text"] == "10.0"));
    let mut composed = Scene::new();
    for offset in [(20., 30.), (920., 30.)] {
        let mut panel = chart().render().unwrap();
        panel.add_border(12., BorderStyle::default()).unwrap();
        panel.add_frame(16., Some("Steps")).unwrap();
        panel.place_at(offset).unwrap();
        composed.append(panel).unwrap();
    }
    let es = elements(&composed);
    let copies: Vec<_> = es
        .iter()
        .filter(|e| e["strokeColor"] == "#e03131")
        .collect();
    assert_eq!(copies.len(), 2);
    for e in &copies {
        for field in [
            "points",
            "width",
            "height",
            "roundness",
            "strokeWidth",
            "roughness",
        ] {
            assert_eq!(e[field], source[field]);
        }
        assert!(!e["frameId"].is_null());
    }
    assert_ne!(copies[0]["frameId"], copies[1]["frameId"]);
    assert_ne!(copies[0]["groupIds"], copies[1]["groupIds"]);
    assert_eq!(
        copies[1]["x"].as_f64().unwrap() - copies[0]["x"].as_f64().unwrap(),
        900.
    );
    assert_eq!(
        es.iter()
            .map(|e| &e["id"])
            .collect::<std::collections::HashSet<_>>()
            .len(),
        es.len()
    );
    let library: Value = serde_json::from_slice(&composed.to_library_bytes().unwrap()).unwrap();
    assert_eq!(
        library["libraryItems"][0]["elements"],
        serde_json::json!(es)
    );
}

#[test]
fn invalid_styles_and_measured_layout_fail_without_a_scene() {
    use excaliplot::TickFormat;
    let chart = || StepChart::ecdf(&[0.2, 0.4, 0.6], 0.0..1.0).unwrap();
    for width in [0, 11, 20, 21] {
        // >10 breaches the halo at the right endpoint.
        assert!(chart().line_width(width).render().is_err());
    }
    for size in [(399, 400), (640, 299), (16385, 400), (640, 16385)] {
        assert!(chart().size(size).render().is_err());
    }
    for ticks in [(1, 6), (6, 21)] {
        assert!(chart().tick_density(ticks.0, ticks.1).render().is_err());
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
        assert!(chart().labels(&title, "X", "Probability").render().is_err());
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
fn step_ecdf_demo_is_native_and_refuses_to_overwrite_editor_work() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("steps.excalidraw");
    let run = |overwrite| {
        let mut command = std::process::Command::new(env!("CARGO"));
        command.args(["run", "--locked", "--quiet", "--example", "steps", "--"]);
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
    let paths: Vec<_> = es
        .iter()
        .filter(|e| e["strokeColor"] == "#1971c2")
        .collect();
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0]["points"].as_array().unwrap().len(), 8);
    assert_eq!(paths[1]["points"].as_array().unwrap().len(), 10);
    assert!(
        paths
            .iter()
            .all(|e| e["type"] == "line" && e["roundness"].is_null())
    );
    assert_ne!(paths[0]["groupIds"], paths[1]["groupIds"]);
    assert!(String::from_utf8_lossy(&output.stdout).contains("vertices:"));
    std::fs::write(&destination, b"editor work").unwrap();
    assert!(!run(false).status.success());
    assert_eq!(std::fs::read(&destination).unwrap(), b"editor work");
    assert!(run(true).status.success());
    assert!(serde_json::from_slice::<Value>(&std::fs::read(destination).unwrap()).is_ok());
}
