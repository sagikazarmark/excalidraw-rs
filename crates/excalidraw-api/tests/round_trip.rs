//! The property the crate exists for: download, convert, upload, without loss.
use excalidraw_api::{
    Error, Operation, SceneId, op,
    plus::SceneContent,
    scene_content::{self, ElementIds, Embedding, PatchFields, PlusProjection},
};
use excalidraw_document::Document;
use serde_json::{Value, json};

const COMPLETE: &[u8] = include_bytes!("fixtures/scene_content.json");
const PARTIAL: &[u8] = include_bytes!("fixtures/scene_content_partial.json");
const NATIVE: &[u8] = include_bytes!("fixtures/native_export.json");

fn value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("fixture parses")
}

/// A GET body minus the two transport-only fields: what a PUT body must equal.
fn expected_put_body(bytes: &[u8]) -> Value {
    let mut root = value(bytes);
    let map = root.as_object_mut().expect("object");
    map.remove("sceneVersion");
    map.remove("filesFailedToEmbed");
    root
}

#[test]
fn get_to_put_preserves_the_whole_payload() {
    let downloaded = SceneContent::from_slice(COMPLETE).expect("fixture is a valid GET body");
    let body = scene_content::into_replacement(downloaded, Embedding::RequireComplete)
        .expect("a complete download converts");

    let sent: Value = serde_json::from_slice(&body.to_vec().unwrap()).unwrap();

    // Value equality, not byte equality: the document model is a BTreeMap, so
    // object keys come back sorted. Nothing may be added, dropped or renamed.
    assert_eq!(sent, expected_put_body(COMPLETE));
}

#[test]
fn round_trip_preserves_exact_numbers_and_unknown_fields() {
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    let body = scene_content::into_replacement(downloaded, Embedding::RequireComplete).unwrap();
    let sent = String::from_utf8(body.to_vec().unwrap()).unwrap();

    // Exact spelling survives: not 100.5 -> 100.50000000000001, not 120 -> 120.0.
    assert!(sent.contains("100.5"), "fractional coordinate preserved");
    assert!(sent.contains("1789000000000"), "large integer preserved");
    // An element kind and field the public reference does not list are retained.
    assert!(
        sent.contains("stickynote"),
        "unknown element kind preserved"
    );
    assert!(
        sent.contains("\"created\":1789000000000"),
        "unknown field preserved"
    );
    // A tombstone is a record like any other; nothing prunes it.
    assert!(
        sent.contains("Zv9YtRw4mNbXc7QsLpE2H"),
        "tombstone preserved"
    );
}

#[test]
fn scene_version_is_carried_outside_the_document() {
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    assert_eq!(
        downloaded.scene_version(),
        "8f14e45fceea167a5a36dedd4bea2543"
    );
    assert!(
        downloaded
            .document()
            .as_object()
            .get("sceneVersion")
            .is_none()
    );
}

#[test]
fn unembedded_files_block_a_replacement_by_default() {
    let downloaded = SceneContent::from_slice(PARTIAL).expect("still a valid GET body");
    assert_eq!(
        downloaded.files_failed_to_embed(),
        Some(&["file-abc_123".to_owned()][..])
    );

    let refused = scene_content::into_replacement(downloaded, Embedding::RequireComplete);
    match refused {
        Err(Error::Invalid { what, detail }) => {
            assert_eq!(what, "incomplete embedding");
            assert!(detail.contains("file-abc_123"), "names the lost file");
        }
        other => panic!("expected the guard to refuse, got {other:?}"),
    }
}

#[test]
fn allow_missing_omits_the_files_the_download_could_not_embed() {
    let downloaded = SceneContent::from_slice(PARTIAL).unwrap();
    let body = scene_content::into_replacement(downloaded, Embedding::AllowMissing)
        .expect("the caller accepted the loss");
    let sent: Value = serde_json::from_slice(&body.to_vec().unwrap()).unwrap();

    // State what the body contains rather than glossing it: the file records are
    // absent while elements still reference them, which the service rejects with
    // "Referenced file ... is missing from scene files" (confirmed 2026-09-21).
    assert_eq!(sent["files"], json!({}));
    assert!(sent["files"].as_object().unwrap().is_empty());
}

#[test]
fn native_export_is_refused_by_a_strict_projection() {
    let document = Document::from_slice(NATIVE).unwrap();
    let refused = scene_content::replacement_from_document(&document, &PlusProjection::strict());
    match refused {
        Err(Error::Content(error)) => {
            assert!(
                error.path.starts_with("/appState/grid"),
                "names the offending path, got {}",
                error.path
            );
        }
        other => panic!("expected a path-addressed rejection, got {other:?}"),
    }
}

#[test]
fn pruning_reports_every_dropped_key() {
    let document = Document::from_slice(NATIVE).unwrap();
    let (body, changes) = scene_content::replacement_from_document(
        &document,
        &PlusProjection {
            prune_app_state: true,
            default_view_background: None,
            source: None,
            materialize_files: false,
        },
    )
    .expect("pruning makes it legal");

    let mut dropped: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
    dropped.sort_unstable();
    assert_eq!(
        dropped,
        [
            "/appState/gridModeEnabled",
            "/appState/gridSize",
            "/appState/gridStep"
        ]
    );
    for change in &changes {
        assert!(change.before.is_some(), "the removed value is reported");
        assert!(change.after.is_none(), "removal is reported as removal");
    }

    let sent: Value = serde_json::from_slice(&body.to_vec().unwrap()).unwrap();
    assert_eq!(
        sent["appState"],
        json!({ "viewBackgroundColor": "#ffffff" })
    );
}

#[test]
fn a_generated_document_needs_no_changes() {
    // What this workspace's own generators produce: Document::new writes
    // files:{} and version:2, and the backend writes viewBackgroundColor.
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "excaliplot",
        "elements": [],
        "appState": { "viewBackgroundColor": "#ffffff" },
        "files": {}
    }))
    .unwrap();

    let (_, changes) =
        scene_content::replacement_from_document(&document, &PlusProjection::strict())
            .expect("already a legal Plus body");
    assert!(changes.is_empty(), "a chart upload changes nothing");
}

#[test]
fn a_missing_files_map_is_refused_strictly_and_reported_when_supplied() {
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "test",
        "elements": [],
        "appState": { "viewBackgroundColor": "#ffffff" }
    }))
    .unwrap();

    match scene_content::replacement_from_document(&document, &PlusProjection::strict()) {
        Err(Error::Content(error)) => assert_eq!(error.path, "/files"),
        other => panic!("strict changes nothing, so it must refuse; got {other:?}"),
    }

    let (body, changes) =
        scene_content::replacement_from_document(&document, &PlusProjection::lenient())
            .expect("lenient supplies the map");
    assert_eq!(
        changes.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
        ["/files"]
    );
    let sent: Value = serde_json::from_slice(&body.to_vec().unwrap()).unwrap();
    assert_eq!(sent["files"], json!({}));
}

#[test]
fn patch_bodies_never_carry_envelope_fields() {
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    let document = downloaded.document().clone();

    for fields in [
        PatchFields::elements(),
        PatchFields::app_state(),
        PatchFields::files(),
        PatchFields::elements().with_app_state().with_files(),
    ] {
        let body = scene_content::patch_from(&document, fields, ElementIds::RequireCanonical)
            .expect("selected fields exist");
        let sent: Value = serde_json::from_slice(&body.to_vec().unwrap()).unwrap();
        let keys: Vec<&String> = sent.as_object().unwrap().keys().collect();
        for forbidden in [
            "type",
            "version",
            "source",
            "sceneVersion",
            "filesFailedToEmbed",
        ] {
            assert!(
                !keys.iter().any(|k| k.as_str() == forbidden),
                "PATCH root is closed; {forbidden} must not appear in {keys:?}"
            );
        }
    }
}

#[test]
fn an_empty_patch_selection_is_refused() {
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    let refused = scene_content::patch_from(
        downloaded.document(),
        PatchFields::default(),
        ElementIds::RequireCanonical,
    );
    assert!(matches!(refused, Err(Error::Invalid { what, .. }) if what == "patch fields"));
}

#[test]
fn selecting_an_absent_field_is_a_caller_error_not_an_empty_map() {
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "excaliplot",
        "elements": []
    }))
    .unwrap();
    let refused = scene_content::patch_from(
        &document,
        PatchFields::files(),
        ElementIds::RequireCanonical,
    );
    match refused {
        Err(Error::Invalid { detail, .. }) => assert!(detail.contains("files")),
        other => panic!("expected refusal, got {other:?}"),
    }
}

#[test]
fn put_and_patch_responses_decode_as_scene_content() {
    let scene = SceneId::new("abc").unwrap();
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    let body = scene_content::into_replacement(downloaded, Embedding::RequireComplete).unwrap();

    // A PUT 200 body has no filesFailedToEmbed; the same decoder must accept it.
    let put_response = expected_put_body(COMPLETE);
    let mut with_version = put_response.clone();
    with_version["sceneVersion"] = json!("recomputed-by-server");
    let bytes = serde_json::to_vec(&with_version).unwrap();

    let op = op::ReplaceSceneContent { scene, body };
    let decoded = op
        .decode(200, &excalidraw_api::NoHeaders, &bytes)
        .expect("PUT 200 decodes");
    assert_eq!(decoded.scene_version(), "recomputed-by-server");
}

#[test]
fn a_patch_reusing_a_client_side_id_is_refused() {
    // The service rewrites ids that are not its own 21-character form, so such a
    // merge inserts a duplicate instead of updating. Confirmed 2026-09-21.
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "excaliplot",
        "elements": [
            { "id": "3f2a1c44-0000-4000-8000-000000000001", "type": "rectangle" },
            { "id": 12345, "type": "ellipse" },
            { "type": "diamond" }
        ],
        "appState": { "viewBackgroundColor": "#ffffff" },
        "files": {}
    }))
    .unwrap();

    match scene_content::patch_from(
        &document,
        PatchFields::elements(),
        ElementIds::RequireCanonical,
    ) {
        Err(Error::Invalid { what, detail }) => {
            assert_eq!(what, "provisional element ids");
            assert!(detail.contains("/elements/0"), "names the UUID element");
            assert!(detail.contains("/elements/1"), "names the numeric id");
            assert!(
                detail.contains("/elements/2"),
                "names the element with no id"
            );
        }
        other => panic!("expected the guard to refuse, got {other:?}"),
    }

    // Inserting genuinely new elements is legitimate, and says so.
    scene_content::patch_from(
        &document,
        PatchFields::elements(),
        ElementIds::AllowProvisional,
    )
    .expect("provisional ids are allowed when the caller means to insert");
}

#[test]
fn canonical_ids_from_a_download_pass_the_guard() {
    let downloaded = SceneContent::from_slice(COMPLETE).unwrap();
    for id in downloaded.document().as_object()["elements"]
        .as_array()
        .unwrap()
    {
        let id = id["id"].as_str().unwrap();
        assert!(
            scene_content::is_canonical_element_id(id),
            "the fixture models a real download, so {id} must be canonical"
        );
    }
    scene_content::patch_from(
        downloaded.document(),
        PatchFields::elements(),
        ElementIds::RequireCanonical,
    )
    .expect("elements straight from a download are updatable");
}

#[test]
fn a_generated_chart_still_publishes_through_put() {
    // The guard must not touch replacement: plotters-excalidraw mints UUIDs, and
    // PUT rewrites every id and reference together.
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "excaliplot",
        "elements": [{ "id": "3f2a1c44-0000-4000-8000-000000000001", "type": "rectangle" }],
        "appState": { "viewBackgroundColor": "#ffffff" },
        "files": {}
    }))
    .unwrap();
    scene_content::replacement_from_document(&document, &PlusProjection::strict())
        .expect("provisional ids are correct for a full replacement");
}
