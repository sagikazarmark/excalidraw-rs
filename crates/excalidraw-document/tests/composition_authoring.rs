use excalidraw_document::*;
use serde_json::{Value, json};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

fn element(profile: Profile, kind: ElementKind, id: &str) -> Element {
    Element::new(kind, profile, id.into(), 123_u64.into()).unwrap()
}

fn point(x: i64, y: i64) -> Point {
    [x.into(), y.into()]
}

fn by_id(document: &Document, id: &str) -> Value {
    document.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap()
        .clone()
}

fn order(document: &Document) -> Vec<String> {
    document.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect()
}

fn geometry(profile: Profile) -> BindingGeometry {
    match profile {
        Profile::V0_18_1 => BindingGeometry::Release {
            focus: 0_i64.into(),
            gap: 1_u64.into(),
            fixed_point: Some(point(0, 1)),
        },
        Profile::SnapshotAfa3a653 => BindingGeometry::Snapshot {
            fixed_point: point(0, 1),
            mode: BindMode::Orbit,
        },
    }
}

fn mapping(ids: &[&str], groups: &[&str]) -> IdMap {
    IdMap {
        elements: ids
            .iter()
            .map(|id| ((*id).into(), ElementId(format!("copy-{id}"))))
            .collect(),
        groups: groups
            .iter()
            .map(|id| ((*id).into(), GroupId(format!("copy-{id}"))))
            .collect(),
        ..IdMap::default()
    }
}

fn assert_valid(document: &Document, profile: Profile) {
    let report = document.validate(profile, Purpose::Author);
    assert!(report.is_valid(), "{report:?}");
}

/// Historical nested frame input, deliberately built with raw membership fields.
/// Native frame assignment cannot create this tree; traversal must preserve it.
fn nested(profile: Profile) -> Document {
    let mut document = Document::new("composition-tests");
    let mut inner = element(profile, ElementKind::Frame, "inner");
    inner.set_raw("frameId", json!("outer"));
    let mut a = element(profile, ElementKind::Rectangle, "a");
    let mut b = element(profile, ElementKind::Rectangle, "b");
    let mut label = element(profile, ElementKind::Text, "label");
    for e in [&mut a, &mut b, &mut label] {
        e.set(element::FRAME_ID, "inner".into()).unwrap();
        e.set(element::GROUP_IDS, vec!["small".into(), "large".into()])
            .unwrap();
    }
    {
        let mut author = document.author(profile).unwrap();
        author
            .insert(vec![
                a,
                label,
                b,
                inner,
                element(profile, ElementKind::Frame, "outer"),
                element(profile, ElementKind::Frame, "destination"),
                element(profile, ElementKind::Rectangle, "unrelated"),
            ])
            .unwrap();
        author.bind_label(&"label".into(), &"a".into()).unwrap();
    }
    document
}

fn connected(profile: Profile) -> Document {
    let mut document = Document::new("composition-tests");
    let mut arrow = element(profile, ElementKind::Arrow, "arrow");
    arrow.set_path(vec![point(0, 0), point(20, 30)]).unwrap();
    {
        let mut author = document.author(profile).unwrap();
        author
            .insert(vec![
                element(profile, ElementKind::Rectangle, "a"),
                element(profile, ElementKind::Text, "label"),
                arrow,
                element(profile, ElementKind::Rectangle, "b"),
                element(profile, ElementKind::Arrow, "arrow2"),
                element(profile, ElementKind::Rectangle, "c"),
                element(profile, ElementKind::Rectangle, "unrelated"),
            ])
            .unwrap();
        author.bind_label(&"label".into(), &"a".into()).unwrap();
        for (arrow, start, end) in [("arrow", "a", "b"), ("arrow2", "b", "c")] {
            author
                .bind_arrow(
                    &arrow.into(),
                    ArrowEndpoint::Start,
                    &start.into(),
                    geometry(profile),
                )
                .unwrap();
            author
                .bind_arrow(
                    &arrow.into(),
                    ArrowEndpoint::End,
                    &end.into(),
                    geometry(profile),
                )
                .unwrap();
        }
    }
    document
}

#[test]
fn reparenting_expands_groups_and_labels_but_retains_nested_frame_memberships() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .set_frame(&["inner".into()], Some(&"destination".into()))
                .is_err()
        );
        assert_eq!(author.document(), &before);
        author.set_frame(&["label".into()], None).unwrap();
        for id in ["a", "b", "label"] {
            let now = by_id(author.document(), id);
            assert!(now["frameId"].is_null());
            assert_eq!(now["groupIds"], json!(["small", "large"]));
        }
        assert_valid(author.document(), profile);
    }
}

#[test]
fn frame_cycles_bad_destinations_and_bad_selections_are_atomic() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        for (ids, destination) in [
            (vec!["outer".into()], "inner"),
            (vec!["inner".into()], "inner"),
            (vec!["a".into()], "b"),
            (vec!["a".into()], "missing"),
            (vec!["missing".into()], "outer"),
            (vec!["a".into(), "a".into()], "outer"),
        ] {
            assert!(author.set_frame(&ids, Some(&destination.into())).is_err());
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn grouping_wraps_existing_levels_and_ungrouping_removes_only_one_level_and_lock() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        if profile == Profile::SnapshotAfa3a653 {
            let mut state = doc.app_state().unwrap();
            state.set_raw("lockedMultiSelections", json!({"small":true, "large":true}));
            state.set_raw("host", json!({"keep":true}));
            doc.set_app_state(state);
        }
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        for group in ["", "small"] {
            assert!(author.group(&["a".into()], group.into()).is_err());
            assert_eq!(author.document(), &before);
        }
        assert!(author.group(&[], "new".into()).is_err());
        assert!(
            author
                .group(&["a".into(), "unrelated".into()], "new".into())
                .is_err()
        );
        assert!(author.group(&["outer".into()], "new".into()).is_err());
        assert!(author.ungroup(&"missing".into()).is_err());
        assert_eq!(author.document(), &before);
        author.group(&["label".into()], "new".into()).unwrap();
        for id in ["a", "b", "label"] {
            assert_eq!(
                by_id(author.document(), id)["groupIds"],
                json!(["small", "large", "new"])
            );
        }
        author.ungroup(&"large".into()).unwrap();
        for id in ["a", "b", "label"] {
            assert_eq!(
                by_id(author.document(), id)["groupIds"],
                json!(["small", "new"])
            );
            assert_eq!(by_id(author.document(), id)["updated"], 123);
        }
        if profile == Profile::SnapshotAfa3a653 {
            assert_eq!(
                author.document().as_object()["appState"]["lockedMultiSelections"],
                json!({"small":true})
            );
            assert_eq!(
                author.document().as_object()["appState"]["host"],
                json!({"keep":true})
            );
        }
        assert_eq!(
            by_id(author.document(), "unrelated"),
            by_id(&before, "unrelated")
        );
        assert_valid(author.document(), profile);
    }
}

#[test]
fn translation_moves_entire_connected_component_without_routing_or_revision_changes() {
    for profile in PROFILES {
        let mut doc = connected(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        author
            .translate_connected(&["label".into()], point(17, -9))
            .unwrap();
        for id in ["a", "label", "arrow", "b", "arrow2", "c"] {
            let mut expected = by_id(&before, id);
            expected["x"] = json!(17.0);
            expected["y"] = json!(-9.0);
            assert_eq!(by_id(author.document(), id), expected);
        }
        assert_eq!(
            by_id(author.document(), "unrelated"),
            by_id(&before, "unrelated")
        );
        assert_valid(author.document(), profile);
    }
}

#[test]
fn translation_expands_nested_frames_and_groups_and_rolls_back_overflow() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        {
            let mut author = doc.author(profile).unwrap();
            author
                .translate_connected(&["outer".into()], point(10, 20))
                .unwrap();
            for id in ["outer", "inner", "a", "b", "label"] {
                assert_eq!(by_id(author.document(), id)["x"], 10.0);
            }
            for id in ["destination", "unrelated"] {
                assert_eq!(by_id(author.document(), id), by_id(&before, id));
            }
            author
                .translate_connected(&["a".into()], point(2, 0))
                .unwrap();
            for id in ["a", "b", "label"] {
                assert_eq!(by_id(author.document(), id)["x"], 12.0);
            }
            assert_eq!(by_id(author.document(), "inner")["x"], 10.0);
        }
        doc.edit_element(2, |e| e.set(element::X, "1e308".parse::<Number>().unwrap()))
            .unwrap();
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        for delta in [
            ["1e308".parse().unwrap(), 0_i64.into()],
            [0_i64.into(), "1e400".parse().unwrap()],
        ] {
            assert!(author.translate_connected(&["a".into()], delta).is_err());
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn reorder_keeps_frames_groups_and_labels_in_stable_blocks() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        author.reorder(&["b".into(), "a".into()], None).unwrap();
        assert_eq!(
            order(author.document()),
            [
                "destination",
                "unrelated",
                "a",
                "label",
                "b",
                "inner",
                "outer"
            ]
        );
        author
            .reorder(&["unrelated".into()], Some(&"label".into()))
            .unwrap();
        assert_eq!(
            order(author.document()),
            [
                "destination",
                "unrelated",
                "a",
                "label",
                "b",
                "inner",
                "outer"
            ]
        );
        author
            .reorder(&["label".into()], Some(&"destination".into()))
            .unwrap();
        assert_eq!(author.document(), &before);
        assert!(author.reorder(&["a".into()], Some(&"b".into())).is_err());
        assert!(
            author
                .reorder(&["a".into()], Some(&"missing".into()))
                .is_err()
        );
        assert_eq!(author.document(), &before);
        assert_valid(author.document(), profile);
    }
}

#[test]
fn duplication_requires_complete_fresh_maps_for_the_reference_closure() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        let complete = mapping(&["a", "label", "b", "inner", "outer"], &["small", "large"]);
        let mut author = doc.author(profile).unwrap();
        let mut bad_maps = vec![
            mapping(&["a"], &[]),
            mapping(&["a", "label", "b", "inner", "outer"], &[]),
        ];
        for (old, new) in [("a", "unrelated"), ("a", "a"), ("a", "copy-b"), ("a", "")] {
            let mut bad = complete.clone();
            bad.elements.insert(old.into(), new.into());
            bad_maps.push(bad);
        }
        let mut bad = complete.clone();
        bad.groups.insert("small".into(), "large".into());
        bad_maps.push(bad);
        let mut bad = complete.clone();
        bad.elements
            .insert("unrelated".into(), "copy-unrelated".into());
        bad_maps.push(bad);
        for bad in bad_maps {
            assert!(
                author
                    .duplicate(&["a".into()], &bad, point(1, 2), OpaquePolicy::Preserve)
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
        assert!(
            author
                .duplicate(&["a".into()], &complete, point(5, 6), OpaquePolicy::Reject)
                .unwrap()
                .is_empty()
        );
        for id in [
            "a",
            "label",
            "b",
            "inner",
            "outer",
            "destination",
            "unrelated",
        ] {
            assert_eq!(by_id(author.document(), id), by_id(&before, id));
        }
        assert_eq!(
            by_id(author.document(), "copy-inner")["frameId"],
            "copy-outer"
        );
        assert_eq!(by_id(author.document(), "copy-a")["frameId"], "copy-inner");
        assert_eq!(
            by_id(author.document(), "copy-a")["groupIds"],
            json!(["copy-small", "copy-large"])
        );
        assert_eq!(
            by_id(author.document(), "copy-label")["containerId"],
            "copy-a"
        );
        assert_eq!(
            by_id(author.document(), "copy-a")["boundElements"],
            json!([{"id":"copy-label", "type":"text"}])
        );
        assert_eq!(by_id(author.document(), "copy-a")["x"], 5.0);
        for key in ["version", "versionNonce", "seed", "updated", "created"] {
            assert_eq!(
                by_id(author.document(), "copy-a")[key],
                by_id(&before, "a")[key]
            );
        }
        assert_valid(author.document(), profile);
    }
}

#[test]
fn duplicated_arrows_and_both_endpoints_remain_internal_and_translate_together() {
    for profile in PROFILES {
        let mut doc = connected(profile);
        let before = doc.clone();
        let map = mapping(&["a", "label", "arrow", "b", "arrow2", "c"], &[]);
        let mut author = doc.author(profile).unwrap();
        author
            .duplicate(&["arrow".into()], &map, point(7, 8), OpaquePolicy::Reject)
            .unwrap();
        for id in ["a", "label", "arrow", "b", "arrow2", "c", "unrelated"] {
            assert_eq!(by_id(author.document(), id), by_id(&before, id));
        }
        for (arrow, start, end) in [("arrow", "a", "b"), ("arrow2", "b", "c")] {
            let copy = by_id(author.document(), &format!("copy-{arrow}"));
            assert_eq!(copy["startBinding"]["elementId"], format!("copy-{start}"));
            assert_eq!(copy["endBinding"]["elementId"], format!("copy-{end}"));
            assert_eq!(copy["points"], by_id(&before, arrow)["points"]);
            assert_eq!(copy["x"], 7.0);
            assert_eq!(copy["y"], 8.0);
        }
        assert_valid(author.document(), profile);
    }
}

#[test]
fn duplicate_preserves_or_rejects_opaque_metadata_and_returns_destination_paths() {
    for profile in PROFILES {
        let mut doc = connected(profile);
        doc.edit_element(0, |e| {
            e.set_raw("customData", json!({"elementId":"a"}));
            Ok(())
        })
        .unwrap();
        doc.edit_element(2, |e| {
            let Field::Value(mut b) = e.get(element::START_BINDING)? else {
                panic!("binding")
            };
            b.set_raw("future/ref", json!({"id":"b"}));
            e.set(element::START_BINDING, b)
        })
        .unwrap();
        doc.set_root("host", json!({"unrelated":"a"})).unwrap();
        let before = doc.clone();
        let map = mapping(&["a", "label", "arrow", "b", "arrow2", "c"], &[]);
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .duplicate(&["a".into()], &map, point(0, 0), OpaquePolicy::Reject)
                .is_err()
        );
        assert_eq!(author.document(), &before);
        let paths = author
            .duplicate(&["a".into()], &map, point(0, 0), OpaquePolicy::Preserve)
            .unwrap();
        assert!(paths.contains(&"/elements/7/customData".to_owned()));
        assert!(paths.contains(&"/elements/9/startBinding/future~1ref".to_owned()));
        assert!(!paths.contains(&"/host".to_owned()));
        assert_eq!(
            by_id(author.document(), "copy-a")["customData"],
            json!({"elementId":"a"})
        );
        assert_eq!(
            by_id(author.document(), "copy-arrow")["startBinding"]["future/ref"],
            json!({"id":"b"})
        );
        assert_eq!(
            author.document().as_object()["host"],
            before.as_object()["host"]
        );
    }
}

#[test]
fn duplicate_shares_or_explicitly_copies_assets_and_preserves_group_locks() {
    for profile in PROFILES {
        for copy_file in [false, true] {
            let mut doc = Document::new("assets");
            doc.set_root("files", json!({
                "asset/a":{"id":"asset/a","mimeType":"image/png","dataURL":"data:image/png;base64,AA==","created":1,"host/ref":"image"},
                "unrelated":{"id":"unrelated","mimeType":"image/png","dataURL":"data:image/png;base64,AA==","created":2}
            })).unwrap();
            let mut image = element(profile, ElementKind::Image, "image");
            image.set(element::FILE_ID, "asset/a".into()).unwrap();
            image.set(element::GROUP_IDS, vec!["g".into()]).unwrap();
            doc.author(profile).unwrap().insert(vec![image]).unwrap();
            if profile == Profile::SnapshotAfa3a653 {
                doc.set_root(
                    "appState",
                    json!({"lockedMultiSelections":{"g":true,"reserved":true},"host":"keep"}),
                )
                .unwrap();
            }
            let before = doc.clone();
            let mut map = mapping(&["image"], &["g"]);
            if copy_file {
                map.files.insert("asset/a".into(), "copied/b".into());
            }
            let mut author = doc.author(profile).unwrap();
            for destination in ["asset/a", "unrelated"] {
                let mut bad = map.clone();
                bad.files.insert("asset/a".into(), destination.into());
                assert!(
                    author
                        .duplicate(&["image".into()], &bad, point(0, 0), OpaquePolicy::Preserve)
                        .is_err()
                );
                assert_eq!(author.document(), &before);
            }
            if profile == Profile::SnapshotAfa3a653 {
                assert!(author.group(&["image".into()], "reserved".into()).is_err());
                let mut bad = map.clone();
                bad.groups.insert("g".into(), "reserved".into());
                assert!(
                    author
                        .duplicate(&["image".into()], &bad, point(0, 0), OpaquePolicy::Preserve)
                        .is_err()
                );
                assert_eq!(author.document(), &before);
            }
            assert!(
                author
                    .duplicate(&["image".into()], &map, point(0, 0), OpaquePolicy::Reject)
                    .is_err()
            );
            assert_eq!(author.document(), &before);
            let paths = author
                .duplicate(&["image".into()], &map, point(0, 0), OpaquePolicy::Preserve)
                .unwrap();
            let file_id = if copy_file { "copied/b" } else { "asset/a" };
            assert_eq!(by_id(author.document(), "copy-image")["fileId"], file_id);
            assert!(paths.contains(&format!("/files/{}/host~1ref", file_id.replace('/', "~1"))));
            for id in ["asset/a", "unrelated"] {
                assert_eq!(
                    author.document().as_object()["files"][id],
                    before.as_object()["files"][id]
                );
            }
            if copy_file {
                let mut expected = before.as_object()["files"]["asset/a"].clone();
                expected["id"] = json!("copied/b");
                assert_eq!(author.document().as_object()["files"]["copied/b"], expected);
            }
            if profile == Profile::SnapshotAfa3a653 {
                assert_eq!(
                    author.document().as_object()["appState"]["lockedMultiSelections"],
                    json!({"g":true,"copy-g":true,"reserved":true})
                );
                author.ungroup(&"copy-g".into()).unwrap();
                assert_eq!(
                    author.document().as_object()["appState"],
                    before.as_object()["appState"]
                );
            }
            assert_valid(author.document(), profile);
        }
    }
}

#[test]
fn duplicate_overflow_and_tombstone_identity_collisions_roll_back_everything() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        doc.edit_element(2, |e| e.set(element::X, "1e308".parse::<Number>().unwrap()))
            .unwrap();
        let mut deleted = element(profile, ElementKind::Rectangle, "dead");
        deleted.set(element::IS_DELETED, true).unwrap();
        deleted
            .set(element::GROUP_IDS, vec!["dead-group".into()])
            .unwrap();
        doc.author(profile).unwrap().insert(vec![deleted]).unwrap();
        let before = doc.clone();
        let map = mapping(&["a", "label", "b", "inner", "outer"], &["small", "large"]);
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .duplicate(
                    &["a".into()],
                    &map,
                    ["1e308".parse().unwrap(), 0_i64.into()],
                    OpaquePolicy::Preserve
                )
                .is_err()
        );
        assert_eq!(author.document(), &before);
        let mut bad = map.clone();
        bad.elements.insert("a".into(), "dead".into());
        assert!(
            author
                .duplicate(&["a".into()], &bad, point(0, 0), OpaquePolicy::Preserve)
                .is_err()
        );
        bad = map.clone();
        bad.groups.insert("small".into(), "dead-group".into());
        assert!(
            author
                .duplicate(&["a".into()], &bad, point(0, 0), OpaquePolicy::Preserve)
                .is_err()
        );
        assert!(
            author
                .translate_connected(&["dead".into()], point(0, 0))
                .is_err()
        );
        assert_eq!(author.document(), &before);
    }
}

#[test]
fn all_selection_operations_reject_unknown_and_duplicate_requests() {
    for profile in PROFILES {
        let mut doc = nested(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        for ids in [vec!["missing".into()], vec!["a".into(), "a".into()]] {
            assert!(author.group(&ids, "new".into()).is_err());
            assert!(author.translate_connected(&ids, point(1, 1)).is_err());
            assert!(author.reorder(&ids, None).is_err());
            assert!(
                author
                    .duplicate(&ids, &IdMap::default(), point(1, 1), OpaquePolicy::Preserve)
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn frame_assignment_rejects_frame_roots_reached_through_groups() {
    for profile in PROFILES {
        for kind in [ElementKind::Frame, ElementKind::Magicframe] {
            let mut doc = Document::new("frame-roots");
            let mut root = element(profile, kind, "root");
            let mut a = element(profile, ElementKind::Rectangle, "a");
            for e in [&mut root, &mut a] {
                e.set(element::GROUP_IDS, vec!["g".into()]).unwrap();
            }
            let mut author = doc.author(profile).unwrap();
            author
                .insert(vec![
                    a,
                    root,
                    element(profile, ElementKind::Frame, "target"),
                ])
                .unwrap();
            let before = author.document().clone();
            for id in ["root", "a"] {
                assert!(
                    author
                        .set_frame(&[id.into()], Some(&"target".into()))
                        .is_err()
                );
                assert_eq!(author.document(), &before);
            }
        }
    }
}

#[test]
fn grouping_gathers_at_highest_selected_position_in_scene_order() {
    for profile in PROFILES {
        let mut doc = Document::new("interleaved-group");
        let mut author = doc.author(profile).unwrap();
        author
            .insert(
                ["a", "b", "c", "d"]
                    .into_iter()
                    .map(|id| element(profile, ElementKind::Rectangle, id))
                    .collect(),
            )
            .unwrap();
        author.group(&["c".into(), "a".into()], "g".into()).unwrap();
        assert_eq!(order(author.document()), ["b", "a", "c", "d"]);
        assert_eq!(by_id(author.document(), "a")["groupIds"], json!(["g"]));
        assert_eq!(by_id(author.document(), "c")["groupIds"], json!(["g"]));
        assert_valid(author.document(), profile);
    }
}

fn interleaved_frames(profile: Profile) -> Document {
    let mut doc = Document::new("interleaved-frames");
    let mut elements = Vec::new();
    for id in [
        "a",
        "old1",
        "outside",
        "label",
        "old2",
        "old-frame",
        "b",
        "dest1",
        "gap",
        "dest-frame",
        "dest2",
        "tail",
    ] {
        let kind = match id {
            "old-frame" | "dest-frame" => ElementKind::Frame,
            "label" => ElementKind::Text,
            _ => ElementKind::Rectangle,
        };
        let mut e = element(profile, kind, id);
        if ["a", "label", "b", "old1", "old2"].contains(&id) {
            e.set(element::FRAME_ID, "old-frame".into()).unwrap();
        } else if ["dest1", "dest2"].contains(&id) {
            e.set(element::FRAME_ID, "dest-frame".into()).unwrap();
        }
        if ["a", "label", "b"].contains(&id) {
            e.set(element::GROUP_IDS, vec!["inner".into(), "outer".into()])
                .unwrap();
        }
        elements.push(e);
    }
    let mut author = doc.author(profile).unwrap();
    author.insert(elements).unwrap();
    author.bind_label(&"label".into(), &"a".into()).unwrap();
    doc
}

fn assert_block(document: &Document, ids: &[&str]) {
    assert!(
        order(document).windows(ids.len()).any(|block| block == ids),
        "expected {ids:?} in {:?}",
        order(document)
    );
}

#[test]
fn frame_assignment_normalizes_existing_and_added_children_and_old_frame() {
    for profile in PROFILES {
        let mut doc = interleaved_frames(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        author
            .set_frame(&["label".into()], Some(&"dest-frame".into()))
            .unwrap();
        assert_block(author.document(), &["old1", "old2", "old-frame"]);
        assert_block(
            author.document(),
            &["dest1", "dest2", "a", "label", "b", "dest-frame"],
        );
        for id in ["a", "label", "b"] {
            assert_eq!(by_id(author.document(), id)["frameId"], "dest-frame");
            assert_eq!(
                by_id(author.document(), id)["groupIds"],
                json!(["inner", "outer"])
            );
        }
        for id in ["outside", "gap", "tail"] {
            let mut expected = by_id(&before, id);
            expected["index"] = by_id(author.document(), id)["index"].clone();
            assert_eq!(by_id(author.document(), id), expected);
        }
        assert_eq!(by_id(author.document(), "label")["containerId"], "a");
        assert_valid(author.document(), profile);
    }
}

#[test]
fn frame_detach_keeps_remaining_frame_and_detached_nested_group_contiguous() {
    for profile in PROFILES {
        let mut doc = interleaved_frames(profile);
        let mut author = doc.author(profile).unwrap();
        author.set_frame(&["label".into()], None).unwrap();
        assert_block(author.document(), &["old1", "old2", "old-frame"]);
        assert_block(author.document(), &["a", "label", "b"]);
        for id in ["a", "label", "b"] {
            assert!(by_id(author.document(), id)["frameId"].is_null());
            assert_eq!(
                by_id(author.document(), id)["groupIds"],
                json!(["inner", "outer"])
            );
        }
        assert_valid(author.document(), profile);
    }
}

#[test]
fn grouping_interleaved_nested_group_and_label_keeps_frame_block_intact() {
    for profile in PROFILES {
        let mut doc = interleaved_frames(profile);
        let mut author = doc.author(profile).unwrap();
        author
            .group(&["old1".into(), "label".into()], "new".into())
            .unwrap();
        assert_block(author.document(), &["old1", "a", "label", "b"]);
        // The former inner group must itself remain a contiguous unit.
        assert_block(author.document(), &["a", "label", "b"]);
        assert_block(
            author.document(),
            &["old2", "old1", "a", "label", "b", "old-frame"],
        );
        assert_valid(author.document(), profile);
    }
}

#[test]
fn frame_assignment_orders_new_children_below_frame_on_either_side() {
    for profile in PROFILES {
        for ids in [["a", "gap", "frame", "tail"], ["frame", "gap", "a", "tail"]] {
            let mut doc = Document::new("frame-position");
            let mut author = doc.author(profile).unwrap();
            author
                .insert(
                    ids.into_iter()
                        .map(|id| {
                            element(
                                profile,
                                if id == "frame" {
                                    ElementKind::Frame
                                } else {
                                    ElementKind::Rectangle
                                },
                                id,
                            )
                        })
                        .collect(),
                )
                .unwrap();
            author
                .set_frame(&["a".into()], Some(&"frame".into()))
                .unwrap();
            assert_eq!(
                order(author.document()),
                if ids[0] == "a" {
                    ["gap", "a", "frame", "tail"]
                } else {
                    ["a", "frame", "gap", "tail"]
                }
            );
            assert_eq!(by_id(author.document(), "a")["frameId"], "frame");
            assert_valid(author.document(), profile);
        }
    }
}

#[test]
fn crossing_historical_groups_reject_ordering_changes_atomically() {
    for profile in PROFILES {
        let mut doc = Document::new("crossing-groups");
        let mut elements = Vec::new();
        for (id, groups) in [("a", vec!["g"]), ("b", vec!["g", "h"]), ("c", vec!["h"])] {
            let mut e = element(profile, ElementKind::Rectangle, id);
            e.set_raw("groupIds", json!(groups));
            elements.push(e);
        }
        elements.push(element(profile, ElementKind::Frame, "frame"));
        let mut author = doc.author(profile).unwrap();
        author.insert(elements).unwrap();
        let before = author.document().clone();
        assert!(author.group(&["a".into()], "new".into()).is_err());
        assert_eq!(author.document(), &before);
        assert!(
            author
                .set_frame(&["a".into()], Some(&"frame".into()))
                .is_err()
        );
        assert_eq!(author.document(), &before);
    }
}

#[test]
fn duplicate_reserves_file_references_without_resources_including_tombstones() {
    for profile in PROFILES {
        for deleted in [false, true] {
            let mut doc = Document::new("external-assets");
            doc.set_root("files", json!({"asset": {
                "id":"asset", "mimeType":"image/png", "dataURL":"data:image/png;base64,AA==", "created":1
            }})).unwrap();
            let mut image = element(profile, ElementKind::Image, "image");
            image.set(element::FILE_ID, "asset".into()).unwrap();
            let mut external = element(profile, ElementKind::Image, "external-image");
            external.set(element::FILE_ID, "external".into()).unwrap();
            external.set(element::IS_DELETED, deleted).unwrap();
            let mut author = doc.author(profile).unwrap();
            author.insert(vec![image, external]).unwrap();
            let before = author.document().clone();
            let mut map = mapping(&["image"], &[]);
            map.files.insert("asset".into(), "external".into());
            assert!(
                author
                    .duplicate(&["image".into()], &map, point(1, 2), OpaquePolicy::Reject)
                    .is_err()
            );
            assert_eq!(author.document(), &before);
            map.files.insert("asset".into(), "fresh".into());
            author
                .duplicate(&["image".into()], &map, point(1, 2), OpaquePolicy::Reject)
                .unwrap();
            assert_eq!(
                by_id(author.document(), "external-image"),
                by_id(&before, "external-image")
            );
            assert_eq!(by_id(author.document(), "copy-image")["fileId"], "fresh");
            assert!(
                author.document().as_object()["files"]
                    .get("external")
                    .is_none()
            );
            assert_valid(author.document(), profile);
        }
    }
}
