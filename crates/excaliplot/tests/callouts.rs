use excaliplot::{ArrowHead, ArrowStyle, Scene};
use serde_json::{Value, json};

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

fn assert_leader_clears_label(arrow: &Value, label: &Value) {
    // Intersect the open text rectangle with the arrow segment. Touching an
    // edge is allowed, but no part of the leader may pass through the text.
    let mut enter: f64 = 0.;
    let mut exit: f64 = 1.;
    for (axis, extent, coordinate) in [("x", "width", 0), ("y", "height", 1)] {
        let start = arrow[axis].as_f64().unwrap();
        let delta = arrow["points"][1][coordinate].as_f64().unwrap();
        let min = label[axis].as_f64().unwrap();
        let max = min + label[extent].as_f64().unwrap();
        if delta == 0. {
            if start <= min || start >= max {
                return;
            }
        } else {
            let a = (min - start) / delta;
            let b = (max - start) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    assert!(
        enter >= exit - 1e-9,
        "leader crosses {} between t={enter} and t={exit}",
        label["text"]
    );
}

#[test]
fn recovery_leader_does_not_cross_its_label() {
    use excaliplot::{Callout, LineChart};
    let es = elements(
        &LineChart::new(&[(0., 0.), (9., 4.)], 0.0..10.0, 0.0..10.0)
            .callout(Callout::new((9., 4.), "Recovery", (-150., 45.)))
            .render()
            .unwrap(),
    );
    let index = es.iter().position(|e| e["type"] == "arrow").unwrap();
    assert_leader_clears_label(&es[index], &es[index + 1]);
}

#[test]
fn callout_leaders_clear_labels_in_every_direction() {
    use excaliplot::{Callout, LineChart};
    for offset in [
        (-150., -60.),
        (-25., -60.),
        (50., -60.),
        (-150., -10.),
        (50., -10.),
        (-150., 45.),
        (-25., 45.),
        (50., 45.),
    ] {
        let es = elements(
            &LineChart::new(&[(0., 0.), (10., 10.)], 0.0..10.0, 0.0..10.0)
                .callout(Callout::new((5., 5.), "Recovery", offset))
                .render()
                .unwrap(),
        );
        let index = es.iter().position(|e| e["type"] == "arrow").unwrap();
        assert_leader_clears_label(&es[index], &es[index + 1]);
    }
}

#[test]
fn scene_arrow_is_native_unbound_and_bounds_include_the_head() {
    for (head, name, half_height) in [
        (ArrowHead::Arrow, "arrow", 9.550503583141717),
        (ArrowHead::Triangle, "triangle", 7.339273926110492),
    ] {
        let mut scene = Scene::new();
        scene
            .add_arrow(
                (10., 20.),
                (110., 20.),
                ArrowStyle {
                    head,
                    ..ArrowStyle::default()
                },
            )
            .unwrap();
        let es = elements(&scene);
        let arrow = &es[0];
        assert_eq!(arrow["type"], "arrow");
        assert_eq!(arrow["points"], json!([[0., 0.], [100., 0.]]));
        assert_eq!(arrow["endArrowhead"], name);
        for field in [
            "startBinding",
            "endBinding",
            "startArrowhead",
            "lastCommittedPoint",
            "roundness",
        ] {
            assert!(arrow[field].is_null());
        }
        assert_eq!(arrow["elbowed"], false);
        assert_eq!(arrow["boundElements"], json!([]));
        let bounds = scene.bounds().unwrap().unwrap();
        assert_eq!((bounds.x, bounds.width), (9., 102.));
        assert!((bounds.y - (20. - half_height)).abs() < 1e-9);
        assert!((bounds.height - half_height * 2.).abs() < 1e-9);
        assert_eq!(scene.diagnostics().elements["arrow"], 1);
        assert_eq!(scene.diagnostics().vertices, 2);
    }
}

#[test]
fn mapped_callout_preserves_data_and_paints_grouped_arrow_then_measured_note_before_legend() {
    use excaliplot::{Callout, LineChart, NamedSeries, ReferenceRule};
    let series = [NamedSeries::new(
        "Data",
        &[(1., 1.), (10., 10.), (100., 100.)],
        (25, 113, 194),
    )];
    let chart = || {
        LineChart::from_series(&series, 1.0..100.0, 1.0..100.0)
            .x_scale(excaliplot::AxisScale::Log10)
            .y_scale(excaliplot::AxisScale::Log10)
    };
    let plain = elements(&chart().render().unwrap());
    let es = elements(
        &chart()
            .reference_rule(ReferenceRule::horizontal(10., (224, 49, 49)))
            .callout(Callout::new((10., 10.), "Outlier", (30.5, -50.25)))
            .render()
            .unwrap(),
    );
    let data = es
        .iter()
        .find(|e| e["points"].as_array().is_some_and(|p| p.len() == 3))
        .unwrap();
    let anchor = (
        data["x"].as_f64().unwrap() + data["points"][1][0].as_f64().unwrap(),
        data["y"].as_f64().unwrap() + data["points"][1][1].as_f64().unwrap(),
    );
    let index = es.iter().position(|e| e["type"] == "arrow").unwrap();
    let (arrow, label) = (&es[index], &es[index + 1]);
    assert_eq!(es[index - 1]["strokeColor"], "#e03131");
    assert_eq!(label["text"], "Outlier");
    assert_eq!(label["x"], anchor.0 + 30.5);
    assert_eq!(label["y"], anchor.1 - 50.25);
    // For a label above and right of the anchor, its nearest point is bottom-left.
    assert_eq!(arrow["x"], label["x"]);
    assert_eq!(arrow["y"], label["y"].as_f64().unwrap() + 25.);
    assert_eq!(
        arrow["x"].as_f64().unwrap() + arrow["points"][1][0].as_f64().unwrap(),
        anchor.0
    );
    assert_eq!(
        arrow["y"].as_f64().unwrap() + arrow["points"][1][1].as_f64().unwrap(),
        anchor.1
    );
    assert_eq!(arrow["groupIds"], label["groupIds"]);
    assert_eq!(arrow["groupIds"][1], data["groupIds"][1]);
    assert_ne!(arrow["groupIds"][0], data["groupIds"][0]);
    assert_eq!(es.last().unwrap()["text"], "Data");
    let remaining: Vec<_> = es
        .iter()
        .enumerate()
        .filter(|(i, _)| !((index - 1)..=(index + 1)).contains(i))
        .map(|(_, e)| e)
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
            "strokeColor",
            "opacity",
        ] {
            assert_eq!(a[field], b[field], "{field}");
        }
    }
}

#[test]
fn calendar_callouts_follow_elapsed_mapping_on_all_annotation_helpers() {
    use chrono::{NaiveDate, TimeZone, Utc};
    use excaliplot::{
        AreaChart, Callout, DateLineChart, DateScatterChart, ScatterChart, UtcLineChart,
        UtcScatterChart,
    };
    let day = |d| NaiveDate::from_ymd_opt(2024, 2, d).unwrap();
    let dates = [(day(1), 2.), (day(2), 5.), (day(5), 8.)];
    let time = |s| Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, s).unwrap();
    let times = [(time(0), 2.), (time(10), 5.), (time(40), 8.)];
    let scenes = [
        DateLineChart::new(&dates, day(1)..day(5), 0.0..10.0)
            .callout(Callout::new((day(2), 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
        DateScatterChart::new(&dates, day(1)..day(5), 0.0..10.0)
            .callout(Callout::new((day(2), 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
        UtcLineChart::new(&times, time(0)..time(40), 0.0..10.0)
            .size((1600, 400))
            .tick_density(2, 6)
            .callout(Callout::new((time(10), 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
        UtcScatterChart::new(&times, time(0)..time(40), 0.0..10.0)
            .size((1600, 400))
            .tick_density(2, 6)
            .callout(Callout::new((time(10), 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
        AreaChart::new(&[(1., 2.), (2., 5.), (5., 8.)], 1.0..5.0, 0.0..10.0)
            .callout(Callout::new((2., 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
        ScatterChart::new(&[(1., 2.), (2., 5.), (5., 8.)], 1.0..5.0, 0.0..10.0)
            .callout(Callout::new((2., 5.), "Event", (30., -60.)))
            .render()
            .unwrap(),
    ];
    for scene in scenes {
        let es = elements(&scene);
        let anchor = if let Some(line) = es
            .iter()
            .find(|e| e["type"] == "line" && e["points"].as_array().unwrap().len() == 3)
        {
            (
                line["x"].as_f64().unwrap() + line["points"][1][0].as_f64().unwrap(),
                line["y"].as_f64().unwrap() + line["points"][1][1].as_f64().unwrap(),
            )
        } else {
            let marker = es.iter().filter(|e| e["type"] == "ellipse").nth(1).unwrap();
            (
                marker["x"].as_f64().unwrap() + marker["width"].as_f64().unwrap() / 2.,
                marker["y"].as_f64().unwrap() + marker["height"].as_f64().unwrap() / 2.,
            )
        };
        let arrow = &es[es.len() - 2];
        assert_eq!(
            arrow["x"].as_f64().unwrap() + arrow["points"][1][0].as_f64().unwrap(),
            anchor.0
        );
        assert_eq!(
            arrow["y"].as_f64().unwrap() + arrow["points"][1][1].as_f64().unwrap(),
            anchor.1
        );
    }
}

#[test]
fn invalid_arrows_are_atomic_and_invalid_callouts_publish_no_scene() {
    use excaliplot::{Callout, LineChart};
    let mut scene = Scene::with_namespace("atomic");
    scene.add_note("Keep", (0., 0.), 20.).unwrap();
    let before = scene.to_bytes().unwrap();
    for (start, end, style) in [
        ((0., 0.), (0., 0.), ArrowStyle::default()),
        ((0., 0.), (0.1, 0.), ArrowStyle::default()),
        ((f64::NAN, 0.), (10., 10.), ArrowStyle::default()),
        ((0., 0.), (f64::INFINITY, 10.), ArrowStyle::default()),
        ((-f64::MAX, 0.), (f64::MAX, 0.), ArrowStyle::default()),
        (
            (0., 0.),
            (10., 10.),
            ArrowStyle {
                width: 0,
                ..ArrowStyle::default()
            },
        ),
        (
            (0., 0.),
            (10., 10.),
            ArrowStyle {
                width: 21,
                ..ArrowStyle::default()
            },
        ),
        (
            (0., 0.),
            (10., 10.),
            ArrowStyle {
                opacity: f64::NAN,
                ..ArrowStyle::default()
            },
        ),
        (
            (0., 0.),
            (10., 10.),
            ArrowStyle {
                opacity: 0.004,
                ..ArrowStyle::default()
            },
        ),
        (
            (0., 0.),
            (10., 10.),
            ArrowStyle {
                opacity: 1.1,
                ..ArrowStyle::default()
            },
        ),
    ] {
        assert!(scene.add_arrow(start, end, style).is_err());
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    for callout in [
        Callout::new((f64::NAN, 5.), "Label", (20., -50.)),
        Callout::new((5., f64::INFINITY), "Label", (20., -50.)),
        Callout::new((11., 5.), "Label", (20., -50.)),
        Callout::new((5., -1.), "Label", (20., -50.)),
        Callout::new((5., 5.), "Label", (f64::NAN, 0.)),
        Callout::new((5., 5.), "Label", (0., f64::MAX)),
        Callout::new((5., 5.), "Label", (0., -25.)), // zero-length leader
        Callout::new((5., 5.), "Label", (-10., -10.)), // anchor inside label
        Callout::new((5., 5.), "Label", (-1000., 0.)),
        Callout::new(
            (5., 5.),
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            (20., -50.),
        ),
        Callout::new((5., 5.), "", (20., -50.)),
        Callout::new((5., 5.), "Two\nlines", (20., -50.)),
        Callout::new((5., 5.), "Ω", (20., -50.)),
        Callout::new((5., 5.), "Label", (20., -50.)).font_size(0.),
        Callout::new((5., 5.), "Label", (20., -50.)).font_size(f64::MAX),
        Callout::new((5., 5.), "Label", (20., -50.)).arrow_style(ArrowStyle {
            width: 0,
            ..ArrowStyle::default()
        }),
    ] {
        assert!(
            LineChart::new(&[(0., 0.), (10., 10.)], 0.0..10.0, 0.0..10.0)
                .callout(callout)
                .render()
                .is_err()
        );
    }
    // A failed backend scene cannot be rescued by a scene-level arrow operation.
    assert!(excaliplot::ExcalidrawBackend::new(&mut scene, (0, 0)).is_err());
    assert!(
        scene
            .add_arrow((0., 0.), (10., 10.), ArrowStyle::default())
            .is_err()
    );
}

#[test]
fn reversed_arrow_geometry_scopes_and_heads_survive_wrapping_placement_and_duplicate_composition() {
    use excaliplot::{BorderStyle, StrokeStyle};
    let mut report = Scene::with_namespace("same");
    for x in [0., 300.] {
        let mut panel = Scene::with_namespace("same");
        let options = panel.drawing_options();
        options.new_group().scope(|| {
            options.with_stroke_style(StrokeStyle::Dashed, || {
                panel
                    .add_arrow(
                        (110., 20.),
                        (10., 20.),
                        ArrowStyle {
                            opacity: 0.255,
                            ..ArrowStyle::default()
                        },
                    )
                    .unwrap();
                panel.add_note("AV", (110., -5.), 20.).unwrap();
            })
        });
        panel.add_border(10., BorderStyle::default()).unwrap();
        panel.add_frame(8., Some("Callout")).unwrap();
        panel.place_at((x, 0.)).unwrap();
        assert_eq!(panel.bounds().unwrap().unwrap().x, x);
        report.append(panel).unwrap();
    }
    let es = elements(&report);
    assert_eq!(es.len(), 8);
    for copy in es.chunks(4) {
        assert_eq!(copy[0]["points"], json!([[0., 0.], [-100., 0.]]));
        assert_eq!(copy[0]["strokeStyle"], "dashed");
        assert_eq!(copy[0]["opacity"], 26);
        assert_eq!(copy[0]["groupIds"], copy[1]["groupIds"]);
        assert_eq!(copy[0]["groupIds"][1], copy[2]["groupIds"][0]);
        assert_eq!(copy[0]["frameId"], copy[3]["id"]);
        // Head is above and below the horizontal shaft and stays inside frame.
        assert!(
            copy[0]["y"].as_f64().unwrap() + 9.550503583141717
                < copy[3]["y"].as_f64().unwrap() + copy[3]["height"].as_f64().unwrap()
        );
    }
    assert_ne!(es[0]["groupIds"], es[4]["groupIds"]);
    assert_eq!(
        es.iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    assert_eq!(report.diagnostics().elements["arrow"], 2);
    let library: Value = serde_json::from_slice(&report.to_library_bytes().unwrap()).unwrap();
    assert_eq!(library["libraryItems"][0]["elements"], json!(es));
    let before = report.to_bytes().unwrap();
    assert!(report.translate((f64::INFINITY, 0.)).is_err());
    assert_eq!(before, report.to_bytes().unwrap());
}

#[test]
fn outlier_demo_has_both_native_heads_and_protects_manual_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outlier.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--quiet", "--example", "callouts", "--"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let es = document["elements"].as_array().unwrap();
    let arrows: Vec<_> = es.iter().filter(|e| e["type"] == "arrow").collect();
    assert_eq!(arrows.len(), 2);
    assert_eq!(arrows[0]["endArrowhead"], "arrow");
    assert_eq!(arrows[1]["endArrowhead"], "triangle");
    assert!(es.iter().any(|e| e["text"] == "Outlier"));
    let recovery = es.iter().find(|e| e["text"] == "Recovery").unwrap();
    assert_leader_clears_label(arrows[1], recovery);
    std::fs::write(&path, b"manual edits").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
}

#[test]
fn extreme_finite_arrow_insertion_keeps_bounds_and_composition_usable() {
    let mut scene = Scene::new();
    scene
        .add_arrow((0.9 * f64::MAX, 0.), (f64::MAX, 0.), ArrowStyle::default())
        .unwrap();
    assert!(scene.bounds().unwrap().unwrap().width.is_finite());
    let before = scene.to_bytes().unwrap();
    assert!(scene.translate((f64::MAX, 0.)).is_err());
    assert_eq!(scene.to_bytes().unwrap(), before);
    let mut composed = Scene::new();
    composed.append(scene).unwrap();
    composed.to_library_bytes().unwrap();
}
