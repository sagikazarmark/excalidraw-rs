use crate::Error;
use std::sync::OnceLock;

pub(crate) const LINE_HEIGHT: f64 = 1.25;

pub(crate) fn measure_note(text: &str, size: f64) -> Result<(f64, f64), Error> {
    if text.is_empty() {
        return Err(Error::Invalid("note text must not be empty"));
    }
    let mut width: f64 = 0.;
    let mut height = 0.;
    for line in text.split('\n') {
        // Excalidraw measures an empty line as a space, including the final line.
        let (w, h) = measure(if line.is_empty() { " " } else { line }, size)?;
        width = width.max(w);
        height += h;
        if !height.is_finite() || height > f64::from(u32::MAX) {
            return Err(Error::Invalid("text height must fit u32 dimensions"));
        }
    }
    Ok((width, height))
}

/// Measure a single line using the same bundled Excalifont metrics as rendering.
/// Returns fractional (width, height) in scene units, with a 1.25-em line box.
/// Supports printable ASCII plus `é`, `−`, `°`, and `±`; newlines and other glyphs
/// are rejected. Size must be finite and positive, and dimensions must fit u32.
/// Empty text has zero width. Measurement does not mutate a scene.
pub fn measure(text: &str, size: f64) -> Result<(f64, f64), Error> {
    if !size.is_finite() || size <= 0.0 || size * LINE_HEIGHT > f64::from(u32::MAX) {
        return Err(Error::Invalid(
            "font size must be finite, positive, and fit u32 dimensions",
        ));
    }
    static FACE: OnceLock<rustybuzz::Face<'static>> = OnceLock::new();
    let face = FACE.get_or_init(|| {
        rustybuzz::Face::from_slice(include_bytes!("../assets/Excalifont-Regular.ttf"), 0)
            .expect("bundled pinned font is valid")
    });
    for ch in text.chars() {
        if !((' '..='~').contains(&ch) || matches!(ch, 'é' | '−' | '°' | '±'))
            || face.glyph_index(ch).is_none()
        {
            return Err(Error::UnsupportedGlyph(ch));
        }
    }
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    buffer.set_script(rustybuzz::script::LATIN);
    let glyphs = rustybuzz::shape(face, &[], buffer);
    let advance: i64 = glyphs
        .glyph_positions()
        .iter()
        .map(|p| i64::from(p.x_advance))
        .sum();
    let width = advance as f64 * size / f64::from(face.units_per_em());
    if !width.is_finite() || width > f64::from(u32::MAX) {
        return Err(Error::Invalid("text width must fit u32 dimensions"));
    }
    Ok((width, size * LINE_HEIGHT))
}
