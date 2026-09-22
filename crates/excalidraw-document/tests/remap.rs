use excalidraw_document::{Document, IdMap, OpaquePolicy};
use serde_json::json;

#[test]
fn remapping_updates_all_known_identity_domains_without_touching_geometry_or_metadata() {
    let doc=Document::from_value(json!({"type":"excalidraw","elements":[
        {"id":"box","type":"rectangle","frameId":"frame","groupIds":["g"],"boundElements":[{"id":"text","type":"text"},{"id":"arrow","type":"arrow"}],"customData":{"sourceId":"box"}},
        {"id":"text","type":"text","containerId":"box","text":"box","x":1},
        {"id":"arrow","type":"arrow","startBinding":{"elementId":"box","focus":0,"gap":1},"endBinding":null},
        {"id":"image","type":"image","fileId":"asset"},{"id":"frame","type":"frame"}
    ],"appState":{"lockedMultiSelections":{"g":true}},"files":{"asset":{"id":"asset","dataURL":"unchanged"}}})).unwrap();
    let mut map = IdMap::default();
    for id in ["box", "text", "arrow", "image", "frame"] {
        map.elements
            .insert(id.into(), format!("new-{id}").as_str().into());
    }
    map.groups.insert("g".into(), "new-g".into());
    map.files.insert("asset".into(), "new-asset".into());
    assert!(doc.remap_ids(&map, OpaquePolicy::Reject).is_err());
    let result = doc.remap_ids(&map, OpaquePolicy::Preserve).unwrap();
    let v = result.document.into_value();
    assert_eq!(v["elements"][0]["boundElements"][0]["id"], "new-text");
    assert_eq!(v["elements"][0]["frameId"], "new-frame");
    assert_eq!(v["elements"][1]["containerId"], "new-box");
    assert_eq!(v["elements"][2]["startBinding"]["elementId"], "new-box");
    assert_eq!(v["elements"][3]["fileId"], "new-asset");
    assert_eq!(v["files"]["new-asset"]["id"], "new-asset");
    assert_eq!(
        v["appState"]["lockedMultiSelections"],
        json!({"new-g":true})
    );
    assert_eq!(v["elements"][0]["customData"], json!({"sourceId":"box"}));
    assert_eq!(v["elements"][1]["text"], "box");
    assert!(!result.unassessed_paths.is_empty());
    assert_eq!(doc.as_object()["elements"][0]["id"], "box");
}

#[test]
fn remapping_rejects_collisions_duplicates_and_dangling_references_atomically() {
    for elements in [
        json!([{"id":"a","type":"rectangle"},{"id":"b","type":"rectangle"}]),
        json!([{"id":"a","type":"rectangle"},{"id":"a","type":"rectangle"}]),
        json!([{"id":"a","type":"text","containerId":"missing"}]),
    ] {
        let doc = Document::from_value(json!({"type":"excalidraw","elements":elements})).unwrap();
        let before = doc.clone();
        let mut map = IdMap::default();
        map.elements.insert("a".into(), "b".into());
        assert!(doc.remap_ids(&map, OpaquePolicy::Preserve).is_err());
        assert_eq!(doc, before);
    }
}

#[test]
fn unknown_kind_properties_are_opaque_even_when_their_names_resemble_references() {
    let doc=Document::from_value(json!({"type":"excalidraw","elements":[{"type":"future","id":"a","fileId":"host-owned","containerId":"external","groupIds":{"host":"value"}}]})).unwrap();
    let mut map = IdMap::default();
    map.elements.insert("a".into(), "b".into());
    let result = doc.remap_ids(&map, OpaquePolicy::Preserve).unwrap();
    assert_eq!(
        result.document.as_object()["elements"][0],
        json!({"type":"future","id":"b","fileId":"host-owned","containerId":"external","groupIds":{"host":"value"}})
    );
    assert!(
        result
            .unassessed_paths
            .contains(&"/elements/0/type".to_owned())
    );
}

/// The vocabulary and the reference classification must stay in step.
///
/// These two assertions are the guardrail for a specific failure: adding a field
/// to the `fields!` vocabulary used to *remove* it from the unassessed set, so a
/// new reference-carrying field would be silently preserved by `remap_ids` and
/// pass `OpaquePolicy::Reject`. Classification is now separate, and a field
/// added without a row here fails this test instead of weakening the guarantee.
#[test]
fn every_element_field_has_a_reference_classification() {
    let unclassified: Vec<&str> = excalidraw_document::element_fields()
        .iter()
        .copied()
        .filter(|field| !excalidraw_document::classified_element_fields().contains(field))
        .collect();
    assert!(
        unclassified.is_empty(),
        "these element fields have no reference classification: {unclassified:?}"
    );
}

#[test]
fn the_reference_classification_names_no_field_outside_the_vocabulary() {
    let stale: Vec<&str> = excalidraw_document::classified_element_fields()
        .iter()
        .copied()
        .filter(|field| !excalidraw_document::element_fields().contains(field))
        .collect();
    assert!(
        stale.is_empty(),
        "these classified names are not element fields: {stale:?}"
    );
}
