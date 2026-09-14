#![no_main]
use excalidraw_document::{
    ClipboardDocument, Document, ExportMode, IdMap, LibraryDocument, OpaquePolicy, Profile, Purpose,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    if let Ok(document) = Document::from_slice(input) {
        let bytes = document.to_vec().unwrap();
        assert_eq!(Document::from_slice(&bytes).unwrap(), document);
        for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
            let _ = document.migrate(
                profile,
                if profile == Profile::V0_18_1 {
                    Profile::SnapshotAfa3a653
                } else {
                    Profile::V0_18_1
                },
            );
            for purpose in [Purpose::Inspect, Purpose::Author, Purpose::SelfContained] {
                let _ = document.validate(profile, purpose);
            }
            for mode in [ExportMode::Local, ExportMode::Database] {
                if let Ok(export) = document.project_native(profile, mode, "fuzz") {
                    assert_eq!(
                        Document::from_slice(&export.document.to_vec().unwrap()).unwrap(),
                        export.document
                    );
                }
            }
        }
        let _ = document.remap_ids(&IdMap::default(), OpaquePolicy::Preserve);
        let _ = ClipboardDocument::from_document(&document);
        assert_eq!(document.to_vec().unwrap(), bytes);
    } else if let Ok(library) = LibraryDocument::from_slice(input) {
        let bytes = library.to_vec().unwrap();
        assert_eq!(LibraryDocument::from_slice(&bytes).unwrap(), library);
        for profile in [Profile::V0_18_1, Profile::SnapshotAfa3a653] {
            let _ = library.validate(profile, Purpose::Inspect);
            let _ = library.validate(profile, Purpose::Author);
        }
        assert_eq!(library.to_vec().unwrap(), bytes);
    }
    let _ = excalidraw_document::embedded::extract_png(input, 64 * 1024);
    if input.starts_with(b"\x89PNG") {
        let _ = excalidraw_document::embedded::embed_png(input, &Document::new("fuzz"));
    }
    if let Ok(text) = std::str::from_utf8(input) {
        let _ = excalidraw_document::embedded::extract_svg(text, 64 * 1024);
    }
    if let Ok(text) = std::str::from_utf8(input)
        && (text.contains("<svg") || text.contains(":svg"))
    {
        let _ = excalidraw_document::embedded::embed_svg(text, &Document::new("fuzz"));
    }
    let _ = excalidraw_document::plus::SceneContent::from_slice(input);
    let _ = excalidraw_document::plus::PatchSceneContent::from_slice(input);
});
