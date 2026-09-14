use excaliplot::{FillStyle, LineChart, NamedSeries, Scene, SketchStyle, StrokeStyle};
use serde_json::Value;

#[test]
fn comparison_and_bounded_probes_are_native_and_protect_existing_directories() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("dashes");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args([
                "run",
                "--locked",
                "--quiet",
                "--example",
                "series_dashes",
                "--",
            ])
            .arg(&destination)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(destination.join("generation.json")).unwrap())
            .unwrap();
    assert_eq!(report.as_array().unwrap().len(), 15);
    for fixture in report.as_array().unwrap() {
        let doc: Value = serde_json::from_slice(
            &std::fs::read(
                destination.join(format!("{}.excalidraw", fixture["name"].as_str().unwrap())),
            )
            .unwrap(),
        )
        .unwrap();
        let paths: Vec<_> = doc["elements"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["points"].as_array().is_some_and(|ps| ps.len() > 2))
            .collect();
        if fixture["name"].as_str().unwrap().starts_with("comparison") {
            assert_eq!(paths.len(), 3);
            for (path, style) in paths.iter().zip(["solid", "dashed", "dotted"]) {
                assert_eq!(path["strokeStyle"], style);
                assert_eq!(path["strokeColor"], "#1971c2");
                assert_eq!(path["points"].as_array().unwrap().len(), 4);
            }
        } else {
            assert_eq!(paths.len(), 1);
            assert_eq!(
                paths[0]["points"].as_array().unwrap().len(),
                fixture["count"].as_u64().unwrap() as usize
            );
        }
        assert!(fixture["calls"]["draw_pixel"].is_null());
        assert!(fixture["calls"]["blit_bitmap"].is_null());
    }
    let protected = destination.join("comparison-0.excalidraw");
    std::fs::write(&protected, b"editor work").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(protected).unwrap(), b"editor work");
}

fn elements(scene: Scene) -> Vec<Value> {
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn non_line_helpers_reject_explicit_patterns_instead_of_ignoring_them() {
    use excaliplot::{AreaChart, ScatterChart};
    for style in [StrokeStyle::Solid, StrokeStyle::Dashed, StrokeStyle::Dotted] {
        let series =
            [NamedSeries::new("S", &[(1., 2.), (9., 8.)], (25, 113, 194)).stroke_style(style)];
        assert!(
            AreaChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
        assert!(
            ScatterChart::from_series(&series, 0.0..10.0, 0.0..10.0)
                .render()
                .is_err()
        );
    }
}

#[test]
fn path_scopes_snapshot_restore_and_isolate_even_on_error_or_unwind() {
    use excaliplot::ExcalidrawBackend;
    use plotters::prelude::*;
    let mut scene = Scene::new();
    let mut independent = Scene::new();
    let options = scene.drawing_options();
    let group = options.new_group();
    {
        let root = ExcalidrawBackend::new(&mut scene, (200, 100))
            .unwrap()
            .into_drawing_area();
        let other = ExcalidrawBackend::new(&mut independent, (200, 100))
            .unwrap()
            .into_drawing_area();
        let (left, right) = root.split_horizontally(100);
        let path = PathElement::new(
            [(10, 10), (30, 20), (50, 10)],
            BLUE.mix(0.5).stroke_width(6),
        );
        options.with_stroke_style(StrokeStyle::Dashed, || {
            group.scope(|| left.draw(&path).unwrap());
            let result: Result<(), &str> = options.with_stroke_style(StrokeStyle::Dotted, || {
                right.draw(&path).unwrap();
                left.draw(&Circle::new((50, 50), 4, BLUE)).unwrap();
                left.draw(&Polygon::new([(10, 10), (20, 30), (30, 10)], BLUE.filled()))
                    .unwrap();
                other.draw(&path).unwrap();
                Err("caller error")
            });
            assert!(result.is_err());
            group.scope(|| right.draw(&path).unwrap());
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            options.with_stroke_style(StrokeStyle::Dotted, || {
                left.draw(&path).unwrap();
                panic!("caller unwind");
            });
        }));
        assert!(result.is_err());
        left.draw(&path).unwrap();
    }
    let es = elements(scene);
    assert_eq!(
        es.iter()
            .map(|e| e["strokeStyle"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "dashed", "dotted", "solid", "solid", "dashed", "dotted", "solid"
        ]
    );
    assert_eq!(es[0]["groupIds"], es[4]["groupIds"]);
    assert_eq!(es[6]["groupIds"], serde_json::json!([]));
    for i in [0, 1, 4, 5, 6] {
        assert_eq!(es[i]["strokeWidth"], 6);
        assert_eq!(es[i]["opacity"], 50);
        assert_eq!(
            es[i]["points"],
            serde_json::json!([[0., 0.], [20., 10.], [40., 0.]])
        );
    }
    assert_eq!(elements(independent)[0]["strokeStyle"], "solid");
}

#[test]
fn native_patterns_keep_original_paths_and_match_delayed_same_color_legends() {
    let points = [(1., 2.), (1., 2.), (5., 8.), (9., 4.)];
    for roughness in 0..=2 {
        let base = NamedSeries::new("Same", &points, (25, 113, 194))
            .line_width(4)
            .opacity(0.255)
            .marker(4, false);
        let render = |series: &[NamedSeries<'_>]| {
            elements(
                LineChart::from_series(series, 0.0..10.0, 0.0..10.0)
                    .sketch(SketchStyle::new(roughness, FillStyle::Solid).unwrap())
                    .render()
                    .unwrap(),
            )
        };
        let plain = render(&[base.clone(), base.clone(), base.clone()]);
        let styled = render(&[
            base.clone().stroke_style(StrokeStyle::Solid),
            base.clone().stroke_style(StrokeStyle::Dashed),
            base.stroke_style(StrokeStyle::Dotted),
        ]);
        let labels: Vec<_> = styled.iter().filter(|e| e["text"] == "Same").collect();
        assert_eq!(labels.len(), 3);
        for (i, label) in labels.iter().enumerate() {
            for other in &labels[..i] {
                assert_ne!(label["groupIds"][0], other["groupIds"][0]);
                assert_eq!(label["groupIds"][1], other["groupIds"][1]);
            }
            let paths: Vec<_> = styled
                .iter()
                .filter(|e| e["type"] == "line" && e["groupIds"] == label["groupIds"])
                .collect();
            assert_eq!(paths.len(), 2);
            assert_eq!(paths[0]["points"].as_array().unwrap().len(), 4);
            assert_eq!(paths[0]["points"][0], paths[0]["points"][1]);
            assert_eq!(paths[1]["points"].as_array().unwrap().len(), 2);
            for path in paths {
                assert_eq!(path["strokeStyle"], ["solid", "dashed", "dotted"][i]);
                assert_eq!(path["strokeWidth"], 4);
                assert_eq!(path["opacity"], 26);
            }
        }
        assert_eq!(plain.len(), styled.len());
        for (a, b) in plain.iter().zip(&styled) {
            for field in [
                "type",
                "points",
                "x",
                "y",
                "width",
                "height",
                "strokeWidth",
                "opacity",
                "roughness",
                "strokeColor",
                "backgroundColor",
                "text",
            ] {
                assert_eq!(a[field], b[field], "{field}");
            }
            if b["type"] != "line" || b["groupIds"].as_array().unwrap().len() != 2 {
                assert_eq!(b["strokeStyle"], "solid");
            }
        }
    }
}
