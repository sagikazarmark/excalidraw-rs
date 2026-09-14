use excalidraw_document::*;

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

fn crossing(profile: Profile, indexed: bool) -> Document {
    let mut elements = Vec::new();
    for (i, (id, kind, groups)) in [
        ("first", ElementKind::Rectangle, vec!["a", "border"]),
        ("border", ElementKind::Rectangle, vec!["border"]),
        ("delayed", ElementKind::Rectangle, vec!["a"]),
        ("frame", ElementKind::Frame, vec![]),
        ("other-frame", ElementKind::Frame, vec![]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut element = Element::new(kind, profile, id.into(), 123_u64.into()).unwrap();
        element
            .set(
                element::GROUP_IDS,
                groups.into_iter().map(GroupId::from).collect(),
            )
            .unwrap();
        if indexed {
            element
                .set(element::INDEX, FractionalIndex(format!("a{i}")))
                .unwrap();
        }
        elements.push(element);
    }
    let mut document = Document::new("frame-order-tests");
    document.set_elements(elements);
    document
}

#[test]
fn preserve_expands_crossing_groups_without_changing_order_indices_or_other_fields() {
    for profile in PROFILES {
        for indexed in [false, true] {
            let mut document = crossing(profile, indexed);
            let before = document.clone();
            let mut author = document.author(profile).unwrap();
            assert!(
                author
                    .set_frame(&["delayed".into()], Some(&"frame".into()))
                    .is_err()
            );
            assert_eq!(author.document(), &before);
            author
                .set_frame_with_order(
                    &["delayed".into()],
                    Some(&"frame".into()),
                    FrameOrder::Preserve,
                )
                .unwrap();
            let mut expected = before.elements().unwrap();
            for e in &mut expected[..3] {
                e.set(element::FRAME_ID, "frame".into()).unwrap();
            }
            assert_eq!(author.document().elements().unwrap(), expected);
            let report = author.document().validate(profile, Purpose::Author);
            assert!(report.is_valid(), "{report:?}");
            author
                .set_frame_with_order(&["border".into()], None, FrameOrder::Preserve)
                .unwrap();
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn preserving_assignment_retains_frame_constraints_and_selection_errors_atomically() {
    for profile in PROFILES {
        let mut document = crossing(profile, false);
        let mut dead =
            Element::new(ElementKind::Rectangle, profile, "dead".into(), 0_u64.into()).unwrap();
        dead.set(element::IS_DELETED, true).unwrap();
        let mut elements = document.elements().unwrap();
        elements.push(dead);
        document.set_elements(elements);
        let before = document.clone();
        let mut author = document.author(profile).unwrap();
        for (ids, destination) in [
            (vec!["missing"], "frame"),
            (vec!["dead"], "frame"),
            (vec!["first", "first"], "frame"),
            (vec!["first"], "missing"),
            (vec!["first"], "border"),
            (vec!["frame"], "frame"),
            (vec!["frame"], "other-frame"),
        ] {
            let ids: Vec<_> = ids.into_iter().map(ElementId::from).collect();
            assert!(
                author
                    .set_frame_with_order(&ids, Some(&destination.into()), FrameOrder::Preserve)
                    .is_err()
            );
            assert_eq!(author.document(), &before);
            // The indexed closure is also shared by translation.
            if destination == "frame" && ids[0] != ElementId::from("frame") {
                assert!(
                    author
                        .translate_connected(&ids, [1_i64.into(), 2_i64.into()])
                        .is_err()
                );
                assert_eq!(author.document(), &before);
            }
        }
    }
}

#[test]
fn preserving_assignment_expands_labels_and_descendants_and_rejects_reached_frames_or_tombstones() {
    for profile in PROFILES {
        let mut document = crossing(profile, false);
        let label = Element::new(ElementKind::Text, profile, "label".into(), 0_u64.into()).unwrap();
        document
            .author(profile)
            .unwrap()
            .insert(vec![label])
            .unwrap();
        document
            .author(profile)
            .unwrap()
            .bind_label(&"label".into(), &"first".into())
            .unwrap();
        let before = document.clone();
        document
            .author(profile)
            .unwrap()
            .set_frame_with_order(
                &["label".into()],
                Some(&"frame".into()),
                FrameOrder::Preserve,
            )
            .unwrap();
        let mut expected = before.elements().unwrap();
        for e in &mut expected {
            if matches!(
                e.kind().unwrap(),
                Field::Value(ElementKind::Rectangle | ElementKind::Text)
            ) {
                e.set(element::FRAME_ID, "frame".into()).unwrap();
            }
        }
        assert_eq!(document.elements().unwrap(), expected);
        document
            .author(profile)
            .unwrap()
            .translate_connected(&["frame".into()], [3_i64.into(), 4_i64.into()])
            .unwrap();
        for e in document.elements().unwrap() {
            let moved = e.get(element::ID).unwrap() != Field::Value("other-frame".into());
            assert_eq!(
                e.as_object()["x"].as_f64().unwrap(),
                if moved { 3. } else { 0. }
            );
        }

        // Group expansion must not bypass nested-frame or tombstone rejection.
        for (id, deleted) in [("frame", false), ("delayed", true)] {
            let mut document = crossing(profile, false);
            let mut elements = document.elements().unwrap();
            let e = elements
                .iter_mut()
                .find(|e| e.get(element::ID).unwrap() == Field::Value(id.into()))
                .unwrap();
            e.set(element::GROUP_IDS, vec!["a".into()]).unwrap();
            e.set(element::IS_DELETED, deleted).unwrap();
            document.set_elements(elements);
            let before = document.clone();
            assert!(
                document
                    .author(profile)
                    .unwrap()
                    .set_frame_with_order(
                        &["first".into()],
                        Some(&"other-frame".into()),
                        FrameOrder::Preserve
                    )
                    .is_err()
            );
            assert_eq!(document, before);
        }
    }
}
