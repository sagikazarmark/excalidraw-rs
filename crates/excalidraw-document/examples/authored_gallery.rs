//! Independent browser acceptance inputs, authored entirely through public Rust APIs.
use excalidraw_document::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const SOURCE: &str = "excalidraw-document/authored-gallery";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn point(x: i64, y: i64) -> Point {
    [x.into(), y.into()]
}

fn metadata(profile: Profile, id: &str) -> Authoring {
    Authoring {
        id: id.into(),
        updated: 1_i64.into(),
        created: if profile == Profile::V0_18_1 {
            Field::Missing
        } else {
            Field::Null
        },
        seed: 17_i64.into(),
        version: 1_i64.into(),
        version_nonce: 0_i64.into(),
    }
}

fn shape(profile: Profile, kind: ElementKind, id: &str, bounds: [i64; 4]) -> Result<Element> {
    let mut e = Element::authored(kind, profile, metadata(profile, id))?;
    for (key, value) in [element::X, element::Y, element::WIDTH, element::HEIGHT]
        .into_iter()
        .zip(bounds)
    {
        e.set(key, value.into())?;
    }
    e.set(element::ROUGHNESS, 0_i64.into())?;
    Ok(e)
}

fn text(profile: Profile, id: &str, x: i64, y: i64, content: &str) -> Result<Element> {
    let mut e = shape(profile, ElementKind::Text, id, [x, y, 1, 25])?;
    e.set(element::FONT_FAMILY, KnownFont::Cascadia.to_number())?;
    // Caller-supplied monospace metrics. Acceptance requires exact geometry on
    // initial load/mount and native save/reopen, including after native editing.
    let width = content
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1) as i64
        * 12;
    e.set_text_content(TextContent::plain(
        content,
        width.into(),
        (content.lines().count() as i64 * 25).into(),
    ))?;
    Ok(e)
}

fn pixel() -> Result<BinaryFile> {
    Ok(BinaryFile::new("gallery-pixel".into(), MimeType::Png,
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=".into(), 1_i64.into())?)
}

fn scene(profile: Profile, elements: Vec<Element>) -> Result<Document> {
    let mut doc = Document::new(SOURCE);
    doc.author(profile)?.insert(elements)?;
    Ok(doc)
}

fn native(profile: Profile, doc: &Document) -> Result<Value> {
    let checked = doc
        .validated(profile, Purpose::SelfContained)
        .map_err(|report| format!("gallery validation failed: {report:?}"))?;
    Ok(checked
        .project_native(ExportMode::Local, SOURCE)?
        .document
        .into_value())
}

fn mapping(prefix: &str) -> IdMap {
    let mut map = IdMap::default();
    for id in ["frame", "box", "label", "asset"] {
        map.elements
            .insert(id.into(), ElementId(format!("{prefix}-{id}")));
    }
    map.groups
        .insert("cards".into(), GroupId(format!("{prefix}-cards")));
    map.files
        .insert("gallery-pixel".into(), FileId(format!("{prefix}-pixel")));
    map
}

fn main() -> Result<()> {
    let profile = match std::env::args().nth(1).as_deref() {
        Some("release") => Profile::V0_18_1,
        Some("snapshot") => Profile::SnapshotAfa3a653,
        _ => return Err("usage: authored_gallery release|snapshot".into()),
    };
    let mut kinds = vec![
        ElementKind::Rectangle,
        ElementKind::Diamond,
        ElementKind::Ellipse,
        ElementKind::Text,
        ElementKind::Line,
        ElementKind::Arrow,
        ElementKind::Freedraw,
        ElementKind::Image,
        ElementKind::Frame,
        ElementKind::Magicframe,
        ElementKind::Iframe,
        ElementKind::Embeddable,
    ];
    if profile == Profile::SnapshotAfa3a653 {
        kinds.push(ElementKind::Stickynote);
    }
    let mut gallery = Document::new(SOURCE);
    for (i, kind) in kinds.iter().enumerate() {
        let id = kind.as_str();
        let x = 100 + (i as i64 % 4) * 220;
        let y = 100 + (i as i64 / 4) * 160;
        let mut e = shape(profile, kind.clone(), id, [x, y, 120, 80])?;
        match kind {
            ElementKind::Text => e = text(profile, id, x, y, "Rust\ngallery β")?,
            ElementKind::Line | ElementKind::Arrow | ElementKind::Freedraw => {
                e.set_path(vec![point(0, 0), point(60, 80), point(120, 0)])?;
                if *kind == ElementKind::Freedraw {
                    e.set(
                        element::PRESSURES,
                        vec![
                            Number::from_f64(0.2)?,
                            Number::from_f64(0.8)?,
                            Number::from_f64(0.4)?,
                        ],
                    )?;
                    e.set(element::SIMULATE_PRESSURE, false)?;
                }
            }
            ElementKind::Frame | ElementKind::Magicframe => {
                e.set(element::NAME, format!("Rust {id}"))?
            }
            ElementKind::Iframe => {
                let mut generation = GenerationData::default();
                generation.set(generation_data::STATUS, GenerationStatus::Done)?;
                generation.set(
                    generation_data::HTML,
                    "<p>Rust-authored inert data</p>".into(),
                )?;
                e.set_generation_data(Field::Value(generation))?;
            }
            ElementKind::Embeddable => {
                e.set(element::LINK, "https://example.test/rust-gallery".into())?
            }
            ElementKind::Stickynote => {
                e = Element::sticky_note(
                    profile,
                    metadata(profile, id),
                    120_i64.into(),
                    80_i64.into(),
                )?;
                e.set(element::X, x.into())?;
                e.set(element::Y, y.into())?;
            }
            _ => {}
        }
        assert_eq!(e.view()?.get(element::TYPE)?, Field::Value(kind.clone()));
        if *kind == ElementKind::Image {
            gallery.author(profile)?.insert_image(e, pixel()?)?;
        } else {
            gallery.author(profile)?.insert(vec![e])?;
        }
    }
    // The separate interactive gallery is also selected and projected in Rust.
    let mut interactive = gallery.clone();
    interactive.set_elements(
        gallery
            .elements()?
            .into_iter()
            .filter(|e| {
                !matches!(
                    e.view(),
                    Ok(ElementView::Iframe(_)
                        | ElementView::Embeddable(_)
                        | ElementView::Magicframe(_))
                )
            })
            .collect(),
    );

    let mut image = Document::new(SOURCE);
    image.author(profile)?.register_file(pixel()?)?;
    image.author(profile)?.insert_image(
        shape(
            profile,
            ElementKind::Image,
            "image-subject",
            [350, 250, 160, 120],
        )?,
        pixel()?,
    )?;
    assert_eq!(
        image.files()?.len(),
        1,
        "identical resource registration must deduplicate"
    );
    let mut free = shape(
        profile,
        ElementKind::Freedraw,
        "free-subject",
        [350, 250, 160, 120],
    )?;
    free.set_path(vec![point(0, 0), point(80, 120), point(160, 0)])?;
    free.set(
        element::PRESSURES,
        vec![
            Number::from_f64(0.2)?,
            Number::from_f64(0.8)?,
            Number::from_f64(0.4)?,
        ],
    )?;
    free.set(element::SIMULATE_PRESSURE, false)?;
    let freehand = scene(profile, vec![free])?;
    let multiline = scene(
        profile,
        vec![text(
            profile,
            "multiline",
            350,
            250,
            "Rust first line\nSecond line β\nThird line",
        )?],
    )?;

    let mut elbow = Element::elbow_arrow(
        profile,
        metadata(profile, "elbow"),
        vec![point(0, 0), point(100, 0), point(100, 80), point(199, 80)],
    )?;
    elbow.set(element::X, 500_i64.into())?;
    elbow.set(element::Y, 360_i64.into())?;
    elbow.set(element::START_IS_SPECIAL, false)?;
    elbow.set(element::END_IS_SPECIAL, false)?;
    let mut segment = FixedSegment::default();
    segment.set(fixed_segment::START, point(100, 0))?;
    segment.set(fixed_segment::END, point(100, 80))?;
    segment.set(fixed_segment::INDEX, 2_i64.into())?;
    elbow.set(element::FIXED_SEGMENTS, vec![segment])?;
    let mut routed = scene(
        profile,
        vec![
            shape(profile, ElementKind::Ellipse, "target", [700, 400, 100, 80])?,
            elbow,
        ],
    )?;
    let fixed = [0_i64.into(), Number::from_f64(0.5001)?];
    routed.author(profile)?.bind_arrow(
        &"elbow".into(),
        ArrowEndpoint::End,
        &"target".into(),
        match profile {
            Profile::V0_18_1 => BindingGeometry::Release {
                focus: 0_i64.into(),
                gap: 1_i64.into(),
                fixed_point: Some(fixed),
            },
            Profile::SnapshotAfa3a653 => BindingGeometry::Snapshot {
                fixed_point: fixed,
                mode: BindMode::Orbit,
            },
        },
    )?;

    let mut frame = shape(profile, ElementKind::Frame, "frame", [80, 170, 260, 230])?;
    frame.set(element::NAME, "Rust composition".into())?;
    let mut label = text(profile, "label", 125, 232, "Card")?;
    label.set(element::TEXT_ALIGN, TextAlign::Center)?;
    label.set(element::VERTICAL_ALIGN, VerticalAlign::Middle)?;
    let mut composition = scene(
        profile,
        vec![
            frame,
            shape(profile, ElementKind::Rectangle, "box", [110, 210, 100, 70])?,
            label,
        ],
    )?;
    {
        let mut author = composition.author(profile)?;
        author.bind_label(&"label".into(), &"box".into())?;
        author.insert_image(
            shape(profile, ElementKind::Image, "asset", [240, 220, 60, 60])?,
            pixel()?,
        )?;
        author.group(&["box".into(), "asset".into()], "temporary".into())?;
        author.ungroup(&"temporary".into())?;
        author.group(&["box".into(), "asset".into()], "cards".into())?;
        author.set_frame(&["box".into()], Some(&"frame".into()))?;
        author.translate_connected(&["frame".into()], point(10, 10))?;
    }
    let item = LibraryItem::new(
        profile,
        "rust-gallery-item",
        1_i64.into(),
        composition.elements()?,
    )?;
    let library = LibraryDocument::new(SOURCE, vec![item.clone()]);
    assert!(library.validate(profile, Purpose::Author).is_valid());
    let duplicate_map = mapping("copy");
    let library_map = mapping("library");
    {
        let mut author = composition.author(profile)?;
        assert!(
            author
                .duplicate(
                    &["box".into()],
                    &duplicate_map,
                    point(300, 0),
                    OpaquePolicy::Reject
                )?
                .is_empty()
        );
        assert!(
            author
                .insert_library_item(
                    &item,
                    &library_map,
                    BTreeMap::from([("gallery-pixel".into(), pixel()?)]),
                    point(600, 0),
                    OpaquePolicy::Reject
                )?
                .is_empty()
        );
        author.reorder(&["library-box".into()], Some(&"frame".into()))?;
    }

    let sticky = if profile == Profile::SnapshotAfa3a653 {
        let mut note = Element::sticky_note(
            profile,
            metadata(profile, "sticky"),
            180_i64.into(),
            120_i64.into(),
        )?;
        note.set(element::X, 350_i64.into())?;
        note.set(element::Y, 300_i64.into())?;
        let mut label = text(profile, "sticky-label", 375, 335, "Sticky note")?;
        label.set(element::BASE_FONT_SIZE, 20_i64.into())?;
        label.set(element::TEXT_ALIGN, TextAlign::Center)?;
        label.set(element::VERTICAL_ALIGN, VerticalAlign::Middle)?;
        let mut doc = scene(profile, vec![note, label])?;
        doc.author(profile)?
            .bind_label(&"sticky-label".into(), &"sticky".into())?;
        native(profile, &doc)?
    } else {
        Value::Null
    };
    // Historical state is deliberately built with the preserving setters: a
    // deleted label retains its old edge, while the live box has no reverse edge.
    // Native release projection would prune the tombstone before browser loading,
    // so send preserving encoding directly and validate both supported purposes.
    let mut historical_box = shape(
        profile,
        ElementKind::Rectangle,
        "historical-box",
        [350, 250, 200, 100],
    )?;
    historical_box.set(element::BOUND_ELEMENTS, vec![])?;
    historical_box.set(element::INDEX, "a0".into())?;
    let mut deleted_label = text(profile, "deleted-label", 375, 285, "Deleted label")?;
    deleted_label.set(element::CONTAINER_ID, "historical-box".into())?;
    deleted_label.set(element::IS_DELETED, true)?;
    deleted_label.set(element::INDEX, "a1".into())?;
    let mut historical = Document::new(SOURCE);
    historical.set_elements(vec![historical_box, deleted_label]);
    for purpose in [Purpose::Inspect, Purpose::Author] {
        let report = historical.validate(profile, purpose);
        assert!(report.is_valid(), "historical {purpose:?}: {report:?}");
    }
    let historical_bytes = historical.to_vec()?;
    assert_eq!(Document::from_slice(&historical_bytes)?, historical);
    let historical: Value = serde_json::from_slice(&historical_bytes)?;
    let output = json!({"gallery":native(profile, &gallery)?, "interactive":native(profile, &interactive)?,
        "image":native(profile, &image)?, "freehand":native(profile, &freehand)?,
        "multiline":native(profile, &multiline)?, "elbow":native(profile, &routed)?,
        "composition":native(profile, &composition)?, "library":library.as_object(), "sticky":sticky,
        "historical":historical, "historicalValidation":{"inspect":true,"author":true,"preservingRoundtrip":true}});
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    Ok(())
}
