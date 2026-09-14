//! Rust-only diagram creation: no input JSON, font engine or editor restoration.
use excalidraw_document::*;
use std::io::{self, Write};

fn shape(
    profile: Profile,
    kind: ElementKind,
    id: &str,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) -> Result<Element, Error> {
    let mut element = Element::new(kind, profile, id.into(), 1_u64.into())?;
    element.set(element::X, x.into())?;
    element.set(element::Y, y.into())?;
    element.set(element::WIDTH, width.into())?;
    element.set(element::HEIGHT, height.into())?;
    element.set(element::ROUGHNESS, 0_u64.into())?;
    Ok(element)
}

fn endpoint(profile: Profile, x: f64) -> Result<BindingGeometry, Error> {
    Ok(match profile {
        Profile::V0_18_1 => BindingGeometry::Release {
            focus: 0_i64.into(),
            gap: 5_u64.into(),
            fixed_point: None,
        },
        Profile::SnapshotAfa3a653 => BindingGeometry::Snapshot {
            // Upstream normalizeFixedPoint nudges exact midpoints to 0.5001 to
            // stabilize headings. Supply that geometry explicitly.
            fixed_point: [Number::from_f64(x)?, Number::from_f64(0.5001)?],
            mode: BindMode::Orbit,
        },
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = match std::env::args().nth(1).as_deref() {
        Some("release") => Profile::V0_18_1,
        Some("snapshot") => Profile::SnapshotAfa3a653,
        _ => return Err("usage: authored_diagram release|snapshot".into()),
    };
    let left = shape(profile, ElementKind::Rectangle, "left", 100, 150, 200, 100)?;
    let right = shape(profile, ElementKind::Rectangle, "right", 500, 150, 200, 100)?;
    let mut left_label = shape(profile, ElementKind::Text, "left-label", 160, 187, 80, 25)?;
    let mut right_label = shape(profile, ElementKind::Text, "right-label", 560, 187, 80, 25)?;
    // Metrics are supplied by the caller; use a known installed monospace family.
    for label in [&mut left_label, &mut right_label] {
        label.set(element::FONT_FAMILY, 3_u64.into())?;
        label.set(element::TEXT_ALIGN, TextAlign::Center)?;
        label.set(element::VERTICAL_ALIGN, VerticalAlign::Middle)?;
    }
    left_label.set_text_content(TextContent::plain("Before", 72_u64.into(), 25_u64.into()))?;
    right_label.set_text_content(TextContent::plain("Output", 72_u64.into(), 25_u64.into()))?;
    let mut arrow = shape(profile, ElementKind::Arrow, "connector", 305, 200, 190, 0)?;
    arrow.set_path(vec![
        [0_i64.into(), 0_i64.into()],
        [190_i64.into(), 0_i64.into()],
    ])?;
    let mut document = Document::new("excalidraw-document/authored-diagram");
    {
        let mut author = document.author(profile)?;
        author.insert(vec![left, left_label, right, right_label, arrow])?;
        author.batch(|batch| {
            batch.bind_label(&"left-label".into(), &"left".into())?;
            batch.bind_label(&"right-label".into(), &"right".into())?;
            batch.bind_arrow(
                &"connector".into(),
                ArrowEndpoint::Start,
                &"left".into(),
                endpoint(profile, 1.)?,
            )?;
            batch.bind_arrow(
                &"connector".into(),
                ArrowEndpoint::End,
                &"right".into(),
                endpoint(profile, 0.)?,
            )?;
            batch.replace_text(
                &"left-label".into(),
                TextContent::plain("Input", 60_u64.into(), 25_u64.into()),
            )?;
            Ok(())
        })?;
    }
    let checked = document
        .validated(profile, Purpose::SelfContained)
        .map_err(|report| format!("authored diagram failed validation: {report:?}"))?;
    let exported =
        checked.project_native(ExportMode::Local, "excalidraw-document/authored-diagram")?;
    io::stdout().write_all(&exported.document.to_vec_pretty()?)?;
    Ok(())
}
