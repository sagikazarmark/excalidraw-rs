use excaliplot::{BorderStyle, Error, ExcalidrawBackend, LineChart, Overwrite, Scene};
use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use serde_json::{Value, json};

fn fixture() -> Scene {
    let mut scene = Scene::with_namespace("library-fixture");
    ExcalidrawBackend::new(&mut scene, (200, 100))
        .unwrap()
        .draw_line((10, 20), (80, 50), &BLUE)
        .unwrap();
    scene
}

#[test]
fn whole_scene_exports_as_one_deterministic_unpublished_library_item() {
    let scene = fixture();
    let scene_bytes = scene.to_bytes().unwrap();
    let document: Value = serde_json::from_slice(&scene_bytes).unwrap();
    let bytes = scene.to_library_bytes().unwrap();
    let library: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        library,
        json!({
            "type": "excalidrawlib", "version": 2, "source": "excaliplot",
            "libraryItems": [{
                "id": "library-fixture-library", "status": "unpublished", "created": 0,
                "elements": document["elements"]
            }]
        })
    );
    assert_eq!(bytes, fixture().to_library_bytes().unwrap());
    assert_eq!(scene_bytes, scene.to_bytes().unwrap());
}

#[test]
fn empty_and_failed_content_are_refused_but_rejected_scene_operations_are_atomic() {
    assert!(matches!(
        Scene::new().to_library_bytes(),
        Err(Error::Invalid(_))
    ));
    for bitmap in [false, true] {
        let mut scene = fixture();
        let mut backend = ExcalidrawBackend::new(&mut scene, (200, 100)).unwrap();
        if bitmap {
            assert!(backend.blit_bitmap((0, 0), (1, 1), &[0, 0, 0]).is_err());
        } else {
            assert!(
                backend
                    .draw_line((0, 0), (1, 1), &BLUE.mix(f64::NAN))
                    .is_err()
            );
        }
        assert!(matches!(scene.to_library_bytes(), Err(Error::FailedScene)));
    }
    let mut scene = fixture();
    let before = scene.to_library_bytes().unwrap();
    assert!(scene.translate((f64::INFINITY, 0.)).is_err());
    assert!(scene.add_frame(f64::NAN, None).is_err());
    assert_eq!(scene.to_library_bytes().unwrap(), before);
}

#[test]
fn library_writes_protect_existing_work_and_validate_before_opening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("diagram.excalidrawlib");
    let new = directory.path().join("new.excalidrawlib");
    let mut scene = fixture();
    scene.write_library(&path, Overwrite::Refuse).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        scene.to_library_bytes().unwrap()
    );
    std::fs::write(&path, b"manual library").unwrap();
    assert!(matches!(scene.write_library(&path, Overwrite::Refuse),
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::AlreadyExists));
    scene.write_library(&path, Overwrite::Allow).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        scene.to_library_bytes().unwrap()
    );
    std::fs::write(&path, b"manual library").unwrap();
    assert!(
        ExcalidrawBackend::new(&mut scene, (200, 100))
            .unwrap()
            .draw_pixel((0, 0), BLACK.to_backend_color())
            .is_err()
    );
    for invalid in [scene, Scene::new()] {
        assert!(invalid.write_library(&path, Overwrite::Allow).is_err());
        assert!(invalid.write_library(&new, Overwrite::Refuse).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"manual library");
        assert!(!new.exists());
    }
    assert!(
        fixture()
            .write_library(directory.path().join("missing/item"), Overwrite::Refuse)
            .is_err()
    );
}

#[test]
fn composed_items_preserve_every_native_kind_paint_and_relationship() {
    let mut scene = Scene::new();
    for index in 0..2 {
        let mut panel = LineChart::new(&[(0., 1.), (1., 2.)], 0.0..1.0, 0.0..3.0)
            .labels("Panel", "Time", "Value")
            .render()
            .unwrap();
        ExcalidrawBackend::new(&mut panel, (640, 400))
            .unwrap()
            .draw_circle((200, 200), 5, &RED, true)
            .unwrap();
        panel.add_border(10., BorderStyle::default()).unwrap();
        panel.add_frame(10., Some("Frame")).unwrap();
        panel.place_at((index as f64 * 800., 0.)).unwrap();
        scene.append(panel).unwrap();
    }
    let document: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
    let library: Value = serde_json::from_slice(&scene.to_library_bytes().unwrap()).unwrap();
    let elements = library["libraryItems"][0]["elements"].as_array().unwrap();
    assert_eq!(library["libraryItems"][0]["elements"], document["elements"]);
    let kinds: std::collections::BTreeSet<_> = elements
        .iter()
        .map(|e| e["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["ellipse", "frame", "line", "rectangle", "text"].into()
    );
    let ids: std::collections::BTreeSet<_> =
        elements.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), elements.len());
    for e in elements {
        if let Some(id) = e["frameId"].as_str() {
            assert_eq!(
                elements.iter().find(|f| f["id"] == id).unwrap()["type"],
                "frame"
            );
        }
    }
    assert_eq!(
        elements
            .iter()
            .filter(|e| e["backgroundColor"] == "#ffffff")
            .count(),
        2
    );
    assert!(
        elements
            .iter()
            .any(|e| e["groupIds"].as_array().unwrap().len() > 1)
    );
    let other: Value = serde_json::from_slice(
        &LineChart::new(&[(0., 1.), (1., 2.)], 0.0..1.0, 0.0..3.0)
            .render()
            .unwrap()
            .to_library_bytes()
            .unwrap(),
    )
    .unwrap();
    assert_ne!(
        library["libraryItems"][0]["id"],
        other["libraryItems"][0]["id"]
    );
}

#[test]
fn runnable_library_example_emits_two_protected_single_item_files() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("library");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--example", "library", "--"])
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    for name in ["diagram", "other"] {
        let path = output.join(format!("{name}.excalidrawlib"));
        let library: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(library["type"], "excalidrawlib");
        assert_eq!(library["libraryItems"].as_array().unwrap().len(), 1);
        assert!(
            !library["libraryItems"][0]["elements"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        std::fs::write(path, b"edited library").unwrap();
    }
    assert!(!run().status.success());
    for name in ["diagram", "other"] {
        assert_eq!(
            std::fs::read(output.join(format!("{name}.excalidrawlib"))).unwrap(),
            b"edited library"
        );
    }
}
