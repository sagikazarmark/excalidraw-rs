use crate::cartesian::{Frame, MARGIN, TICK_SIZE, Ticks};
use crate::{AxisScale, Error, ExcalidrawBackend, LegendPosition, Scene, SketchStyle, TickFormat};
use plotters::prelude::*;
use std::ops::Range;

type DrawResult = Result<(), Box<dyn std::error::Error>>;

/// One caller-supplied varying lower/upper envelope on explicit linear axes.
/// Shared X must strictly increase (duplicates are rejected); all arrays must
/// align and contain finite values inside the inclusive bounds. No band values
/// are computed. A native fill joins upper samples to reversed lower samples.
/// The fill and delayed legend share an inner group within the chart group.
/// Zero-width samples are allowed, but the entire envelope must have area.
/// Adjacent X and every nonzero width must remain distinct after integer mapping.
/// Default paint is blue, 35% fill, with no boundary/center paths; optional solid
/// paths are opaque, width 3. Their full stroke extents must fit the plot's
/// five-unit halo (half-width rounded up). The required nonblank label appears in
/// a right legend by default, using a fill swatch and opaque text.
///
/// Painter order: axes, fill, optional upper, lower, center paths, then legend.
/// Fill edits do not update independent paths. The accepted editor's first/last
/// endpoints are independent; moving a closure handle can remove the fill until
/// re-closed. No statistics, resampling, gaps, automatic ranges or log axes.
pub struct BandChart<'a> {
    xs: &'a [f64],
    lower: &'a [f64],
    upper: &'a [f64],
    center: Option<&'a [f64]>,
    boundaries: bool,
    width: u32,
    name: &'a str,
    x: Range<f64>,
    y: Range<f64>,
    legend: bool,
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    size: (u32, u32),
    color: RGBColor,
    opacity: f64,
    sketch: SketchStyle,
    tick_count: (usize, usize),
    x_format: TickFormat,
    y_format: TickFormat,
}

impl<'a> BandChart<'a> {
    pub fn new(
        xs: &'a [f64],
        lower: &'a [f64],
        upper: &'a [f64],
        name: &'a str,
        x: Range<f64>,
        y: Range<f64>,
    ) -> Self {
        Self {
            xs,
            lower,
            upper,
            name,
            x,
            y,
            center: None,
            boundaries: false,
            width: 3,
            legend: true,
            title: "Supplied band",
            x_label: "X",
            y_label: "Y",
            size: (640, 400),
            color: RGBColor(25, 113, 194),
            opacity: 0.35,
            sketch: SketchStyle::default(),
            tick_count: (6, 6),
            x_format: TickFormat::Auto,
            y_format: TickFormat::Auto,
        }
    }

    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position == LegendPosition::Right;
        self
    }

    /// Add an opaque estimate path; values must align with X and lie in-envelope.
    pub fn center(mut self, values: &'a [f64]) -> Self {
        self.center = Some(values);
        self
    }

    /// Draw independent upper then lower paths over the fill (default false).
    /// Native fill edits do not move these paths or the optional center path.
    pub fn boundary_lines(mut self, enabled: bool) -> Self {
        self.boundaries = enabled;
        self
    }

    /// Boundary/center width, 1..=20 scene units (default 3). Paths are opaque.
    pub fn line_width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Fill and legend swatch alpha, 0.005..=1, rounded to native percent.
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn color(mut self, rgb: (u8, u8, u8)) -> Self {
        self.color = RGBColor(rgb.0, rgb.1, rgb.2);
        self
    }

    pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
        self.title = title;
        self.x_label = x;
        self.y_label = y;
        self
    }

    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.size = size;
        self
    }

    pub fn sketch(mut self, sketch: SketchStyle) -> Self {
        self.sketch = sketch;
        self
    }

    /// Desired major tick density per axis, 2..=20 (default six).
    pub fn tick_density(mut self, x: usize, y: usize) -> Self {
        self.tick_count = (x, y);
        self
    }

    pub fn x_tick_format(mut self, format: TickFormat) -> Self {
        self.x_format = format;
        self
    }

    pub fn y_tick_format(mut self, format: TickFormat) -> Self {
        self.y_format = format;
        self
    }

    /// Return a complete vector scene only after validation and drawing succeed.
    pub fn render(&self) -> Result<Scene, Error> {
        let frame = Frame {
            title: self.title,
            x_label: self.x_label,
            y_label: self.y_label,
            size: self.size,
        };
        frame.validate(self.tick_count)?;
        self.x_format.validate()?;
        self.y_format.validate()?;
        AxisScale::Linear.validate(&self.x)?;
        AxisScale::Linear.validate(&self.y)?;
        crate::typography::measure(self.name, f64::from(TICK_SIZE))?;
        if self.name.trim().is_empty() {
            return Err(Error::Invalid("band label must not be empty"));
        }
        crate::cartesian::validate_opacity(self.opacity)?;
        if !(1..=20).contains(&self.width) {
            return Err(Error::Invalid("band line width must be 1..=20 scene units"));
        }
        if self.xs.len() < 2
            || self.lower.len() != self.xs.len()
            || self.upper.len() != self.xs.len()
        {
            return Err(Error::Invalid(
                "band needs at least two aligned X/lower/upper samples",
            ));
        }
        if self.xs.windows(2).any(|xs| xs[0] >= xs[1]) {
            return Err(Error::Invalid(
                "band X must strictly increase; duplicate X is unsupported",
            ));
        }
        for ((&x, &lower), &upper) in self.xs.iter().zip(self.lower).zip(self.upper) {
            if ![x, lower, upper].iter().all(|v| v.is_finite())
                || lower > upper
                || x < self.x.start
                || x > self.x.end
                || lower < self.y.start
                || upper > self.y.end
            {
                return Err(Error::Invalid(
                    "band needs finite lower <= upper inside explicit containing bounds",
                ));
            }
        }
        if self.lower == self.upper {
            return Err(Error::Invalid("band must have nonzero area"));
        }
        if let Some(center) = self.center {
            if center.len() != self.xs.len() {
                return Err(Error::Invalid("band center values must align with X"));
            }
            if center
                .iter()
                .enumerate()
                .any(|(i, &v)| !v.is_finite() || v < self.lower[i] || v > self.upper[i])
            {
                return Err(Error::Invalid(
                    "band center must be finite and inside the envelope",
                ));
            }
        }
        let mut scene = Scene::new();
        scene
            .drawing_options()
            .new_group()
            .scope(|| self.draw(&mut scene, &frame))
            .map_err(|e| Error::Drawing(e.to_string()))?;
        Ok(scene)
    }

    fn draw(&self, scene: &mut Scene, frame: &Frame<'_>) -> DrawResult {
        let options = scene.drawing_options();
        let series = options.new_group();
        let x = AxisScale::Linear.coordinate(self.x.clone());
        let y = AxisScale::Linear.coordinate(self.y.clone());
        let xt = Ticks::new(
            Ticks::select(&x, self.tick_count.0),
            self.tick_count.0,
            |v| self.x_format.label(*v),
        )?;
        let yt = Ticks::new(
            Ticks::select(&y, self.tick_count.1),
            self.tick_count.1,
            |v| self.y_format.label(*v),
        )?;
        let label_width = crate::typography::measure(self.name, f64::from(TICK_SIZE))?.0;
        if self.legend && label_width > f64::from(self.size.0) {
            return Err(Error::Invalid("legend label exceeds chart width").into());
        }
        let legend = if self.legend {
            label_width.ceil() as u32 + 28 + 8 + 28
        } else {
            0
        };
        let layout = frame.layout([xt.max_width(), yt.max_width()], legend)?;
        let root = ExcalidrawBackend::new(scene, self.size)?.into_drawing_area();
        root.fill(&WHITE)?;
        options.with_style(self.sketch, || -> DrawResult {
            let mut chart = frame.build(&root, &layout, x, y)?;
            frame.check_ticks(&chart, &layout, &xt, &yt)?;
            let map_path = |values: &[f64]| -> Vec<_> {
                self.xs
                    .iter()
                    .zip(values)
                    .map(|(&x, &y)| chart.plotting_area().map_coordinate(&(x, y)))
                    .collect()
            };
            let upper = map_path(self.upper);
            let lower = map_path(self.lower);
            if upper.windows(2).any(|ps| ps[0].0 == ps[1].0)
                || upper
                    .iter()
                    .zip(&lower)
                    .enumerate()
                    .any(|(i, (u, l))| self.lower[i] != self.upper[i] && u.1 == l.1)
            {
                return Err(Error::Invalid(
                    "band collapses at this pixel resolution; narrow bounds or increase size",
                )
                .into());
            }
            let center = self.center.map(map_path);
            let paths: Vec<_> = [
                self.boundaries.then_some(&upper),
                self.boundaries.then_some(&lower),
                center.as_ref(),
            ]
            .into_iter()
            .flatten()
            .collect();
            let bottom_left = chart
                .plotting_area()
                .map_coordinate(&(self.x.start, self.y.start));
            let top_right = chart
                .plotting_area()
                .map_coordinate(&(self.x.end, self.y.end));
            // Reuse the plot halo policy for full path stroke extents.
            for path in &paths {
                crate::cartesian::check_markers(
                    path.iter().copied(),
                    self.width.div_ceil(2),
                    bottom_left,
                    top_right,
                )?;
            }
            frame.draw_axes(&root, &mut chart, &xt, &yt)?;
            series.scope(|| -> DrawResult {
                let polygon: Vec<_> = upper.iter().chain(lower.iter().rev()).copied().collect();
                root.draw(&Polygon::new(
                    polygon,
                    self.color.mix(self.opacity).filled(),
                ))?;
                for path in paths {
                    root.draw(&PathElement::new(
                        path.clone(),
                        self.color.stroke_width(self.width),
                    ))?;
                }
                Ok(())
            })?;
            if self.legend {
                series.scope(|| -> DrawResult {
                    let x = (self.size.0 - MARGIN - legend + 12) as i32;
                    root.draw(&Rectangle::new(
                        [(x, 85), (x + 28, 99)],
                        self.color.mix(self.opacity).filled(),
                    ))?;
                    root.draw_text(
                        self.name,
                        &TextStyle::from(("Excalifont", TICK_SIZE).into_font()).color(&self.color),
                        (x + 36, 82),
                    )?;
                    Ok(())
                })?;
            }
            root.present()?;
            Ok(())
        })
    }
}
