use excalidraw_document::{Document, Profile, Purpose};
use serde_json::json;

#[test]
fn schema_versions_use_exact_integer_semantics() {
    for version in ["2.0000000000000001", "1.99999999999999999"] {
        let document=Document::from_slice(format!(r#"{{"type":"excalidraw","version":{version},"source":"test","elements":[],"appState":{{}},"files":{{}}}}"#).as_bytes()).unwrap();
        assert!(
            !document
                .validate(Profile::V0_18_1, Purpose::Author)
                .is_valid()
        );
        assert!(
            excalidraw_document::LibraryDocument::from_slice(
                format!(r#"{{"type":"excalidrawlib","version":{version},"libraryItems":[]}}"#)
                    .as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn independent_collections_and_nested_numeric_fields_are_always_validated() {
    let document = Document::from_value(
        json!({"type":"excalidraw","appState":{"gridSize":"bad"},"files":{"x":17}}),
    )
    .unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/appState/gridSize")
    );
    assert!(report.diagnostics.iter().any(|d| d.path == "/files/x"));
    let document = Document::from_slice(br#"{"type":"excalidraw","elements":[{"type":"line","points":[[0,0],[1e400,1]]},{"type":"iframe","customData":{"generationData":7}}],"files":{"f":{"id":"f","mimeType":"image/png","dataURL":"x","created":1e400,"version":null}}}"#).unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    for path in [
        "/elements/0/points/1/0",
        "/elements/1/customData/generationData",
        "/files/f/created",
        "/files/f/version",
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.path == path),
            "{path}: {report:?}"
        );
    }
}

#[test]
fn graph_diagnostics_distinguish_missing_wrong_kind_and_reciprocal_links() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[
        {"type":"rectangle","id":"box","frameId":"text"},
        {"type":"text","id":"text","containerId":"box"},
        {"type":"arrow","id":"arrow","startBinding":{"elementId":"gone","fixedPoint":[0,0],"mode":"inside"}},
        {"type":"image","id":"image","fileId":"missing"}
    ],"appState":{},"files":{}})).unwrap();
    let before = document.clone();
    let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for (path, code) in [
        ("/elements/0/frameId", "reference-kind"),
        ("/elements/1/containerId", "reciprocal-binding"),
        ("/elements/2/startBinding/elementId", "missing-reference"),
        ("/elements/3/fileId", "missing-file"),
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == code),
            "{path}: {report:?}"
        );
    }
    assert_eq!(document, before);
}

#[test]
fn malformed_nested_fields_and_profile_mismatches_are_reported() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[
        {"type":"stickynote","id":"sticky","baseHeight":120},
        {"type":"arrow","id":"a","startBinding":{"elementId":"sticky","fixedPoint":[0,1],"mode":"inside"},"endArrowhead":"cardinality_zero_or_many"},
        {"type":"image","id":"i","crop":{"naturalWidth":"bad"}}
    ],"appState":{"lockedMultiSelections":{"g":false}},"files":{}})).unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/0/type" && d.code == "profile")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/2/crop/naturalWidth" && d.code == "field-type")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/appState/lockedMultiSelections/g" && d.code == "literal")
    );
}
