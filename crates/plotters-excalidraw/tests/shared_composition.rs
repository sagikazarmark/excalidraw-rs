use excalidraw_document::{Profile, Purpose};
use plotters_backend::{BackendColor, DrawingBackend};
use plotters_excalidraw::{BorderStyle, ExcalidrawBackend, Scene};
use std::time::Instant;

#[test]
fn frame_preserves_painter_order_when_a_group_is_reentered_after_a_border() {
    let mut scene = Scene::with_namespace("crossing");
    let group = scene.drawing_options().new_group();
    group.scope(|| scene.add_note("First", (0., 0.), 20.).unwrap());
    scene.add_border(5., BorderStyle::default()).unwrap();
    group.scope(|| scene.add_note("Delayed", (100., 0.), 20.).unwrap());
    let before = scene.to_document().unwrap().elements().unwrap();

    scene.add_frame(10., Some("Panel")).unwrap();
    let document = scene.to_document().unwrap();
    let after = document.elements().unwrap();
    let frame = after.last().unwrap();
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(frame.as_object()["type"], "frame");
    for (mut expected, actual) in before.into_iter().zip(&after) {
        expected
            .set(
                excalidraw_document::element::FRAME_ID,
                excalidraw_document::ElementId(frame.as_object()["id"].as_str().unwrap().into()),
            )
            .unwrap();
        assert_eq!(&expected, actual);
    }
    assert!(after.iter().all(|e| e.as_object()["index"].is_null()));
    let report = document.validate(Profile::V0_18_1, Purpose::Author);
    assert!(report.is_valid(), "{report:?}");
    scene.to_library_bytes().unwrap();
    let before = scene.to_bytes().unwrap();
    assert!(scene.add_frame(0., None).is_err());
    assert_eq!(scene.to_bytes().unwrap(), before);
}

/// Manual scaling probe; timings exclude construction and have no CI threshold.
#[test]
#[ignore = "manual release-mode composition scaling measurement"]
fn composition_scaling() {
    for count in [4_000, 8_000, 16_000] {
        let mut scene = Scene::with_namespace("scaling");
        {
            let mut backend = ExcalidrawBackend::new(&mut scene, (20_000, 100)).unwrap();
            let color = BackendColor {
                rgb: (30, 30, 30),
                alpha: 1.,
            };
            for x in 0..count {
                backend.draw_rect((x, 0), (x + 1, 1), &color, true).unwrap();
            }
        }
        let start = Instant::now();
        scene.translate((1., 2.)).unwrap();
        let translate = start.elapsed();
        let start = Instant::now();
        scene.add_frame(1., None).unwrap();
        let frame = start.elapsed();
        println!("{count} rectangles: translate={translate:?}, frame={frame:?}");
    }
}
