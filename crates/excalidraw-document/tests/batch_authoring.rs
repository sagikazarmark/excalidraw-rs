use excalidraw_document::*;
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

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

fn content(text: &str) -> TextContent {
    TextContent::plain(text, 80_u64.into(), 25_u64.into())
}

fn scene(profile: Profile) -> Document {
    let mut doc = Document::new("batch-test");
    doc.author(profile)
        .unwrap()
        .insert(
            [
                ("box1", ElementKind::Rectangle),
                ("box2", ElementKind::Rectangle),
                ("label", ElementKind::Text),
                ("other", ElementKind::Text),
                ("arrow", ElementKind::Arrow),
                ("deleted", ElementKind::Rectangle),
            ]
            .into_iter()
            .map(|(id, kind)| {
                let mut e = Element::new(kind, profile, id.into(), 1_u64.into()).unwrap();
                e.set_raw("futureElement", json!({"keep": [1, 2]}));
                if id == "deleted" {
                    e.set(element::IS_DELETED, true).unwrap();
                }
                e
            })
            .collect(),
        )
        .unwrap();
    doc.set_root("futureRoot", json!({"keep": true})).unwrap();
    doc
}

fn by_id<'a>(doc: &'a Document, id: &str) -> &'a Value {
    doc.as_object()["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap()
}

fn recover(author: &mut SceneAuthor<'_>, before: &Document) {
    assert_eq!(author.document(), before);
    author
        .batch(|batch| batch.replace_text(&"label".into(), content("recovered")))
        .unwrap();
    author
        .replace_text(&"other".into(), content("single edit still works"))
        .unwrap();
    assert_eq!(by_id(author.document(), "label")["text"], "recovered");
}

#[test]
fn dependent_edits_match_sequential_composition_in_both_profiles() {
    for profile in PROFILES {
        let mut sequential = scene(profile);
        let mut batched = sequential.clone();
        let ids = ["box1".into(), "box2".into()];
        {
            let mut a = sequential.author(profile).unwrap();
            a.replace_text(&"label".into(), content("updated")).unwrap();
            a.bind_label(&"label".into(), &"box1".into()).unwrap();
            a.bind_arrow(
                &"arrow".into(),
                ArrowEndpoint::End,
                &"box1".into(),
                geometry(profile),
            )
            .unwrap();
            a.bind_arrow(
                &"arrow".into(),
                ArrowEndpoint::End,
                &"box2".into(),
                geometry(profile),
            )
            .unwrap();
            a.group(&ids, "inner".into()).unwrap();
            a.group(&["box1".into()], "outer".into()).unwrap();
            a.translate_connected(&["box1".into()], [12_i64.into(), (-5_i64).into()])
                .unwrap();
            a.reorder(&["box2".into()], None).unwrap();
        }
        batched
            .author(profile)
            .unwrap()
            .batch(|b| {
                b.replace_text(&"label".into(), content("updated"))?;
                b.bind_label(&"label".into(), &"box1".into())?;
                b.bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::End,
                    &"box1".into(),
                    geometry(profile),
                )?;
                b.bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::End,
                    &"box2".into(),
                    geometry(profile),
                )?;
                b.group(&ids, "inner".into())?;
                b.group(&["box1".into()], "outer".into())?;
                b.translate_connected(&["box1".into()], [12_i64.into(), (-5_i64).into()])?;
                b.reorder(&["box2".into()], None)
            })
            .unwrap();
        assert_eq!(batched, sequential);
        for id in ["box1", "box2", "label", "arrow"] {
            assert_eq!(by_id(&batched, id)["x"], 12.0);
            assert_eq!(by_id(&batched, id)["y"], -5.0);
            assert_eq!(by_id(&batched, id)["version"], 1);
            assert_eq!(
                by_id(&batched, id)["futureElement"],
                json!({"keep": [1, 2]})
            );
        }
        assert!(batched.validate(profile, Purpose::Author).is_valid());
    }
}

#[test]
fn callback_error_and_panic_discard_queue_and_author_recovers() {
    for profile in PROFILES {
        for panic in [false, true] {
            let mut doc = scene(profile);
            let before = doc.clone();
            let mut author = doc.author(profile).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                author.batch(|b| {
                    b.replace_text(&"label".into(), content("discard"))?;
                    b.bind_label(&"label".into(), &"box1".into())?;
                    if panic {
                        panic!("callback panic");
                    }
                    Err(Error {
                        path: "/callback".into(),
                        message: "cancelled".into(),
                    })
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().unwrap_err().path, "/callback");
            }
            recover(&mut author, &before);
        }
    }
}

#[test]
fn ignored_deferred_errors_abort_even_after_candidate_was_modified() {
    for profile in PROFILES {
        // Some failures occur after a binding has already changed, exercising
        // whole-candidate rather than field rollback.
        for case in 0..14 {
            let mut doc = scene(profile);
            let before = doc.clone();
            let mut author = doc.author(profile).unwrap();
            let mut callback_finished = false;
            let result = author.batch(|b| {
                b.replace_text(&"label".into(), content("discard"))?;
                let ignored = match case {
                    0 => b.replace_text(&"missing".into(), content("bad")),
                    1 => b.replace_text(&"box1".into(), content("bad")),
                    2 => b.replace_text(&"deleted".into(), content("bad")),
                    3 => b.bind_label(&"box1".into(), &"box2".into()),
                    4 => b.bind_label(&"label".into(), &"missing".into()),
                    5 => b.bind_label(&"label".into(), &"deleted".into()),
                    6 => b.bind_arrow(
                        &"arrow".into(),
                        ArrowEndpoint::End,
                        &"missing".into(),
                        geometry(profile),
                    ),
                    7 => b.bind_arrow(
                        &"arrow".into(),
                        ArrowEndpoint::End,
                        &"deleted".into(),
                        geometry(profile),
                    ),
                    8 => b.bind_arrow(
                        &"box1".into(),
                        ArrowEndpoint::End,
                        &"box2".into(),
                        geometry(profile),
                    ),
                    9 => b.bind_arrow(
                        &"arrow".into(),
                        ArrowEndpoint::End,
                        &"box1".into(),
                        geometry(if profile == PROFILES[0] {
                            PROFILES[1]
                        } else {
                            PROFILES[0]
                        }),
                    ),
                    10 => b.replace_text(
                        &"label".into(),
                        TextContent::plain("bad", (-1_i64).into(), 1_u64.into()),
                    ),
                    11 => b.translate_connected(
                        &["box1".into(), "box1".into()],
                        [1_i64.into(), 0_i64.into()],
                    ),
                    12 => b.reorder(&["box1".into()], Some(&"box1".into())),
                    _ => b.group(&[], "empty".into()),
                };
                // Queue methods have not inspected IDs/geometry yet.
                assert!(ignored.is_ok());
                let _ = ignored;
                b.replace_text(&"label".into(), content("later repair cannot hide error"))?;
                callback_finished = true;
                Ok(())
            });
            assert!(callback_finished);
            assert!(result.is_err(), "case {case} for {profile:?}");
            recover(&mut author, &before);
        }
    }
}

#[test]
fn graph_constraints_are_deferred_and_can_be_repaired() {
    for profile in PROFILES {
        let mut doc = scene(profile);
        let mut author = doc.author(profile).unwrap();
        author.bind_label(&"label".into(), &"box1".into()).unwrap();
        let before = author.document().clone();
        // Two labels are locally representable but fail final Author validation.
        assert!(
            author
                .batch(|b| {
                    b.replace_text(&"other".into(), content("replacement"))?;
                    b.bind_label(&"other".into(), &"box1".into())
                })
                .is_err()
        );
        assert_eq!(author.document(), &before);
        author
            .batch(|b| {
                b.bind_label(&"other".into(), &"box1".into())?;
                b.unbind_label(&"label".into())?;
                b.replace_text(&"other".into(), content("replacement"))?;
                b.group(&["box1".into()], "replacement-group".into())
            })
            .unwrap();
        assert_eq!(
            by_id(author.document(), "box1")["boundElements"],
            json!([{"id":"other", "type":"text"}])
        );
        assert!(by_id(author.document(), "label")["containerId"].is_null());
        assert!(
            author
                .document()
                .validate(profile, Purpose::Author)
                .is_valid()
        );
    }
}

#[test]
fn invalid_target_kinds_reject_final_graph_or_can_be_unbound_safely() {
    for profile in PROFILES {
        for target in ["label", "arrow"] {
            let mut doc = scene(profile);
            // Standalone text is a legal arrow target; a bound label is not.
            doc.author(profile)
                .unwrap()
                .bind_label(&"label".into(), &"box1".into())
                .unwrap();
            let before = doc.clone();
            let mut author = doc.author(profile).unwrap();
            assert!(
                author
                    .batch(|b| {
                        b.bind_arrow(
                            &"arrow".into(),
                            ArrowEndpoint::End,
                            &target.into(),
                            geometry(profile),
                        )?;
                        b.translate_connected(&["arrow".into()], [1_i64.into(), 0_i64.into()])?;
                        b.group(&["arrow".into()], "temporary".into())?;
                        b.reorder(&["arrow".into()], None)
                    })
                    .is_err()
            );
            assert_eq!(author.document(), &before);
            author
                .batch(|b| {
                    b.bind_arrow(
                        &"arrow".into(),
                        ArrowEndpoint::End,
                        &target.into(),
                        geometry(profile),
                    )?;
                    b.translate_connected(&["arrow".into()], [0_i64.into(), 0_i64.into()])?;
                    b.unbind_arrow(&"arrow".into(), ArrowEndpoint::End)
                })
                .unwrap();
            assert!(
                author
                    .document()
                    .validate(profile, Purpose::Author)
                    .is_valid()
            );
        }
        let mut doc = scene(profile);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .batch(|b| {
                    b.bind_label(&"label".into(), &"label".into())?;
                    b.group(&["label".into()], "self-label".into())?;
                    b.reorder(&["label".into()], None)
                })
                .is_err()
        );
        recover(&mut author, &before);
    }
}

#[test]
fn new_group_ids_and_existing_snapshot_locks_are_reserved() {
    for profile in PROFILES {
        let mut doc = scene(profile);
        if profile == Profile::SnapshotAfa3a653 {
            let mut state = doc.app_state().unwrap();
            state
                .set(
                    app_state::LOCKED_MULTI_SELECTIONS,
                    std::collections::BTreeMap::from([("locked".into(), true)]),
                )
                .unwrap();
            doc.set_app_state(state);
        }
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .batch(|b| {
                    b.group(&["box1".into()], "fresh".into())?;
                    b.group(&["box2".into()], "fresh".into())
                })
                .is_err()
        );
        assert_eq!(author.document(), &before);
        if profile == Profile::SnapshotAfa3a653 {
            assert!(
                author
                    .batch(|b| b.group(&["box1".into()], "locked".into()))
                    .is_err()
            );
        }
        recover(&mut author, &before);
    }
}

#[test]
fn shared_endpoint_backrefs_and_extensions_survive_rebinding() {
    for profile in PROFILES {
        let mut doc = scene(profile);
        doc.author(profile)
            .unwrap()
            .batch(|b| {
                b.bind_label(&"label".into(), &"box1".into())?;
                for end in [ArrowEndpoint::Start, ArrowEndpoint::End] {
                    b.bind_arrow(&"arrow".into(), end, &"box1".into(), geometry(profile))?;
                }
                Ok(())
            })
            .unwrap();
        let mut elements = doc.elements().unwrap();
        for e in &mut elements {
            if e.get(element::ID).unwrap() == Field::Value("arrow".into()) {
                let Field::Value(mut binding) = e.get(element::START_BINDING).unwrap() else {
                    panic!()
                };
                binding.set_raw("futureBinding", json!({"preserve":true}));
                if profile == Profile::V0_18_1 {
                    binding
                        .set(binding::FIXED_POINT, [0_i64.into(), 1_i64.into()])
                        .unwrap();
                }
                e.set(element::START_BINDING, binding).unwrap();
            }
            if e.get(element::ID).unwrap() == Field::Value("box1".into()) {
                let Field::Value(mut refs) = e.get(element::BOUND_ELEMENTS).unwrap() else {
                    panic!()
                };
                for r in &mut refs {
                    r.set_raw("futureReverse", json!(["keep"]));
                }
                e.set(element::BOUND_ELEMENTS, refs).unwrap();
            }
        }
        doc.set_elements(elements);
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        author
            .batch(|b| {
                b.bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::Start,
                    &"box2".into(),
                    geometry(profile),
                )?;
                b.bind_arrow(
                    &"arrow".into(),
                    ArrowEndpoint::Start,
                    &"box1".into(),
                    geometry(profile),
                )?;
                b.unbind_arrow(&"arrow".into(), ArrowEndpoint::End)?;
                b.replace_text(&"label".into(), content("preserving"))
            })
            .unwrap();
        assert_eq!(
            by_id(author.document(), "box1")["boundElements"],
            by_id(&before, "box1")["boundElements"]
        );
        let arrow = by_id(author.document(), "arrow");
        assert_eq!(
            arrow["startBinding"]["futureBinding"],
            json!({"preserve":true})
        );
        assert!(arrow["endBinding"].is_null());
        if profile == Profile::V0_18_1 {
            assert!(arrow["startBinding"].get("fixedPoint").is_none());
        }
        author
            .batch(|b| {
                b.unbind_arrow(&"arrow".into(), ArrowEndpoint::Start)?;
                b.bind_label(&"label".into(), &"box2".into())?;
                b.unbind_label(&"label".into())
            })
            .unwrap();
        assert_eq!(by_id(author.document(), "box1")["boundElements"], json!([]));
        assert_eq!(by_id(author.document(), "box2")["boundElements"], json!([]));
        assert_eq!(
            author.document().as_object()["futureRoot"],
            before.as_object()["futureRoot"]
        );
    }
}

#[test]
fn empty_batch_preserves_exact_document() {
    let mut doc = scene(PROFILES[0]);
    let before = doc.clone();
    doc.author(PROFILES[0]).unwrap().batch(|_| Ok(())).unwrap();
    assert_eq!(doc, before);
}

#[test]
fn arithmetic_failure_after_partial_translation_rolls_back() {
    for profile in PROFILES {
        let mut doc = scene(profile);
        doc.edit_element(0, |e| e.set(element::Y, "1e308".parse().unwrap()))
            .unwrap();
        let before = doc.clone();
        let mut author = doc.author(profile).unwrap();
        assert!(
            author
                .batch(|b| {
                    b.replace_text(&"label".into(), content("discard"))?;
                    // x succeeds, then y overflows finite f64 scene coordinates.
                    b.translate_connected(
                        &["box1".into()],
                        [1_i64.into(), "1e308".parse().unwrap()],
                    )?;
                    b.translate_connected(
                        &["box1".into()],
                        [(-1_i64).into(), "-1e308".parse().unwrap()],
                    )
                })
                .is_err()
        );
        recover(&mut author, &before);
    }
}

#[test]
fn malformed_or_duplicate_records_cannot_enter_a_batch() {
    for profile in PROFILES {
        for malformed in [false, true] {
            let mut doc = scene(profile);
            let mut elements = doc.elements().unwrap();
            if malformed {
                elements[0].set_raw("groupIds", json!({"bad": "shape"}));
            } else {
                elements.push(elements[0].clone());
            }
            doc.set_elements(elements);
            let before = doc.clone();
            assert!(doc.author(profile).is_err());
            assert_eq!(doc, before);
        }
    }
}

#[test]
#[ignore = "manual release-mode timing; no timing assertion"]
fn measure_twenty_text_edits_and_typical_composition() {
    use std::{hint::black_box, time::Instant};
    for profile in PROFILES {
        let mut initial = scene(profile);
        initial
            .author(profile)
            .unwrap()
            .insert(
                (0..1000)
                    .map(|i| {
                        Element::new(
                            ElementKind::Text,
                            profile,
                            ElementId(format!("text{i}")),
                            1_u64.into(),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
        let ids: Vec<ElementId> = (0..20).map(|i| ElementId(format!("text{i}"))).collect();
        for mixed in [false, true] {
            let mut outputs = Vec::new();
            for batched in [false, true] {
                let mut doc = initial.clone();
                let mut author = doc.author(profile).unwrap();
                let start = Instant::now();
                for run in 0..10 {
                    if batched {
                        author
                            .batch(|b| {
                                for id in ids.iter().take(if mixed { 1 } else { 20 }) {
                                    b.replace_text(id, content("measured"))?;
                                }
                                if mixed {
                                    b.bind_arrow(
                                        &"arrow".into(),
                                        ArrowEndpoint::End,
                                        &if run % 2 == 0 { "box1" } else { "box2" }.into(),
                                        geometry(profile),
                                    )?;
                                    b.group(
                                        &["box1".into(), "box2".into()],
                                        GroupId(format!("group{run}")),
                                    )?;
                                }
                                Ok(())
                            })
                            .unwrap();
                    } else {
                        for id in ids.iter().take(if mixed { 1 } else { 20 }) {
                            author.replace_text(id, content("measured")).unwrap();
                        }
                        if mixed {
                            author
                                .bind_arrow(
                                    &"arrow".into(),
                                    ArrowEndpoint::End,
                                    &if run % 2 == 0 { "box1" } else { "box2" }.into(),
                                    geometry(profile),
                                )
                                .unwrap();
                            author
                                .group(
                                    &["box1".into(), "box2".into()],
                                    GroupId(format!("group{run}")),
                                )
                                .unwrap();
                        }
                    }
                    black_box(author.document());
                }
                eprintln!(
                    "{profile:?} mixed={mixed} batch={batched}: {:?} per transaction",
                    start.elapsed() / 10
                );
                outputs.push(doc);
            }
            assert_eq!(outputs[0], outputs[1]);
        }
    }
}
