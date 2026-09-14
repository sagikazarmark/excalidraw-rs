use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_excalidraw::{BorderStyle, Bounds, ExcalidrawBackend, Scene, StrokeStyle};
use serde_json::Value;

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

fn path_scene() -> Scene {
    let mut scene = Scene::with_namespace("same");
    let group = scene.drawing_options().new_group();
    group.scope(|| {
        ExcalidrawBackend::new(&mut scene, (200, 200))
            .unwrap()
            .draw_path([(80, 50), (20, 90), (110, 10)], &BLUE.stroke_width(4))
            .unwrap();
    });
    scene
}

#[test]
fn placement_uses_complete_path_bounds_and_translation_keeps_local_vertices() {
    let mut scene = path_scene();
    assert_eq!(
        scene.bounds().unwrap(),
        Some(Bounds {
            x: 18.,
            y: 8.,
            width: 94.,
            height: 84.
        })
    );
    let original = elements(&scene);
    scene.place_at((200., -100.)).unwrap();
    assert_eq!(
        scene.bounds().unwrap(),
        Some(Bounds {
            x: 200.,
            y: -100.,
            width: 94.,
            height: 84.
        })
    );
    let placed = elements(&scene);
    assert_eq!(placed[0]["x"], 262.);
    assert_eq!(placed[0]["y"], -58.);
    assert_eq!(placed[0]["points"], original[0]["points"]);
    scene.translate((-10., 30.)).unwrap();
    assert_eq!(scene.bounds().unwrap().unwrap().x, 190.);
    assert_eq!(elements(&scene)[0]["points"], original[0]["points"]);
}

#[test]
fn consuming_composition_remaps_collisions_and_detaches_source_scopes() {
    let mut destination = path_scene();
    let source = path_scene();
    let escaped = source.drawing_options();
    let escaped_group = escaped.new_group();
    let original = elements(&source);
    destination.append(source).unwrap();
    let future_group = destination.drawing_options().new_group();
    escaped_group.scope(|| {
        future_group.scope(|| {
            ExcalidrawBackend::new(&mut destination, (200, 200))
                .unwrap()
                .draw_line((0, 0), (10, 0), &RED)
                .unwrap();
        })
    });
    let combined = elements(&destination);
    assert_ne!(combined[0]["id"], combined[1]["id"]);
    assert_ne!(combined[1]["id"], combined[2]["id"]);
    assert_ne!(combined[0]["groupIds"], combined[1]["groupIds"]);
    assert_ne!(combined[1]["groupIds"], combined[2]["groupIds"]);
    assert_eq!(combined[2]["groupIds"].as_array().unwrap().len(), 1);
    for field in [
        "points",
        "x",
        "y",
        "seed",
        "strokeColor",
        "strokeWidth",
        "roughness",
    ] {
        assert_eq!(combined[1][field], original[0][field]);
    }
    let mut report = path_scene();
    report.append(destination).unwrap();
    let all = elements(&report);
    assert_eq!(
        all.iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(report.diagnostics().elements["line"], 4);
    assert_eq!(report.diagnostics().vertices, 11);
    assert_eq!(report.diagnostics().calls["draw_path"], 3);
}

#[test]
fn rejected_operations_are_atomic_and_failed_scenes_stay_failed() {
    let mut scene = path_scene();
    let before = scene.to_bytes().unwrap();
    for offset in [(f64::NAN, 0.), (0., f64::INFINITY)] {
        assert!(scene.translate(offset).is_err());
        assert!(scene.place_at(offset).is_err());
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    let mut failed = Scene::new();
    assert!(
        ExcalidrawBackend::new(&mut failed, (10, 10))
            .unwrap()
            .draw_pixel((0, 0), BLACK.to_backend_color())
            .is_err()
    );
    assert!(scene.append(failed).is_err());
    assert_eq!(scene.to_bytes().unwrap(), before);
    let mut empty = Scene::new();
    assert_eq!(empty.bounds().unwrap(), None);
    empty.translate((1., 2.)).unwrap();
    assert!(empty.place_at((1., 2.)).is_err());
    scene.append(empty).unwrap();
    assert_eq!(scene.to_bytes().unwrap(), before);
    scene.translate((f64::MAX, 0.)).unwrap();
    let extreme = scene.to_bytes().unwrap();
    assert!(scene.translate((f64::MAX, 0.)).is_err());
    assert_eq!(scene.to_bytes().unwrap(), extreme);
    let mut opposite = path_scene();
    opposite.translate((-f64::MAX, 0.)).unwrap();
    assert!(scene.append(opposite).is_err());
    assert_eq!(scene.to_bytes().unwrap(), extreme);
    assert!(
        ExcalidrawBackend::new(&mut scene, (10, 10))
            .unwrap()
            .draw_pixel((0, 0), BLACK.to_backend_color())
            .is_err()
    );
    assert!(scene.append(path_scene()).is_err());
    assert!(scene.translate((0., 0.)).is_err());
    assert!(scene.to_bytes().is_err());
}

#[test]
fn bounds_include_rotated_text_flat_lines_and_finite_backgrounds() {
    let mut scene = Scene::new();
    ExcalidrawBackend::new(&mut scene, (200, 200))
        .unwrap()
        .draw_text(
            "Title",
            &TextStyle::from(
                ("Excalifont", 20)
                    .into_font()
                    .transform(FontTransform::Rotate90),
            ),
            (100, 100),
        )
        .unwrap();
    let text = elements(&scene).remove(0);
    let width = text["width"].as_f64().unwrap();
    let height = text["height"].as_f64().unwrap();
    let b = scene.bounds().unwrap().unwrap();
    assert!((b.width - height).abs() < 1e-8);
    assert!((b.height - width).abs() < 1e-8);
    scene.place_at((0., 0.)).unwrap();
    let b = scene.bounds().unwrap().unwrap();
    assert!(b.x.abs() < 1e-8 && b.y.abs() < 1e-8);
    let mut background = Scene::new();
    let mut backend = ExcalidrawBackend::new(&mut background, (500, 500)).unwrap();
    backend.draw_rect((0, 0), (400, 300), &WHITE, true).unwrap();
    backend.draw_line((0, 0), (0, 300), &BLACK).unwrap();
    backend.draw_line((0, 0), (400, 0), &BLACK).unwrap();
    background.append(scene).unwrap();
    assert_eq!(
        background.bounds().unwrap(),
        Some(Bounds {
            x: -0.5,
            y: -0.5,
            width: 401.,
            height: 301.
        })
    );
}

#[test]
fn borders_preserve_content_and_measure_padding_to_the_inner_stroke_edge() {
    for (style, name) in [
        (StrokeStyle::Solid, "solid"),
        (StrokeStyle::Dashed, "dashed"),
        (StrokeStyle::Dotted, "dotted"),
    ] {
        for width in [1, 8] {
            let thickness = f64::from(width);
            for padding in [0., 12.] {
                let mut scene = path_scene();
                let original = elements(&scene);
                scene
                    .add_border(
                        padding,
                        BorderStyle {
                            color: (224, 49, 49),
                            width,
                            stroke: style,
                        },
                    )
                    .unwrap();
                let all = elements(&scene);
                let border = &all[1];
                assert_eq!(border["type"], "rectangle");
                assert_eq!(border["backgroundColor"], "transparent");
                assert_eq!(border["strokeStyle"], name);
                assert_eq!(border["strokeColor"], "#e03131");
                assert_eq!(
                    border["x"].as_f64().unwrap() + thickness / 2.,
                    18. - padding
                );
                assert_eq!(border["y"].as_f64().unwrap() + thickness / 2., 8. - padding);
                assert_eq!(
                    border["width"].as_f64().unwrap() - thickness,
                    94. + 2. * padding
                );
                assert_eq!(
                    border["height"].as_f64().unwrap() - thickness,
                    84. + 2. * padding
                );
                assert_eq!(all[0]["groupIds"][0], original[0]["groupIds"][0]);
                assert_eq!(all[0]["groupIds"][1], border["groupIds"][0]);
                for field in [
                    "x",
                    "y",
                    "width",
                    "height",
                    "points",
                    "strokeStyle",
                    "strokeColor",
                    "seed",
                ] {
                    assert_eq!(all[0][field], original[0][field]);
                }
                let previous = scene.bounds().unwrap().unwrap();
                scene.add_border(10., BorderStyle::default()).unwrap();
                let nested = scene.bounds().unwrap().unwrap();
                assert_eq!(nested.x, previous.x - 11.);
                assert_eq!(elements(&scene)[0]["groupIds"].as_array().unwrap().len(), 3);
                assert_eq!(scene.diagnostics().elements["rectangle"], 2);
            }
        }
    }
}

#[test]
fn native_frames_wrap_assemblies_and_keep_sibling_membership_after_colliding_insertion() {
    let mut report = Scene::with_namespace("same");
    for x in [0., 200.] {
        let mut assembly = path_scene();
        assembly.append(path_scene()).unwrap();
        assembly.add_border(5., BorderStyle::default()).unwrap();
        let content = elements(&assembly);
        let bounds = assembly.bounds().unwrap().unwrap();
        assembly.add_frame(12., Some("Panel")).unwrap();
        let framed = elements(&assembly);
        let frame = framed.last().unwrap();
        assert_eq!(frame["type"], "frame");
        assert_eq!(frame["name"], "Panel");
        assert_eq!(frame["x"], bounds.x - 12.);
        assert_eq!(frame["width"], bounds.width + 24.);
        assert!(frame["frameId"].is_null());
        for (before, after) in content.iter().zip(&framed) {
            assert_eq!(after["frameId"], frame["id"]);
            for field in ["x", "y", "points", "groupIds", "seed"] {
                assert_eq!(before[field], after[field]);
            }
        }
        assembly.place_at((x, 0.)).unwrap();
        assert_eq!(assembly.bounds().unwrap().unwrap().x, x);
        report.append(assembly).unwrap();
    }
    let all = elements(&report);
    let frames: Vec<_> = all.iter().filter(|e| e["type"] == "frame").collect();
    assert_eq!(frames.len(), 2);
    assert_ne!(frames[0]["id"], frames[1]["id"]);
    for frame in &frames {
        assert_eq!(
            all.iter().filter(|e| e["frameId"] == frame["id"]).count(),
            3
        );
    }
    let before = report.to_bytes().unwrap();
    assert!(report.add_frame(0., None).is_err());
    assert_eq!(report.to_bytes().unwrap(), before);
    report.add_border(10., BorderStyle::default()).unwrap();
    let bordered = elements(&report);
    for (old, new) in all.iter().zip(&bordered) {
        assert_eq!(old["frameId"], new["frameId"]);
    }
    assert_eq!(report.diagnostics().elements["frame"], 2);
    report.translate((30., -20.)).unwrap();
    for (old, new) in bordered.iter().zip(elements(&report)) {
        assert_eq!(new["x"].as_f64().unwrap(), old["x"].as_f64().unwrap() + 30.);
        assert_eq!(new["y"].as_f64().unwrap(), old["y"].as_f64().unwrap() - 20.);
    }
}

#[test]
fn decoration_rejects_invalid_inputs_without_mutating_or_poisoning_content() {
    let mut scene = path_scene();
    let before = scene.to_bytes().unwrap();
    for padding in [-1., f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(scene.add_border(padding, BorderStyle::default()).is_err());
        assert!(scene.add_frame(padding, None).is_err());
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    assert!(
        scene
            .add_border(
                1.,
                BorderStyle {
                    width: 0,
                    ..BorderStyle::default()
                }
            )
            .is_err()
    );
    for name in ["new\nline", "emoji 🦀"] {
        assert!(scene.add_frame(1., Some(name)).is_err());
    }
    assert_eq!(scene.to_bytes().unwrap(), before);
    let mut empty = Scene::new();
    assert!(empty.add_border(0., BorderStyle::default()).is_err());
    assert!(empty.add_frame(0., None).is_err());
    scene.add_frame(0., None).unwrap();
    assert!(elements(&scene).last().unwrap()["name"].is_null());
    let mut backend = ExcalidrawBackend::new(&mut scene, (100, 100)).unwrap();
    assert!(
        backend
            .draw_pixel((0, 0), BLACK.to_backend_color())
            .is_err()
    );
    assert!(scene.add_border(0., BorderStyle::default()).is_err());
    assert!(scene.add_frame(0., None).is_err());
    assert!(scene.to_bytes().is_err());
}
