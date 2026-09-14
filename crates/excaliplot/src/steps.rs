use crate::cartesian::{Frame, Ticks};
use crate::{AxisScale, Error, ExcalidrawBackend, Scene, SketchStyle, TickFormat};
use plotters::prelude::*;
use std::borrow::Cow;
use std::ops::Range;

type DrawResult = Result<(), Box<dyn std::error::Error>>;

/// A bounded right-continuous post-step path on explicit linear axes.
/// X must strictly increase: generic steps never sort or aggregate duplicates.
/// Each `(x, y)` holds Y on `[x, next_x)`; at the next X the path jumps to
/// its new Y. The first point starts the path (no left extrapolation), and the
/// last value extends to the inclusive right bound, which must exceed its X.
/// A singleton is a constant segment. Finite containing X/Y bounds follow the
/// linear ±1e9 / minimum-span 1e-6 policy; empty/nonfinite data are rejected.
///
/// All nonzero holds and jumps must survive integer scene mapping. Native sharp
/// corners are retained without smoothing. Measured axes and the path share one
/// chart group; ungroup once to edit corners independently. Editing is one-way:
/// moving a corner does not update data or enforce staircase semantics.
/// Defaults: 640×400, opaque blue solid width 3, clean style, six desired ticks
/// per axis. Painter order is background/title, axes, then the single data path.
/// No legend, missing-value/gap convention, automatic/log scales or interpolation.
pub struct StepChart<'a> {
    points: Cow<'a, [(f64, f64)]>,
    x: Range<f64>,
    y: Range<f64>,
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    size: (u32, u32),
    color: RGBColor,
    width: u32,
    sketch: SketchStyle,
    tick_count: (usize, usize),
    x_format: TickFormat,
    y_format: TickFormat,
}

impl<'a> StepChart<'a> {
    pub fn new(points: &'a [(f64, f64)], x: Range<f64>, y: Range<f64>) -> Self {
        Self {
            points: Cow::Borrowed(points),
            x,
            y,
            title: "Step plot",
            x_label: "X",
            y_label: "Y",
            size: (640, 400),
            color: RGBColor(25, 113, 194),
            width: 3,
            sketch: SketchStyle::default(),
            tick_count: (6, 6),
            x_format: TickFormat::Auto,
            y_format: TickFormat::Auto,
        }
    }

    /// Construct the empirical CDF `F(x) = count(sample <= x) / n`.
    /// Copies and sorts finite samples, combines equal values (including signed
    /// zero), and computes each cumulative fraction from its integer count.
    /// Empty/nonfinite input is rejected. A singleton or all-equal sample makes
    /// one jump from zero to one. Explicit X bounds must strictly enclose all
    /// samples, exposing both tails; Y is fixed to 0..=1. Rendering also rejects
    /// tails, holds or jumps that collapse at the chosen scene resolution.
    ///
    /// ```
    /// use excaliplot::StepChart;
    /// let scene = StepChart::ecdf(&[6., 2., 4., 2.], 0.0..10.0)?
    ///     .labels("Empirical distribution", "Value", "Cumulative probability")
    ///     .size((800, 500))
    ///     .render()?;
    /// # Ok::<(), excaliplot::Error>(())
    /// ```
    pub fn ecdf(samples: &[f64], x: Range<f64>) -> Result<Self, Error> {
        AxisScale::Linear.validate(&x)?;
        if samples.is_empty()
            || samples
                .iter()
                .any(|v| !v.is_finite() || *v <= x.start || *v >= x.end)
        {
            return Err(Error::Invalid(
                "ECDF needs nonempty finite samples strictly inside explicit X bounds",
            ));
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let mut points = vec![(x.start, 0.)];
        for (i, &value) in sorted.iter().enumerate() {
            if sorted.get(i + 1).is_none_or(|&next| value != next) {
                points.push((value, (i + 1) as f64 / sorted.len() as f64));
            }
        }
        let mut chart = Self::new(&[], x, 0.0..1.0);
        chart.points = Cow::Owned(points);
        chart.title = "Empirical CDF";
        chart.x_label = "Value";
        chart.y_label = "Probability";
        Ok(chart)
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

    pub fn color(mut self, rgb: (u8, u8, u8)) -> Self {
        self.color = RGBColor(rgb.0, rgb.1, rgb.2);
        self
    }

    /// Solid path width, 1..=20 scene units (default 3). Full stroke extents
    /// must fit the shared five-unit plot halo; wide paths at edges can fail.
    pub fn line_width(mut self, width: u32) -> Self {
        self.width = width;
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

    /// Validate and render atomically; errors never publish a partial scene.
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
        if !(1..=20).contains(&self.width) {
            return Err(Error::Invalid("step line width must be 1..=20 scene units"));
        }
        if self.points.is_empty()
            || self.points.iter().any(|&(x, y)| {
                !x.is_finite()
                    || !y.is_finite()
                    || x < self.x.start
                    || x >= self.x.end
                    || y < self.y.start
                    || y > self.y.end
            })
        {
            return Err(Error::Invalid(
                "steps need finite points inside containing bounds with right extent",
            ));
        }
        if self.points.windows(2).any(|ps| ps[0].0 >= ps[1].0) {
            return Err(Error::Invalid(
                "step X must strictly increase; duplicate X is unsupported",
            ));
        }
        let mut vertices = vec![self.points[0]];
        for ps in self.points.windows(2) {
            vertices.push((ps[1].0, ps[0].1));
            if ps[0].1 != ps[1].1 {
                vertices.push(ps[1]);
            }
        }
        vertices.push((self.x.end, self.points.last().unwrap().1));
        let mut scene = Scene::new();
        scene
            .drawing_options()
            .new_group()
            .scope(|| self.draw(&mut scene, &frame, &vertices))
            .map_err(|e| Error::Drawing(e.to_string()))?;
        Ok(scene)
    }

    fn draw(&self, scene: &mut Scene, frame: &Frame<'_>, vertices: &[(f64, f64)]) -> DrawResult {
        let options = scene.drawing_options();
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
        let layout = frame.layout([xt.max_width(), yt.max_width()], 0)?;
        let root = ExcalidrawBackend::new(scene, self.size)?.into_drawing_area();
        root.fill(&WHITE)?;
        options.with_style(self.sketch, || -> DrawResult {
            let mut chart = frame.build(&root, &layout, x, y)?;
            frame.check_ticks(&chart, &layout, &xt, &yt)?;
            let mapped: Vec<_> = vertices.iter()
                .map(|p| chart.plotting_area().map_coordinate(p)).collect();
            if mapped.windows(2).any(|ps| ps[0] == ps[1]) {
                return Err(Error::Invalid("step hold or jump collapses at this pixel resolution; narrow bounds or increase size").into());
            }
            crate::cartesian::check_markers(mapped.iter().copied(), self.width.div_ceil(2),
                chart.plotting_area().map_coordinate(&(self.x.start, self.y.start)),
                chart.plotting_area().map_coordinate(&(self.x.end, self.y.end)))?;
            frame.draw_axes(&root, &mut chart, &xt, &yt)?;
            root.draw(&PathElement::new(mapped, self.color.stroke_width(self.width)))?;
            root.present()?;
            Ok(())
        })
    }
}
