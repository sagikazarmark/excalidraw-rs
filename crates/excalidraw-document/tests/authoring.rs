use excalidraw_document::*;
use serde_json::json;

fn element(kind: ElementKind, profile: Profile, id: &str) -> Element {
    Element::new(kind, profile, id.into(), 1_u64.into()).unwrap()
}
fn geometry(profile: Profile) -> BindingGeometry {
    match profile {
        Profile::V0_18_1 => BindingGeometry::Release {
            focus: 0_i64.into(),
            gap: 1_u64.into(),
            fixed_point: None,
        },
        Profile::SnapshotAfa3a653 => BindingGeometry::Snapshot {
            fixed_point: [0_i64.into(), 1_i64.into()],
            mode: BindMode::Orbit,
        },
    }
}
fn by_id(document: &Document, id: &str) -> serde_json::Value {
    document.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap()
        .clone()
}

#[test]
fn labels_rebind_atomically_preserving_unrelated_references_and_order() {
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        let mut doc = Document::new("authoring-test");
        let mut box1 = element(ElementKind::Rectangle, profile, "box1");
        box1.set_raw("customData", json!({"preserve": {"nested": true}}));
        let mut author = doc.author(profile).unwrap();
        author
            .insert(vec![
                element(ElementKind::Text, profile, "label"),
                box1,
                element(ElementKind::Rectangle, profile, "box2"),
                element(ElementKind::Arrow, profile, "arrow"),
                element(ElementKind::Text, profile, "other-label"),
            ])
            .unwrap();
        author
            .bind_arrow(
                &"arrow".into(),
                ArrowEndpoint::End,
                &"box1".into(),
                geometry(profile),
            )
            .unwrap();
        author.bind_label(&"label".into(), &"box1".into()).unwrap();
        let scene = author.document().as_object()["elements"]
            .as_array()
            .unwrap();
        assert_eq!(scene[0]["id"], "box1");
        assert_eq!(scene[1]["id"], "label");
        let before = author.document().clone();
        assert!(
            author
                .bind_label(&"other-label".into(), &"box1".into())
                .is_err()
        );
        assert_eq!(author.document(), &before);
        author.bind_label(&"label".into(), &"box2".into()).unwrap();
        assert_eq!(
            by_id(author.document(), "box1")["boundElements"],
            json!([{"id":"arrow","type":"arrow"}])
        );
        assert_eq!(by_id(author.document(), "label")["containerId"], "box2");
        assert_eq!(
            by_id(author.document(), "box2")["boundElements"],
            json!([{"id":"label","type":"text"}])
        );
        author.unbind_label(&"label".into()).unwrap();
        assert!(by_id(author.document(), "label")["containerId"].is_null());
        assert_eq!(by_id(author.document(), "box2")["boundElements"], json!([]));
        assert_eq!(
            by_id(author.document(), "box1")["customData"],
            json!({"preserve":{"nested":true}})
        );
        assert_eq!(by_id(author.document(), "box1")["version"], 1);
    }
}

#[test]
fn arrow_endpoints_share_backrefs_and_invalid_changes_roll_back() {
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        let mut doc = Document::new("test");
        let mut author = doc.author(profile).unwrap();
        author
            .insert(vec![
                element(ElementKind::Rectangle, profile, "box1"),
                element(ElementKind::Rectangle, profile, "box2"),
                element(ElementKind::Text, profile, "label"),
                element(ElementKind::Arrow, profile, "arrow"),
            ])
            .unwrap();
        author.bind_label(&"label".into(), &"box1".into()).unwrap();
        for endpoint in [ArrowEndpoint::Start, ArrowEndpoint::End] {
            author
                .bind_arrow(&"arrow".into(), endpoint, &"box1".into(), geometry(profile))
                .unwrap();
        }
        let before = author.document().clone();
        for bad_target in ["absent", "label", "arrow"] {
            assert!(
                author
                    .bind_arrow(
                        &"arrow".into(),
                        ArrowEndpoint::End,
                        &bad_target.into(),
                        geometry(profile)
                    )
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
        let wrong_profile = if profile == Profile::V0_18_1 {
            Profile::SnapshotAfa3a653
        } else {
            Profile::V0_18_1
        };
        assert!(
            author
                .bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::End,
                    &"box2".into(),
                    geometry(wrong_profile)
                )
                .is_err()
        );
        assert_eq!(author.document(), &before);
        author
            .bind_arrow(
                &"arrow".into(),
                ArrowEndpoint::End,
                &"box2".into(),
                geometry(profile),
            )
            .unwrap();
        assert_eq!(
            by_id(author.document(), "box1")["boundElements"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        author
            .unbind_arrow(&"arrow".into(), ArrowEndpoint::Start)
            .unwrap();
        assert_eq!(
            by_id(author.document(), "box1")["boundElements"],
            json!([{"id":"label","type":"text"}])
        );
        author
            .unbind_arrow(&"arrow".into(), ArrowEndpoint::End)
            .unwrap();
        assert_eq!(by_id(author.document(), "box2")["boundElements"], json!([]));
        assert!(
            author
                .document()
                .validate(profile, Purpose::SelfContained)
                .is_valid()
        );
    }
}

#[test]
fn text_content_keeps_source_and_wrapped_display_explicit_and_rolls_back_bad_metrics() {
    let profile = Profile::V0_18_1;
    let mut doc = Document::new("test");
    let mut text = element(ElementKind::Text, profile, "label");
    text.set_text_content(TextContent::plain("Old", 60_u64.into(), 25_u64.into()))
        .unwrap();
    let mut author = doc.author(profile).unwrap();
    author.insert(vec![text]).unwrap();
    author
        .replace_text(
            &"label".into(),
            TextContent {
                original: "New source".into(),
                display: "New\nsource".into(),
                width: 70_u64.into(),
                height: 50_u64.into(),
            },
        )
        .unwrap();
    let current = by_id(author.document(), "label");
    assert_eq!(current["originalText"], "New source");
    assert_eq!(current["text"], "New\nsource");
    assert_eq!(current["height"], 50);
    let before = author.document().clone();
    for width in [
        Number::from(-1_i64),
        "1e400".parse().unwrap(),
        "-1e-400".parse().unwrap(),
    ] {
        assert!(
            author
                .replace_text(
                    &"label".into(),
                    TextContent::plain("bad", width, 25_u64.into())
                )
                .is_err()
        );
        assert_eq!(author.document(), &before);
    }
    assert!(
        author
            .replace_text(
                &"absent".into(),
                TextContent::plain("bad", 1_u64.into(), 1_u64.into())
            )
            .is_err()
    );
    assert_eq!(author.document(), &before);
}

#[test]
fn batch_insertion_is_atomic_and_indices_cross_base62_lengths() {
    let profile = Profile::SnapshotAfa3a653;
    let mut doc = Document::new("test");
    let mut author = doc.author(profile).unwrap();
    author
        .insert(
            (0..130)
                .map(|i| element(ElementKind::Rectangle, profile, &format!("box{i}")))
                .collect(),
        )
        .unwrap();
    let before = author.document().clone();
    assert!(
        author
            .insert(vec![element(ElementKind::Rectangle, profile, "box0")])
            .is_err()
    );
    assert_eq!(author.document(), &before);
    let mut deleted = element(ElementKind::Rectangle, profile, "deleted");
    deleted.set(element::IS_DELETED, true).unwrap();
    author.insert(vec![deleted]).unwrap();
    let before = author.document().clone();
    assert!(
        author
            .bind_label(&"deleted".into(), &"box0".into())
            .is_err()
    );
    assert_eq!(author.document(), &before);
    assert!(
        author
            .document()
            .validate(profile, Purpose::SelfContained)
            .is_valid()
    );
    let exported = author
        .document()
        .validated(profile, Purpose::SelfContained)
        .unwrap()
        .project_native(ExportMode::Local, "test")
        .unwrap();
    assert!(
        exported
            .document
            .validate(profile, Purpose::SelfContained)
            .is_valid()
    );
}

#[test]
fn rebinding_preserves_endpoint_extensions_and_removes_replaced_optional_geometry() {
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        let mut doc = Document::new("extensions");
        {
            let mut author = doc.author(profile).unwrap();
            author
                .insert(vec![
                    element(ElementKind::Rectangle, profile, "box1"),
                    element(ElementKind::Rectangle, profile, "box2"),
                    element(ElementKind::Arrow, profile, "arrow"),
                ])
                .unwrap();
            author
                .bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::Start,
                    &"box1".into(),
                    geometry(profile),
                )
                .unwrap();
        }
        doc.edit_element(2, |arrow| {
            let Field::Value(mut b) = arrow.get(element::START_BINDING)? else {
                panic!("binding")
            };
            b.set_raw("future", json!({"keep":true}));
            if profile == Profile::V0_18_1 {
                b.set(binding::FIXED_POINT, [0_i64.into(), 1_i64.into()])?;
            }
            arrow.set(element::START_BINDING, b)
        })
        .unwrap();
        let mut author = doc.author(profile).unwrap();
        for target in ["box1", "box2"] {
            author
                .bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::Start,
                    &target.into(),
                    geometry(profile),
                )
                .unwrap();
            let arrow = by_id(author.document(), "arrow");
            assert_eq!(arrow["startBinding"]["future"], json!({"keep":true}));
            assert_eq!(arrow["startBinding"]["elementId"], target);
            if profile == Profile::V0_18_1 {
                assert!(arrow["startBinding"].get("fixedPoint").is_none());
            }
        }
    }
}
