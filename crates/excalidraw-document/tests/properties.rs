use excalidraw_document::{Document, Number, element};
use proptest::prelude::*;
use serde_json::json;

proptest! {
    #[test]
    fn arbitrary_metadata_survives_local_edits(text in ".{0,200}", integer in any::<u64>(), x in -1e9_f64..1e9_f64) {
        let value = json!({"type":"excalidraw","elements":[{"type":"rectangle","id":"r","x":0,"future":{"text":text,"integer":integer}}],"extension":[null,true,integer]});
        let mut document = Document::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        document.edit_element(0,|e| e.set(element::X, Number::from_f64(x).unwrap())).unwrap();
        let mut expected = value; expected["elements"][0]["x"] = json!(x);
        prop_assert_eq!(document.clone().into_value(),expected);
        prop_assert_eq!(Document::from_slice(&document.to_vec().unwrap()).unwrap(),document);
    }
}
