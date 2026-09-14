use excalidraw_document::{
    Document,
    plus::{PatchSceneContent, ReplaceSceneContent, SceneContent},
};
use serde_json::json;
#[test]
fn plus_metadata_stays_outside_native_document_and_patch_has_only_action_fields() {
    let input=br##"{"type":"excalidraw","version":2,"source":"test","elements":[],"appState":{"viewBackgroundColor":"#fff"},"files":{},"sceneVersion":"opaque:version","filesFailedToEmbed":["missing"]}"##;
    let response = SceneContent::from_slice(input).unwrap();
    assert_eq!(response.scene_version(), "opaque:version");
    assert_eq!(
        response.files_failed_to_embed(),
        Some(&["missing".to_owned()][..])
    );
    assert!(
        response
            .document()
            .as_object()
            .get("sceneVersion")
            .is_none()
    );
    assert_eq!(
        SceneContent::from_slice(&response.to_vec().unwrap()).unwrap(),
        response
    );
    let replace = ReplaceSceneContent::new(response.document().clone()).unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&replace.to_vec().unwrap())
            .unwrap()
            .get("sceneVersion")
            .is_none()
    );
    let patch = PatchSceneContent::from_value(json!({"elements":[]})).unwrap();
    assert_eq!(
        patch.as_object(),
        json!({"elements":[]}).as_object().unwrap()
    );
    assert!(PatchSceneContent::from_value(json!({})).is_err());
    assert!(PatchSceneContent::from_value(json!({"elements":[],"sceneVersion":"v"})).is_err());
}
#[test]
fn plus_adapter_reports_constraints_instead_of_silently_dropping_native_state() {
    assert!(PatchSceneContent::from_value(json!({"elements":[{}]})).is_err());
    assert!(
        PatchSceneContent::from_value(
            json!({"elements":[{"type":"arrow","startBinding":{"mode":"bad"}}]})
        )
        .is_err()
    );
    let uncertain =
        PatchSceneContent::from_value(json!({"elements":[{"type":"stickynote","baseHeight":20}]}))
            .unwrap();
    assert_eq!(
        uncertain.unconfirmed_element_paths(),
        vec!["/elements/0/type"]
    );
    let mut document = Document::new("test");
    document
        .set_root(
            "appState",
            json!({"viewBackgroundColor":"white","gridSize":20}),
        )
        .unwrap();
    assert!(ReplaceSceneContent::new(document).is_err());
    assert!(
        PatchSceneContent::from_value(json!({"appState":{"lockedMultiSelections":{"g":false}}}))
            .is_err()
    );
    for created in [0, -1] {
        assert!(PatchSceneContent::from_value(json!({"files":{"a":{"id":"a","mimeType":"image/png","dataURL":"x","created":created}}})).is_err());
    }
    assert!(
        PatchSceneContent::from_value(
            json!({"files":{"a":{"id":"bad id","mimeType":"image/png","dataURL":"x","created":1}}})
        )
        .is_err()
    );
}
