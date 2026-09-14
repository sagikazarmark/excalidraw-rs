use plotters_excalidraw::{BorderStyle, Error, Scene, text};
use serde_json::{Value, json};

#[test]
fn fractional_polygon_is_closed_grouped_and_composable() {
    let mut scene = Scene::with_namespace("polygon");
    scene.drawing_options().new_group().scope(|| {
        scene
            .add_polygon(
                &[(10.25, 20.5), (40.75, 20.5), (20.5, 50.25)],
                (25, 113, 194),
                0.5,
            )
            .unwrap();
    });
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let polygon = &doc["elements"][0];
    assert_eq!(
        polygon["points"],
        json!([[0., 0.], [30.5, 0.], [10.25, 29.75], [0., 0.]])
    );
    assert_eq!(polygon["backgroundColor"], "#1971c2");
    assert_eq!(polygon["strokeColor"], "transparent");
    assert_eq!(polygon["opacity"], 50);
    assert_eq!(polygon["groupIds"].as_array().unwrap().len(), 1);
    scene.add_border(8., BorderStyle::default()).unwrap();
    scene.place_at((0., 0.)).unwrap();
    let mut report = Scene::new();
    report.append(scene).unwrap();
    report.to_library_bytes().unwrap();
}

#[test]
fn invalid_native_polygons_leave_the_scene_unchanged() {
    let mut scene = Scene::new();
    scene.add_note("Keep", (0., 0.), 20.).unwrap();
    let before = scene.to_bytes().unwrap();
    for points in [
        vec![],
        vec![(0., 0.), (1., 1.)],
        vec![(0., 0.), (1., 1.), (2., 2.)],
        vec![(0., 0.), (1., 3.), (15., 45.)],
        vec![(0., 0.), (f64::NAN, 1.), (1., 0.)],
        vec![(-f64::MAX, 0.), (f64::MAX, 1.), (0., 2.)],
    ] {
        assert!(scene.add_polygon(&points, (0, 0, 0), 1.).is_err());
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
    for opacity in [f64::NAN, -1., 0., 0.004, 1.1] {
        assert!(
            scene
                .add_polygon(&[(0., 0.), (1., 0.), (0., 1.)], (0, 0, 0), opacity)
                .is_err()
        );
        assert_eq!(scene.to_bytes().unwrap(), before);
    }
}

#[test]
fn finite_extreme_aspect_ratio_polygon_is_not_collapsed_by_validation() {
    let mut scene = Scene::new();
    scene
        .add_polygon(&[(0., 0.), (1e200, 0.), (0., 1e-200)], (0, 0, 0), 1.)
        .unwrap();
    scene.to_bytes().unwrap();
}

#[test]
fn mixed_magnitude_polygon_acceptance_is_independent_of_start_vertex() {
    let mut points = [(0., 0.), (1e-200, 1e-200), (1e200, 0.), (0., 1e200)];
    for _ in 0..points.len() {
        let mut scene = Scene::new();
        scene.add_polygon(&points, (0, 0, 0), 1.).unwrap();
        scene.to_bytes().unwrap();
        points.rotate_left(1);
    }
}

#[test]
fn public_metrics_match_native_note_dimensions() {
    let (width, height) = text::measure("Café ±2°C", 20.).unwrap();
    let mut scene = Scene::new();
    scene.add_note("Café ±2°C", (0., 0.), 20.).unwrap();
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    assert_eq!(doc["elements"][0]["width"], width);
    assert_eq!(doc["elements"][0]["height"], height);
    assert!(matches!(
        text::measure("Ω", 20.),
        Err(Error::UnsupportedGlyph('Ω'))
    ));
    assert!(text::measure("two\nlines", 20.).is_err());
}

#[test]
fn native_scene_access_preserves_scopes_and_failed_scene_state() {
    use plotters_backend::{BackendColor, DrawingBackend};
    use plotters_excalidraw::{ArrowStyle, ExcalidrawBackend};
    let mut scene = Scene::new();
    let group = scene.drawing_options().new_group();
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        group.scope(|| {
            backend
                .scene_mut()
                .add_note("Native", (30.5, 40.25), 20.)
                .unwrap();
            backend
                .scene_mut()
                .add_arrow((30.5, 65.25), (10., 90.), ArrowStyle::default())
                .unwrap();
        });
    }
    let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    assert_eq!(doc["elements"][0]["x"], 30.5);
    assert_eq!(
        doc["elements"][0]["groupIds"],
        doc["elements"][1]["groupIds"]
    );
    let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
    assert!(
        backend
            .draw_pixel(
                (0, 0),
                BackendColor {
                    rgb: (0, 0, 0),
                    alpha: 1.
                }
            )
            .is_err()
    );
    assert!(matches!(
        backend
            .scene_mut()
            .add_polygon(&[(0., 0.), (1., 0.), (0., 1.)], (0, 0, 0), 1.),
        Err(Error::FailedScene)
    ));
    assert!(matches!(scene.to_bytes(), Err(Error::FailedScene)));
}
