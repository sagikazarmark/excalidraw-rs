use excalidraw_document::{Document, Field, Number, element};
use serde_json::{Value, json};

#[test]
fn typed_access_does_not_interpret_metadata_as_serdes_private_number_protocol() {
    let mut document = Document::from_slice(br#"{"type":"excalidraw","elements":[{"type":"rectangle","x":{"$serde_json::private::Number":"123"},"customData":{"nested":{"$serde_json::private::Number":"123"}}}]}"#).unwrap();
    let before = document.clone();
    document
        .edit_element(0, |e| {
            assert!(e.get(element::X).is_err());
            let Field::Value(metadata) = e.get(element::CUSTOM_DATA)? else {
                panic!()
            };
            assert!(metadata["nested"].is_object());
            e.set(element::CUSTOM_DATA, metadata)
        })
        .unwrap();
    assert_eq!(document, before);
    for number in ["1.0000000000000001", "1e-400", "9007199254740990.9"] {
        assert!(number.parse::<Number>().unwrap().as_safe_integer().is_err());
    }
    for number in ["1e0", "1.0", "9007199254740991"] {
        assert!(number.parse::<Number>().unwrap().as_safe_integer().is_ok());
    }
}

#[test]
fn editing_one_field_preserves_unknown_data_numbers_and_presence() {
    let input = br#"{"type":"excalidraw","version":2,"elements":[{"type":"arrow","id":"a","x":1.25,"endArrowhead":null,"customData":{"integer":9007199254740993,"huge":1e400},"future":{"nested":true}}],"files":{},"extension":17}"#;
    let mut document = Document::from_slice(input).unwrap();
    let mut arrow = document.elements().unwrap().remove(0);
    assert_eq!(arrow.get(element::END_ARROWHEAD).unwrap(), Field::Null);
    assert_eq!(arrow.get(element::START_ARROWHEAD).unwrap(), Field::Missing);
    arrow
        .set(element::X, Number::from_f64(42.5).unwrap())
        .unwrap();
    document.set_elements(vec![arrow]);
    let mut expected: Value = serde_json::from_slice(input).unwrap();
    expected["elements"][0]["x"] = json!(42.5);
    assert_eq!(
        serde_json::from_slice::<Value>(&document.to_vec().unwrap()).unwrap(),
        expected
    );
}

#[test]
fn duplicate_keys_are_rejected_but_serde_number_marker_is_ordinary_user_data() {
    for input in [
        r#"{"type":"excalidraw","type":"excalidraw"}"#,
        r#"{"type":"excalidraw","extension":{"x":1,"x":2}}"#,
    ] {
        assert!(
            Document::from_slice(input.as_bytes())
                .unwrap_err()
                .message
                .contains("duplicate")
        );
    }
    let input =
        br#"{"type":"excalidraw","extension":{"$serde_json::private::Number":"123","other":true}}"#;
    let document = Document::from_slice(input).unwrap();
    let output = String::from_utf8(document.to_vec().unwrap()).unwrap();
    assert!(output.contains("$serde_json::private::Number"));
    assert_eq!(Document::from_slice(output.as_bytes()).unwrap(), document);
}

#[test]
fn malformed_neighbors_survive_an_atomic_typed_edit() {
    let mut document = Document::from_value(json!({"type":"excalidraw","elements":[null,{"type":"text","text":17},{"type":"future","id":"f"}]})).unwrap();
    assert!(document.elements().is_err());
    document
        .edit_element(1, |element| {
            assert!(element.get(element::TEXT).is_err());
            element.set(element::TEXT, "recovered".to_owned())
        })
        .unwrap();
    assert_eq!(document.as_object()["elements"][0], Value::Null);
    assert_eq!(
        document.as_object()["elements"][2],
        json!({"type":"future","id":"f"})
    );
    let before = document.clone();
    assert!(
        document
            .edit_element(1, |e| {
                e.set(element::TEXT, "discarded".into())?;
                Err(excalidraw_document::Error {
                    path: "".into(),
                    message: "abort".into(),
                })
            })
            .is_err()
    );
    assert_eq!(document, before);
}
