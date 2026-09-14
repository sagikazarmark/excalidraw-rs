use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_excalidraw::{ExcalidrawBackend, Overwrite, Scene};

#[test]
fn exporting_again_preserves_manual_work_unless_overwrite_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("chart.excalidraw");
    let mut scene = Scene::new();
    ExcalidrawBackend::new(&mut scene, (640, 400))
        .unwrap()
        .draw_line((0, 0), (10, 10), &BLACK)
        .unwrap();
    scene.write(&path, Overwrite::Refuse).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), scene.to_bytes().unwrap());
    std::fs::write(&path, b"manually edited chart").unwrap();
    let error = scene.write(&path, Overwrite::Refuse).unwrap_err();
    assert!(
        matches!(error, plotters_excalidraw::Error::Io(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"manually edited chart");
    scene.write(&path, Overwrite::Allow).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), scene.to_bytes().unwrap());
    assert!(
        scene
            .write(
                directory.path().join("missing/chart.excalidraw"),
                Overwrite::Refuse
            )
            .is_err()
    );
}

#[test]
fn failed_drawing_cannot_create_or_truncate_an_export() {
    let directory = tempfile::tempdir().unwrap();
    let existing = directory.path().join("edited.excalidraw");
    let new = directory.path().join("new.excalidraw");
    std::fs::write(&existing, b"manual work").unwrap();
    let mut scene = Scene::new();
    {
        let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        backend.draw_line((0, 0), (10, 10), &BLACK).unwrap();
        assert!(backend.blit_bitmap((10, 10), (1, 1), &[0, 0, 0]).is_err());
    }
    assert!(scene.write(&existing, Overwrite::Allow).is_err());
    assert!(scene.write(&new, Overwrite::Refuse).is_err());
    assert_eq!(std::fs::read(existing).unwrap(), b"manual work");
    assert!(!new.exists());
}
