use crate::cartesian::{Frame, MARGIN, TICK_SIZE, Ticks};
use crate::{
    AxisScale, Error, ExcalidrawBackend, LegendPosition, Scene, SketchStyle, StrokeStyle,
    TickFormat,
};
use plotters::coord::Shift;
use plotters::prelude::*;
use std::ops::Range;

type DrawResult = Result<(), Box<dyn std::error::Error>>;

struct MappedInterval {
    x: i32,
    lower: i32,
    estimate: i32,
    upper: i32,
}

struct Extents {
    half_stroke: f64,
    marker: f64,
    horizontal: f64,
}

/// One caller-supplied vertical interval. X is an observation key, not identity:
/// unordered/repeated X and equal-valued observations retain their input order.
/// Values are validated at render time; no statistical calculation is performed.
#[derive(Clone, Copy, Debug)]
pub struct VerticalInterval {
    x: f64,
    lower: f64,
    estimate: f64,
    upper: f64,
}

impl VerticalInterval {
    pub fn new(x: f64, lower: f64, estimate: f64, upper: f64) -> Self {
        Self {
            x,
            lower,
            estimate,
            upper,
        }
    }
}

/// Caller-supplied asymmetric vertical intervals on explicit linear axes.
/// Each observation has an inner group within a series group and chart group.
/// The required interpretation label describes the caller's statistical meaning
/// and appears in a measured right legend by default.
/// A zero-length interval draws one cap and center (two elements). Otherwise it
/// draws lower cap, stem, upper cap, then center (four elements). Nonzero limits
/// mapping to the same pixel are rejected; estimates may equal either endpoint.
/// Partial marker overlap is allowed, but an opaque solid-filled center must not
/// completely hide a nonzero interval's cap/stem strokes at mapped resolution.
/// Complete cap/stem/center geometric extents, including half stroke widths,
/// must fit the plot plus its existing five-unit edge halo. Widen the explicit
/// bounds or reduce geometry to clear labels. Rough ink is not exactly bounded.
pub struct ErrorBarChart<'a> {
    observations: &'a [VerticalInterval],
    interpretation: &'a str,
    x: Range<f64>,
    y: Range<f64>,
    legend: bool,
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    size: (u32, u32),
    color: RGBColor,
    width: u32,
    opacity: f64,
    stroke: StrokeStyle,
    cap: u32,
    radius: u32,
    fill: bool,
    sketch: SketchStyle,
    tick_count: (usize, usize),
    x_format: TickFormat,
    y_format: TickFormat,
}

impl<'a> ErrorBarChart<'a> {
    pub fn new(
        observations: &'a [VerticalInterval],
        interpretation: &'a str,
        x: Range<f64>,
        y: Range<f64>,
    ) -> Self {
        Self {
            observations,
            interpretation,
            x,
            y,
            legend: true,
            title: "Supplied intervals",
            x_label: "X",
            y_label: "Y",
            size: (640, 400),
            color: RGBColor(25, 113, 194),
            width: 2,
            opacity: 1.,
            stroke: StrokeStyle::Solid,
            cap: 8,
            radius: 4,
            fill: true,
            sketch: SketchStyle::default(),
            tick_count: (6, 6),
            x_format: TickFormat::Auto,
            y_format: TickFormat::Auto,
        }
    }

    pub fn color(mut self, rgb: (u8, u8, u8)) -> Self {
        self.color = RGBColor(rgb.0, rgb.1, rgb.2);
        self
    }

    /// Cap/stem and hollow center stroke width, 1..=20 scene units (default 2).
    pub fn line_width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Visible alpha, 0.005..=1, rounded to native percent (default 1).
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    /// Native pattern for caps/stem and their legend; center outlines stay solid.
    pub fn stroke_style(mut self, stroke: StrokeStyle) -> Self {
        self.stroke = stroke;
        self
    }

    /// Half cap width, 1..=20 scene units (default 8), independent of radius.
    pub fn cap_half_width(mut self, half_width: u32) -> Self {
        self.cap = half_width;
        self
    }

    /// Center radius, 1..=20 scene units, and fill (default 4, filled).
    /// Filled circles have no outline; hollow circles include half stroke width.
    pub fn marker(mut self, radius: u32, fill: bool) -> Self {
        self.radius = radius;
        self.fill = fill;
        self
    }

    pub fn sketch(mut self, sketch: SketchStyle) -> Self {
        self.sketch = sketch;
        self
    }

    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position == LegendPosition::Right;
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
        if !(1..=20).contains(&self.width) || !(1..=20).contains(&self.cap) {
            return Err(Error::Invalid(
                "interval stroke width and cap half width must be 1..=20 scene units",
            ));
        }
        crate::cartesian::validate_radius(self.radius)?;
        crate::cartesian::validate_opacity(self.opacity, crate::cartesian::MARK_OPACITY)?;
        crate::typography::measure(self.interpretation, f64::from(TICK_SIZE))?;
        if self.interpretation.trim().is_empty() {
            return Err(Error::Invalid("interval interpretation must not be empty"));
        }
        if self.observations.is_empty() {
            return Err(Error::Invalid(
                "interval chart needs at least one observation",
            ));
        }
        for o in self.observations {
            if ![o.x, o.lower, o.estimate, o.upper]
                .iter()
                .all(|v| v.is_finite())
                || o.lower > o.estimate
                || o.estimate > o.upper
                || o.x < self.x.start
                || o.x > self.x.end
                || o.lower < self.y.start
                || o.upper > self.y.end
            {
                return Err(Error::Invalid(
                    "intervals need finite lower <= estimate <= upper and X/limits inside explicit bounds",
                ));
            }
        }
        let mut scene = Scene::new();
        let options = scene.drawing_options();
        options
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
        let extents = self.extents();
        let extent = extents.horizontal.ceil() as u32;
        let legend = crate::cartesian::single_legend_width(
            self.interpretation,
            self.legend,
            self.size.0,
            2 * extent,
        )?;
        let layout = frame.layout([xt.max_width(), yt.max_width()], legend)?;
        let root = ExcalidrawBackend::new(scene, self.size)?.into_drawing_area();
        root.fill(&WHITE)?;
        options.with_style(self.sketch, || -> DrawResult {
        let mut chart = frame.build(&root, &layout, x, y)?;
        frame.check_ticks(&chart, &layout, &xt, &yt)?;
        let (left, bottom) = chart.plotting_area().map_coordinate(&(self.x.start, self.y.start));
        let (right, top) = chart.plotting_area().map_coordinate(&(self.x.end, self.y.end));
        frame.draw_axes(&root, &mut chart, &xt, &yt)?;
        series.scope(|| -> DrawResult {
            for observation in self.observations {
                let (x, lower) = chart.plotting_area().map_coordinate(&(observation.x, observation.lower));
                let (_, estimate) = chart.plotting_area().map_coordinate(&(observation.x, observation.estimate));
                let (_, upper) = chart.plotting_area().map_coordinate(&(observation.x, observation.upper));
                if crate::cartesian::collapsed((observation.lower, observation.upper), (lower, upper)) {
                    return Err(Error::Invalid("nonzero interval collapses at this pixel resolution; narrow bounds or increase size").into());
                }
                let Extents { half_stroke, marker: marker_extent, horizontal } = extents;
                // The farthest cap endpoint bounds all three round-ended strokes.
                // Use native rounded opacity; patterned/translucent/hollow centers leave
                // the underlying interval visible. Degenerate pairs are exempt.
                let farthest_limit = (lower - estimate).abs().max((upper - estimate).abs());
                if lower != upper && self.fill && (self.opacity * 100.).round() == 100.
                    && matches!(self.sketch.fill_style(), crate::FillStyle::Solid)
                    && f64::from(self.cap).hypot(f64::from(farthest_limit)) + half_stroke <= f64::from(self.radius)
                {
                    return Err(Error::Invalid("interval completely hidden by opaque center; reduce marker size, increase cap width, narrow Y bounds or enlarge chart").into());
                }
                if f64::from(x) - horizontal < f64::from(left - 5)
                    || f64::from(x) + horizontal > f64::from(right + 5)
                    || (f64::from(upper) - half_stroke).min(f64::from(estimate) - marker_extent) < f64::from(top - 5)
                    || (f64::from(lower) + half_stroke).max(f64::from(estimate) + marker_extent) > f64::from(bottom + 5)
                {
                    return Err(Error::Invalid("interval cap/marker stroke crowds plot-edge labels; reduce geometry or widen bounds").into());
                }
                options.new_group().scope(|| options.with_stroke_style(self.stroke, || self.draw_observation(&root, MappedInterval { x, lower, estimate, upper })))?;
            }
            Ok(())
        })?;
        if self.legend {
            series.scope(|| -> DrawResult {
                let x = crate::cartesian::legend_left(self.size.0, legend) + extent as i32;
                // Keep eight visible units of stem between the full marker and
                // each cap stroke. The schematic remains asymmetric and its
                // complete height stays above the bottom label area.
                let gap = (extents.marker + extents.half_stroke + 8.).ceil() as i32;
                let upper = 82;
                let estimate = upper + gap + 10;
                let lower = estimate + 10.max(gap);
                if f64::from(lower) + extents.half_stroke > f64::from(self.size.1 - MARGIN - crate::cartesian::X_LABEL_AREA) {
                    return Err(Error::Invalid("interval legend exceeds chart height").into());
                }
                options.with_stroke_style(self.stroke, || self.draw_observation(&root, MappedInterval { x, lower, estimate, upper }))?;
                root.draw_text(self.interpretation, &TextStyle::from(("Excalifont", TICK_SIZE).into_font()).color(&self.color), (x + extent as i32 + 8, estimate - 10))?;
                Ok(())
            })?;
        }
        root.present()?;
        Ok(())
        })
    }

    fn extents(&self) -> Extents {
        let half_stroke = f64::from(self.width) / 2.;
        let marker = f64::from(self.radius) + if self.fill { 0. } else { half_stroke };
        Extents {
            half_stroke,
            marker,
            horizontal: (f64::from(self.cap) + half_stroke).max(marker),
        }
    }

    fn draw_observation(
        &self,
        root: &DrawingArea<ExcalidrawBackend<'_>, Shift>,
        interval: MappedInterval,
    ) -> DrawResult {
        let MappedInterval {
            x,
            lower,
            estimate,
            upper,
        } = interval;
        let style = self.color.mix(self.opacity).stroke_width(self.width);
        let cap = self.cap as i32;
        // Re-enter the same native path pattern for data and delayed legends.
        root.draw(&PathElement::new(
            [(x - cap, lower), (x + cap, lower)],
            style,
        ))?;
        if lower != upper {
            root.draw(&PathElement::new([(x, lower), (x, upper)], style))?;
            root.draw(&PathElement::new(
                [(x - cap, upper), (x + cap, upper)],
                style,
            ))?;
        }
        root.draw(&Circle::new(
            (x, estimate),
            self.radius,
            ShapeStyle {
                filled: self.fill,
                ..style
            },
        ))?;
        Ok(())
    }
}
