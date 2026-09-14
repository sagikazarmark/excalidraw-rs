use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_backend::text_anchor::{HPos, Pos, VPos};
use plotters_excalidraw::{ExcalidrawBackend, Scene};
use serde_json::{Value, json};

fn document(scene: &Scene) -> Value {
    serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap()
}

#[test]
fn native_paths_keep_first_point_origin_and_survive_repeated_presentation() {
    let mut scene = Scene::with_namespace("fixture");
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        assert_eq!(backend.get_size(), (640, 400));
        backend.ensure_prepared().unwrap();
        backend
            .draw_path([(100, 100), (60, 120), (110, 70)], &BLUE.stroke_width(3))
            .unwrap();
        backend.present().unwrap();
        backend.ensure_prepared().unwrap();
        backend.present().unwrap();
        backend.draw_line((0, 0), (10, 0), &BLACK).unwrap();
    }
    let doc = document(&scene);
    assert_eq!(doc["type"], "excalidraw");
    assert_eq!(doc["version"], 2);
    assert_eq!(doc["files"], json!({}));
    let elements = doc["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 2);
    let line = &elements[0];
    assert_eq!(line["type"], "line");
    assert_eq!(line["x"], 100.0);
    assert_eq!(line["y"], 100.0);
    assert_eq!(
        line["points"],
        json!([[0.0, 0.0], [-40.0, 20.0], [10.0, -30.0]])
    );
    assert_eq!(line["width"], 50.0);
    assert_eq!(line["height"], 50.0);
    assert_eq!(line["strokeWidth"], 3);
    assert_eq!(line["strokeColor"], "#0000ff");
    assert_eq!(line["roughness"], 0);
    assert_eq!(line["roundness"], Value::Null);
    assert_eq!(elements[1]["height"], 0.0);
}

#[test]
fn finite_fills_and_outlines_preserve_paint_order_without_scanlines() {
    let mut scene = Scene::with_namespace("paint");
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        backend.draw_rect((0, 0), (640, 400), &WHITE, true).unwrap();
        backend
            .draw_rect((30, 40), (10, 20), &RED.stroke_width(4), false)
            .unwrap();
        backend.draw_rect((0, 0), (640, 400), &BLUE, true).unwrap();
    }
    let doc = document(&scene);
    let elements = doc["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 3);
    assert!(elements.iter().all(|e| e["type"] == "rectangle"));
    assert_eq!(elements[0]["width"], 640.0);
    assert_eq!(elements[0]["height"], 400.0);
    assert_eq!(elements[0]["strokeColor"], "transparent");
    assert_eq!(elements[0]["backgroundColor"], "#ffffff");
    assert_eq!(elements[1]["x"], 10.0);
    assert_eq!(elements[1]["y"], 20.0);
    assert_eq!(elements[1]["width"], 20.0);
    assert_eq!(elements[1]["height"], 20.0);
    assert_eq!(elements[1]["strokeColor"], "#ff0000");
    assert_eq!(elements[1]["strokeWidth"], 4);
    assert_eq!(elements[1]["backgroundColor"], "transparent");
    assert_eq!(elements[2]["backgroundColor"], "#0000ff");
    assert_eq!(doc["appState"]["viewBackgroundColor"], "#ffffff");
    let stats = scene.diagnostics();
    assert_eq!(stats.elements["rectangle"], 3);
    assert_eq!(stats.calls.get("draw_pixel"), None);
    assert_eq!(stats.calls.get("draw_line"), None);
}

#[test]
fn native_text_uses_unrotated_estimates_and_rotates_about_the_requested_anchor() {
    let mut scene = Scene::with_namespace("text");
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        let font = ("Excalifont", 20).into_font();
        let style = TextStyle::from(font).pos(Pos::new(HPos::Center, VPos::Top));
        let rotated = style.clone().transform(FontTransform::Rotate270);
        assert_eq!(
            backend.estimate_text_size("Value", &style).unwrap(),
            (49, 25)
        );
        assert_eq!(
            backend.estimate_text_size("Value", &rotated).unwrap(),
            (49, 25)
        );
        backend.draw_text("Value", &rotated, (40, 200)).unwrap();
        backend.draw_text("Value", &style, (320, 20)).unwrap();
    }
    let doc = document(&scene);
    let label = &doc["elements"][0];
    assert_eq!(label["type"], "text");
    assert_eq!(label["text"], "Value");
    assert_eq!(label["originalText"], "Value");
    assert_eq!(label["fontFamily"], 5);
    assert_eq!(label["fontSize"], 20.0);
    assert_eq!(label["lineHeight"], 1.25);
    assert_eq!(label["width"], 48.56);
    assert_eq!(label["height"], 25.0);
    // Center is (52.5, 200); rotating its top-center anchor 270° gives (40, 200).
    assert!((label["x"].as_f64().unwrap() - 28.22).abs() < 1e-10);
    assert_eq!(label["y"], 187.5);
    assert_eq!(label["angle"], 3.0 * std::f64::consts::FRAC_PI_2);
    assert_eq!(label["autoResize"], true);
    assert_eq!(label["containerId"], Value::Null);
    assert_eq!(doc["elements"][1]["x"], 295.72);
    assert_eq!(doc["elements"][1]["y"], 20.0);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
}

#[test]
fn invisible_and_degenerate_paints_are_skipped_and_alpha_is_applied_once() {
    let mut scene = Scene::with_namespace("visibility");
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        backend
            .draw_path([(0, 0), (10, 10)], &BLUE.mix(0.5))
            .unwrap();
        backend
            .draw_rect((0, 0), (10, 10), &RED.mix(0.5), true)
            .unwrap();
        backend.draw_line((0, 0), (10, 10), &TRANSPARENT).unwrap();
        backend
            .draw_pixel((0, 0), TRANSPARENT.to_backend_color())
            .unwrap();
        backend.draw_path([(1, 1), (1, 1)], &BLACK).unwrap();
        backend.draw_path([(1, 1)], &BLACK).unwrap();
        backend.draw_rect((0, 0), (0, 20), &BLACK, true).unwrap();
        backend
            .draw_line((0, 0), (10, 10), &BLACK.stroke_width(0))
            .unwrap();
        backend
            .draw_text("", &TextStyle::from(("Excalifont", 20).into_font()), (0, 0))
            .unwrap();
    }
    let doc = document(&scene);
    assert_eq!(doc["elements"].as_array().unwrap().len(), 2);
    assert_eq!(doc["elements"][0]["strokeColor"], "#0000ff");
    assert_eq!(doc["elements"][0]["opacity"], 50);
    assert_eq!(doc["elements"][1]["backgroundColor"], "#ff0000");
    assert_eq!(doc["elements"][1]["opacity"], 50);
    assert_eq!(scene.diagnostics().calls["draw_pixel"], 1);
}

#[test]
fn invalid_text_is_rejected_in_measurement_and_drawing_and_blocks_export() {
    let regular = TextStyle::from(("Excalifont", 20).into_font());
    for (text, style) in [
        ("label", TextStyle::from(("serif", 20).into_font())),
        (
            "label",
            TextStyle::from(("Excalifont", 20).into_font().style(FontStyle::Bold)),
        ),
        ("two\nlines", regular.clone()),
        ("tab\there", regular.clone()),
        (
            "label",
            TextStyle::from(("Excalifont", f64::NAN).into_font()),
        ),
        ("label", TextStyle::from(("Excalifont", 0).into_font())),
        (
            "label",
            TextStyle::from(("Excalifont", f64::MAX).into_font()),
        ),
    ] {
        let mut scene = Scene::new();
        {
            let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
            backend.draw_line((0, 0), (10, 10), &BLACK).unwrap();
            assert!(backend.estimate_text_size(text, &style).is_err());
            assert!(backend.draw_text(text, &style, (0, 0)).is_err());
        }
        assert!(
            scene.to_bytes().is_err(),
            "a partial failed drawing must not export"
        );
    }
}

#[test]
fn unsupported_operations_and_invalid_alpha_fail_without_publishing_partial_scenes() {
    for operation in ["draw_pixel", "blit_bitmap", "alpha"] {
        let mut scene = Scene::new();
        {
            let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
            backend.draw_line((0, 0), (10, 10), &BLACK).unwrap();
            let result = match operation {
                "draw_pixel" => backend.draw_pixel((0, 0), BLACK.to_backend_color()),
                "blit_bitmap" => backend.blit_bitmap((0, 0), (10, 10), &[]),
                _ => backend.draw_line((0, 0), (10, 10), &BLACK.mix(f64::NAN)),
            };
            let error = result.unwrap_err().to_string();
            assert!(error.contains(operation), "{error}");
        }
        assert!(scene.to_bytes().is_err());
        assert_eq!(scene.diagnostics().elements["line"], 1);
    }
    for alpha in [-0.1, 1.1, f64::INFINITY] {
        let mut scene = Scene::new();
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        assert!(
            backend
                .draw_rect((0, 0), (10, 10), &BLACK.mix(alpha), true)
                .is_err()
        );
    }
}

#[test]
fn exports_have_distinct_identities_but_reproducible_nonzero_seeds() {
    let draw = |mut scene: Scene| {
        ExcalidrawBackend::new(&mut scene, (640, 400))
            .unwrap()
            .draw_line((0, 0), (10, 10), &BLACK)
            .unwrap();
        document(&scene)
    };
    let first = draw(Scene::new());
    let second = draw(Scene::new());
    assert_ne!(first["elements"][0]["id"], second["elements"][0]["id"]);
    assert_eq!(first["elements"][0]["seed"], second["elements"][0]["seed"]);
    assert!(first["elements"][0]["seed"].as_u64().unwrap() > 0);
    assert_eq!(
        draw(Scene::with_namespace("fixed")),
        draw(Scene::with_namespace("fixed"))
    );
}
