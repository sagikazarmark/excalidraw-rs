use excalidraw_document::{
    Document, Element, ElementId, ElementKind, ExportMode, Number, Profile, Purpose, Severity,
};
use serde_json::{Value, json};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];
const PURPOSES: [Purpose; 3] = [Purpose::Inspect, Purpose::Author, Purpose::SelfContained];

fn element(profile: Profile, kind: ElementKind, id: &str) -> Element {
    Element::new(kind, profile, ElementId::from(id), Number::from(1_u64)).unwrap()
}

fn scene(elements: Vec<Element>) -> Document {
    let mut document = Document::new("review-regression");
    document.set_elements(elements);
    document
}

fn label_pair(profile: Profile) -> (Element, Element) {
    let mut container = element(profile, ElementKind::Rectangle, "box");
    container.set_raw("boundElements", json!([{"id":"label","type":"text"}]));
    let mut label = element(profile, ElementKind::Text, "label");
    label.set_raw("containerId", json!("box"));
    (container, label)
}

fn binding(profile: Profile, target: &str) -> Value {
    match profile {
        Profile::V0_18_1 => json!({"elementId":target,"focus":0,"gap":1}),
        Profile::SnapshotAfa3a653 => {
            json!({"elementId":target,"fixedPoint":[0,0.5],"mode":"orbit"})
        }
    }
}

fn assert_compatibility(document: &Document, profile: Profile, path: &str, code: &str) {
    let before = document.to_vec().unwrap();
    for purpose in PURPOSES {
        let report = document.validate(profile, purpose);
        let inspecting = purpose == Purpose::Inspect;
        assert_eq!(
            report.is_valid(),
            inspecting,
            "{profile:?} {purpose:?}: {report:?}"
        );
        assert!(
            report.diagnostics.iter().any(|d| d.path == path
                && d.code == code
                && d.severity
                    == if inspecting {
                        Severity::Warning
                    } else {
                        Severity::Error
                    }),
            "{profile:?} {purpose:?}: {report:?}"
        );
        assert_eq!(document.validated(profile, purpose).is_ok(), inspecting);
    }
    assert_eq!(document.to_vec().unwrap(), before);
}

fn assert_valid(document: &Document, profile: Profile) {
    for purpose in PURPOSES {
        let report = document.validate(profile, purpose);
        assert!(
            report.diagnostics.is_empty(),
            "{profile:?} {purpose:?}: {report:?}"
        );
    }
}

#[test]
fn live_relationships_cannot_target_tombstones_in_checked_authored_scenes() {
    for profile in PROFILES {
        let (mut container, label) = label_pair(profile);
        container.set_raw("isDeleted", json!(true));
        assert_compatibility(
            &scene(vec![container, label]),
            profile,
            "/elements/1/containerId",
            "deleted-reference",
        );

        let mut frame = element(profile, ElementKind::Frame, "frame");
        frame.set_raw("isDeleted", json!(true));
        let mut child = element(profile, ElementKind::Rectangle, "child");
        child.set_raw("frameId", json!("frame"));
        assert_compatibility(
            &scene(vec![frame, child]),
            profile,
            "/elements/1/frameId",
            "deleted-reference",
        );

        for field in ["startBinding", "endBinding"] {
            let mut target = element(profile, ElementKind::Rectangle, "target");
            target.set_raw("isDeleted", json!(true));
            let mut arrow = element(profile, ElementKind::Arrow, "arrow");
            arrow.set_raw(field, binding(profile, "target"));
            assert_compatibility(
                &scene(vec![target, arrow]),
                profile,
                &format!("/elements/1/{field}/elementId"),
                "deleted-reference",
            );
        }

        for kind in [ElementKind::Text, ElementKind::Arrow] {
            let mut target = element(profile, kind.clone(), "deleted");
            target.set_raw("isDeleted", json!(true));
            let mut container = element(profile, ElementKind::Rectangle, "box");
            container.set_raw("boundElements", json!([{"id":"deleted","type":if kind == ElementKind::Text {"text"} else {"arrow"}}]));
            assert_compatibility(
                &scene(vec![container, target]),
                profile,
                "/elements/0/boundElements/0/id",
                "deleted-reference",
            );
        }
    }
}

#[test]
fn deleted_sources_preserve_historical_edges_without_live_reciprocity() {
    for profile in PROFILES {
        let (mut container, mut label) = label_pair(profile);
        container.set_raw("boundElements", json!([]));
        label.set_raw("isDeleted", json!(true));
        let mut arrow = element(profile, ElementKind::Arrow, "old-arrow");
        arrow.set_raw("isDeleted", json!(true));
        arrow.set_raw("startBinding", binding(profile, "box"));
        arrow.set_raw("endBinding", binding(profile, "removed"));
        let mut old_container = element(profile, ElementKind::Rectangle, "old-box");
        old_container.set_raw("isDeleted", json!(true));
        old_container.set_raw(
            "boundElements",
            json!([{"id":"label","type":"text"},{"id":"removed","type":"arrow"}]),
        );
        old_container.set_raw("frameId", json!("removed-frame"));
        let document = scene(vec![label, container, arrow, old_container]);
        let before = document.to_vec().unwrap();
        assert_valid(&document, profile);
        let projected = document
            .validated(profile, Purpose::SelfContained)
            .unwrap()
            .project_native(ExportMode::Local, "test")
            .unwrap()
            .document;
        assert_valid(&projected, profile);
        assert_eq!(
            projected.elements().unwrap().len(),
            if profile == Profile::V0_18_1 { 1 } else { 4 }
        );
        assert_eq!(document.to_vec().unwrap(), before);
    }
}

#[test]
fn bound_text_is_not_an_arrow_target_but_standalone_text_is() {
    for profile in PROFILES {
        for field in ["startBinding", "endBinding"] {
            let (container, mut label) = label_pair(profile);
            label.set_raw("boundElements", json!([{"id":"arrow","type":"arrow"}]));
            let mut arrow = element(profile, ElementKind::Arrow, "arrow");
            arrow.set_raw(field, binding(profile, "label"));
            assert_compatibility(
                &scene(vec![container, label.clone(), arrow.clone()]),
                profile,
                &format!("/elements/2/{field}/elementId"),
                "non-bindable-target",
            );
            label.set_raw("containerId", Value::Null);
            assert_valid(&scene(vec![label, arrow]), profile);
        }
    }
}

#[test]
fn containers_have_at_most_one_live_label_and_report_both_conflicting_paths() {
    for profile in PROFILES {
        let (mut container, label) = label_pair(profile);
        let mut second = element(profile, ElementKind::Text, "second");
        second.set_raw("containerId", json!("box"));
        container.set_raw(
            "boundElements",
            json!([{"id":"label","type":"text"},{"id":"second","type":"text"}]),
        );
        let document = scene(vec![container.clone(), label.clone(), second.clone()]);
        assert_compatibility(
            &document,
            profile,
            "/elements/2/containerId",
            "multiple-labels",
        );
        assert!(
            document
                .validate(profile, Purpose::Author)
                .diagnostics
                .iter()
                .any(|d| d.code == "multiple-labels"
                    && d.message.contains("/elements/1/containerId"))
        );
        second.set_raw("isDeleted", json!(true));
        container.set_raw("boundElements", json!([{"id":"label","type":"text"}]));
        assert_valid(&scene(vec![container, label, second]), profile);
    }
}

#[test]
fn live_label_order_is_checked_even_with_monotonic_or_missing_indices() {
    for profile in PROFILES {
        for indexed in [false, true] {
            let (mut container, mut label) = label_pair(profile);
            if indexed {
                label.set_raw("index", json!("a0"));
                container.set_raw("index", json!("a1"));
            }
            assert_compatibility(
                &scene(vec![label.clone(), container.clone()]),
                profile,
                "/elements/0/containerId",
                "bound-text-order",
            );
            if indexed {
                container.set_raw("index", json!("a0"));
                label.set_raw("index", json!("a2"));
            }
            let mut middle = element(profile, ElementKind::Rectangle, "middle");
            if indexed {
                middle.set_raw("index", json!("a1"));
            }
            let document = scene(vec![container, middle, label]);
            assert_valid(&document, profile);
            let exported = document
                .validated(profile, Purpose::SelfContained)
                .unwrap()
                .project_native(ExportMode::Local, "test")
                .unwrap()
                .document;
            assert_valid(&exported, profile);
            assert_eq!(
                exported.as_object()["elements"],
                document.as_object()["elements"]
            );
        }
    }
}

#[test]
fn authored_revisions_must_be_positive_without_rewriting_historical_values() {
    for profile in PROFILES {
        for revision in [0_i64, -1] {
            let mut shape = element(profile, ElementKind::Rectangle, "box");
            shape.set_raw("version", json!(revision));
            assert_compatibility(&scene(vec![shape]), profile, "/elements/0/version", "range");
        }
        for revision in [1_i64, 9_007_199_254_740_991] {
            let mut shape = element(profile, ElementKind::Rectangle, "box");
            shape.set_raw("version", json!(revision));
            assert_valid(&scene(vec![shape]), profile);
        }
    }
}
