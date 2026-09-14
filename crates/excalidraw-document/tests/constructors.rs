use excalidraw_document::*;
use serde_json::json;

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];

fn metadata(profile: Profile) -> Authoring {
    Authoring {
        id: ElementId::from("authored"),
        updated: 123_i64.into(),
        created: match profile {
            Profile::V0_18_1 => Field::Missing,
            Profile::SnapshotAfa3a653 => Field::Value(100_i64.into()),
        },
        seed: 7_i64.into(),
        version: 2_i64.into(),
        version_nonce: 9_i64.into(),
    }
}

fn path(points: &[[i64; 2]]) -> Vec<Point> {
    points.iter().map(|p| p.map(Number::from)).collect()
}

fn bounds(element: &Element) -> (f64, f64) {
    (
        element.as_object()["width"].as_f64().unwrap(),
        element.as_object()["height"].as_f64().unwrap(),
    )
}

fn assert_authored(element: Element, profile: Profile) {
    let mut document = Document::new("constructors test");
    document.set_elements(vec![element]);
    for purpose in [Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(profile, purpose);
        assert!(report.is_valid(), "{profile:?}/{purpose:?}: {report:?}");
    }
}

#[test]
fn path_updates_use_extrema_and_preserve_position_and_neighboring_fields() {
    for profile in PROFILES {
        for kind in [ElementKind::Line, ElementKind::Arrow, ElementKind::Freedraw] {
            let mut element = Element::authored(kind, profile, metadata(profile)).unwrap();
            element.set(element::X, 70_i64.into()).unwrap();
            element.set(element::Y, (-90_i64).into()).unwrap();
            element.set_raw("future", json!({"number": 42, "nested": [null]}));
            let before = element.clone();
            for (points, expected) in [
                (path(&[[0, 0], [200, 0], [200, 100]]), (200., 100.)),
                (path(&[[0, 0], [-40, -60], [20, 10], [-5, 2]]), (60., 70.)),
                (path(&[[0, 0], [-40, -60], [-10, -20]]), (40., 60.)),
            ] {
                element.set_path(points.clone()).unwrap();
                assert_eq!(element.get(element::POINTS).unwrap(), Field::Value(points));
                assert_eq!(bounds(&element), expected);
                let mut neighbors = element.clone();
                for key in ["points", "width", "height"] {
                    neighbors.set_raw(key, before.as_object()[key].clone());
                }
                assert_eq!(neighbors, before);
                assert_authored(element.clone(), profile);
            }
        }
    }
}

#[test]
fn path_keeps_exact_input_numbers_while_computing_fractional_bounds() {
    for profile in PROFILES {
        let mut element = Element::authored(ElementKind::Line, profile, metadata(profile)).unwrap();
        let points = vec![
            ["0.0".parse().unwrap(), "-0".parse().unwrap()],
            ["-1.25".parse().unwrap(), "2.75".parse().unwrap()],
            ["3.5".parse().unwrap(), "-0.25".parse().unwrap()],
        ];
        element.set_path(points.clone()).unwrap();
        assert_eq!(element.get(element::POINTS).unwrap(), Field::Value(points));
        assert_eq!(bounds(&element), (4.75, 3.));
    }
}

#[test]
fn invalid_paths_and_overflow_roll_back_the_entire_element() {
    for profile in PROFILES {
        for kind in [ElementKind::Line, ElementKind::Arrow, ElementKind::Freedraw] {
            let mut element = Element::authored(kind, profile, metadata(profile)).unwrap();
            element.set_path(path(&[[0, 0], [10, 20]])).unwrap();
            element.set(element::X, 100_i64.into()).unwrap();
            element.set_raw("future", json!({"preserve": true}));
            let before = element.clone();
            let overflow: Number = "1e400".parse().unwrap();
            for (points, error_path) in [
                (path(&[[1, 0], [2, 0]]), "/points/0/0"),
                (path(&[[0, -1], [2, 0]]), "/points/0/1"),
                (
                    vec![["1e-400".parse().unwrap(), 0_i64.into()]],
                    "/points/0/0",
                ),
                (vec![[overflow.clone(), 0_i64.into()]], "/points/0/0"),
                (
                    vec![
                        [0_i64.into(), 0_i64.into()],
                        [overflow.clone(), 0_i64.into()],
                    ],
                    "/points/1/0",
                ),
                (
                    vec![[0_i64.into(), 0_i64.into()], [0_i64.into(), overflow]],
                    "/points/1/1",
                ),
                (
                    vec![
                        [0_i64.into(), 0_i64.into()],
                        [Number::from_f64(-f64::MAX).unwrap(), 0_i64.into()],
                        [Number::from_f64(f64::MAX).unwrap(), 0_i64.into()],
                    ],
                    "/width",
                ),
                (
                    vec![
                        [0_i64.into(), 0_i64.into()],
                        [0_i64.into(), Number::from_f64(-f64::MAX).unwrap()],
                        [0_i64.into(), Number::from_f64(f64::MAX).unwrap()],
                    ],
                    "/height",
                ),
            ] {
                assert_eq!(element.set_path(points).unwrap_err().path, error_path);
                assert_eq!(element, before, "failed at {error_path}");
            }
        }
    }
}

#[test]
fn path_rejects_nonpath_and_malformed_kinds_atomically() {
    for profile in PROFILES {
        let original =
            Element::authored(ElementKind::Rectangle, profile, metadata(profile)).unwrap();
        for kind in [
            json!("rectangle"),
            json!("stickynote"),
            json!("draw"),
            json!("selection"),
            json!("future-kind"),
            json!(false),
            json!(null),
        ] {
            let mut element = original.clone();
            element.set_raw("type", kind);
            let before = element.clone();
            assert_eq!(
                element.set_path(path(&[[0, 0], [1, 1]])).unwrap_err().path,
                "/type"
            );
            assert_eq!(element, before);
        }
        let mut missing = Element::default();
        assert_eq!(missing.set_path(vec![]).unwrap_err().path, "/type");
        assert_eq!(missing, Element::default());
    }
}

#[test]
fn elbow_constructor_sets_bounds_and_checks_the_path_in_both_profiles() {
    for profile in PROFILES {
        for (points, expected) in [
            (path(&[[0, 0], [200, 0], [200, 100]]), (200., 100.)),
            (path(&[[0, 0], [-200, 0], [-200, -100]]), (200., 100.)),
        ] {
            let element = Element::elbow_arrow(profile, metadata(profile), points.clone()).unwrap();
            assert_eq!(element.get(element::POINTS).unwrap(), Field::Value(points));
            assert_eq!(bounds(&element), expected);
            assert_eq!(element.get(element::ELBOWED).unwrap(), Field::Value(true));
            assert_eq!(element.get(element::FIXED_SEGMENTS).unwrap(), Field::Null);
            assert_authored(element, profile);
        }
        assert!(Element::elbow_arrow(profile, metadata(profile), path(&[[1, 0], [2, 0]])).is_err());
        assert!(
            Element::elbow_arrow(
                profile,
                metadata(profile),
                vec![
                    [0_i64.into(), 0_i64.into()],
                    ["1e400".parse().unwrap(), 0_i64.into()]
                ],
            )
            .is_err()
        );
    }
}

#[test]
fn authored_metadata_checks_profile_presence_and_retains_snapshot_null() {
    for profile in PROFILES {
        for created in [Field::Missing, Field::Null, Field::Value(100_i64.into())] {
            let mut meta = metadata(profile);
            meta.created = created.clone();
            let result = Element::authored(ElementKind::Rectangle, profile, meta);
            let compatible = match profile {
                Profile::V0_18_1 => matches!(created, Field::Missing),
                Profile::SnapshotAfa3a653 => !matches!(created, Field::Missing),
            };
            if compatible {
                let element = result.unwrap();
                assert_eq!(element.get(element::CREATED).unwrap(), created);
                assert_eq!(element.as_object()["updated"], 123);
                assert_eq!(element.as_object()["seed"], 7);
                assert_eq!(element.as_object()["version"], 2);
                assert_eq!(element.as_object()["versionNonce"], 9);
                assert_authored(element, profile);
            } else {
                assert_eq!(result.unwrap_err().path, "/created");
            }
        }
    }
}

#[test]
fn authored_rejects_invalid_explicit_integer_metadata_and_nonpositive_versions() {
    for profile in PROFILES {
        for field in ["updated", "seed", "version", "versionNonce", "created"] {
            if field == "created" && profile == Profile::V0_18_1 {
                continue;
            }
            for invalid in [
                "1.5",
                "9007199254740992",
                "-9007199254740992",
                "1e400",
                "1e-400",
                "1.0000000000000001",
            ] {
                let mut meta = metadata(profile);
                let value = invalid.parse().unwrap();
                match field {
                    "updated" => meta.updated = value,
                    "seed" => meta.seed = value,
                    "version" => meta.version = value,
                    "versionNonce" => meta.version_nonce = value,
                    "created" => meta.created = Field::Value(value),
                    _ => unreachable!(),
                }
                let error = Element::authored(ElementKind::Rectangle, profile, meta).unwrap_err();
                assert_eq!(error.path, format!("/{field}"), "{invalid}");
            }
        }
        for version in ["0", "-0", "-1"] {
            let mut meta = metadata(profile);
            meta.version = version.parse().unwrap();
            assert_eq!(
                Element::authored(ElementKind::Rectangle, profile, meta)
                    .unwrap_err()
                    .path,
                "/version"
            );
        }
        let mut meta = metadata(profile);
        meta.seed = "-9007199254740991".parse().unwrap();
        meta.version_nonce = "9007199254740991".parse().unwrap();
        meta.version = "1.0".parse().unwrap();
        meta.updated = "1e3".parse().unwrap();
        assert_authored(
            Element::authored(ElementKind::Rectangle, profile, meta).unwrap(),
            profile,
        );
    }
}

#[test]
fn sticky_notes_have_upstream_color_and_coherent_explicit_size() {
    let profile = Profile::SnapshotAfa3a653;
    let element = Element::sticky_note(
        profile,
        metadata(profile),
        "200.5".parse().unwrap(),
        "100.25".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(bounds(&element), (200.5, 100.25));
    assert_eq!(
        element.as_object()["height"],
        element.as_object()["baseHeight"]
    );
    assert_eq!(element.as_object()["backgroundColor"], "#ffdf6b");
    assert_authored(element, profile);

    for dimension in ["-1", "-1e-400", "1e400", "-1e400"] {
        for field in ["width", "height"] {
            let (width, height) = if field == "width" {
                (dimension.parse().unwrap(), 100_i64.into())
            } else {
                (200_i64.into(), dimension.parse().unwrap())
            };
            assert_eq!(
                Element::sticky_note(profile, metadata(profile), width, height)
                    .unwrap_err()
                    .path,
                format!("/{field}")
            );
        }
    }
    let release = Profile::V0_18_1;
    assert_eq!(
        Element::sticky_note(release, metadata(release), 200_i64.into(), 100_i64.into())
            .unwrap_err()
            .path,
        "/type"
    );
}

#[test]
fn default_and_explicit_zero_geometry_remains_available_for_drafts() {
    for profile in PROFILES {
        for kind in [ElementKind::Line, ElementKind::Arrow, ElementKind::Freedraw] {
            let mut element = Element::authored(kind, profile, metadata(profile)).unwrap();
            assert_eq!(bounds(&element), (0., 0.));
            assert_authored(element.clone(), profile);
            for points in [vec![], path(&[[0, 0]]), path(&[[0, 0], [0, 0]])] {
                element.set_path(path(&[[0, 0], [10, 20]])).unwrap();
                element.set_path(points).unwrap();
                assert_eq!(bounds(&element), (0., 0.));
                assert_authored(element.clone(), profile);
            }
        }
        assert_authored(
            Element::elbow_arrow(profile, metadata(profile), vec![]).unwrap(),
            profile,
        );
    }
    let profile = Profile::SnapshotAfa3a653;
    for element in [
        Element::new(
            ElementKind::Stickynote,
            profile,
            ElementId::from("draft"),
            1_i64.into(),
        )
        .unwrap(),
        Element::sticky_note(profile, metadata(profile), 0_i64.into(), 0_i64.into()).unwrap(),
        Element::sticky_note(
            profile,
            metadata(profile),
            "-0.0".parse().unwrap(),
            "-0.0".parse().unwrap(),
        )
        .unwrap(),
    ] {
        assert_eq!(bounds(&element), (0., 0.));
        assert_eq!(element.as_object()["baseHeight"].as_f64(), Some(0.));
        assert_eq!(element.as_object()["backgroundColor"], "#ffdf6b");
        assert_authored(element, profile);
    }
    // The preserving construction boundary still accepts historical metadata.
    let element = Element::new(
        ElementKind::Rectangle,
        profile,
        ElementId::from("historical"),
        "1e400".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(
        element.get(element::UPDATED).unwrap(),
        Field::Value("1e400".parse().unwrap())
    );
}
