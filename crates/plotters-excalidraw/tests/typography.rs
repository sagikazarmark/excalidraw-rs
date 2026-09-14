use plotters::prelude::*;
use plotters_backend::DrawingBackend;
use plotters_excalidraw::{ExcalidrawBackend, Scene};
use serde_json::Value;

#[test]
fn native_dimensions_match_loaded_excalifont_in_chromium() {
    // Canvas measureText from Excalidraw 0.18.0's Latin shard in Chromium
    // 152.0.7977.82 at 20px, after FontFace.load. Independent browser oracle.
    for (text, width) in [
        ("Value", 48.559967041015625),
        ("AV", 25.3599853515625),
        ("To", 27.3399658203125),
        ("Wi", 20.5999755859375),
        ("0123456789", 121.29986572265625),
        ("fi fl", 37.25996398925781),
        ("Café −2", 78.05992126464844),
        ("°", 8.239990234375),
        ("±", 10.999984741210938),
        ("20°C", 48.099945068359375),
        ("20 ± 2 °C", 97.09991455078125),
        ("−5.0°C ±0.2", 113.89987182617188),
        ("Café: 20°C (±2)", 156.2998504638672),
        (" ° ± ", 43.23997497558594),
        ("°±°", 27.479965209960938),
    ] {
        let mut scene = Scene::new();
        let estimate;
        {
            let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
            let style = TextStyle::from(("Excalifont", 20).into_font());
            estimate = backend.estimate_text_size(text, &style).unwrap();
            backend.draw_text(text, &style, (100, 100)).unwrap();
        }
        let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
        let label = &doc["elements"][0];
        assert!(
            (label["width"].as_f64().unwrap() - width).abs() < 0.01,
            "{text}: {label}"
        );
        assert_eq!(label["height"], 25.0);
        assert_eq!(estimate, (width.ceil() as u32, 25));
    }
}

#[test]
fn all_anchors_and_rotations_place_the_requested_line_box_anchor() {
    use plotters_backend::text_anchor::{HPos, Pos, VPos};
    for (h, ax) in [(HPos::Left, 0.0), (HPos::Center, 0.5), (HPos::Right, 1.0)] {
        for (v, ay) in [(VPos::Top, 0.0), (VPos::Center, 0.5), (VPos::Bottom, 1.0)] {
            for rotation in [
                FontTransform::None,
                FontTransform::Rotate90,
                FontTransform::Rotate180,
                FontTransform::Rotate270,
            ] {
                let mut scene = Scene::new();
                {
                    let mut backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
                    let style = TextStyle::from(("Excalifont", 19).into_font())
                        .pos(Pos::new(h, v))
                        .transform(rotation);
                    assert_eq!(backend.estimate_text_size("AV", &style).unwrap(), (25, 24));
                    backend.draw_text("AV", &style, (123, 87)).unwrap();
                }
                let doc: Value = serde_json::from_slice(&scene.to_bytes().unwrap()).unwrap();
                let e = &doc["elements"][0];
                let (w, h) = (e["width"].as_f64().unwrap(), e["height"].as_f64().unwrap());
                let angle = e["angle"].as_f64().unwrap();
                let (dx, dy) = ((ax - 0.5) * w, (ay - 0.5) * h);
                let x = e["x"].as_f64().unwrap() + w / 2.0 + dx * angle.cos() - dy * angle.sin();
                let y = e["y"].as_f64().unwrap() + h / 2.0 + dx * angle.sin() + dy * angle.cos();
                assert!((x - 123.0).abs() < 1e-9 && (y - 87.0).abs() < 1e-9);
                assert_eq!(h, 23.75);
            }
        }
    }
}

#[test]
fn unsupported_glyphs_report_the_codepoint_and_poison_export() {
    for (text, code) in [
        ("Ω", "U+03A9"),
        ("µs", "U+00B5"), // Micro sign is absent from the pinned editor's font.
        ("μs", "U+03BC"), // Greek mu is a different, unsupported codepoint.
        ("à", "U+00E0"),  // Coverage alone does not expand the supported policy.
        ("e\u{301}", "U+0301"),
        ("\n", "U+000A"),
        ("😀", "U+1F600"),
    ] {
        let mut scene = Scene::new();
        let backend = ExcalidrawBackend::new(&mut scene, (640, 400)).unwrap();
        let error = backend
            .estimate_text_size(text, &TextStyle::from(("Excalifont", 20).into_font()))
            .unwrap_err();
        assert!(error.to_string().contains(code));
        assert!(scene.to_bytes().is_err());
    }
}
