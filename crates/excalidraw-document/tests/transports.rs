use excalidraw_document::{ClipboardDocument, Document};
use serde_json::json;

#[test]
fn clipboard_is_a_distinct_preserving_envelope() {
    let input=br#"{"type":"excalidraw/clipboard","elements":[{"type":"image","id":"i","fileId":"f","extra":true}],"files":{"f":{"id":"f","dataURL":"raw"}},"future":42}"#;
    let clipboard = ClipboardDocument::from_slice(input).unwrap();
    assert_eq!(
        ClipboardDocument::from_slice(&clipboard.to_vec().unwrap()).unwrap(),
        clipboard
    );
    let scene = clipboard.to_document("test");
    assert_eq!(scene.as_object()["elements"][0]["fileId"], "f");
    assert!(Document::from_slice(input).is_err());
    let converted = ClipboardDocument::from_document(&scene).unwrap();
    assert_eq!(
        converted.as_object()["files"],
        clipboard.as_object()["files"]
    );
    assert_eq!(converted.as_object()["type"], json!("excalidraw/clipboard"));
    assert!(converted.as_object().get("appState").is_none());
}

#[cfg(feature = "embedded")]
mod embedded {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use excalidraw_document::embedded::*;
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    #[test]
    fn legacy_prefixed_and_nested_svg_metadata_have_explicit_behavior() {
        let old = Document::new("old");
        let new = Document::new("new");
        let legacy = format!(
            "<svg><!-- payload-type:application/vnd.excalidraw+json --><!-- payload-start -->{}<!-- payload-end --></svg>",
            STANDARD.encode(old.to_vec().unwrap())
        );
        assert_eq!(extract_svg(&legacy, 4096).unwrap(), old);
        let nested = format!("<svg>{legacy}</svg>");
        assert_eq!(extract_svg(&nested, 4096).unwrap(), old);
        assert!(embed_svg(&nested, &new).is_err());
        let replaced = embed_svg(&legacy, &new).unwrap();
        assert_eq!(extract_svg(&replaced, 4096).unwrap(), new);
        assert_eq!(replaced.matches("payload-start").count(), 1);
        let prefixed = embed_svg(r#"<s:svg xmlns:s="http://www.w3.org/2000/svg"/>"#, &new).unwrap();
        assert_eq!(extract_svg(&prefixed, 4096).unwrap(), new);
        let marker = "<!-- payload-type:application/vnd.excalidraw+json -->";
        assert!(
            embed_svg(
                &format!("<svg><metadata>{marker}<metadata>{marker}</metadata></metadata></svg>"),
                &new
            )
            .is_err()
        );
    }
    #[test]
    fn png_duplicate_headers_and_missing_data_are_rejected() {
        let png = STANDARD.decode(PNG).unwrap();
        let mut duplicated = png[..33].to_vec();
        duplicated.extend(&png[8..33]);
        duplicated.extend(&png[33..]);
        assert!(embed_png(&duplicated, &Document::new("test")).is_err());
        let mut no_data = png[..33].to_vec();
        no_data.extend(&png[png.len() - 12..]);
        assert!(embed_png(&no_data, &Document::new("test")).is_err());
    }
    #[test]
    fn png_and_svg_carry_exact_scene_data_and_replace_previous_payloads() {
        let scene = Document::from_slice(
            br#"{"type":"excalidraw","elements":[],"unknown":9007199254740993}"#,
        )
        .unwrap();
        let png = STANDARD.decode(PNG).unwrap();
        let png = embed_png(&png, &scene).unwrap();
        assert_eq!(extract_png(&png, 1024 * 1024).unwrap(), scene);
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><metadata>unrelated</metadata><text>β</text></svg>"#;
        let svg = embed_svg(svg, &scene).unwrap();
        assert!(svg.contains("<text>β</text>"));
        assert_eq!(extract_svg(&svg, 1024 * 1024).unwrap(), scene);
        let other = Document::new("replacement");
        assert_eq!(
            extract_png(&embed_png(&png, &other).unwrap(), 1024 * 1024).unwrap(),
            other
        );
        assert_eq!(
            extract_svg(&embed_svg(&svg, &other).unwrap(), 1024 * 1024).unwrap(),
            other
        );
        assert!(extract_png(&png, 1).is_err());
        assert!(extract_svg(&svg, 1).is_err());
    }
    #[test]
    fn corrupt_or_ambiguous_containers_fail_without_panics() {
        let mut png = STANDARD.decode(PNG).unwrap();
        png[20] ^= 1;
        assert!(extract_png(&png, 1024).is_err());
        assert!(embed_png(&png, &Document::new("test")).is_err());
        for svg in ["<svg", "<html/>", "<svg/>"] {
            assert!(extract_svg(svg, 1024).is_err());
        }
        let doc = Document::new("test");
        let svg = embed_svg("<svg/>", &doc).unwrap();
        assert_eq!(extract_svg(&svg, 1024).unwrap(), doc);
    }
}
