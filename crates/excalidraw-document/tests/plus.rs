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
        assert!(PatchSceneContent::from_value(json!({"files":{"a":{"id":"a","mimeType":"image/png","dataURL":"data:image/png;base64,AA==","created":created}}})).is_err());
    }
    assert!(
        PatchSceneContent::from_value(
            json!({"files":{"a":{"id":"bad id","mimeType":"image/png","dataURL":"data:image/png;base64,AA==","created":1}}})
        )
        .is_err()
    );
}

/// Build a Plus-shaped document carrying exactly one file record.
fn scene_with_file(record: serde_json::Value) -> Document {
    Document::from_value(json!({
        "type":"excalidraw","version":2,"source":"test","elements":[],
        "appState":{"viewBackgroundColor":"#fff"},
        "files":{"f":record}
    }))
    .unwrap()
}

#[test]
fn transport_refuses_a_file_record_contradicting_its_own_mime_type() {
    // The envelope is what actually reaches the server, and nothing obliges a
    // caller to run `Document::validate` first: a record whose dataURL does not
    // carry its declared type would otherwise be uploaded as an unusable image.
    for data_url in [
        "x",
        "data:image/jpeg;base64,AQ==",
        "data:image/png;base64,",
        "data:,AA",
    ] {
        let record =
            json!({"id":"f","mimeType":"image/png","dataURL":data_url,"created":1,"version":1});
        assert!(
            ReplaceSceneContent::new(scene_with_file(record.clone())).is_err(),
            "PUT accepted {data_url}"
        );
        assert!(
            PatchSceneContent::from_value(json!({"files":{"f":record}})).is_err(),
            "PATCH accepted {data_url}"
        );
    }
}

#[test]
fn transport_agreement_on_the_media_type_is_ascii_case_insensitive() {
    // RFC 2045 §5.1: type/subtype names are case-insensitive. The transport
    // asks the same predicate `Document::validate` does, so it inherits that
    // rather than re-introducing a case-sensitive comparison of its own.
    for (mime, data_url) in [
        ("image/png", "data:IMAGE/PNG;base64,AA=="),
        ("image/png", "data:Image/Png,x"),
        ("image/svg+xml", "data:image/svg+xml;charset=utf-8,<svg/>"),
    ] {
        let record = json!({"id":"f","mimeType":mime,"dataURL":data_url,"created":1,"version":1});
        assert!(
            ReplaceSceneContent::new(scene_with_file(record.clone())).is_ok(),
            "PUT rejected {data_url}"
        );
        assert!(
            PatchSceneContent::from_value(json!({"files":{"f":record}})).is_ok(),
            "PATCH rejected {data_url}"
        );
    }
}

#[test]
fn transport_accepts_a_self_consistent_but_unrecognised_mime_type() {
    // Deliberate, and the counterpart to the contradiction rule above. The
    // published Plus schema constrains `mimeType` to "string" with no enum or
    // pattern, so `MimeType::KNOWN` is this crate's vocabulary rather than the
    // transport's: rejecting on it would refuse records the service accepts —
    // a format newer than the crate, or the uppercase spelling below that
    // RFC 2045 makes equivalent but `KNOWN` (case-sensitive by design) omits.
    // `Document::validate` still reports these, as a warning under Inspect and
    // Author and an error only under `Purpose::SelfContained`; the transport
    // does not turn that judgement into a hard refusal it cannot justify.
    for (mime, data_url) in [
        ("image/future", "data:image/future,AA"),
        ("IMAGE/PNG", "data:image/png;base64,AA=="),
    ] {
        let record = json!({"id":"f","mimeType":mime,"dataURL":data_url,"created":1,"version":1});
        assert!(
            ReplaceSceneContent::new(scene_with_file(record.clone())).is_ok(),
            "PUT rejected {mime}"
        );
        assert!(
            PatchSceneContent::from_value(json!({"files":{"f":record}})).is_ok(),
            "PATCH rejected {mime}"
        );
    }
}

#[test]
fn a_valid_file_record_still_round_trips_through_the_transport_unchanged() {
    let record = json!({
        "id":"f","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgo=",
        "created":1789000000000_i64,"lastRetrieved":1789000000500_i64,"version":2
    });
    let files = json!({"f":record});
    let replace = ReplaceSceneContent::new(scene_with_file(record.clone())).unwrap();
    let sent: serde_json::Value = serde_json::from_slice(&replace.to_vec().unwrap()).unwrap();
    assert_eq!(sent["files"], files);
    let patch = PatchSceneContent::from_value(json!({"files":files})).unwrap();
    assert_eq!(patch.as_object()["files"], files);
}
