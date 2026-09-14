use excalidraw_document::{
    Element, ElementId, ElementKind, LibraryDocument, LibraryItem, LibraryStatus, Number, Profile,
    Purpose, library_item,
};
use serde_json::json;

#[test]
fn library_items_have_independent_identity_scopes_and_preserve_metadata() {
    let element = Element::new(
        ElementKind::Rectangle,
        Profile::V0_18_1,
        ElementId::from("shared"),
        Number::from(1_u64),
    )
    .unwrap();
    let mut item = LibraryItem::default();
    item.set(library_item::ID, "one".to_owned()).unwrap();
    item.set(library_item::STATUS, LibraryStatus::Unpublished)
        .unwrap();
    item.set(library_item::CREATED, Number::from(0_u64))
        .unwrap();
    item.set(library_item::ELEMENTS, vec![element]).unwrap();
    item.set_raw("future", json!({"retained":true}));
    let mut second = item.clone();
    second.set(library_item::ID, "two".to_owned()).unwrap();
    let library = LibraryDocument::new("test", vec![item, second]);
    assert!(
        library
            .validate(Profile::V0_18_1, Purpose::Inspect)
            .is_valid()
    );
    let bytes = library.to_vec().unwrap();
    assert_eq!(LibraryDocument::from_slice(&bytes).unwrap(), library);
    assert_eq!(library.as_object()["libraryItems"][0]["created"], json!(0));
}

#[test]
fn legacy_library_is_not_implicitly_migrated() {
    let input = br#"{"type":"excalidrawlib","version":1,"library":[[{"type":"rectangle","id":"x"}]],"extension":true}"#;
    let library = LibraryDocument::from_slice(input).unwrap();
    assert!(library.items().is_err());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&library.to_vec().unwrap()).unwrap(),
        serde_json::from_slice::<serde_json::Value>(input).unwrap()
    );
}
