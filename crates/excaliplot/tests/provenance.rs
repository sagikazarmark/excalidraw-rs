use excaliplot::{ExcalidrawBackend, LineChart, Provenance, ReportUrl, Scene, SourceLink};
use plotters::prelude::*;
use serde_json::{Value, json};

fn source() -> SourceLink {
    SourceLink::new(
        ReportUrl::new("https://example.org/reports/latency?window=day#summary").unwrap(),
    )
    .with_provenance(
        Provenance::new(
            "report-label",
            Some("latency"),
            json!({"period": "2026-09"}),
        )
        .unwrap(),
    )
}

fn chart(linked: bool) -> Scene {
    let mut scene = LineChart::new(&[(0., 2.), (1., 5.)], 0.0..1.0, 0.0..6.0)
        .labels("Latency", "Time", "Milliseconds")
        .render()
        .unwrap();
    let options = scene.drawing_options();
    {
        let root = ExcalidrawBackend::new(&mut scene, (640, 440))
            .unwrap()
            .into_drawing_area();
        let source = source();
        options.with_source(linked.then_some(&source), || {
            root.draw(&Text::new(
                "Source report",
                (80, 420),
                ("Excalifont", 18).into_font(),
            ))
            .unwrap();
        });
        root.draw(&Circle::new((600, 420), 4, BLUE.filled()))
            .unwrap();
    }
    scene
}

fn elements(scene: &Scene) -> Vec<Value> {
    serde_json::from_slice::<Value>(&scene.to_bytes().unwrap()).unwrap()["elements"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn explicit_emission_attaches_only_the_report_label_without_changing_chart_geometry() {
    let plain = chart(false);
    let linked = chart(true);
    assert_eq!(plain.bounds().unwrap(), linked.bounds().unwrap());
    assert_eq!(plain.diagnostics().elements, linked.diagnostics().elements);
    let expected = elements(&plain);
    let actual = elements(&linked);
    assert_eq!(actual.len(), expected.len());
    for (mut a, mut b) in actual.into_iter().zip(expected) {
        assert_eq!(b.get("link"), Some(&Value::Null));
        assert!(b.get("customData").is_none());
        if a["text"] == "Source report" {
            assert_eq!(
                a["link"],
                "https://example.org/reports/latency?window=day#summary"
            );
            assert_eq!(
                a["customData"],
                json!({"excaliplot": {
                    "schemaVersion": 1, "role": "report-label", "sourceKey": "latency",
                    "attributes": {"period": "2026-09"}
                }})
            );
            a["link"] = Value::Null;
            a.as_object_mut().unwrap().remove("customData");
        } else {
            assert_eq!(a.get("link"), Some(&Value::Null));
            assert!(a.get("customData").is_none());
        }
        for field in ["id", "groupIds", "updated"] {
            a.as_object_mut().unwrap().remove(field);
            b.as_object_mut().unwrap().remove(field);
        }
        assert_eq!(a, b);
    }
}

#[test]
fn report_urls_reject_unsafe_or_repaired_input_before_drawing() {
    let scene = chart(true);
    let before = scene.to_bytes().unwrap();
    for url in [
        "",
        "/reports/1",
        "//example.org/report",
        "http://example.org",
        "javascript:alert(1)",
        "data:text/html,hello",
        "file:///tmp/report",
        "https:example.org",
        "https:///example.org",
        "https://",
        "https://user:password@example.org/report",
        "https://user@example.org",
        "https://@example.org",
        " https://example.org",
        "https://example.org\n",
        "https://example.org/a b",
        "https://example.org/\tpath",
        "https://example.org\\other",
        "https://example.org/%zz",
        "https://example.org:99999",
        "https://[invalid]/",
    ] {
        assert!(ReportUrl::new(url).is_err(), "accepted {url:?}");
    }
    assert!(ReportUrl::new(&format!("https://example.org/{}", "x".repeat(2048))).is_err());
    for url in [
        "https://example.org",
        "HTTPS://EXAMPLE.ORG:443/a%20b?q=1#part",
        "https://[::1]:8443/report",
    ] {
        assert!(ReportUrl::new(url).is_ok(), "rejected {url:?}");
    }
    assert_eq!(scene.to_bytes().unwrap(), before);
}

#[test]
fn provenance_requires_bounded_objects_and_nonempty_roles_and_source_keys() {
    for attributes in [
        Value::Null,
        json!(false),
        json!(7),
        json!("text"),
        json!([]),
    ] {
        assert!(Provenance::new("mark", None, attributes).is_err());
    }
    for role in ["", " ", "mark\n", &"x".repeat(129)] {
        assert!(Provenance::new(role, None, json!({})).is_err());
    }
    for key in ["", " ", "source\t", &"x".repeat(257)] {
        assert!(Provenance::new("mark", Some(key), json!({})).is_err());
    }
    // Include the complete compact native envelope, not just caller attributes.
    let envelope = r#"{"excaliplot":{"schemaVersion":1,"role":"mark","attributes":{"note":""}}}"#;
    let limit = 4096 - envelope.len();
    assert!(Provenance::new("mark", None, json!({"note": "x".repeat(limit)})).is_ok());
    assert!(Provenance::new("mark", None, json!({"note": "x".repeat(limit + 1)})).is_err());
    assert!(Provenance::new("mark", None, json!({"note": "\n".repeat(limit)})).is_err());
    let mut nested = json!({});
    for _ in 0..17 {
        nested = json!({"child": nested});
    }
    assert!(Provenance::new("mark", None, nested).is_err());
}

#[test]
fn source_scopes_restore_after_errors_and_unwind_and_remain_scene_local() {
    use plotters_backend::DrawingBackend;
    let mut scene = Scene::new();
    let mut other = Scene::new();
    let options = scene.drawing_options();
    let link = source();
    let bare = SourceLink::new(ReportUrl::new("https://example.org/other").unwrap());
    {
        let mut a = ExcalidrawBackend::new(&mut scene, (100, 100)).unwrap();
        let mut b = ExcalidrawBackend::new(&mut other, (100, 100)).unwrap();
        options.with_source(Some(&link), || {
            a.draw_circle((10, 10), 3, &BLUE, true).unwrap();
            b.draw_circle((10, 10), 3, &BLUE, true).unwrap();
            let error: Result<(), &str> = options.with_source(Some(&bare), || {
                a.draw_circle((20, 10), 3, &BLUE, true).unwrap();
                Err("caller error")
            });
            assert!(error.is_err());
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                options.with_source(None, || {
                    a.draw_circle((30, 10), 3, &BLUE, true).unwrap();
                    panic!("caller panic");
                });
            }));
            assert!(panic.is_err());
            a.draw_circle((40, 10), 3, &BLUE, true).unwrap();
        });
        a.draw_circle((50, 10), 3, &BLUE, true).unwrap();
    }
    let e = elements(&scene);
    assert_eq!(e[0]["customData"], e[3]["customData"]);
    assert_eq!(e[0]["link"], e[3]["link"]);
    assert_eq!(e[1]["link"], "https://example.org/other");
    assert!(e[1].get("customData").is_none());
    for unlinked in [&e[2], &e[4], &elements(&other)[0]] {
        assert_eq!(unlinked.get("link"), Some(&Value::Null));
        assert!(unlinked.get("customData").is_none());
    }
}

#[test]
fn placement_composition_and_library_export_preserve_sources_while_remapping_references() {
    let mut report = Scene::with_namespace("report");
    let mut original_ids = std::collections::BTreeSet::new();
    for index in 0..2 {
        let mut panel = chart(true);
        panel
            .add_border(10., excaliplot::BorderStyle::default())
            .unwrap();
        panel.add_frame(10., Some("Linked chart")).unwrap();
        let before = elements(&panel);
        panel.place_at((index as f64 * 800., 100.)).unwrap();
        for (old, placed) in before.iter().zip(elements(&panel)) {
            original_ids.insert(old["id"].as_str().unwrap().to_owned());
            for field in [
                "link",
                "customData",
                "width",
                "height",
                "points",
                "groupIds",
                "frameId",
            ] {
                assert_eq!(old[field], placed[field]);
            }
        }
        let escaped = panel.drawing_options();
        let destination = report.drawing_options();
        // Neither active destination scopes nor escaped source handles retag insertion.
        destination
            .with_source(Some(&source()), || report.append(panel))
            .unwrap();
        escaped.with_source(Some(&source()), || {
            use plotters_backend::DrawingBackend;
            ExcalidrawBackend::new(&mut report, (2000, 1000))
                .unwrap()
                .draw_circle((1900, 900), 2, &BLACK, true)
                .unwrap();
        });
    }
    let e = elements(&report);
    let ids: std::collections::BTreeSet<_> = e.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), e.len());
    assert!(ids.iter().all(|id| !original_ids.contains(*id)));
    let linked: Vec<_> = e.iter().filter(|e| e["link"].is_string()).collect();
    assert_eq!(linked.len(), 2);
    assert_eq!(linked[0]["customData"], linked[1]["customData"]);
    assert_eq!(
        linked[0]["customData"]["excaliplot"]["sourceKey"],
        "latency"
    );
    assert_ne!(linked[0]["groupIds"], linked[1]["groupIds"]);
    assert_ne!(linked[0]["frameId"], linked[1]["frameId"]);
    for element in &e {
        if let Some(frame) = element["frameId"].as_str() {
            assert_eq!(
                e.iter().find(|e| e["id"] == frame).unwrap()["type"],
                "frame"
            );
        }
    }
    let library: Value = serde_json::from_slice(&report.to_library_bytes().unwrap()).unwrap();
    assert_eq!(library["libraryItems"][0]["elements"], json!(e));
}

#[test]
fn invalid_source_construction_never_enters_emission_or_changes_protected_output() {
    use excaliplot::{Error, Overwrite};
    let mut scene = chart(true);
    let before = scene.to_bytes().unwrap();
    let attempt = |scene: &mut Scene| -> Result<(), Error> {
        let link = SourceLink::new(ReportUrl::new("https://example.org/report")?)
            .with_provenance(Provenance::new("mark", None, json!([]))?);
        scene
            .drawing_options()
            .with_source(Some(&link), || scene.add_frame(10., None))
    };
    assert!(attempt(&mut scene).is_err());
    assert_eq!(before, scene.to_bytes().unwrap());
    let directory = tempfile::tempdir().unwrap();
    for library in [false, true] {
        let path = directory.path().join(if library {
            "item.excalidrawlib"
        } else {
            "scene.excalidraw"
        });
        std::fs::write(&path, "manual edits").unwrap();
        let write = |policy| {
            if library {
                scene.write_library(&path, policy)
            } else {
                scene.write(&path, policy)
            }
        };
        assert!(write(Overwrite::Refuse).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"manual edits");
        write(Overwrite::Allow).unwrap();
        let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let e = if library {
            &document["libraryItems"][0]["elements"]
        } else {
            &document["elements"]
        };
        assert_eq!(
            e.as_array()
                .unwrap()
                .iter()
                .filter(|e| e["link"].is_string())
                .count(),
            1
        );
    }
}

#[test]
fn linked_chart_example_emits_one_source_and_protects_manual_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("linked.excalidraw");
    let run = || {
        std::process::Command::new(env!("CARGO"))
            .args(["run", "--locked", "--example", "linked_chart", "--"])
            .arg(&path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let linked: Vec<_> = document["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["link"].is_string())
        .collect();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0]["text"], "Source report");
    assert_eq!(
        linked[0]["customData"]["excaliplot"]["sourceKey"],
        "synthetic-latency"
    );
    std::fs::write(&path, "manually edited chart").unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&path).unwrap(), b"manually edited chart");
}
