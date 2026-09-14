use excalidraw_document::{
    Document, Element, ElementId, ElementKind, Number, Profile, Purpose, Severity,
};
use serde_json::{Value, json};

fn scene(elements: Value) -> Document {
    Document::from_value(json!({"type":"excalidraw","version":2,"source":"test","elements":elements,"appState":{},"files":{}})).unwrap()
}

#[test]
fn editor_crop_roundoff_and_inactive_elbow_metadata_are_tolerated() {
    // cropElement expands to the image edge via x += previousWidth - width.
    let x = 0.1_f64 + (999.9_f64 - 1000.0_f64);
    assert!(x < 0.);
    let cropped = scene(
        json!([{"type":"image","crop":{"x":x,"y":0,"width":1000,"height":500,"naturalWidth":1000,"naturalHeight":1000}}]),
    );
    assert!(
        cropped
            .validate(Profile::SnapshotAfa3a653, Purpose::Inspect)
            .is_valid()
    );
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        let mut arrow = Element::new(
            ElementKind::Arrow,
            profile,
            ElementId::from("a"),
            Number::from(1_u64),
        )
        .unwrap();
        // Upstream arrow-type conversion leaves the old routing properties behind.
        arrow.set_raw(
            "fixedSegments",
            json!([{ "index":2,"start":[10,0],"end":[10,10] }]),
        );
        arrow.set_raw("startIsSpecial", json!(false));
        arrow.set_raw("endIsSpecial", Value::Null);
        let mut document = Document::new("converted-arrow");
        document.set_elements(vec![arrow]);
        let report = document.validate(profile, Purpose::Author);
        assert!(report.is_valid(), "{report:?}");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "inactive-routing" && d.severity == Severity::Warning)
        );
    }
}

#[test]
fn author_validation_rejects_semantic_defects_in_otherwise_complete_records() {
    let cases = [
        (
            ElementKind::Rectangle,
            "text",
            json!("not text"),
            "field-kind",
        ),
        (ElementKind::Text, "fontSize", json!(0), "range"),
        (ElementKind::Text, "fontFamily", json!(4), "font-family"),
        (
            ElementKind::Rectangle,
            "roundness",
            json!({"type":7}),
            "roundness-type",
        ),
        (ElementKind::Line, "polygon", json!(true), "polygon-closure"),
        (ElementKind::Arrow, "endArrowhead", json!("dot"), "profile"),
        (
            ElementKind::Image,
            "crop",
            json!({"x":5,"y":0,"width":10,"height":10,"naturalWidth":10,"naturalHeight":10}),
            "crop-bounds",
        ),
    ];
    for (kind, key, value, code) in cases {
        let mut element = Element::new(
            kind,
            Profile::SnapshotAfa3a653,
            ElementId::from("one"),
            Number::from(1_u64),
        )
        .unwrap();
        let mut document = Document::new("test");
        document.set_elements(vec![element.clone()]);
        assert!(
            document
                .validate(Profile::SnapshotAfa3a653, Purpose::Author)
                .is_valid()
        );
        element.set_raw(key, value);
        document.set_elements(vec![element]);
        let before = document.to_vec().unwrap();
        let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Author);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == code && d.severity == Severity::Error),
            "{key}: {report:?}"
        );
        assert!(
            document
                .validated(Profile::SnapshotAfa3a653, Purpose::Author)
                .is_err()
        );
        assert_eq!(document.to_vec().unwrap(), before);
    }
}

#[test]
fn partial_and_malformed_geometry_is_reported_without_panics_or_invented_points() {
    let document = scene(json!([
        {"type":"arrow","elbowed":true,"points":[null,[1,"bad"]],"fixedSegments":[null,{"index":9007199254740991_i64,"start":null,"end":[1,2]}]},
        {"type":"arrow","elbowed":true,"fixedSegments":[{"index":9007199254740991_i64,"start":[0,0],"end":[0,1]}]},
        {"type":"line","points":[],"polygon":true},
        {"type":"image","crop":{"x":0,"width":1}},
        {"type":"freedraw","pressures":[null,"bad"],"simulatePressure":false}
    ]));
    let before = document.clone();
    for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(Profile::SnapshotAfa3a653, purpose);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/elements/0/points/0" && d.code == "field-type")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/elements/2/polygon" && d.code == "polygon-closure")
        );
    }
    assert_eq!(document, before);
}

#[test]
fn nullable_fields_are_profile_aware_and_extensions_do_not_become_geometry() {
    let document=Document::from_slice(br#"{"type":"excalidraw","elements":[{"type":"rectangle","futureNumber":1e400,"customData":{"number":1e400},"roundness":null},{"type":"arrow","startBinding":null,"endBinding":null,"startArrowhead":null,"endArrowhead":null,"elbowed":true,"fixedSegments":null,"startIsSpecial":null,"endIsSpecial":null}],"appState":{"gridSize":null},"files":{}}"#).unwrap();
    assert!(
        document
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .is_valid()
    );
    let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Author);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/appState/gridSize" && d.code == "null-field")
    );
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.path.contains("futureNumber") || d.path.contains("customData"))
    );
    let invalid = scene(json!([
        {"type":"arrow","startBinding":{"elementId":"x","fixedPoint":null,"mode":null}},
        {"type":"image","scale":null,"crop":{"x":null,"naturalWidth":null}},
        {"type":"text","fontSize":null,"baseFontSize":null,"labelPosition":null}
    ]));
    let report = invalid.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for path in [
        "/elements/0/startBinding/fixedPoint",
        "/elements/0/startBinding/mode",
        "/elements/1/scale",
        "/elements/1/crop/x",
        "/elements/1/crop/naturalWidth",
        "/elements/2/fontSize",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == "null-field"),
            "{path}: {report:?}"
        );
    }
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/2/baseFontSize" || d.path == "/elements/2/labelPosition")
    );
}

#[test]
fn snapshot_constant_pressure_semantics_do_not_leak_into_release_validation() {
    let document = scene(
        json!([{"type":"freedraw","points":[[0,0],[10,10]],"pressures":[],"simulatePressure":false,"strokeOptions":{"variability":"constant","streamline":0.5}}]),
    );
    assert!(
        document
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .diagnostics
            .iter()
            .any(|d| d.code == "pressure-count")
    );
    assert!(
        !document
            .validate(Profile::SnapshotAfa3a653, Purpose::Inspect)
            .diagnostics
            .iter()
            .any(|d| d.code == "pressure-count")
    );
}

#[test]
fn streamline_domain_is_an_inspection_warning_and_authoring_error() {
    let document = scene(
        json!([{"type":"freedraw","strokeOptions":{"variability":"variable","streamline":2}}]),
    );
    for (purpose, severity) in [
        (Purpose::Inspect, Severity::Warning),
        (Purpose::Author, Severity::Error),
    ] {
        assert!(
            document
                .validate(Profile::SnapshotAfa3a653, purpose)
                .diagnostics
                .iter()
                .any(|d| d.path == "/elements/0/strokeOptions/streamline"
                    && d.code == "range"
                    && d.severity == severity)
        );
    }
}

#[test]
fn freehand_pressure_and_polygon_checks_preserve_supported_degenerate_inputs() {
    let bad = scene(json!([
        {"type":"freedraw","points":[[0,0],[1,2]],"pressures":[-0.1],"simulatePressure":false,"strokeOptions":{"variability":"variable","streamline":2}},
        {"type":"line","points":[[0,0],[10,0],[10,10],[1,1]],"polygon":true}
    ]));
    let report = bad.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for (path, code) in [
        ("/elements/0/pressures", "pressure-count"),
        ("/elements/0/pressures/0", "range"),
        ("/elements/0/strokeOptions/streamline", "range"),
        ("/elements/1/polygon", "polygon-closure"),
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == code),
            "{path}: {report:?}"
        );
    }
    let good = scene(json!([
        {"type":"freedraw","points":[[0,0],[0,0]],"pressures":[],"simulatePressure":true},
        {"type":"freedraw","points":[[0,0],[1,1]],"pressures":[],"simulatePressure":false,"strokeOptions":{"variability":"constant","streamline":0}},
        {"type":"freedraw","points":[],"pressures":[],"simulatePressure":true},
        {"type":"line","points":[[0,0],[10,0]],"width":10,"height":0},
        {"type":"line","points":[[0,0],[10,0],[10,10],[0.00001,0]],"polygon":true}
    ]));
    assert!(
        good.validate(Profile::SnapshotAfa3a653, Purpose::Inspect)
            .diagnostics
            .is_empty()
    );
}

#[test]
fn elbow_segments_require_valid_indices_and_agree_with_the_path() {
    let bad = scene(
        json!([{"type":"arrow","elbowed":true,"points":[[0,0],[10,0],[10,20],[30,20]],"fixedSegments":[
            {"index":0,"start":[0,0],"end":[10,0]},
            {"index":1.0000000000000002,"start":[0,0],"end":[10,0]},
            {"index":2,"start":[11,0],"end":[10,20]},
            {"index":2,"start":[10,0],"end":[10,20]},
            {"index":8,"start":[0,0],"end":[1,1]}
        ]}]),
    );
    let report = bad.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for (path, code) in [
        ("/elements/0/fixedSegments/0/index", "segment-index"),
        ("/elements/0/fixedSegments/1/index", "segment-index"),
        ("/elements/0/fixedSegments/2/start", "segment-points"),
        ("/elements/0/fixedSegments/3/index", "duplicate-segment"),
        ("/elements/0/fixedSegments/4/index", "segment-index"),
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == code),
            "{path}: {report:?}"
        );
    }
    let good = scene(
        json!([{"type":"arrow","elbowed":true,"points":[[0,0],[10,0],[10,20],[30,20]],"fixedSegments":[{"index":2,"start":[10.0,0],"end":[10,20.0]}],"startIsSpecial":false,"endIsSpecial":null}]),
    );
    for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
        assert!(
            good.validate(profile, Purpose::Inspect)
                .diagnostics
                .is_empty()
        );
    }
}

#[test]
fn cropped_image_geometry_is_checked_without_imposing_ui_size_limits() {
    let bad = scene(
        json!([{"type":"image","scale":[0,1],"crop":{"x":-1,"y":0,"width":20,"height":0,"naturalWidth":10,"naturalHeight":0}},
        {"type":"image","crop":{"x":1,"y":0,"width":20,"height":1,"naturalWidth":10,"naturalHeight":1}}]),
    );
    let report = bad.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for path in [
        "/elements/0/scale/0",
        "/elements/0/crop/x",
        "/elements/0/crop/height",
        "/elements/0/crop/naturalHeight",
        "/elements/1/crop/width",
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.path == path),
            "{path}: {report:?}"
        );
    }
    let good = scene(
        json!([{"type":"image","scale":[-1,1],"crop":{"x":0.1,"y":0,"width":0.2,"height":0.5,"naturalWidth":0.3,"naturalHeight":0.5,"extension":true}}]),
    );
    assert!(
        good.validate(Profile::SnapshotAfa3a653, Purpose::Inspect)
            .diagnostics
            .is_empty()
    );
}

#[test]
fn known_variant_fields_are_checked_for_applicability_without_mutation() {
    let document = scene(json!([
        {"type":"rectangle","id":"box","text":"wrong kind","polygon":true},
        {"type":"arrow","id":"a","elbowed":false,"fixedSegments":[]},
        {"type":"text","id":"t","fileId":"not-an-image"},
        {"type":"future","id":"future","text":"unknown semantics","polygon":true}
    ]));
    let before = document.to_vec().unwrap();
    for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(Profile::SnapshotAfa3a653, purpose);
        for path in [
            "/elements/0/text",
            "/elements/0/polygon",
            "/elements/2/fileId",
        ] {
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.path == path && d.code == "field-kind"),
                "{path}: {report:?}"
            );
        }
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.path.starts_with("/elements/3/") && d.code == "field-kind")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/elements/1/fixedSegments"
                    && d.code == "inactive-routing"
                    && d.severity == Severity::Warning)
        );
    }
    assert_eq!(document.to_vec().unwrap(), before);
}

#[test]
fn binding_targets_follow_the_selected_profile_and_reverse_reference_kind() {
    let document = scene(json!([
        {"type":"stickynote","id":"sticky","boundElements":[{"id":"a","type":"arrow"}]},
        {"type":"arrow","id":"a","startBinding":{"elementId":"sticky","fixedPoint":[-2,3],"mode":"inside"}}
    ]));
    let release = document.validate(Profile::V0_18_1, Purpose::Inspect);
    assert!(
        release
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/1/startBinding/elementId" && d.code == "reference-kind")
    );
    let current = document.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    assert!(
        !current
            .diagnostics
            .iter()
            .any(|d| d.path.starts_with("/elements/1/startBinding"))
    );
    let invalid = scene(json!([
        {"type":"rectangle","id":"box","boundElements":[{"id":"other","type":"rectangle"}]},
        {"type":"rectangle","id":"other","startBinding":{"elementId":"box"}}
    ]));
    let report = invalid.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/0/boundElements/0/type" && d.code == "enum")
    );
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.code == "reciprocal-binding")
    );
}

#[test]
fn text_roundness_and_legacy_profile_values_remain_preserved_but_are_diagnosed() {
    let document = scene(json!([
        {"type":"text","id":"t","fontSize":0,"fontFamily":4,"lineHeight":-1,"baseFontSize":0},
        {"type":"rectangle","id":"r","roundness":{"type":4,"value":-1}},
        {"type":"arrow","id":"a","endArrowhead":"crowfoot_many"},
        {"type":"freedraw","id":"f","lastCommittedPoint":null}
    ]));
    let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for (path, code) in [
        ("/elements/0/fontSize", "range"),
        ("/elements/0/fontFamily", "font-family"),
        ("/elements/0/lineHeight", "range"),
        ("/elements/0/baseFontSize", "range"),
        ("/elements/1/roundness/type", "roundness-type"),
        ("/elements/1/roundness/value", "range"),
        ("/elements/2/endArrowhead", "profile"),
        ("/elements/3/lastCommittedPoint", "profile"),
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == code),
            "{path}: {report:?}"
        );
    }
    assert_eq!(document.as_object()["elements"][0]["fontFamily"], 4);
    let supported = scene(
        json!([{"type":"text","fontFamily":10,"fontSize":0.5,"lineHeight":1.25},{"type":"rectangle","roundness":{"type":1}}]),
    );
    assert!(
        supported
            .validate(Profile::SnapshotAfa3a653, Purpose::Inspect)
            .is_valid()
    );
    assert!(
        supported
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .diagnostics
            .iter()
            .any(|d| d.code == "font-family" && d.severity == Severity::Warning)
    );
}
