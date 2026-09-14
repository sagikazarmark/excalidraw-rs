use excalidraw_document::*;
use serde_json::json;
use std::collections::BTreeMap;

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

fn shape(profile: Profile, kind: ElementKind, id: &str) -> Element {
    Element::new(kind, profile, id.into(), 1_u64.into()).unwrap()
}

fn asset(id: &str) -> BinaryFile {
    BinaryFile::new(
        id.into(),
        MimeType::Png,
        "data:image/png;base64,AQ==".into(),
        1_u64.into(),
    )
    .unwrap()
}

fn source_files() -> BTreeMap<FileId, BinaryFile> {
    BTreeMap::from([("asset".into(), asset("asset"))])
}

fn item(profile: Profile) -> LibraryItem {
    let mut source = Document::new("source");
    let mut frame = shape(profile, ElementKind::Frame, "frame");
    frame.set(element::X, 10_i64.into()).unwrap();
    let mut image = shape(profile, ElementKind::Image, "image");
    image.set(element::FRAME_ID, "frame".into()).unwrap();
    image.set(element::X, 20_i64.into()).unwrap();
    image.set(element::Y, 30_i64.into()).unwrap();
    image.set(element::GROUP_IDS, vec!["group".into()]).unwrap();
    let mut box_element = shape(profile, ElementKind::Rectangle, "box");
    box_element
        .set(element::GROUP_IDS, vec!["group".into()])
        .unwrap();
    let mut author = source.author(profile).unwrap();
    author
        .insert(vec![
            frame,
            box_element,
            shape(profile, ElementKind::Text, "label"),
        ])
        .unwrap();
    author.bind_label(&"label".into(), &"box".into()).unwrap();
    author.insert_image(image, asset("asset")).unwrap();
    LibraryItem::new(profile, "item", 1_u64.into(), source.elements().unwrap()).unwrap()
}

fn mapping(suffix: &str) -> IdMap {
    IdMap {
        elements: ["frame", "image", "box", "label"]
            .into_iter()
            .map(|id| (id.into(), ElementId(format!("{id}-{suffix}"))))
            .collect(),
        groups: BTreeMap::from([("group".into(), GroupId(format!("group-{suffix}")))]),
        files: BTreeMap::new(),
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
fn binary_file_constructor_and_registration_validate_known_data() {
    for (id, mime, url, created) in [
        ("", MimeType::Png, "data:image/png,x", 1_i64.into()),
        (
            "asset",
            MimeType::Unknown("image/future".into()),
            "data:image/future,x",
            1_i64.into(),
        ),
        ("asset", MimeType::Png, "", 1_i64.into()),
        (
            "asset",
            MimeType::Png,
            "https://example.com/image.png",
            1_i64.into(),
        ),
        ("asset", MimeType::Png, "data:image/jpeg,x", 1_i64.into()),
        (
            "asset",
            MimeType::Png,
            "data:image/png;base64,",
            1_i64.into(),
        ),
        (
            "asset",
            MimeType::Png,
            "data:image/png,x",
            "1.5".parse().unwrap(),
        ),
        (
            "asset",
            MimeType::Png,
            "data:image/png,x",
            "9007199254740992".parse().unwrap(),
        ),
        (
            "asset",
            MimeType::Png,
            "data:image/png,x",
            "1e400".parse().unwrap(),
        ),
    ] {
        assert!(BinaryFile::new(id.into(), mime, url.into(), created).is_err());
    }
    assert!(
        BinaryFile::new(
            "svg".into(),
            MimeType::Svg,
            "data:image/svg+xml;charset=utf-8,%3Csvg/%3E".into(),
            0_i64.into()
        )
        .is_ok()
    );
    for profile in PROFILES {
        let mut doc = Document::new("register");
        let mut file = asset("asset");
        file.set_raw("future", json!({"preserve":true}));
        let mut author = doc.author(profile).unwrap();
        author.register_file(file.clone()).unwrap();
        let before = author.document().clone();
        author.register_file(file.clone()).unwrap();
        assert_eq!(author.document(), &before);
        file.set_raw("future", json!({"preserve":false}));
        assert!(author.register_file(file).is_err());
        assert_eq!(author.document(), &before);
        for (field, value) in [
            ("created", json!(false)),
            ("version", json!(1.5)),
            ("lastRetrieved", json!(null)),
            ("mimeType", json!("image/unknown")),
            ("dataURL", json!("data:image/jpeg,x")),
            ("id", json!("")),
        ] {
            let mut bad = asset("new");
            bad.set_raw(field, value);
            assert!(author.register_file(bad).is_err(), "{field}");
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn images_assign_saved_resources_and_roll_back_files_on_scene_errors() {
    for profile in PROFILES {
        let mut scene = Document::new("images");
        scene.set_root("future", json!({"keep":true})).unwrap();
        let mut author = scene.author(profile).unwrap();
        let mut image = shape(profile, ElementKind::Image, "one");
        image.set_raw("customData", json!({"keep":true}));
        image.set(element::FILE_ID, "replaced".into()).unwrap();
        image.set(element::STATUS, ImageStatus::Error).unwrap();
        author.insert_image(image, asset("asset")).unwrap();
        author
            .insert_image(shape(profile, ElementKind::Image, "two"), asset("asset"))
            .unwrap();
        assert_eq!(author.document().files().unwrap().len(), 1);
        assert_eq!(by_id(author.document(), "one")["fileId"], "asset");
        assert_eq!(by_id(author.document(), "one")["status"], "saved");
        assert_eq!(
            by_id(author.document(), "one")["customData"],
            json!({"keep":true})
        );
        assert_eq!(
            author.document().as_object()["future"],
            json!({"keep":true})
        );
        let before = author.document().clone();
        let mut dangling = shape(profile, ElementKind::Image, "dangling");
        dangling.set(element::FRAME_ID, "absent".into()).unwrap();
        let mut negative = shape(profile, ElementKind::Image, "negative");
        negative.set(element::WIDTH, (-1_i64).into()).unwrap();
        for bad in [
            shape(profile, ElementKind::Rectangle, "not-image"),
            shape(profile, ElementKind::Image, "one"),
            dangling,
            negative,
        ] {
            assert!(author.insert_image(bad, asset("new-resource")).is_err());
            assert_eq!(author.document(), &before);
        }
        let mut conflict = asset("asset");
        conflict.set(binary_file::CREATED, 2_i64.into()).unwrap();
        assert!(
            author
                .insert_image(shape(profile, ElementKind::Image, "three"), conflict)
                .is_err()
        );
        assert_eq!(author.document(), &before);
        assert!(
            author
                .document()
                .validate(profile, Purpose::SelfContained)
                .is_valid()
        );
    }
}

#[test]
fn library_constructor_validates_item_scopes_without_requiring_embedded_files() {
    for profile in PROFILES {
        let valid = item(profile);
        assert_eq!(
            valid.get(library_item::STATUS).unwrap(),
            Field::Value(LibraryStatus::Unpublished)
        );
        assert!(!valid.as_object().contains_key("files"));
        let elements = match valid.get(library_item::ELEMENTS).unwrap() {
            Field::Value(elements) => elements,
            _ => unreachable!(),
        };
        let library = LibraryDocument::new(
            "independent-scopes",
            vec![
                valid,
                LibraryItem::new(profile, "second", 2_i64.into(), elements.clone()).unwrap(),
            ],
        );
        assert!(library.validate(profile, Purpose::Author).is_valid());
        for (id, created, elements, path) in [
            ("", 1_i64.into(), elements.clone(), "/libraryItems/0/id"),
            (
                "item",
                "1.5".parse().unwrap(),
                elements.clone(),
                "/libraryItems/0/created",
            ),
            ("item", 1_i64.into(), vec![], "/libraryItems/0/elements"),
        ] {
            assert_eq!(
                LibraryItem::new(profile, id, created, elements)
                    .unwrap_err()
                    .path,
                path
            );
        }
        let mut dangling = shape(profile, ElementKind::Text, "label");
        dangling.set(element::CONTAINER_ID, "box".into()).unwrap();
        let error = LibraryItem::new(profile, "item", 1_i64.into(), vec![dangling]).unwrap_err();
        assert!(error.path.starts_with("/libraryItems/0/elements/0/"));
        let mut deleted = shape(profile, ElementKind::Rectangle, "deleted");
        deleted.set(element::IS_DELETED, true).unwrap();
        assert!(LibraryItem::new(profile, "item", 1_i64.into(), vec![deleted]).is_err());
        let duplicate = shape(profile, ElementKind::Rectangle, "same");
        assert!(
            LibraryItem::new(
                profile,
                "item",
                1_i64.into(),
                vec![duplicate.clone(), duplicate]
            )
            .is_err()
        );
    }
}

#[test]
fn repeated_library_insertion_has_independent_graphs_and_shared_or_new_assets() {
    for profile in PROFILES {
        let item = item(profile);
        let original = item.clone();
        for _ in 0..2 {
            let mut scene = Document::new("independent-scene");
            let mut author = scene.author(profile).unwrap();
            for (suffix, dx, dy, fresh_file) in [
                ("a", 100_i64, -50_i64, false),
                ("b", -20, 80, false),
                ("c", 0, 0, true),
            ] {
                let mut mapping = mapping(suffix);
                if fresh_file {
                    mapping.files.insert("asset".into(), "asset-copy".into());
                }
                assert!(
                    author
                        .insert_library_item(
                            &item,
                            &mapping,
                            source_files(),
                            [dx.into(), dy.into()],
                            OpaquePolicy::Reject
                        )
                        .unwrap()
                        .is_empty()
                );
                let image = by_id(author.document(), &format!("image-{suffix}"));
                assert_eq!(image["x"].as_f64(), Some((20 + dx) as f64));
                assert_eq!(image["y"].as_f64(), Some((30 + dy) as f64));
                assert_eq!(image["frameId"], format!("frame-{suffix}"));
                assert_eq!(image["groupIds"], json!([format!("group-{suffix}")]));
                assert_eq!(
                    image["fileId"],
                    if fresh_file { "asset-copy" } else { "asset" }
                );
                assert_eq!(image["status"], "saved");
                assert_eq!(
                    by_id(author.document(), &format!("frame-{suffix}"))["x"].as_f64(),
                    Some((10 + dx) as f64)
                );
                assert_eq!(
                    by_id(author.document(), &format!("label-{suffix}"))["containerId"],
                    format!("box-{suffix}")
                );
                assert_eq!(
                    by_id(author.document(), &format!("box-{suffix}"))["boundElements"],
                    json!([{"id":format!("label-{suffix}"),"type":"text"}])
                );
            }
            assert_eq!(author.document().elements().unwrap().len(), 12);
            assert_eq!(author.document().files().unwrap().len(), 2);
            assert_eq!(
                author.document().files().unwrap()[&FileId::from("asset-copy")]
                    .get(binary_file::ID)
                    .unwrap(),
                Field::Value("asset-copy".into())
            );
            assert!(
                author
                    .document()
                    .validate(profile, Purpose::SelfContained)
                    .is_valid()
            );
        }
        assert_eq!(item, original);
    }
}

#[test]
fn library_mapping_conflicts_missing_resources_and_external_references_are_atomic() {
    for profile in PROFILES {
        let item = item(profile);
        let mut scene = Document::new("rollback");
        let mut author = scene.author(profile).unwrap();
        author
            .insert_library_item(
                &item,
                &mapping("a"),
                source_files(),
                [0_i64.into(), 0_i64.into()],
                OpaquePolicy::Reject,
            )
            .unwrap();
        let before = author.document().clone();
        let mut missing = mapping("b");
        missing.elements.remove(&ElementId::from("image"));
        let mut reused = mapping("b");
        reused.elements.insert("image".into(), "image".into());
        let mut source_swap = mapping("b");
        source_swap.elements.insert("image".into(), "box".into());
        let mut duplicate = mapping("b");
        duplicate.elements.insert("image".into(), "box-b".into());
        let mut no_groups = mapping("b");
        no_groups.groups.clear();
        let mut old_group = mapping("b");
        old_group.groups.insert("group".into(), "group-a".into());
        let mut source_group = mapping("b");
        source_group.groups.insert("group".into(), "group".into());
        let mut extra = mapping("b");
        extra.elements.insert("unknown".into(), "fresh".into());
        let mut bad_file_map = mapping("b");
        bad_file_map.files.insert("absent".into(), "new".into());
        for bad in [
            mapping("a"),
            missing,
            reused,
            source_swap,
            duplicate,
            no_groups,
            old_group,
            source_group,
            extra,
            bad_file_map,
        ] {
            assert!(
                author
                    .insert_library_item(
                        &item,
                        &bad,
                        source_files(),
                        [0_i64.into(), 0_i64.into()],
                        OpaquePolicy::Reject
                    )
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
        let mut conflicting = source_files();
        conflicting
            .get_mut(&FileId::from("asset"))
            .unwrap()
            .set(binary_file::CREATED, 2_i64.into())
            .unwrap();
        let mut extra_file = source_files();
        extra_file.insert("unused".into(), asset("unused"));
        for files in [
            BTreeMap::new(),
            BTreeMap::from([("asset".into(), asset("wrong-id"))]),
            conflicting,
            extra_file,
        ] {
            assert!(
                author
                    .insert_library_item(
                        &item,
                        &mapping("b"),
                        files,
                        [0_i64.into(), 0_i64.into()],
                        OpaquePolicy::Reject
                    )
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
        let mut dangling = item.clone();
        let Field::Value(mut elements) = dangling.get(library_item::ELEMENTS).unwrap() else {
            unreachable!()
        };
        elements
            .iter_mut()
            .find(|e| e.kind().unwrap() == Field::Value(ElementKind::Image))
            .unwrap()
            .set(element::FRAME_ID, "frame-a".into())
            .unwrap();
        dangling.set(library_item::ELEMENTS, elements).unwrap();
        assert!(
            author
                .insert_library_item(
                    &dangling,
                    &mapping("b"),
                    source_files(),
                    [0_i64.into(), 0_i64.into()],
                    OpaquePolicy::Reject
                )
                .is_err()
        );
        assert_eq!(author.document(), &before);
        let mut huge = item.clone();
        let Field::Value(mut elements) = huge.get(library_item::ELEMENTS).unwrap() else {
            unreachable!()
        };
        elements[0]
            .set(element::X, "1e308".parse().unwrap())
            .unwrap();
        huge.set(library_item::ELEMENTS, elements).unwrap();
        for offset in [
            ["1e400".parse().unwrap(), 0_i64.into()],
            ["1e308".parse().unwrap(), 0_i64.into()],
        ] {
            assert!(
                author
                    .insert_library_item(
                        &huge,
                        &mapping("b"),
                        source_files(),
                        offset,
                        OpaquePolicy::Reject
                    )
                    .is_err()
            );
            assert_eq!(author.document(), &before);
        }
    }
}

#[test]
fn opaque_library_and_resource_metadata_require_policy_and_are_preserved() {
    let profile = Profile::V0_18_1;
    let mut item = item(profile);
    item.set_raw("futureItem", json!({"id":"box"}));
    let Field::Value(mut elements) = item.get(library_item::ELEMENTS).unwrap() else {
        unreachable!()
    };
    elements[0].set_raw("customData", json!({"id":"image"}));
    item.set(library_item::ELEMENTS, elements).unwrap();
    let mut files = source_files();
    files
        .get_mut(&FileId::from("asset"))
        .unwrap()
        .set_raw("futureFile", json!({"id":"image"}));
    let original = item.clone();
    let mut scene = Document::new("opaque");
    let before = scene.clone();
    let mut author = scene.author(profile).unwrap();
    assert!(
        author
            .insert_library_item(
                &item,
                &mapping("a"),
                files.clone(),
                [0_i64.into(), 0_i64.into()],
                OpaquePolicy::Reject
            )
            .is_err()
    );
    assert_eq!(author.document(), &before);
    let paths = author
        .insert_library_item(
            &item,
            &mapping("a"),
            files,
            [0_i64.into(), 0_i64.into()],
            OpaquePolicy::Preserve,
        )
        .unwrap();
    assert!(paths.contains(&"/libraryItems/0/futureItem".into()));
    assert!(paths.contains(&"/elements/0/customData".into()));
    assert!(paths.contains(&"/files/asset/futureFile".into()));
    assert_eq!(
        by_id(author.document(), "frame-a")["customData"],
        json!({"id":"image"})
    );
    assert_eq!(
        author.document().as_object()["files"]["asset"]["futureFile"],
        json!({"id":"image"})
    );
    assert_eq!(item, original);
}

#[test]
fn library_resource_batch_rolls_back_earlier_assets_and_can_share_remapped_identity() {
    let profile = Profile::SnapshotAfa3a653;
    let mut a = shape(profile, ElementKind::Image, "a");
    let mut z = shape(profile, ElementKind::Image, "z");
    a.set(element::FILE_ID, "a-file".into()).unwrap();
    z.set(element::FILE_ID, "z-file".into()).unwrap();
    let item = LibraryItem::new(profile, "images", 1_i64.into(), vec![a, z]).unwrap();
    let mut scene = Document::new("batch");
    let mut author = scene.author(profile).unwrap();
    author.register_file(asset("existing")).unwrap();
    let before = author.document().clone();
    let mut mapping = IdMap {
        elements: BTreeMap::from([("a".into(), "a-copy".into()), ("z".into(), "z-copy".into())]),
        files: BTreeMap::from([("z-file".into(), "existing".into())]),
        ..IdMap::default()
    };
    let mut conflict = asset("z-file");
    conflict.set(binary_file::CREATED, 2_i64.into()).unwrap();
    assert!(
        author
            .insert_library_item(
                &item,
                &mapping,
                BTreeMap::from([
                    ("a-file".into(), asset("a-file")),
                    ("z-file".into(), conflict)
                ]),
                [0_i64.into(), 0_i64.into()],
                OpaquePolicy::Reject
            )
            .is_err()
    );
    assert_eq!(author.document(), &before);
    // Two source resources cannot collapse onto one destination, even if identical.
    mapping.files.insert("a-file".into(), "existing".into());
    let files = BTreeMap::from([
        ("a-file".into(), asset("a-file")),
        ("z-file".into(), asset("z-file")),
    ]);
    assert!(
        author
            .insert_library_item(
                &item,
                &mapping,
                files.clone(),
                [0_i64.into(), 0_i64.into()],
                OpaquePolicy::Reject
            )
            .is_err()
    );
    assert_eq!(author.document(), &before);
    mapping.files.remove(&FileId::from("a-file"));
    author
        .insert_library_item(
            &item,
            &mapping,
            files,
            [0_i64.into(), 0_i64.into()],
            OpaquePolicy::Reject,
        )
        .unwrap();
    assert_eq!(by_id(author.document(), "z-copy")["fileId"], "existing");
    assert_eq!(author.document().files().unwrap().len(), 2);
}

#[test]
fn library_freshness_includes_tombstones_and_unused_group_locks() {
    let profile = Profile::SnapshotAfa3a653;
    let item = item(profile);
    let mut scene = Document::new("reserved-identities");
    let mut tombstone = shape(profile, ElementKind::Rectangle, "image-a");
    tombstone.set(element::IS_DELETED, true).unwrap();
    scene
        .author(profile)
        .unwrap()
        .insert(vec![tombstone])
        .unwrap();
    let mut state = scene.app_state().unwrap();
    state
        .set(
            app_state::LOCKED_MULTI_SELECTIONS,
            BTreeMap::from([("group-b".into(), true)]),
        )
        .unwrap();
    scene.set_app_state(state);
    let before = scene.clone();
    let mut author = scene.author(profile).unwrap();
    for ids in [mapping("a"), mapping("b")] {
        assert!(
            author
                .insert_library_item(
                    &item,
                    &ids,
                    source_files(),
                    [0_i64.into(), 0_i64.into()],
                    OpaquePolicy::Reject
                )
                .is_err()
        );
        assert_eq!(author.document(), &before);
    }
}
