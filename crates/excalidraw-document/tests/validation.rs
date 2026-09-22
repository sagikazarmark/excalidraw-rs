use excalidraw_document::{Document, Profile, Purpose, Severity};
use serde_json::json;

#[test]
fn schema_versions_use_exact_integer_semantics() {
    for version in ["2.0000000000000001", "1.99999999999999999"] {
        let document=Document::from_slice(format!(r#"{{"type":"excalidraw","version":{version},"source":"test","elements":[],"appState":{{}},"files":{{}}}}"#).as_bytes()).unwrap();
        assert!(
            !document
                .validate(Profile::V0_18_1, Purpose::Author)
                .is_valid()
        );
        assert!(
            excalidraw_document::LibraryDocument::from_slice(
                format!(r#"{{"type":"excalidrawlib","version":{version},"libraryItems":[]}}"#)
                    .as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn independent_collections_and_nested_numeric_fields_are_always_validated() {
    let document = Document::from_value(
        json!({"type":"excalidraw","appState":{"gridSize":"bad"},"files":{"x":17}}),
    )
    .unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/appState/gridSize")
    );
    assert!(report.diagnostics.iter().any(|d| d.path == "/files/x"));
    let document = Document::from_slice(br#"{"type":"excalidraw","elements":[{"type":"line","points":[[0,0],[1e400,1]]},{"type":"iframe","customData":{"generationData":7}}],"files":{"f":{"id":"f","mimeType":"image/png","dataURL":"x","created":1e400,"version":null}}}"#).unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    for path in [
        "/elements/0/points/1/0",
        "/elements/1/customData/generationData",
        "/files/f/created",
        "/files/f/version",
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.path == path),
            "{path}: {report:?}"
        );
    }
}

#[test]
fn graph_diagnostics_distinguish_missing_wrong_kind_and_reciprocal_links() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[
        {"type":"rectangle","id":"box","frameId":"text"},
        {"type":"text","id":"text","containerId":"box"},
        {"type":"arrow","id":"arrow","startBinding":{"elementId":"gone","fixedPoint":[0,0],"mode":"inside"}},
        {"type":"image","id":"image","fileId":"missing"}
    ],"appState":{},"files":{}})).unwrap();
    let before = document.clone();
    let report = document.validate(Profile::SnapshotAfa3a653, Purpose::Inspect);
    for (path, code) in [
        ("/elements/0/frameId", "reference-kind"),
        ("/elements/1/containerId", "reciprocal-binding"),
        ("/elements/2/startBinding/elementId", "missing-reference"),
        ("/elements/3/fileId", "missing-file"),
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == path && d.code == code),
            "{path}: {report:?}"
        );
    }
    assert_eq!(document, before);
}

#[test]
fn malformed_nested_fields_and_profile_mismatches_are_reported() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[
        {"type":"stickynote","id":"sticky","baseHeight":120},
        {"type":"arrow","id":"a","startBinding":{"elementId":"sticky","fixedPoint":[0,1],"mode":"inside"},"endArrowhead":"cardinality_zero_or_many"},
        {"type":"image","id":"i","crop":{"naturalWidth":"bad"}}
    ],"appState":{"lockedMultiSelections":{"g":false}},"files":{}})).unwrap();
    let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/0/type" && d.code == "profile")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/2/crop/naturalWidth" && d.code == "field-type")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "/appState/lockedMultiSelections/g" && d.code == "literal")
    );
}

#[test]
fn file_records_contradicting_their_own_mime_type_are_always_errors() {
    for data_url in ["x", "data:image/jpeg;base64,AQ==", "data:image/png,"] {
        let document = Document::from_value(json!({"type":"excalidraw","elements":[],"appState":{},
            "files":{"f":{"id":"f","mimeType":"image/png","dataURL":data_url,"created":1,"version":1}}}))
        .unwrap();
        for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
            let report = document.validate(Profile::V0_18_1, purpose);
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.path == "/files/f/dataURL"
                        && d.code == "data-url"
                        && d.severity == Severity::Error),
                "{data_url} {purpose:?}: {report:?}"
            );
            assert!(!report.is_valid(), "{data_url} {purpose:?}");
        }
    }
}

#[test]
fn an_empty_mime_type_is_not_carried_by_a_data_url_that_omits_its_type() {
    // `data:,AA` omits its media type; it does not declare an empty one. Were
    // the two empty tokens to agree, `Author` would pass this record with only
    // the unsupported-type warning, though `BinaryFile::new` refuses it.
    let document = Document::from_value(json!({"type":"excalidraw","elements":[],"appState":{},
        "files":{"f":{"id":"f","mimeType":"","dataURL":"data:,AA","created":1,"version":1}}}))
    .unwrap();
    for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(Profile::V0_18_1, purpose);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/files/f/dataURL"
                    && d.code == "data-url"
                    && d.severity == Severity::Error),
            "{purpose:?}: {report:?}"
        );
        assert!(!report.is_valid(), "{purpose:?}");
    }
}

#[test]
fn unknown_file_mime_types_are_condemned_only_when_self_contained() {
    let document = Document::from_value(json!({"type":"excalidraw","version":2,"source":"test","elements":[],"appState":{},
        "files":{"f":{"id":"f","mimeType":"image/future","dataURL":"data:image/future,x","created":1,"version":1}}}))
    .unwrap();
    for (purpose, severity) in [
        (Purpose::Inspect, Severity::Warning),
        (Purpose::Author, Severity::Warning),
        (Purpose::SelfContained, Severity::Error),
    ] {
        let report = document.validate(Profile::V0_18_1, purpose);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/files/f/mimeType"
                    && d.code == "mime-type"
                    && d.severity == severity),
            "{purpose:?}: {report:?}"
        );
        assert_eq!(
            report.is_valid(),
            severity == Severity::Warning,
            "{purpose:?}: {report:?}"
        );
    }
}

#[test]
fn data_url_mime_token_agreement_is_ascii_case_insensitive() {
    // RFC 2045 §5.1: media type and subtype names are case-insensitive, so a
    // dataURL whose header spells the type differently than the declared
    // mimeType is not a contradiction and must not be condemned.
    for data_url in [
        "data:IMAGE/PNG;base64,AA==",
        "data:Image/Png,x",
        "data:image/svg+xml;charset=utf-8,<svg/>",
    ] {
        let mime_type = if data_url.starts_with("data:image/svg") {
            "image/svg+xml"
        } else {
            "image/png"
        };
        let document = Document::from_value(json!({"type":"excalidraw","elements":[],"appState":{},
            "files":{"f":{"id":"f","mimeType":mime_type,"dataURL":data_url,"created":1,"version":1}}}))
        .unwrap();
        for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
            let report = document.validate(Profile::V0_18_1, purpose);
            assert!(
                !report
                    .diagnostics
                    .iter()
                    .any(|d| d.path == "/files/f/dataURL" && d.code == "data-url"),
                "{data_url} {purpose:?}: {report:?}"
            );
        }
    }
}

#[test]
fn data_url_mime_token_mismatch_is_still_rejected_regardless_of_case() {
    let document = Document::from_value(json!({"type":"excalidraw","elements":[],"appState":{},
        "files":{"f":{"id":"f","mimeType":"image/png","dataURL":"data:image/jpeg,x","created":1,"version":1}}}))
    .unwrap();
    for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(Profile::V0_18_1, purpose);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/files/f/dataURL"
                    && d.code == "data-url"
                    && d.severity == Severity::Error),
            "{purpose:?}: {report:?}"
        );
    }
}

#[test]
fn unknown_mime_type_membership_stays_case_sensitive_even_when_data_url_agrees() {
    // Deliberate: `MimeType::KNOWN` is not case-normalised, so an
    // uppercase-but-otherwise-recognised token (e.g. "IMAGE/PNG") is still
    // reported as an unsupported MIME type -- consistent with `from_wire`
    // used elsewhere, which would also produce `Unknown("IMAGE/PNG")`. This
    // is independent from, not contradicted by, the dataURL agreement check
    // below reporting no mismatch for the very same record.
    let document = Document::from_value(json!({"type":"excalidraw","version":2,"source":"test","elements":[],"appState":{},
        "files":{"f":{"id":"f","mimeType":"IMAGE/PNG","dataURL":"data:image/png,x","created":1,"version":1}}}))
    .unwrap();
    for (purpose, severity) in [
        (Purpose::Inspect, Severity::Warning),
        (Purpose::Author, Severity::Warning),
        (Purpose::SelfContained, Severity::Error),
    ] {
        let report = document.validate(Profile::V0_18_1, purpose);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == "/files/f/mimeType"
                    && d.code == "mime-type"
                    && d.severity == severity),
            "{purpose:?}: {report:?}"
        );
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.path == "/files/f/dataURL" && d.code == "data-url"),
            "{purpose:?}: {report:?}"
        );
    }
}

#[test]
fn a_file_record_identity_that_authoring_rejects_is_never_reported_valid() {
    // `required` asks only whether `id` is present and non-null, so an empty
    // string passed it while `BinaryFile`'s own construction refused the same
    // record. The two routes have to agree about what a usable resource is.
    let document = Document::from_value(json!({
        "type": "excalidraw",
        "version": 2,
        "source": "empty-file-identity",
        "elements": [],
        "appState": {},
        "files": {"": {
            "id": "",
            "mimeType": "image/png",
            "dataURL": "data:image/png;base64,AAA",
            "created": 1,
        }},
    }))
    .unwrap();
    let before = document.clone();

    for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
        let report = document.validate(Profile::V0_18_1, purpose);
        assert!(
            report.diagnostics.iter().any(|d| d.path == "/files//id"
                && d.code == "empty-id"
                && d.severity == Severity::Error),
            "{purpose:?} accepted an empty resource identity: {report:?}"
        );
        assert!(!report.is_valid(), "{purpose:?}: {report:?}");
    }
    assert_eq!(document, before, "validation must not mutate the document");
}

#[test]
fn a_dimension_that_underflows_to_negative_zero_is_outside_its_range() {
    // `-1e-400` underflows to `-0.0`, which is not less than the zero floor,
    // so a range check consulting f64 alone admitted a negative number that
    // `Number::check_dimension` refuses on the authoring side.
    for field in ["width", "height", "strokeWidth", "roughness"] {
        let document = Document::from_slice(
            format!(
                r#"{{"type":"excalidraw","version":2,"source":"t","appState":{{}},"files":{{}},
                     "elements":[{{"type":"rectangle","id":"r","{field}":-1e-400}}]}}"#
            )
            .as_bytes(),
        )
        .unwrap();
        let report = document.validate(Profile::V0_18_1, Purpose::Inspect);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.path == format!("/elements/0/{field}") && d.code == "range"),
            "{field}: -1e-400 was accepted: {report:?}"
        );
    }

    // Exact `-0` stays admissible: it is zero, spelled with a sign. `wire.rs`
    // records that decision and this pins it against an over-eager fix.
    let document = Document::from_slice(
        br#"{"type":"excalidraw","version":2,"source":"t","appState":{},"files":{},
             "elements":[{"type":"rectangle","id":"r","width":-0}]}"#,
    )
    .unwrap();
    assert!(
        !document
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .diagnostics
            .iter()
            .any(|d| d.path == "/elements/0/width" && d.code == "range"),
        "exact -0 must stay permitted"
    );
}
