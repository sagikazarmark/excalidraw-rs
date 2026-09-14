use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_excalidraw::{ExcalidrawBackend, Scene};
use serde_json::Value;

#[test]
fn invisible_circles_are_skipped_invalid_alpha_poisoned_and_extreme_coordinates_do_not_overflow() {
    let mut scene = Scene::new();
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (100, 100)).unwrap();
        backend.draw_circle((0, 0), 0, &BLUE, true).unwrap();
        backend.draw_circle((0, 0), 5, &TRANSPARENT, true).unwrap();
        backend
            .draw_circle((0, 0), 5, &BLUE.stroke_width(0), false)
            .unwrap();
        backend
            .draw_circle((i32::MIN, i32::MAX), u32::MAX, &BLUE.stroke_width(0), true)
            .unwrap();
    }
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    assert_eq!(doc["elements"].as_array().unwrap().len(), 1);
    let e = &doc["elements"][0];
    assert_eq!(e["x"], -6442450943.0);
    assert_eq!(e["y"], -2147483648.0);
    assert_eq!(e["width"], 8589934590.0);
    assert_eq!(e["strokeColor"], "transparent");
    assert_eq!(e["strokeWidth"], 1);
    for alpha in [-0.1, 1.1, f64::NAN] {
        let mut scene = Scene::new();
        assert!(
            ExcalidrawBackend::new(&mut scene, (100, 100))
                .unwrap()
                .draw_circle((10, 10), 5, &BLUE.mix(alpha), true)
                .is_err()
        );
        assert!(scene.to_bytes().is_err());
    }
}

#[test]
fn circles_emit_native_ellipses_with_center_radius_and_separate_paints() {
    let mut scene = Scene::new();
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (100, 100)).unwrap();
        backend
            .draw_circle((20, 30), 7, &BLUE.mix(0.5), true)
            .unwrap();
        backend
            .draw_circle((20, 30), 7, &RED.stroke_width(3), false)
            .unwrap();
    }
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 2);
    for e in elements {
        assert_eq!(e["type"], "ellipse");
        assert_eq!((e["x"].as_f64(), e["y"].as_f64()), (Some(13.0), Some(23.0)));
        assert_eq!(e["width"], 14.0);
        assert_eq!(e["height"], 14.0);
    }
    assert_eq!(elements[0]["backgroundColor"], "#0000ff");
    assert_eq!(elements[0]["strokeColor"], "transparent");
    assert_eq!(elements[0]["opacity"], 50);
    assert_eq!(elements[1]["backgroundColor"], "transparent");
    assert_eq!(elements[1]["strokeColor"], "#ff0000");
    assert_eq!(elements[1]["strokeWidth"], 3);
    assert_eq!(scene.diagnostics().elements["ellipse"], 2);
    assert_eq!(scene.diagnostics().vertices, 0);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
}
