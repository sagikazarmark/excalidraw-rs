use excalidraw_document::{Document, Profile};
use serde_json::json;

#[test]
fn exact_profile_migration_reports_changes_and_preserves_input() {
    let input=Document::from_value(json!({"type":"excalidraw","elements":[{"type":"arrow","id":"a","startBinding":null,"endBinding":null,"endArrowhead":"crowfoot_many","lastCommittedPoint":[1,2]},{"type":"line","id":"l"},{"type":"text","id":"t"}]})).unwrap();
    let result = input
        .migrate(Profile::V0_18_1, Profile::SnapshotAfa3a653)
        .unwrap();
    assert_eq!(
        result.document.as_object()["elements"][0]["endArrowhead"],
        "cardinality_many"
    );
    assert!(
        result.document.as_object()["elements"][0]
            .get("lastCommittedPoint")
            .is_none()
    );
    assert_eq!(result.document.as_object()["elements"][1]["polygon"], false);
    assert_eq!(
        result.document.as_object()["elements"][2]["baseFontSize"],
        json!(null)
    );
    assert!(!result.changes.is_empty());
    assert_eq!(
        input.as_object()["elements"][0]["endArrowhead"],
        "crowfoot_many"
    );
}

#[test]
fn geometric_bindings_and_lossy_downgrades_are_explicit_blockers() {
    for font in ["10", "10.0", "1e1"] {
        let input = Document::from_slice(
            format!(
                r#"{{"type":"excalidraw","elements":[{{"type":"text","fontFamily":{font}}}]}}"#
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(
            input
                .migrate(Profile::SnapshotAfa3a653, Profile::V0_18_1)
                .is_err()
        );
    }
    let input=Document::from_value(json!({"type":"excalidraw","elements":[{"type":"arrow","startBinding":{"elementId":"x","focus":0,"gap":1}}]})).unwrap();
    assert_eq!(
        input
            .migrate(Profile::V0_18_1, Profile::SnapshotAfa3a653)
            .unwrap_err()
            .path,
        "/elements/0/startBinding"
    );
    for element in [
        json!({"type":"stickynote"}),
        json!({"type":"line","polygon":true}),
        json!({"type":"arrow","endArrowhead":"cardinality_zero_or_many"}),
    ] {
        let input =
            Document::from_value(json!({"type":"excalidraw","elements":[element]})).unwrap();
        assert!(
            input
                .migrate(Profile::SnapshotAfa3a653, Profile::V0_18_1)
                .is_err()
        );
    }
}
