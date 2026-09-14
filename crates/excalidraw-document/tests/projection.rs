use excalidraw_document::{Document, ExportMode, Profile};
use serde_json::json;

#[test]
fn frozen_public_editor_outputs_match_each_profile() {
    let document = Document::from_slice(include_bytes!("fixtures/projection.json")).unwrap();
    for (profile, expected) in [
        (
            Profile::V0_18_1,
            include_bytes!("fixtures/release-local.json").as_slice(),
        ),
        (
            Profile::SnapshotAfa3a653,
            include_bytes!("fixtures/snapshot-local.json").as_slice(),
        ),
    ] {
        let expected = Document::from_slice(expected).unwrap();
        assert_eq!(
            document
                .project_native(profile, ExportMode::Local, "document-oracle")
                .unwrap()
                .document,
            expected
        );
    }
}

#[test]
fn local_resource_projection_matches_javascript_property_keys_and_prototype_setters() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[{"fileId":1},{"fileId":"__proto__"}],"appState":{},"files":{"1":{"id":"1"},"__proto__":{"id":"__proto__"}}})).unwrap();
    let projected = document
        .project_native(Profile::V0_18_1, ExportMode::Local, "test")
        .unwrap();
    assert_eq!(
        projected.document.as_object()["files"],
        json!({"1":{"id":"1"}})
    );
    let bad=Document::from_value(json!({"type":"excalidraw","elements":[{"fileId":{"toString":null}}],"appState":{},"files":{}})).unwrap();
    assert_eq!(
        bad.project_native(Profile::V0_18_1, ExportMode::Local, "test")
            .unwrap_err()
            .path,
        "/elements/0/fileId"
    );
    assert!(
        bad.project_native(Profile::V0_18_1, ExportMode::Database, "test")
            .is_ok()
    );
}

#[test]
fn released_export_cleans_only_its_document_projection_and_reports_replacements() {
    let document = Document::from_value(json!({"type":"excalidraw","version":7,"source":"original","extra":true,
        "elements":[{"type":"line","id":"l","lastCommittedPoint":[1,2]},
        {"type":"image","id":"i","fileId":"shared"}, {"type":"image","id":"d","fileId":"dead","isDeleted":true}],
        "appState":{"viewBackgroundColor":"red","gridModeEnabled":false,"zoom":{"value":2},"lockedMultiSelections":{"g":true}},
        "files":{"shared":{"id":"shared","future":42},"dead":{"id":"dead"},"orphan":{"id":"orphan"}}})).unwrap();
    let before = document.clone();
    let result = document
        .project_native(Profile::V0_18_1, ExportMode::Local, "test")
        .unwrap();
    assert_eq!(
        result.document.into_value(),
        json!({"type":"excalidraw","version":2,"source":"test",
        "elements":[{"type":"line","id":"l","lastCommittedPoint":null},{"type":"image","id":"i","fileId":"shared"}],
        "appState":{"viewBackgroundColor":"red","gridModeEnabled":false},"files":{"shared":{"id":"shared","future":42}}})
    );
    assert!(
        result
            .changes
            .iter()
            .any(|c| c.path == "/elements/0/lastCommittedPoint"
                && c.before == Some(json!([1, 2]))
                && c.after == Some(json!(null)))
    );
    assert!(
        result
            .changes
            .iter()
            .any(|c| c.path == "/source" && c.before == Some(json!("original")))
    );
    assert_eq!(document, before);
    let snapshot = document
        .project_native(Profile::SnapshotAfa3a653, ExportMode::Database, "test")
        .unwrap()
        .document;
    assert_eq!(
        snapshot.as_object()["elements"],
        document.as_object()["elements"]
    );
    assert!(snapshot.as_object().get("files").is_none());
}

#[test]
fn native_projection_refuses_numbers_javascript_would_change() {
    for value in [
        "9007199254740993",
        "1e400",
        "0.10000000000000000001",
        "920741316108827.3",
    ] {
        let input = format!(
            r#"{{"type":"excalidraw","elements":[],"appState":{{}},"files":{{}},"extra":{value}}}"#
        );
        let document = Document::from_slice(input.as_bytes()).unwrap();
        assert_eq!(
            document
                .project_native(Profile::V0_18_1, ExportMode::Local, "test")
                .unwrap_err()
                .path,
            "/extra"
        );
    }
    let document = Document::from_slice(br#"{"type":"excalidraw","elements":[],"appState":{},"files":{},"extra":920741316108827.2}"#).unwrap();
    assert!(
        document
            .project_native(Profile::V0_18_1, ExportMode::Local, "test")
            .is_ok()
    );
}
