use plotters::prelude::*;
use plotters_excalidraw::{ExcalidrawBackend, FillStyle, Scene, SketchStyle};
use serde_json::Value;

#[test]
fn nested_scopes_snapshot_style_and_groups_across_interleaved_areas_and_errors() {
    let mut scene = Scene::with_namespace("scopes");
    let options = scene.drawing_options();
    let chart = options.new_group();
    let series = options.new_group();
    {
        let root = ExcalidrawBackend::new(&mut scene, (200, 100))
            .unwrap()
            .into_drawing_area();
        let (left, right) = root.split_horizontally(100);
        chart.scope(|| {
            left.draw(&PathElement::new([(0, 0), (10, 10)], BLUE.stroke_width(3)))
                .unwrap();
            let result: Result<(), &str> =
                options.with_style(SketchStyle::new(2, FillStyle::Hachure).unwrap(), || {
                    series.scope(|| {
                        right
                            .draw(&Rectangle::new([(0, 0), (20, 20)], RED.mix(0.5).filled()))
                            .unwrap();
                        left.draw(&PathElement::new([(20, 20), (30, 30)], BLUE))
                            .unwrap();
                        Err("caller error")
                    })
                });
            assert_eq!(result, Err("caller error"));
            right
                .draw(&PathElement::new([(30, 30), (40, 40)], BLACK))
                .unwrap();
        });
        left.draw(&PathElement::new([(40, 40), (50, 50)], BLACK))
            .unwrap();
    }
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let e = doc["elements"].as_array().unwrap();
    let outer = e[0]["groupIds"][0].clone();
    assert_eq!(e[0]["roughness"], 0);
    assert_eq!(e[0]["strokeWidth"], 3);
    assert_eq!(e[1]["roughness"], 2);
    assert_eq!(e[1]["fillStyle"], "hachure");
    assert_eq!(e[1]["opacity"], 50);
    assert_eq!(e[1]["strokeColor"], "transparent");
    assert_eq!(e[1]["backgroundColor"], "#ff0000");
    assert_eq!(e[1]["groupIds"].as_array().unwrap().len(), 2);
    assert_eq!(e[1]["groupIds"][1], outer);
    assert_ne!(e[1]["groupIds"][0], outer);
    assert_eq!(e[1]["groupIds"], e[2]["groupIds"]);
    assert_eq!(e[3]["groupIds"], e[0]["groupIds"]);
    assert_eq!(e[3]["roughness"], 0);
    assert_eq!(e[3]["fillStyle"], "solid");
    assert_eq!(e[4]["groupIds"], serde_json::json!([]));
    assert_eq!(e[4]["roughness"], 0);
}

#[test]
fn scopes_restore_on_unwind_and_group_reentry_reuses_identity_without_duplicates() {
    use plotters_backend::DrawingBackend;
    let mut scene = Scene::with_namespace("unwind");
    let options = scene.drawing_options();
    let group = options.new_group();
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (100, 100)).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            group.scope(|| {
                options.with_style(SketchStyle::new(1, FillStyle::CrossHatch).unwrap(), || {
                    group.scope(|| backend.draw_line((0, 0), (10, 10), &BLACK).unwrap());
                    panic!("abort caller draw scope");
                })
            });
        }));
        assert!(result.is_err());
        backend.draw_line((10, 10), (20, 20), &BLACK).unwrap();
        group.scope(|| backend.draw_line((20, 20), (30, 30), &BLACK).unwrap());
    }
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let e = &doc["elements"];
    assert_eq!(e[0]["groupIds"].as_array().unwrap().len(), 1);
    assert_eq!(e[0]["groupIds"], e[2]["groupIds"]);
    assert_eq!(e[1]["groupIds"], serde_json::json!([]));
    assert_eq!(e[1]["roughness"], 0);
    assert_eq!(e[2]["roughness"], 0);
    assert!(SketchStyle::new(3, FillStyle::Solid).is_err());
}

#[test]
fn independent_scenes_do_not_share_scopes_and_backend_errors_still_restore_state() {
    use plotters_backend::DrawingBackend;
    let mut failed = Scene::new();
    let mut independent = Scene::new();
    let options = failed.drawing_options();
    let group = options.new_group();
    {
        let mut a = ExcalidrawBackend::new(&mut failed, (100, 100)).unwrap();
        let mut b = ExcalidrawBackend::new(&mut independent, (100, 100)).unwrap();
        let result = group.scope(|| {
            options.with_style(SketchStyle::new(2, FillStyle::Hachure).unwrap(), || {
                b.draw_line((0, 0), (10, 10), &BLACK).unwrap();
                a.blit_bitmap((10, 10), (1, 1), &[0, 0, 0])
            })
        });
        assert!(result.is_err());
    }
    assert!(failed.to_bytes().is_err());
    let doc: Value = serde_json::from_slice(&independent.to_bytes().unwrap()).unwrap();
    assert_eq!(doc["elements"][0]["groupIds"], serde_json::json!([]));
    assert_eq!(doc["elements"][0]["roughness"], 0);
    // Scope cleanup itself does not clear the backend's failed-scene flag.
    assert!(failed.to_bytes().is_err());
}
