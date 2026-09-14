use excalidraw_document::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn element(value: Value) -> Element {
    Element::from_object(value.as_object().unwrap().clone())
}
fn binding(value: Value) -> Binding {
    Binding::from_object(value.as_object().unwrap().clone())
}

#[test]
fn generation_presence_and_local_edits() {
    for (value, expected) in [
        (json!({}), Field::Missing),
        (json!({"customData":null}), Field::Null),
        (json!({"customData":{}}), Field::Missing),
        (json!({"customData":{"generationData":null}}), Field::Null),
    ] {
        let mut e = element(value);
        assert_eq!(e.generation_data().unwrap(), expected);
        let before = e.clone();
        if !e
            .as_object()
            .get("customData")
            .is_some_and(|v| v.get("generationData").is_some())
        {
            e.set_generation_data(Field::Missing).unwrap();
            assert_eq!(e, before);
        }
        e.set_generation_data(Field::Null).unwrap();
        assert_eq!(e.as_object()["customData"], json!({"generationData":null}));
        e.set_generation_data(Field::Missing).unwrap();
        assert_eq!(e.as_object()["customData"], json!({}));
    }

    let mut doc = Document::from_value(json!({"type":"excalidraw","elements":[
        {"type":"iframe","text":false,"customData":{"sibling":{"deep":[null,42]},
          "generationData":{"status":"done","html":"old","future":[1,2]}}},
        false, {"type":"future","untouched":null}
    ],"extension":{"keep":true}}))
    .unwrap();
    let mut expected = doc.clone().into_value();
    doc.edit_element(0, |e| {
        let Field::Value(mut data) = e.generation_data()? else {
            panic!()
        };
        data.set(generation_data::HTML, "new".to_owned())?;
        e.set_generation_data(Field::Value(data))
    })
    .unwrap();
    expected["elements"][0]["customData"]["generationData"]["html"] = json!("new");
    assert_eq!(doc.clone().into_value(), expected);
    doc.edit_element(0, |e| e.set_generation_data(Field::Missing))
        .unwrap();
    expected["elements"][0]["customData"]
        .as_object_mut()
        .unwrap()
        .remove("generationData");
    assert_eq!(doc.into_value(), expected);
}

#[test]
fn malformed_generation_parent_is_atomic_and_children_stay_inspectable() {
    for parent in [json!(false), json!(4), json!("bad"), json!([])] {
        let mut e = element(json!({"customData":parent,"extension":123}));
        let before = e.clone();
        assert_eq!(e.generation_data().unwrap_err().path, "/customData");
        for field in [
            Field::Missing,
            Field::Null,
            Field::Value(GenerationData::default()),
        ] {
            assert_eq!(
                e.set_generation_data(field).unwrap_err().path,
                "/customData"
            );
            assert_eq!(e, before);
        }
    }
    for bad in [json!(false), json!(1), json!("bad"), json!([])] {
        let e = element(json!({"customData":{"generationData":bad}}));
        assert_eq!(
            e.generation_data().unwrap_err().path,
            "/customData/generationData"
        );
    }
    let e = element(json!({"customData":{"generationData":{"status":42,"html":null}}}));
    let Field::Value(data) = e.generation_data().unwrap() else {
        panic!()
    };
    assert_eq!(
        data.get(generation_data::STATUS).unwrap_err().path,
        "/status"
    );
    assert_eq!(data.get(generation_data::HTML).unwrap(), Field::Null);
}

#[test]
fn root_presence_and_typed_setters() {
    let mut doc = Document::from_value(json!({"type":"excalidraw","extension":[null,1]})).unwrap();
    assert_eq!(doc.source().unwrap(), Field::Missing);
    assert_eq!(doc.version().unwrap(), Field::Missing);
    assert_eq!(doc.elements_field().unwrap(), Field::Missing);
    assert_eq!(doc.app_state_field().unwrap(), Field::Missing);
    assert_eq!(doc.files_field().unwrap(), Field::Missing);
    assert!(doc.elements().is_err());
    assert!(doc.app_state().is_err());
    assert!(doc.files().is_err());

    doc.set_source(Field::Null).unwrap();
    doc.set_version(Field::Null).unwrap();
    doc.set_elements_field(Field::Null).unwrap();
    doc.set_app_state_field(Field::Null).unwrap();
    doc.set_files_field(Field::Null).unwrap();
    assert_eq!(doc.source().unwrap(), Field::Null);
    assert_eq!(doc.version().unwrap(), Field::Null);
    assert_eq!(doc.elements_field().unwrap(), Field::Null);
    assert_eq!(doc.app_state_field().unwrap(), Field::Null);
    assert_eq!(doc.files_field().unwrap(), Field::Null);
    assert!(doc.elements().is_err());
    assert!(doc.app_state().is_err());
    assert!(doc.files().is_err());

    let exact: Number = "123456789012345678901234567890".parse().unwrap();
    let elements = vec![element(json!({"type":false,"future":1}))];
    let state = AppState::from_object(
        json!({"gridSize":false,"future":null})
            .as_object()
            .unwrap()
            .clone(),
    );
    let files = BTreeMap::from([(FileId::from("a/b~c"), BinaryFile::default())]);
    doc.set_source(Field::Value("test".into())).unwrap();
    doc.set_version(Field::Value(exact.clone())).unwrap();
    doc.set_elements_field(Field::Value(elements.clone()))
        .unwrap();
    doc.set_app_state_field(Field::Value(state.clone()))
        .unwrap();
    doc.set_files_field(Field::Value(files.clone())).unwrap();
    assert_eq!(doc.source().unwrap(), Field::Value("test".into()));
    assert_eq!(doc.version().unwrap(), Field::Value(exact));
    assert_eq!(
        doc.elements_field().unwrap(),
        Field::Value(elements.clone())
    );
    assert_eq!(doc.elements().unwrap(), elements);
    assert_eq!(doc.app_state_field().unwrap(), Field::Value(state.clone()));
    assert_eq!(doc.app_state().unwrap(), state);
    assert_eq!(doc.files_field().unwrap(), Field::Value(files.clone()));
    assert_eq!(doc.files().unwrap(), files);

    doc.set_source(Field::Missing).unwrap();
    doc.set_version(Field::Missing).unwrap();
    doc.set_elements_field(Field::Missing).unwrap();
    doc.set_app_state_field(Field::Missing).unwrap();
    doc.set_files_field(Field::Missing).unwrap();
    assert_eq!(
        doc.clone().into_value(),
        json!({"type":"excalidraw","extension":[null,1]})
    );
    let before = doc.clone();
    assert_eq!(doc.remove_root("type").unwrap_err().path, "/type");
    assert_eq!(doc, before);
    doc.remove_root("absent").unwrap();
    assert_eq!(doc, before);
    doc.remove_root("extension").unwrap();
    assert_eq!(doc.into_value(), json!({"type":"excalidraw"}));
}

#[test]
fn root_malformed_records_and_envelope_guard() {
    let mut doc = Document::from_value(json!({"type":"excalidraw","source":false,"version":"2",
        "elements":[{},false],"files":{"a/b~c":null},"appState":{"gridSize":"bad"}}))
    .unwrap();
    let before = doc.clone();
    assert_eq!(doc.source().unwrap_err().path, "/source");
    assert_eq!(doc.version().unwrap_err().path, "/version");
    assert_eq!(doc.elements_field().unwrap_err().path, "/elements/1");
    assert_eq!(doc.files_field().unwrap_err().path, "/files/a~1b~0c");
    let Field::Value(state) = doc.app_state_field().unwrap() else {
        panic!()
    };
    assert_eq!(
        state.get(app_state::GRID_SIZE).unwrap_err().path,
        "/gridSize"
    );
    assert_eq!(doc, before);
    for (key, value) in [
        ("type", json!(null)),
        ("elements", json!({})),
        ("appState", json!([])),
        ("files", json!(true)),
    ] {
        assert_eq!(
            doc.set_root(key, value).unwrap_err().path,
            format!("/{key}")
        );
        assert_eq!(doc, before);
    }
    doc.set_source(Field::Value("repaired".into())).unwrap();
    let mut expected = before.into_value();
    expected["source"] = json!("repaired");
    assert_eq!(doc.into_value(), expected);
}

#[test]
fn pinned_numeric_registries_are_open_and_profile_aware() {
    let fonts = [
        (KnownFont::Virgil, 1_i64, "Virgil"),
        (KnownFont::Helvetica, 2, "Helvetica"),
        (KnownFont::Cascadia, 3, "Cascadia"),
        (KnownFont::Excalifont, 5, "Excalifont"),
        (KnownFont::Nunito, 6, "Nunito"),
        (KnownFont::LilitaOne, 7, "Lilita One"),
        (KnownFont::ComicShanns, 8, "Comic Shanns"),
        (KnownFont::LiberationSans, 9, "Liberation Sans"),
        (KnownFont::Assistant, 10, "Assistant"),
    ];
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        for (font, id, name) in fonts {
            assert_eq!(font.to_number(), Number::from(id));
            assert_eq!(font.name(), name);
            let expected = if id == 10 && profile == Profile::V0_18_1 {
                None
            } else {
                Some(font)
            };
            assert_eq!(KnownFont::from_number(profile, &Number::from(id)), expected);
        }
        for (kind, id, name) in [
            (KnownRoundness::Legacy, 1_i64, "LEGACY"),
            (KnownRoundness::ProportionalRadius, 2, "PROPORTIONAL_RADIUS"),
            (KnownRoundness::AdaptiveRadius, 3, "ADAPTIVE_RADIUS"),
        ] {
            assert_eq!(kind.name(), name);
            assert_eq!(kind.to_number(), Number::from(id));
            assert_eq!(
                KnownRoundness::from_number(profile, &Number::from(id)),
                Some(kind)
            );
        }
        for text in [
            "0",
            "4",
            "100",
            "998",
            "999",
            "1000",
            "-1",
            "1.5",
            "1e400",
            "1.00000000000000000001",
        ] {
            let unknown: Number = text.parse().unwrap();
            assert_eq!(KnownFont::from_number(profile, &unknown), None, "{text}");
            assert_eq!(
                KnownRoundness::from_number(profile, &unknown),
                None,
                "{text}"
            );
            let mut e = Element::default();
            let font: FontFamily = unknown.clone();
            let roundness: RoundnessType = unknown.clone();
            e.set(element::FONT_FAMILY, font).unwrap();
            let mut r = Roundness::default();
            r.set(roundness::TYPE, roundness).unwrap();
            e.set(element::ROUNDNESS, r.clone()).unwrap();
            assert_eq!(e.get(element::FONT_FAMILY).unwrap(), Field::Value(unknown));
            assert_eq!(e.get(element::ROUNDNESS).unwrap(), Field::Value(r));
        }
        assert_eq!(
            KnownFont::from_number(profile, &"5.0e0".parse().unwrap()),
            Some(KnownFont::Excalifont)
        );
    }
}

#[test]
fn element_views_cover_kinds_without_certifying_fields() {
    for kind in ElementKind::KNOWN {
        let e = element(json!({"type":kind,"x":false,"extension":42}));
        let view = e.view().unwrap();
        assert!(std::ptr::eq(view.element(), &e));
        assert_eq!(view.get(element::X).unwrap_err().path, "/x");
        assert_eq!(view.get(element::Y).unwrap(), Field::Missing);
        let actual = match view {
            ElementView::Rectangle(_) => "rectangle",
            ElementView::Diamond(_) => "diamond",
            ElementView::Ellipse(_) => "ellipse",
            ElementView::Text(_) => "text",
            ElementView::Line(_) => "line",
            ElementView::Arrow(_) => "arrow",
            ElementView::Freedraw(_) => "freedraw",
            ElementView::Image(_) => "image",
            ElementView::Frame(_) => "frame",
            ElementView::Magicframe(_) => "magicframe",
            ElementView::Iframe(_) => "iframe",
            ElementView::Embeddable(_) => "embeddable",
            ElementView::Stickynote(_) => "stickynote",
            ElementView::Legacy(_) => "legacy",
            ElementView::Unknown(_) => panic!("known kind classified as unknown"),
        };
        assert_eq!(
            actual,
            if matches!(*kind, "selection" | "draw") {
                "legacy"
            } else {
                kind
            }
        );
        assert_eq!(e.as_object()["extension"], json!(42));
    }
    let future = element(json!({"type":"future","extension":[null]}));
    assert!(matches!(future.view().unwrap(), ElementView::Unknown(_)));
    assert_eq!(
        future.view().unwrap().get(element::TYPE).unwrap(),
        Field::Value(ElementKind::Unknown("future".into()))
    );
    for value in [
        json!({}),
        json!({"type":null}),
        json!({"type":false}),
        json!({"type":{}}),
    ] {
        assert_eq!(element(value).view().unwrap_err().path, "/type");
    }
}

#[test]
fn bindings_discriminate_all_shapes_and_retain_original_extensions() {
    let ordinary = binding(json!({"elementId":"box","focus":0,"gap":3,"future":{"keep":true}}));
    let view = ordinary.view().unwrap();
    assert!(
        matches!(&view, BindingView::ReleaseOrdinary { element_id, focus, gap, .. }
        if element_id.0 == "box" && focus == &Number::from(0_i64) && gap == &Number::from(3_i64))
    );
    assert!(std::ptr::eq(view.binding(), &ordinary));
    assert_eq!(view.get(binding::FIXED_POINT).unwrap(), Field::Missing);
    assert_eq!(view.binding().as_object()["future"], json!({"keep":true}));
    let fixed = binding(json!({"elementId":"box","focus":0,"gap":3,"fixedPoint":[0.5,1]}));
    assert!(
        matches!(fixed.view().unwrap(), BindingView::ReleaseFixed { fixed_point, .. }
        if fixed_point == ["0.5".parse().unwrap(), Number::from(1_i64)])
    );
    for mode in ["inside", "orbit", "skip", "future-mode"] {
        let snapshot =
            binding(json!({"elementId":"box","fixedPoint":[0,1],"mode":mode,"future":false}));
        assert!(
            matches!(snapshot.view().unwrap(), BindingView::Snapshot { mode: parsed, .. } if parsed.as_str() == mode)
        );
    }
    for value in [
        json!({}),
        json!({"elementId":null}),
        json!({"elementId":"box","focus":0}),
        json!({"elementId":"box","focus":0,"gap":3,"fixedPoint":null}),
        json!({"elementId":"box","focus":0,"gap":3,"mode":null}),
        json!({"elementId":"box","focus":0,"gap":3,"fixedPoint":[0,1],"mode":"orbit"}),
        json!({"elementId":"box","fixedPoint":[0,1]}),
        json!({"elementId":"box","fixedPoint":[0,1],"mode":"orbit","focus":null}),
        json!({"elementId":"box","fixedPoint":[0,1],"mode":"orbit","gap":3}),
        json!({"elementId":"box","fixedPoint":[0,1],"mode":null}),
    ] {
        let b = binding(value.clone());
        let view = b.view().unwrap();
        assert!(matches!(view, BindingView::PartialMixed { .. }), "{value}");
        assert!(std::ptr::eq(view.binding(), &b));
        assert_eq!(view.binding().as_object(), value.as_object().unwrap());
    }
    let partial = binding(json!({"focus":null,"fixedPoint":[0,1]}));
    assert!(matches!(
        partial.view().unwrap(),
        BindingView::PartialMixed {
            element_id: Field::Missing,
            focus: Field::Null,
            gap: Field::Missing,
            fixed_point: Field::Value(_),
            mode: Field::Missing,
            ..
        }
    ));
}

#[test]
fn malformed_binding_fields_are_errors_even_in_mixed_records() {
    for (key, bad, path) in [
        ("elementId", json!(123), "/elementId"),
        ("focus", json!("bad"), "/focus"),
        ("gap", json!(false), "/gap"),
        ("fixedPoint", json!([0]), "/fixedPoint"),
        ("fixedPoint", json!([0, "bad"]), "/fixedPoint/1"),
        ("mode", json!({}), "/mode"),
    ] {
        let mut b = binding(
            json!({"elementId":"box","focus":0,"gap":0,"fixedPoint":[0,1],"mode":"orbit","future":true}),
        );
        b.set_raw(key, bad);
        let before = b.clone();
        assert_eq!(b.view().unwrap_err().path, path);
        assert_eq!(b, before);
    }
}
