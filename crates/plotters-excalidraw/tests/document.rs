use excalidraw_document::{Number, Profile, Purpose, TextContent, element};
use plotters_excalidraw::Scene;

#[test]
fn general_document_edits_are_independent_of_the_generated_scene() {
    let mut scene = Scene::with_namespace("document");
    scene.add_note("Original", (10., 20.), 20.).unwrap();
    let before = scene.to_bytes().unwrap();
    let mut document = scene.to_document().unwrap();
    document
        .author(Profile::V0_18_1)
        .unwrap()
        .batch(|batch| {
            batch.replace_text(
                &"document-0".into(),
                TextContent::plain("Edited", 60_u64.into(), 25_u64.into()),
            )?;
            batch.translate_connected(&["document-0".into()], [89_i64.into(), 0_i64.into()])
        })
        .unwrap();
    assert_eq!(scene.to_bytes().unwrap(), before);
    assert_eq!(document.as_object()["elements"][0]["text"], "Edited");
    assert_eq!(
        document.as_object()["elements"][0]["originalText"],
        "Edited"
    );
    assert_eq!(
        document.as_object()["elements"][0]["x"],
        serde_json::json!(99.)
    );
    assert_eq!(document.as_object()["source"], "excaliplot");
}

#[test]
fn all_backend_kinds_are_complete_shared_authored_records() {
    use plotters_backend::{BackendColor, DrawingBackend};
    use plotters_excalidraw::{ArrowStyle, BorderStyle, ExcalidrawBackend};
    let mut scene = Scene::with_namespace("shared");
    let paint = BackendColor {
        rgb: (20, 40, 60),
        alpha: 1.,
    };
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (400, 300)).unwrap();
        backend.draw_rect((10, 10), (70, 50), &paint, true).unwrap();
        backend.draw_circle((100, 60), 10, &paint, false).unwrap();
        backend
            .draw_path([(0, 100), (40, 80), (90, 110)], &paint)
            .unwrap();
    }
    scene.add_note("Label", (30., 130.), 20.).unwrap();
    scene
        .add_arrow((150., 70.), (200., 110.), ArrowStyle::default())
        .unwrap();
    scene.add_border(5., BorderStyle::default()).unwrap();
    scene.add_frame(5., Some("Panel")).unwrap();
    scene.translate((20., 30.)).unwrap();
    let mut composed = Scene::with_namespace("combined");
    composed.append(scene).unwrap();
    let document = composed.to_document().unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::SelfContained);
    assert!(report.is_valid(), "{report:?}");
    let elements = document.elements().unwrap();
    let kinds: std::collections::BTreeSet<_> = elements
        .iter()
        .map(|e| e.as_object()["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["rectangle", "ellipse", "line", "text", "arrow", "frame"].into()
    );
    for e in &elements {
        assert_eq!(e.as_object()["isDeleted"], false);
        assert!(e.as_object().contains_key("index"));
        assert_eq!(e.as_object()["link"], serde_json::Value::Null);
        if e.as_object()["type"] == "text" {
            assert_eq!(
                e.get(element::TEXT).unwrap(),
                e.get(element::ORIGINAL_TEXT).unwrap()
            );
        }
        if e.as_object()["type"] == "line" {
            assert_eq!(
                e.get(element::WIDTH).unwrap(),
                excalidraw_document::Field::Value(Number::from_f64(90.).unwrap())
            );
        }
    }
    let library =
        excalidraw_document::LibraryDocument::from_slice(&composed.to_library_bytes().unwrap())
            .unwrap();
    let report = library.validate(Profile::V0_18_1, Purpose::Author);
    assert!(report.is_valid(), "{report:?}");
}
