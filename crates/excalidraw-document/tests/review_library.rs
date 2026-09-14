use excalidraw_document::{
    Element, ElementId, ElementKind, LibraryDocument, Number, Profile, Purpose, Severity,
};
use serde_json::{Value, json};

const PROFILES: [Profile; 2] = [Profile::V0_18_1, Profile::SnapshotAfa3a653];
const PURPOSES: [Purpose; 3] = [Purpose::Inspect, Purpose::Author, Purpose::SelfContained];

#[test]
fn library_root_and_known_scalars_are_checked_without_coercion() {
    let input = br#"{"type":"excalidrawlib","version":2,"source":false,"libraryItems":[{"id":"item","status":"unpublished","created":1e400,"elements":[],"future":1e400}]}"#;
    let library = LibraryDocument::from_slice(input).unwrap();
    let before = library.to_vec().unwrap();
    for profile in PROFILES {
        for purpose in PURPOSES {
            let report = library.validate(profile, purpose);
            for (path, code) in [
                ("/source", "field-type"),
                ("/libraryItems/0/created", "numeric-range"),
            ] {
                assert!(
                    report
                        .diagnostics
                        .iter()
                        .any(|d| d.path == path && d.code == code && d.severity == Severity::Error),
                    "{report:?}"
                );
            }
            assert!(!report.diagnostics.iter().any(|d| d.path.contains("future")));
        }
    }
    assert_eq!(library.to_vec().unwrap(), before);
    assert_eq!(LibraryDocument::from_slice(&before).unwrap(), library);

    for version in [1, 2] {
        let key = if version == 1 {
            "library"
        } else {
            "libraryItems"
        };
        let library =
            LibraryDocument::from_value(json!({"type":"excalidrawlib","version":version,key:[]}))
                .unwrap();
        for profile in PROFILES {
            assert!(library.validate(profile, Purpose::Inspect).is_valid());
            for purpose in [Purpose::Author, Purpose::SelfContained] {
                assert!(
                    library
                        .validate(profile, purpose)
                        .diagnostics
                        .iter()
                        .any(|d| d.path == "/source" && d.code == "required")
                );
            }
        }
    }
}

#[test]
fn library_items_require_nondeleted_elements_in_both_versions() {
    for profile in PROFILES {
        let mut shape = Element::new(
            ElementKind::Rectangle,
            profile,
            ElementId::from("box"),
            Number::from(1_u64),
        )
        .unwrap();
        for deleted in [false, true] {
            shape.set_raw("isDeleted", json!(deleted));
            for version in [1, 2] {
                let key = if version == 1 {
                    "library"
                } else {
                    "libraryItems"
                };
                let item = if version == 1 {
                    json!([shape])
                } else {
                    json!({"id":"item","status":"unpublished","created":0,"elements":[shape]})
                };
                let library = LibraryDocument::from_value(
                    json!({"type":"excalidrawlib","version":version,"source":"test",key:[item]}),
                )
                .unwrap();
                let before = library.to_vec().unwrap();
                let path = if version == 1 {
                    "/library/0/0/isDeleted"
                } else {
                    "/libraryItems/0/elements/0/isDeleted"
                };
                for purpose in PURPOSES {
                    let report = library.validate(profile, purpose);
                    assert_eq!(
                        report.is_valid(),
                        !deleted || purpose == Purpose::Inspect,
                        "{report:?}"
                    );
                    if deleted {
                        assert!(
                            report.diagnostics.iter().any(|d| d.path == path
                                && d.code == "deleted-library-element"
                                && d.severity
                                    == if purpose == Purpose::Inspect {
                                        Severity::Warning
                                    } else {
                                        Severity::Error
                                    }),
                            "{report:?}"
                        );
                    } else {
                        assert!(report.diagnostics.is_empty(), "{report:?}");
                    }
                }
                assert_eq!(library.to_vec().unwrap(), before);
            }
        }
    }
}

#[test]
fn empty_items_and_invalid_item_metadata_are_diagnosed() {
    for profile in PROFILES {
        for version in [1, 2] {
            let key = if version == 1 {
                "library"
            } else {
                "libraryItems"
            };
            let item = if version == 1 {
                json!([])
            } else {
                json!({"id":"item","status":"unpublished","created":0,"elements":[]})
            };
            let library = LibraryDocument::from_value(
                json!({"type":"excalidrawlib","version":version,"source":"test",key:[item]}),
            )
            .unwrap();
            for purpose in PURPOSES {
                let report = library.validate(profile, purpose);
                assert_eq!(report.is_valid(), purpose == Purpose::Inspect, "{report:?}");
                assert!(
                    report
                        .diagnostics
                        .iter()
                        .any(|d| d.code == "empty-library-item")
                );
            }
        }
        for (field, value, code) in [
            ("created", Value::Null, "null-field"),
            ("created", json!(1.5), "integer"),
            ("id", json!(""), "empty-id"),
        ] {
            let mut item = json!({"id":"item","status":"unpublished","created":0,"elements":[]});
            item[field] = value;
            let library = LibraryDocument::from_value(
                json!({"type":"excalidrawlib","version":2,"source":"test","libraryItems":[item]}),
            )
            .unwrap();
            for purpose in PURPOSES {
                assert!(
                    library
                        .validate(profile, purpose)
                        .diagnostics
                        .iter()
                        .any(|d| d.path == format!("/libraryItems/0/{field}") && d.code == code)
                );
            }
        }
    }
}

#[test]
fn legacy_extension_collection_never_becomes_the_active_typed_items() {
    let mut library = LibraryDocument::from_value(json!({"type":"excalidrawlib","version":1,"source":"test","library":[[{"type":"rectangle","id":"box"}]],"libraryItems":[{"id":"extension","created":false}]})).unwrap();
    let before = library.to_vec().unwrap();
    assert_eq!(library.items().unwrap_err().path, "/version");
    assert_eq!(library.set_items(vec![]).unwrap_err().path, "/version");
    for profile in PROFILES {
        assert!(
            library
                .validate(profile, Purpose::Inspect)
                .diagnostics
                .is_empty()
        );
        for purpose in [Purpose::Author, Purpose::SelfContained] {
            let report = library.validate(profile, purpose);
            assert!(!report.is_valid());
            assert!(
                report
                    .diagnostics
                    .iter()
                    .all(|d| d.path.starts_with("/library/0/0/")),
                "{report:?}"
            );
        }
    }
    assert_eq!(library.to_vec().unwrap(), before);
    let mut current = LibraryDocument::from_value(json!({"type":"excalidrawlib","version":2,"source":"test","library":false,"libraryItems":[]})).unwrap();
    assert!(current.items().unwrap().is_empty());
    current.set_items(vec![]).unwrap();
    for profile in PROFILES {
        assert!(
            current
                .validate(profile, Purpose::Author)
                .diagnostics
                .is_empty()
        );
    }
}
