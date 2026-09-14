use crate::{
    Error, Scene,
    scene::{Kind, Paint},
};
use plotters_backend::{
    BackendColor, BackendCoord, BackendStyle, BackendTextStyle, DrawingBackend, DrawingErrorKind,
};
use plotters_backend::{
    FontTransform,
    text_anchor::{HPos, VPos},
};

type DrawResult<T = ()> = Result<T, DrawingErrorKind<Error>>;

/// Synchronous vector subset. `present` is an idempotent checkpoint, not file I/O.
pub struct ExcalidrawBackend<'a> {
    scene: &'a mut Scene,
    size: (u32, u32),
}

impl<'a> ExcalidrawBackend<'a> {
    /// Access the native scene from a custom Plotters element. Scene operations
    /// take absolute scene coordinates, not drawing-area-relative coordinates.
    /// Propagate errors from native operations; unlike failed backend drawing,
    /// rejected atomic scene operations do not poison the scene.
    pub fn scene_mut(&mut self) -> &mut Scene {
        self.scene
    }

    pub fn new(scene: &'a mut Scene, size: (u32, u32)) -> Result<Self, Error> {
        if size.0 == 0 || size.1 == 0 || size.0 > i32::MAX as u32 || size.1 > i32::MAX as u32 {
            return Err(scene.fail(Error::Invalid(
                "canvas dimensions must be positive i32 values",
            )));
        }
        Ok(Self { scene, size })
    }

    fn unsupported<T>(&self, operation: &'static str) -> DrawResult<T> {
        Err(DrawingErrorKind::DrawingError(
            self.scene.fail(Error::Unsupported(operation)),
        ))
    }

    fn paint(&self, color: BackendColor) -> DrawResult<Option<Paint>> {
        if !color.alpha.is_finite() || !(0.0..=1.0).contains(&color.alpha) {
            return Err(DrawingErrorKind::DrawingError(
                self.scene
                    .fail(Error::Invalid("alpha must be finite and in 0..=1")),
            ));
        }
        let opacity = (color.alpha * 100.0).round() as u8;
        if opacity == 0 {
            return Ok(None);
        }
        let (r, g, b) = color.rgb;
        Ok(Some(Paint {
            color: format!("#{r:02x}{g:02x}{b:02x}"),
            opacity,
        }))
    }

    fn text_size<S: BackendTextStyle>(&self, text: &str, style: &S) -> DrawResult<(f64, f64)> {
        if style.family().as_str() != "Excalifont"
            || !matches!(style.style(), plotters_backend::FontStyle::Normal)
        {
            return self.unsupported("only normal Excalifont text is supported");
        }
        crate::typography::measure(text, style.size())
            .map_err(|error| DrawingErrorKind::DrawingError(self.scene.fail(error)))
    }

    fn path<S: BackendStyle>(
        &mut self,
        points: impl IntoIterator<Item = BackendCoord>,
        style: &S,
        fill: bool,
    ) -> DrawResult {
        let Some(paint) = self.paint(style.color())? else {
            return Ok(());
        };
        if !fill && style.stroke_width() == 0 {
            return Ok(());
        }
        let mut points: Vec<_> = points.into_iter().collect();
        if fill {
            points.dedup();
            if points.first() == points.last() {
                points.pop();
            }
            if points.len() < 3 {
                return Ok(());
            }
            points.push(points[0]);
            let area: i128 = points
                .windows(2)
                .map(|p| {
                    i128::from(p[0].0) * i128::from(p[1].1)
                        - i128::from(p[1].0) * i128::from(p[0].1)
                })
                .sum();
            if area == 0 {
                return Ok(());
            }
        }
        self.scene
            .path(
                points
                    .into_iter()
                    .map(|(x, y)| (f64::from(x), f64::from(y)))
                    .collect(),
                paint,
                style.stroke_width(),
                fill,
            )
            .map_err(|error| DrawingErrorKind::DrawingError(self.scene.fail(error)))
    }
}

impl DrawingBackend for ExcalidrawBackend<'_> {
    type ErrorType = Error;
    fn get_size(&self) -> (u32, u32) {
        self.scene.call("get_size");
        self.size
    }
    fn ensure_prepared(&mut self) -> DrawResult {
        self.scene.call("ensure_prepared");
        Ok(())
    }
    fn present(&mut self) -> DrawResult {
        self.scene.call("present");
        Ok(())
    }
    fn draw_pixel(&mut self, _: BackendCoord, color: BackendColor) -> DrawResult {
        self.scene.call("draw_pixel");
        if self.paint(color)?.is_none() {
            return Ok(());
        }
        self.unsupported("draw_pixel")
    }
    fn draw_line<S: BackendStyle>(
        &mut self,
        from: BackendCoord,
        to: BackendCoord,
        style: &S,
    ) -> DrawResult {
        self.scene.call("draw_line");
        self.path([from, to], style, false)
    }
    fn draw_path<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        points: I,
        style: &S,
    ) -> DrawResult {
        self.scene.call("draw_path");
        self.path(points, style, false)
    }
    fn draw_rect<S: BackendStyle>(
        &mut self,
        a: BackendCoord,
        b: BackendCoord,
        style: &S,
        fill: bool,
    ) -> DrawResult {
        self.scene.call("draw_rect");
        let Some(paint) = self.paint(style.color())? else {
            return Ok(());
        };
        if a.0 == b.0 || a.1 == b.1 || (!fill && style.stroke_width() == 0) {
            return Ok(());
        }
        let result = (|| -> Result<(), Error> {
            let element = self.scene.push(
                (f64::from(a.0.min(b.0)), f64::from(a.1.min(b.1))),
                (
                    (f64::from(a.0) - f64::from(b.0)).abs(),
                    (f64::from(a.1) - f64::from(b.1)).abs(),
                ),
                paint,
                style.stroke_width().max(1),
                Kind::Rectangle,
            )?;
            if fill {
                crate::scene::fill_only(element)?;
            }
            Ok(())
        })();
        result.map_err(|error| DrawingErrorKind::DrawingError(self.scene.fail(error)))
    }
    fn draw_circle<S: BackendStyle>(
        &mut self,
        center: BackendCoord,
        radius: u32,
        style: &S,
        fill: bool,
    ) -> DrawResult {
        self.scene.call("draw_circle");
        let Some(paint) = self.paint(style.color())? else {
            return Ok(());
        };
        if radius == 0 || (!fill && style.stroke_width() == 0) {
            return Ok(());
        }
        let radius = f64::from(radius);
        let result = (|| -> Result<(), Error> {
            let element = self.scene.push(
                (f64::from(center.0) - radius, f64::from(center.1) - radius),
                (2.0 * radius, 2.0 * radius),
                paint,
                style.stroke_width().max(1),
                Kind::Ellipse,
            )?;
            if fill {
                crate::scene::fill_only(element)?;
            }
            Ok(())
        })();
        result.map_err(|error| DrawingErrorKind::DrawingError(self.scene.fail(error)))
    }
    fn fill_polygon<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        points: I,
        style: &S,
    ) -> DrawResult {
        self.scene.call("fill_polygon");
        self.path(points, style, true)
    }
    fn draw_text<S: BackendTextStyle>(
        &mut self,
        text: &str,
        style: &S,
        pos: BackendCoord,
    ) -> DrawResult {
        self.scene.call("draw_text");
        let Some(paint) = self.paint(style.color())? else {
            return Ok(());
        };
        let (w, h) = self.text_size(text, style)?;
        if text.is_empty() {
            return Ok(());
        }
        let ax = match style.anchor().h_pos {
            HPos::Left => 0.0,
            HPos::Center => 0.5,
            HPos::Right => 1.0,
        };
        let ay = match style.anchor().v_pos {
            VPos::Top => 0.0,
            VPos::Center => 0.5,
            VPos::Bottom => 1.0,
        };
        let (dx, dy) = ((0.5 - ax) * w, (0.5 - ay) * h);
        let quarter = std::f64::consts::FRAC_PI_2;
        let (dx, dy, angle) = match style.transform() {
            FontTransform::None => (dx, dy, 0.0),
            FontTransform::Rotate90 => (-dy, dx, quarter),
            FontTransform::Rotate180 => (-dx, -dy, 2.0 * quarter),
            FontTransform::Rotate270 => (dy, -dx, 3.0 * quarter),
        };
        let result = (|| -> Result<(), Error> {
            let element = self.scene.push(
                (
                    f64::from(pos.0) + dx - w / 2.0,
                    f64::from(pos.1) + dy - h / 2.0,
                ),
                (w, h),
                paint,
                1,
                Kind::Text {
                    text: text.into(),
                    font_size: style.size(),
                },
            )?;
            element.set(
                excalidraw_document::element::ANGLE,
                excalidraw_document::Number::from_f64(angle)?,
            )?;
            Ok(())
        })();
        result.map_err(|error| DrawingErrorKind::DrawingError(self.scene.fail(error)))
    }
    fn estimate_text_size<S: BackendTextStyle>(
        &self,
        text: &str,
        style: &S,
    ) -> DrawResult<(u32, u32)> {
        self.scene.call("estimate_text_size");
        self.text_size(text, style)
            .map(|(w, h)| (w.ceil() as u32, h.ceil() as u32))
    }
    fn blit_bitmap(&mut self, _: BackendCoord, _: (u32, u32), _: &[u8]) -> DrawResult {
        self.scene.call("blit_bitmap");
        self.unsupported("blit_bitmap")
    }
}
