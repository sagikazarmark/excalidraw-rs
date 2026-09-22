use crate::cartesian::{Frame, Ticks};
use crate::{AxisScale, Error, ExcalidrawBackend, Scene, SketchStyle, TickFormat};
use plotters::prelude::*;
use std::ops::Range;

type DrawResult = Result<(), Box<dyn std::error::Error>>;

/// A pre-binned frequency histogram with numeric widths and count heights.
/// Each nonzero count emits one native rectangle in source order, grouped with
/// the measured linear axes. No aggregation or density normalization occurs.
///
/// Supply at least two finite, strictly increasing edges and exactly one finite
/// nonnegative count per interval. Explicit increasing bounds contain all edges,
/// counts and zero, inclusively; the linear ±1e9 / minimum-span 1e-6 policy applies.
/// Fractional counts, repeated counts and all-zero inputs are allowed. Every bin
/// width and every nonzero height must survive integer scene mapping. Zero counts
/// retain their intervals but emit no filled area. Invalid data, styles or measured
/// layout return an error without publishing a partial scene.
///
/// Defaults: 640×400, opaque blue borderless rectangles, clean style, six desired
/// numeric ticks per axis. Painter order is background/title, axes/zero baseline,
/// then nonzero bins in source order. One outer group contains the whole chart;
/// ungroup once for individual rectangle edits. Native edits are one-way and do
/// not recalculate counts, edges or neighboring bins. No legend, raw-sample binning,
/// automatic/log axes, weights, density normalization or comparative histograms.
///
/// ```
/// use excaliplot::HistogramChart;
/// let scene = HistogramChart::new(
///     &[-10., -8., -4., 2., 10.], &[4., 0., 8., 4.],
///     -10.0..10.0, 0.0..10.0,
/// ).labels("Supplied frequencies", "Measurement", "Count").render()?;
/// # Ok::<(), excaliplot::Error>(())
/// ```
pub struct HistogramChart<'a> {
    edges: &'a [f64],
    counts: &'a [f64],
    x: Range<f64>,
    y: Range<f64>,
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

impl<'a> HistogramChart<'a> {
    pub fn new(edges: &'a [f64], counts: &'a [f64], x: Range<f64>, y: Range<f64>) -> Self {
        Self {
            edges,
            counts,
            x,
            y,
            title: "Frequency histogram",
            x_label: "Value",
            y_label: "Count",
            size: (640, 400),
            color: RGBColor(25, 113, 194),
            opacity: 1.,
            sketch: SketchStyle::default(),
            tick_count: (6, 6),
            x_format: TickFormat::Auto,
            y_format: TickFormat::Auto,
        }
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

    /// Fill alpha, 0.005..=1, rounded to native integer percent (default 1).
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
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

    /// Return a complete native scene only after validation and drawing succeed.
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
        crate::cartesian::validate_opacity(
            self.opacity,
            "histogram opacity must round to a visible value in 1..=100 percent",
        )?;
        AxisScale::Linear.validate(&self.x)?;
        AxisScale::Linear.validate(&self.y)?;
        if self.y.start > 0. || self.y.end < 0. {
            return Err(Error::Invalid("histogram value bounds must contain zero"));
        }
        if self.edges.len() < 2 || self.counts.len() != self.edges.len() - 1 {
            return Err(Error::Invalid(
                "histogram needs at least two edges and exactly one count per interval",
            ));
        }
        if self
            .edges
            .iter()
            .any(|v| !v.is_finite() || *v < self.x.start || *v > self.x.end)
            || self.edges.windows(2).any(|vs| vs[0] >= vs[1])
        {
            return Err(Error::Invalid(
                "histogram edges must be finite, strictly increasing and inside X bounds",
            ));
        }
        if self
            .counts
            .iter()
            .any(|v| !v.is_finite() || *v < 0. || *v > self.y.end)
        {
            return Err(Error::Invalid(
                "histogram counts must be finite, nonnegative and inside Y bounds",
            ));
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
        let root = ExcalidrawBackend::new(scene, frame.size)?.into_drawing_area();
        root.fill(&WHITE)?;
        options.with_style(self.sketch, || -> DrawResult {
            let mut chart = frame.build(&root, &layout, x, y)?;
            frame.check_ticks(&chart, &layout, &xt, &yt)?;
            let bins: Vec<_> = self
                .edges
                .windows(2)
                .zip(self.counts)
                .map(|(edges, &count)| {
                    let lower = chart.plotting_area().map_coordinate(&(edges[0], 0.));
                    let upper = chart.plotting_area().map_coordinate(&(edges[1], count));
                    (lower, upper, count)
                })
                .collect();
            if bins.iter().any(|(lower, upper, count)| {
                lower.0 == upper.0
                    || crate::cartesian::collapsed((0., *count), (lower.1, upper.1))
            }) {
                return Err(Error::Invalid(
                    "histogram bin collapses at this pixel resolution; narrow bounds or increase size",
                )
                .into());
            }
            frame.draw_axes(&root, &mut chart, &xt, &yt)?;
            if self.y.start < 0. {
                let left = chart.plotting_area().map_coordinate(&(self.x.start, 0.));
                let right = chart.plotting_area().map_coordinate(&(self.x.end, 0.));
                root.draw(&PathElement::new([left, right], BLACK))?;
            }
            for (lower, upper, count) in bins {
                if count != 0. {
                    root.draw(&Rectangle::new(
                        [lower, upper],
                        self.color.mix(self.opacity).filled(),
                    ))?;
                }
            }
            root.present()?;
            Ok(())
        })
    }
}
