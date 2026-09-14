use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_excalidraw::{ExcalidrawBackend, Scene};
use serde_json::{Value, json};

#[test]
fn concave_fill_is_one_exactly_closed_first_relative_path_without_an_outline() {
    let mut scene = Scene::with_namespace("polygon");
    ExcalidrawBackend::new(&mut scene, (200, 200))
        .unwrap()
        .fill_polygon(
            [
                (100, 100),
                (100, 100),
                (60, 120),
                (80, 90),
                (110, 70),
                (100, 100),
            ],
            &BLUE.mix(0.5).stroke_width(0),
        )
        .unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let elements = doc["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 1);
    let fill = &elements[0];
    assert_eq!(fill["type"], "line");
    assert_eq!(fill["x"], 100.0);
    assert_eq!(fill["y"], 100.0);
    assert_eq!(fill["width"], 50.0);
    assert_eq!(fill["height"], 50.0);
    assert_eq!(
        fill["points"],
        json!([
            [0.0, 0.0],
            [-40.0, 20.0],
            [-20.0, -10.0],
            [10.0, -30.0],
            [0.0, 0.0]
        ])
    );
    assert_eq!(fill["strokeColor"], "transparent");
    assert_eq!(fill["backgroundColor"], "#0000ff");
    assert_eq!(fill["strokeWidth"], 1);
    assert_eq!(fill["opacity"], 50);
    assert_eq!(scene.diagnostics().vertices, 5);
    assert_eq!(scene.diagnostics().calls.get("draw_pixel"), None);
    assert_eq!(scene.diagnostics().calls.get("blit_bitmap"), None);
}
