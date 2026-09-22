use crate::cartesian::{
    self, Frame, LEGEND_ROW_HEIGHT, LEGEND_TOP, Layout, TICK_SIZE, Ticks, X_LABEL_AREA,
};
use crate::{
    AxisScale, DrawingGroup, Error, ExcalidrawBackend, Scene, SketchStyle, StrokeStyle, TickFormat,
};
use plotters::prelude::*;
use std::ops::Range;

const TICK_COUNT: usize = 6;

/// Native Cartesian legend placement. Named constructors default to `Right`;
/// unnamed constructors default to `Off` and have no legend names to display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendPosition {
    Right,
    Off,
}

macro_rules! numeric_tick_options {
    ($($chart:ident),+) => { $(
        impl $chart<'_> {
            /// Format numeric X ticks without changing their coordinates.
            pub fn x_tick_format(mut self, format: TickFormat) -> Self {
                self.chart.x_format = format;
                self
            }
        }
    )+ };
}
numeric_tick_options!(LineChart, AreaChart, ScatterChart);

macro_rules! annotation_options {
    ($($chart:ident),+) => { $(
        impl $chart<'_> {
            /// Add an independently grouped full-span rule, after data and before legends.
            /// Coordinates are validated against final chart bounds; annotations never expand them.
            pub fn reference_rule(mut self, rule: crate::ReferenceRule) -> Self {
                self.chart.rules.push(rule);
                self
            }
            /// Add an independently grouped window before axes/data, in window input order.
            /// Endpoints must increase and map to nonzero area inside final chart bounds.
            pub fn shaded_interval(mut self, interval: crate::ShadedInterval) -> Self {
                self.chart.intervals.push(interval);
                self
            }
            /// Add an explicitly offset note/arrow pair after rules and before legends.
            pub fn callout(mut self, callout: crate::Callout) -> Self {
                self.chart.callouts.push(callout);
                self
            }
        }
    )+ };
}
annotation_options!(LineChart, AreaChart, ScatterChart);

macro_rules! scale_options {
    ($($chart:ident),+) => { $(
        impl $chart<'_> {
            /// Select X mapping. Log10 requires explicit bounds; see [`AxisScale`].
            pub fn x_scale(mut self, scale: AxisScale) -> Self {
                self.chart.x_scale = scale;
                self
            }
            /// Select Y mapping independently of X. See [`AxisScale`].
            pub fn y_scale(mut self, scale: AxisScale) -> Self {
                self.chart.y_scale = scale;
                self
            }
        }
    )+ };
}
scale_options!(LineChart, ScatterChart);

macro_rules! tick_options {
    ($($chart:ident),+) => { $(
        impl $chart<'_> {
            /// Show a measured right legend or hide it, retaining named-series groups.
            pub fn legend(mut self, position: LegendPosition) -> Self {
                self.chart.legend = position == LegendPosition::Right;
                self
            }
            /// Desired major ticks per numeric axis, 2..=20 (default six).
            /// Plotters chooses nice values, so the actual count may be lower.
            /// Categories are always all shown; X density is ignored for bars.
            pub fn tick_density(mut self, x: usize, y: usize) -> Self {
                self.chart.tick_count = (x, y);
                self
            }
            /// Format numeric Y ticks without changing data or stack normalization.
            pub fn y_tick_format(mut self, format: TickFormat) -> Self {
                self.chart.y_format = format;
                self
            }
        }
    )+ };
}
tick_options!(LineChart, AreaChart, ScatterChart, BarChart);

/// Named data with an explicit paint; names and colors never determine identity.
#[derive(Clone)]
pub struct NamedSeries<'a> {
    name: &'a str,
    points: &'a [(f64, f64)],
    color: RGBColor,
    line_width: Option<u32>,
    opacity: Option<f64>,
    marker: Option<(u32, bool)>,
    stroke_style: Option<StrokeStyle>,
}

impl<'a> NamedSeries<'a> {
    pub fn new(name: &'a str, points: &'a [(f64, f64)], rgb: (u8, u8, u8)) -> Self {
        Self {
            name,
            points,
            color: RGBColor(rgb.0, rgb.1, rgb.2),
            line_width: None,
            opacity: None,
            marker: None,
            stroke_style: None,
        }
    }

    /// Line/marker stroke width in 1..=20 scene units, checked at render time.
    /// Defaults to 3 for lines and 2 for scatter. Filled circles have no stroke.
    /// Per-series styling is supported by numeric line/scatter charts only.
    pub fn line_width(mut self, width: u32) -> Self {
        self.line_width = Some(width);
        self
    }

    /// Native line pattern (solid by default), supported by line charts only.
    /// The original path and all vertices remain one editable element. Legends
    /// use the same pattern; marker outlines stay solid. Spacing is defined by
    /// the editor, not arbitrary Plotters dash length, gap or phase.
    pub fn stroke_style(mut self, style: StrokeStyle) -> Self {
        self.stroke_style = Some(style);
        self
    }

    /// Visible alpha in 0.005..=1.0, rounded to native integer percent.
    /// Overrides chart-wide scatter opacity; lines default to opaque.
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = Some(opacity);
        self
    }

    /// Add circle markers (radius 1..=20 scene units), or override the scatter
    /// chart's marker. `fill = false` leaves a transparent interior.
    /// Lines default to no markers. Each path is painted before its markers,
    /// in observation order, then the next series; legends are painted last.
    /// Repeated observations retain separate markers and original path vertices.
    pub fn marker(mut self, radius: u32, fill: bool) -> Self {
        self.marker = Some((radius, fill));
        self
    }
}

/// Category-aligned values with an explicit legend name and color.
#[derive(Clone)]
pub struct NamedBarSeries<'a> {
    name: &'a str,
    values: &'a [f64],
    color: RGBColor,
}

impl<'a> NamedBarSeries<'a> {
    pub fn new(name: &'a str, values: &'a [f64], rgb: (u8, u8, u8)) -> Self {
        Self {
            name,
            values,
            color: RGBColor(rgb.0, rgb.1, rgb.2),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stacking {
    None,
    Absolute,
    Percent,
}

// Lower and upper Y boundaries, in input order, shared by area and bar stacks.
type Bands = Vec<Vec<(f64, f64)>>;

/// Numeric-X line chart with explicit or opt-in automatic bounds.
/// Marker-free by default; named series may opt into circles via [`NamedSeries::marker`].
pub struct LineChart<'a> {
    chart: Chart<'a>,
}

impl<'a> LineChart<'a> {
    /// Infer containing linear bounds at render time. See [`Self::auto_from_series`].
    pub fn auto(points: &'a [(f64, f64)]) -> Self {
        let mut chart = Self::new(points, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    /// Infer the union of all series, with 5% padding on each side. Constant
    /// axes use `max(abs(value) * 0.05, 1)` per side; tiny spans get at least
    /// 1e-6 padding per side. Padding stops at ±1e9; data never changes.
    /// Cardinality, ordering, label-fit and mapped-resolution checks still apply.
    pub fn auto_from_series(series: &[NamedSeries<'a>]) -> Self {
        let mut chart = Self::from_series(series, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    pub fn new(points: &'a [(f64, f64)], x: Range<f64>, y: Range<f64>) -> Self {
        Self {
            chart: Chart::new(points, x, y),
        }
    }
    /// Named series use an explicit native legend and one inner group per series.
    pub fn from_series(series: &[NamedSeries<'a>], x: Range<f64>, y: Range<f64>) -> Self {
        Self {
            chart: Chart::from_series(series, x, y),
        }
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.chart = self.chart.sketch(style);
        self
    }
    pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
        self.chart = self.chart.labels(title, x, y);
        self
    }
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.chart = self.chart.size(size);
        self
    }
    pub fn render(&self) -> Result<Scene, Error> {
        self.chart.render()
    }
}

/// Filled areas with separate data-edge borders. Named series overlap by default;
/// stacking requires nonnegative values on a shared, strictly increasing X grid.
pub struct AreaChart<'a> {
    chart: Chart<'a>,
    baseline: f64,
    opacity: f64,
}

impl<'a> AreaChart<'a> {
    /// Infer linear ranges including the configured baseline, using
    /// [`LineChart::auto_from_series`]'s padding policy.
    pub fn auto(points: &'a [(f64, f64)]) -> Self {
        let mut chart = Self::new(points, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    /// Infer ranges across all areas, including cumulative endpoints when stacked
    /// or 0..100 when percent-stacked. Options are resolved at render time.
    pub fn auto_from_series(series: &[NamedSeries<'a>]) -> Self {
        let mut chart = Self::from_series(series, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    pub fn new(points: &'a [(f64, f64)], x: Range<f64>, y: Range<f64>) -> Self {
        let mut chart = Chart::new(points, x, y);
        chart.title = "Area chart";
        // The mark payload is owned here and assembled in `render`. Keeping it
        // out of `chart.marks` is what stops the setters below from depending
        // on a variant match that can miss without saying so.
        Self {
            chart,
            baseline: 0.0,
            opacity: 0.35,
        }
    }
    /// Overlapping areas, painted in input order with a shared baseline.
    pub fn from_series(series: &[NamedSeries<'a>], x: Range<f64>, y: Range<f64>) -> Self {
        let mut area = Self::new(&[], x, y);
        area.chart.series = series.to_vec();
        area.chart.legend = true;
        area.chart.named = true;
        area
    }
    /// Stack nonnegative values at matching, strictly increasing X positions.
    /// Bounds must contain zero and every cumulative total; baseline must be zero.
    pub fn stacked(mut self) -> Self {
        self.chart.stacking = Stacking::Absolute;
        self
    }
    /// Normalize each X total to 100. Requires positive totals and bounds containing 0..100.
    pub fn percent_stacked(mut self) -> Self {
        self.chart.stacking = Stacking::Percent;
        self
    }
    /// Fill alpha in 0.005..=1.0; the separate data-edge border remains opaque.
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }
    pub fn baseline(mut self, baseline: f64) -> Self {
        self.baseline = baseline;
        self
    }
    pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
        self.chart = self.chart.labels(title, x, y);
        self
    }
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.chart = self.chart.size(size);
        self
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.chart = self.chart.sketch(style);
        self
    }
    pub fn render(&self) -> Result<Scene, Error> {
        let mut chart = self.chart.clone();
        chart.marks = Marks::Area {
            baseline: self.baseline,
            opacity: self.opacity,
        };
        chart.render()
    }
}

// Shared layout/scales and explicit mark dispatch behind the Cartesian helpers.
#[derive(Clone)]
struct Chart<'a> {
    rules: Vec<crate::ReferenceRule>,
    callouts: Vec<crate::Callout>,
    intervals: Vec<crate::ShadedInterval>,
    series: Vec<NamedSeries<'a>>,
    legend: bool,
    named: bool,
    sketch: SketchStyle,
    x: Range<f64>,
    y: Range<f64>,
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    size: (u32, u32),
    marks: Marks<'a>,
    stacking: Stacking,
    automatic: bool,
    x_format: TickFormat,
    y_format: TickFormat,
    tick_count: (usize, usize),
    x_scale: AxisScale,
    y_scale: AxisScale,
    horizontal: bool,
}

#[derive(Clone)]
enum Marks<'a> {
    Line,
    Area {
        baseline: f64,
        opacity: f64,
    },
    Bars(Vec<&'a str>),
    Scatter {
        radius: u32,
        fill: bool,
        opacity: f64,
    },
}

/// Numeric scatter with one native ellipse per input point; input order is retained.
pub struct ScatterChart<'a> {
    chart: Chart<'a>,
    radius: u32,
    fill: bool,
    opacity: f64,
}

impl<'a> ScatterChart<'a> {
    /// Infer linear ranges using [`LineChart::auto_from_series`]'s padding policy.
    /// Singleton, unordered and repeated points remain individual marks. Mapped
    /// marker clearance is checked after range selection, including custom radii.
    pub fn auto(points: &'a [(f64, f64)]) -> Self {
        let mut chart = Self::new(points, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    /// Infer containing linear ranges across every named series.
    pub fn auto_from_series(series: &[NamedSeries<'a>]) -> Self {
        let mut chart = Self::from_series(series, 0.0..1.0, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    pub fn new(points: &'a [(f64, f64)], x: Range<f64>, y: Range<f64>) -> Self {
        let mut chart = Chart::new(points, x, y);
        chart.title = "Scatter chart";
        // Owned here rather than in `chart.marks`; see `AreaChart::new`.
        Self {
            chart,
            radius: 5,
            fill: true,
            opacity: 1.0,
        }
    }
    /// Uses the measured native legend route, with an ellipse swatch per series.
    pub fn from_series(series: &[NamedSeries<'a>], x: Range<f64>, y: Range<f64>) -> Self {
        let mut chart = Self::new(&[], x, y);
        chart.chart.series = series.to_vec();
        chart.chart.legend = true;
        chart.chart.named = true;
        chart
    }
    pub fn marker(mut self, radius: u32, fill: bool) -> Self {
        self.radius = radius;
        self.fill = fill;
        self
    }
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }
    pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
        self.chart = self.chart.labels(title, x, y);
        self
    }
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.chart = self.chart.size(size);
        self
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.chart = self.chart.sketch(style);
        self
    }
    pub fn render(&self) -> Result<Scene, Error> {
        let mut chart = self.chart.clone();
        chart.marks = Marks::Scatter {
            radius: self.radius,
            fill: self.fill,
            opacity: self.opacity,
        };
        chart.render()
    }
}

/// One editable rectangle per nonzero value, with a zero baseline. Named series
/// are grouped side by side by default; stacks require nonnegative values.
pub struct BarChart<'a> {
    data: BarData<'a>,
    /// Category labels in input order. `BarData::Multiple` does not carry them,
    /// so `render` reads them from here rather than re-deriving them from the
    /// mark variant it just set.
    categories: Vec<&'a str>,
    chart: Chart<'a>,
}

enum BarData<'a> {
    Single(&'a [(&'a str, f64)]),
    Multiple(Vec<NamedBarSeries<'a>>),
}

impl<'a> BarChart<'a> {
    /// Place categories top-to-bottom and values left-to-right. Labels and tick
    /// options retain their logical roles: X is category, Y is value. Category
    /// text is measured, never rotated or wrapped. Horizontal stacks are unsupported.
    pub fn horizontal(mut self) -> Self {
        self.chart.horizontal = true;
        self
    }
    /// Infer a linear Y range containing all values and zero, using
    /// [`LineChart::auto_from_series`]'s padding policy. Category order is retained.
    pub fn auto(data: &'a [(&'a str, f64)]) -> Self {
        let mut chart = Self::new(data, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    /// Infer Y across all series and zero, including cumulative endpoints when
    /// stacked or 0..100 when percent-stacked. Resolved at render time.
    pub fn auto_from_series(categories: &[&'a str], series: &[NamedBarSeries<'a>]) -> Self {
        let mut chart = Self::from_series(categories, series, 0.0..1.0);
        chart.chart.automatic = true;
        chart
    }
    pub fn new(data: &'a [(&'a str, f64)], y: Range<f64>) -> Self {
        let mut chart = Chart::new(&[], 0.0..data.len() as f64, y);
        chart.title = "Bar chart";
        chart.x_label = "Category";
        let categories: Vec<&'a str> = data.iter().map(|&(label, _)| label).collect();
        chart.marks = Marks::Bars(categories.clone());
        Self {
            data: BarData::Single(data),
            categories,
            chart,
        }
    }
    /// Side-by-side bars in category and series input order. Each series must
    /// contain exactly one finite value per category; signed values are allowed.
    pub fn from_series(
        categories: &[&'a str],
        series: &[NamedBarSeries<'a>],
        y: Range<f64>,
    ) -> Self {
        let mut chart = Chart::new(&[], 0.0..categories.len() as f64, y);
        chart.title = "Bar chart";
        chart.x_label = "Category";
        chart.legend = true;
        chart.named = true;
        let categories = categories.to_vec();
        chart.marks = Marks::Bars(categories.clone());
        Self {
            data: BarData::Multiple(series.to_vec()),
            categories,
            chart,
        }
    }
    /// Stack nonnegative values from zero; bounds must contain cumulative totals.
    pub fn stacked(mut self) -> Self {
        self.chart.stacking = Stacking::Absolute;
        self
    }
    /// Normalize each category to 100. Zero totals and negative values are rejected.
    pub fn percent_stacked(mut self) -> Self {
        self.chart.stacking = Stacking::Percent;
        self
    }
    pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
        self.chart = self.chart.labels(title, x, y);
        self
    }
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.chart = self.chart.size(size);
        self
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.chart = self.chart.sketch(style);
        self
    }
    pub fn render(&self) -> Result<Scene, Error> {
        if self.chart.horizontal && self.chart.stacking != Stacking::None {
            return Err(Error::Invalid("horizontal stacked bars are unsupported"));
        }
        let (points, names): (Vec<Vec<_>>, Vec<_>) = match &self.data {
            BarData::Single(data) => (
                vec![
                    data.iter()
                        .enumerate()
                        .map(|(i, &(_, value))| (i as f64 + 0.5, value))
                        .collect(),
                ],
                vec![("", RGBColor(25, 113, 194))],
            ),
            BarData::Multiple(series) => {
                let labels = &self.categories;
                if series.iter().any(|s| s.values.len() != labels.len()) {
                    return Err(Error::Invalid(
                        "bar series must have one value per category",
                    ));
                }
                (
                    series
                        .iter()
                        .map(|s| {
                            s.values
                                .iter()
                                .enumerate()
                                .map(|(i, &y)| (i as f64 + 0.5, y))
                                .collect()
                        })
                        .collect(),
                    series.iter().map(|s| (s.name, s.color)).collect(),
                )
            }
        };
        let mut chart = self.chart.clone();
        chart.series = points
            .iter()
            .zip(names)
            .map(|(points, (name, color))| NamedSeries::new(name, points, color.rgb()))
            .collect();
        chart.render()
    }
}

impl<'a> Chart<'a> {
    pub fn new(points: &'a [(f64, f64)], x: Range<f64>, y: Range<f64>) -> Self {
        Self {
            series: vec![NamedSeries::new("", points, (25, 113, 194))],
            rules: vec![],
            callouts: vec![],
            intervals: vec![],
            legend: false,
            named: false,
            sketch: SketchStyle::default(),
            x,
            y,
            title: "Line chart",
            x_label: "X",
            y_label: "Y",
            size: (640, 400),
            marks: Marks::Line,
            stacking: Stacking::None,
            automatic: false,
            x_format: TickFormat::Auto,
            y_format: TickFormat::Auto,
            tick_count: (TICK_COUNT, TICK_COUNT),
            x_scale: AxisScale::Linear,
            y_scale: AxisScale::Linear,
            horizontal: false,
        }
    }
    /// Named series use an explicit native legend and one inner group per series.
    pub fn from_series(series: &[NamedSeries<'a>], x: Range<f64>, y: Range<f64>) -> Self {
        let mut chart = Self::new(&[], x, y);
        chart.series = series.to_vec();
        chart.legend = true;
        chart.named = true;
        chart
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.sketch = style;
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

    /// Return a complete scene only after every drawing operation succeeds.
    pub fn render(&self) -> Result<Scene, Error> {
        if self.automatic {
            if self.x_scale != AxisScale::Linear || self.y_scale != AxisScale::Linear {
                return Err(Error::Invalid(
                    "log axes require explicit bounds; automatic ranges are linear only",
                ));
            }
            if self.series.is_empty() || self.series.iter().any(|s| s.points.is_empty()) {
                return Err(Error::Invalid("automatic ranges need nonempty series"));
            }
            if self
                .series
                .iter()
                .flat_map(|s| s.points)
                .any(|p| !p.0.is_finite() || !p.1.is_finite())
            {
                return Err(Error::Invalid("all points must be finite"));
            }
            let mut resolved = self.clone();
            if !matches!(self.marks, Marks::Bars(_)) {
                resolved.x = containing_range(
                    self.series
                        .iter()
                        .flat_map(|s| s.points.iter().map(|p| p.0)),
                )?;
            }
            let baseline = match self.marks {
                Marks::Area { baseline, .. } => Some(baseline),
                Marks::Bars(_) => Some(0.0),
                _ => None,
            };
            resolved.y = if self.stacking == Stacking::None {
                containing_range(
                    self.series
                        .iter()
                        .flat_map(|s| s.points.iter().map(|p| p.1))
                        .chain(baseline),
                )?
            } else {
                // Use the same validated cumulative geometry as the renderer,
                // before applying any axis containment checks or endpoint snap.
                let bands = self.bands(false)?;
                containing_range(
                    bands
                        .iter()
                        .flatten()
                        .flat_map(|&(lower, upper)| [lower, upper]),
                )?
            };
            resolved.automatic = false;
            return resolved.render();
        }
        self.validate()?;
        let bands = self.bands(true)?;
        let mut scene = Scene::new();
        let group = scene.drawing_options().new_group();
        group
            .scope(|| self.draw(&mut scene, &bands))
            .map_err(|error| Error::Drawing(error.to_string()))?;
        Ok(scene)
    }

    fn validate(&self) -> Result<(), Error> {
        self.frame().validate(self.tick_count)?;
        self.x_format.validate()?;
        self.y_format.validate()?;
        for (range, scale) in [(&self.x, self.x_scale), (&self.y, self.y_scale)] {
            scale.validate(range)?;
        }
        if self.series.is_empty() {
            return Err(Error::Invalid("chart needs at least one series"));
        }
        if let Marks::Area { baseline, opacity } = self.marks {
            cartesian::validate_opacity(
                opacity,
                "area opacity must round to a visible value in 1..=100 percent",
            )?;
            if !baseline.is_finite() || baseline < self.y.start || baseline > self.y.end {
                return Err(Error::Invalid(
                    "area baseline must be finite and inside Y bounds",
                ));
            }
            if self.stacking != Stacking::None && baseline != 0.0 {
                return Err(Error::Invalid("stacked areas require a zero baseline"));
            }
            for series in &self.series {
                let above = series.points.iter().any(|p| p.1 > baseline);
                let below = series.points.iter().any(|p| p.1 < baseline);
                if above == below
                    || !series.points.windows(2).any(|p| p[0].0 < p[1].0)
                    || series
                        .points
                        .windows(2)
                        .any(|p| p[0].0 == p[1].0 && p[0].1 != p[1].1)
                {
                    return Err(Error::Invalid(
                        "area needs distinct X positions and values on one side of its baseline; exact repeats are allowed",
                    ));
                }
            }
        }
        if let Marks::Scatter {
            radius, opacity, ..
        } = self.marks
        {
            cartesian::validate_scatter(radius, opacity)?;
        }
        for series in &self.series {
            if series.stroke_style.is_some() && !matches!(self.marks, Marks::Line) {
                return Err(Error::Invalid(
                    "per-series stroke style requires a line chart",
                ));
            }
            if !matches!(self.marks, Marks::Line | Marks::Scatter { .. })
                && (series.line_width.is_some()
                    || series.opacity.is_some()
                    || series.marker.is_some())
            {
                return Err(Error::Invalid(
                    "per-series styling requires a line or scatter chart",
                ));
            }
            if let Some(width) = series.line_width
                && !(1..=20).contains(&width)
            {
                return Err(Error::Invalid(
                    "series line width must be 1..=20 scene units",
                ));
            }
            if let Some(opacity) = series.opacity {
                cartesian::validate_opacity(
                    opacity,
                    "series opacity must round to a visible value in 1..=100 percent",
                )?;
            }
            if let Some((radius, _)) = series.marker {
                cartesian::validate_radius(radius)?;
            }
            if series.points.is_empty() {
                return Err(Error::Invalid("series must not be empty"));
            }
            if matches!(self.marks, Marks::Line) {
                cartesian::validate_line(series.points)?;
            }
            for &(x, y) in series.points {
                if !x.is_finite()
                    || !y.is_finite()
                    || x < self.x.start
                    || x > self.x.end
                    || (self.stacking != Stacking::Percent && (y < self.y.start || y > self.y.end))
                {
                    return Err(Error::Invalid(
                        "all points must be finite and inside the explicit bounds",
                    ));
                }
            }
            if !matches!(self.marks, Marks::Scatter { .. }) {
                cartesian::validate_order(series.points)?;
            }
        }
        if let Marks::Bars(labels) = &self.marks {
            if self.y.start > 0.0 || self.y.end < 0.0 {
                return Err(Error::Invalid("bar bounds must contain zero"));
            }
            for label in labels {
                crate::typography::measure(label, f64::from(TICK_SIZE))?;
                if label.trim().is_empty() {
                    return Err(Error::Invalid(
                        "category labels are empty or crowded; increase chart width",
                    ));
                }
            }
        }
        self.legend_width()?;
        Ok(())
    }

    fn ticks(&self) -> Result<(Ticks<f64>, Ticks<f64>), Error> {
        let (x_axis, y_axis) = self.axes();
        let ((x_count, x_format), (y_count, y_format)) = self.physical_ticks();
        let x_ticks = Ticks::select(&x_axis, x_count);
        let y_ticks = Ticks::select(&y_axis, y_count);
        if (self.x_scale == AxisScale::Log10 && x_ticks.is_empty())
            || (self.y_scale == AxisScale::Log10 && y_ticks.is_empty())
        {
            return Err(Error::Invalid(
                "log axis has no visible ticks; widen explicit bounds or increase tick density",
            ));
        }
        Ok((
            Ticks::new(x_ticks, x_count, |v| x_format.label(*v))?,
            Ticks::new(y_ticks, y_count, |v| y_format.label(*v))?,
        ))
    }

    fn layout(
        &self,
        x_ticks: &Ticks<f64>,
        y_ticks: &Ticks<f64>,
    ) -> Result<Layout, Box<dyn std::error::Error>> {
        let mut widths = [x_ticks.max_width(), y_ticks.max_width()];
        if self.horizontal
            && let Marks::Bars(labels) = &self.marks
        {
            for label in labels {
                widths[1] =
                    widths[1].max(crate::typography::measure(label, f64::from(TICK_SIZE))?.0);
            }
        }
        let layout = self.frame().layout(widths, self.legend_width()?)?;
        if let Marks::Bars(labels) = &self.marks {
            let slot = f64::from(
                if self.horizontal {
                    layout.plot_height
                } else {
                    layout.plot_width
                } - 1,
            ) / labels.len() as f64;
            for label in labels {
                let (width, height) = crate::typography::measure(label, f64::from(TICK_SIZE))?;
                if (if self.horizontal { height } else { width }) + 8.0 > slot {
                    return Err(
                        Error::Invalid("category labels are crowded; increase chart size").into(),
                    );
                }
            }
        }
        if self.legend
            && self.series.len() as u64 * u64::from(self.legend_row_height())
                > u64::from(self.size.1 - LEGEND_TOP - X_LABEL_AREA)
        {
            return Err(Error::Invalid("legend exceeds chart height").into());
        }
        Ok(layout)
    }

    fn legend_width(&self) -> Result<u32, Error> {
        if !self.legend {
            return Ok(0);
        }
        let mut width = 0.0_f64;
        for series in &self.series {
            if series.name.trim().is_empty() {
                return Err(Error::Invalid("legend series names must not be empty"));
            }
            width = width.max(crate::typography::measure(series.name, f64::from(TICK_SIZE))?.0);
        }
        if width > f64::from(self.size.0) {
            return Err(Error::Invalid("legend label exceeds chart width"));
        }
        Ok(crate::cartesian::legend_width(
            width,
            self.legend_label_gap(),
            self.legend_swatch_width(),
        ))
    }

    fn legend_marker_extent(&self) -> u32 {
        self.series
            .iter()
            .filter_map(|s| self.marker_extent(s))
            .max()
            .unwrap_or(0)
    }

    fn legend_swatch_width(&self) -> u32 {
        28.max(2 * self.legend_marker_extent())
    }

    fn legend_row_height(&self) -> u32 {
        LEGEND_ROW_HEIGHT.max(2 * self.legend_marker_extent() + 8)
    }

    fn legend_label_gap(&self) -> u32 {
        if matches!(self.marks, Marks::Line) {
            // Preserve the default gap; wider round caps need additional space.
            8.max(
                2 + self
                    .series
                    .iter()
                    .map(|s| self.series_style(s).stroke_width.div_ceil(2))
                    .max()
                    .unwrap_or(0),
            )
        } else {
            8
        }
    }

    // Compute data-space boundaries once, then use the same geometry for
    // pixel-resolution validation and drawing. Never rely on Plotters clipping.
    fn bands(&self, check_bounds: bool) -> Result<Bands, Error> {
        if !matches!(self.marks, Marks::Area { .. } | Marks::Bars(_)) {
            return Ok(vec![]);
        }
        let baseline = match self.marks {
            Marks::Area { baseline, .. } => baseline,
            _ => 0.0,
        };
        if self.stacking == Stacking::None {
            return Ok(self
                .series
                .iter()
                .map(|s| s.points.iter().map(|p| (baseline, p.1)).collect())
                .collect());
        }
        let first = &self.series[0].points;
        if first.windows(2).any(|p| p[0].0 >= p[1].0)
            || self.series.iter().any(|s| {
                s.points.len() != first.len()
                    || s.points
                        .iter()
                        .zip(first.iter())
                        .any(|(p, q)| p.0 != q.0 || p.1 < 0.0)
            })
        {
            return Err(Error::Invalid(
                "stacks need nonnegative values at identical strictly increasing X positions",
            ));
        }
        if check_bounds
            && (self.y.start > 0.0
                || self.y.end < 0.0
                || (self.stacking == Stacking::Percent && self.y.end < 100.0))
        {
            return Err(Error::Invalid(
                "stack bounds must contain zero and normalized stacks must contain 100",
            ));
        }
        let mut totals = vec![0.0; first.len()];
        for series in &self.series {
            for (total, p) in totals.iter_mut().zip(series.points) {
                *total += p.1;
            }
        }
        if totals.iter().any(|&total| {
            !total.is_finite()
                || (self.stacking == Stacking::Percent && total <= 0.0)
                || (self.stacking == Stacking::Absolute
                    && check_bounds
                    && total > self.y.end
                    && total - self.y.end > f64::EPSILON * self.series.len() as f64 * total.abs())
        }) {
            return Err(Error::Invalid(
                "stack totals must be finite and in bounds; normalized totals must be positive",
            ));
        }
        let mut cumulative = vec![0.0; first.len()];
        self.series
            .iter()
            .map(|series| {
                series
                    .points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let lower = cumulative[i];
                        cumulative[i] += p.1;
                        let scale = |value: f64| {
                            if self.stacking == Stacking::Percent {
                                value / totals[i] * 100.0
                            } else if check_bounds {
                                // Totals were checked above. Snap only accumulated
                                // floating-point noise at the explicit upper bound.
                                value.min(self.y.end)
                            } else {
                                value
                            }
                        };
                        let band = (scale(lower), scale(cumulative[i]));
                        if p.1 > 0.0 && band.0 == band.1 {
                            return Err(Error::Invalid(
                                "nonzero stack segment collapses at floating-point resolution",
                            ));
                        }
                        Ok(band)
                    })
                    .collect()
            })
            .collect()
    }

    fn bar_corners(&self, series: usize, index: usize, band: (f64, f64)) -> [(f64, f64); 2] {
        let x = self.series[series].points[index].0;
        let corners = if self.stacking != Stacking::None || self.series.len() == 1 {
            [(x - 0.35, band.0), (x + 0.35, band.1)]
        } else {
            let slot = 0.7 / self.series.len() as f64;
            let left = x - 0.35 + series as f64 * slot;
            [(left + slot * 0.08, band.0), (left + slot * 0.92, band.1)]
        };
        corners.map(|(category, value)| {
            if self.horizontal {
                (value, category)
            } else {
                (category, value)
            }
        })
    }

    fn axes(&self) -> (crate::axis::NumericCoord, crate::axis::NumericCoord) {
        let (x, y) = self.physical_ranges();
        // Horizontal orientation is exposed only for linear bar charts.
        (self.x_scale.coordinate(x), self.y_scale.coordinate(y))
    }

    fn physical_ranges(&self) -> (Range<f64>, Range<f64>) {
        if self.horizontal {
            (self.y.clone(), self.x.end..self.x.start)
        } else {
            (self.x.clone(), self.y.clone())
        }
    }

    fn physical_labels(&self) -> (&str, &str) {
        if self.horizontal {
            (self.y_label, self.x_label)
        } else {
            (self.x_label, self.y_label)
        }
    }

    fn frame(&self) -> Frame<'_> {
        let (x_label, y_label) = self.physical_labels();
        Frame {
            title: self.title,
            x_label,
            y_label,
            size: self.size,
        }
    }

    fn physical_ticks(&self) -> ((usize, TickFormat), (usize, TickFormat)) {
        let x = (
            if matches!(self.marks, Marks::Bars(_)) {
                0
            } else {
                self.tick_count.0
            },
            self.x_format,
        );
        let y = (self.tick_count.1, self.y_format);
        if self.horizontal { (y, x) } else { (x, y) }
    }

    fn series_style(&self, series: &NamedSeries<'_>) -> ShapeStyle {
        let (width, opacity) = match self.marks {
            Marks::Scatter { opacity, .. } => (2, opacity),
            _ => (3, 1.0),
        };
        series
            .color
            .mix(series.opacity.unwrap_or(opacity))
            .stroke_width(series.line_width.unwrap_or(width))
    }

    fn series_marker(&self, series: &NamedSeries<'_>) -> Option<(u32, bool)> {
        match self.marks {
            Marks::Line => series.marker,
            Marks::Scatter { radius, fill, .. } => Some(series.marker.unwrap_or((radius, fill))),
            _ => None,
        }
    }

    fn marker_extent(&self, series: &NamedSeries<'_>) -> Option<u32> {
        self.series_marker(series).map(|(radius, fill)| {
            radius
                + if fill {
                    0
                } else {
                    self.series_style(series).stroke_width.div_ceil(2)
                }
        })
    }

    fn draw(&self, scene: &mut Scene, bands: &Bands) -> Result<(), Box<dyn std::error::Error>> {
        let options = scene.drawing_options();
        let groups: Vec<DrawingGroup> = self.series.iter().map(|_| options.new_group()).collect();
        let (x_ticks, y_ticks) = self.ticks()?;
        let layout = self.layout(&x_ticks, &y_ticks)?;
        let (x_range, y_range) = self.physical_ranges();
        let frame = self.frame();
        let legend_width = layout.legend;
        let root = ExcalidrawBackend::new(scene, self.size)?.into_drawing_area();
        root.fill(&WHITE)?;
        options.with_style(self.sketch, || {
            let (x_axis, y_axis) = self.axes();
            let mut chart = frame.build(&root, &layout, x_axis, y_axis)?;
            frame.check_ticks(&chart, &layout, &x_ticks, &y_ticks)?;
            let rules = self.rules.iter().map(|rule| rule.map(&chart)).collect::<Result<Vec<_>, _>>()?;
            let callouts = self.callouts.iter().map(|callout| callout.map(&chart, self.size)).collect::<Result<Vec<_>, _>>()?;
            let intervals = self.intervals.iter().map(|interval| interval.map(&chart)).collect::<Result<Vec<_>, _>>()?;
            for (series_index, series) in self.series.iter().enumerate() {
                if let Marks::Area { .. } = self.marks {
                    let mapped: Vec<_> = series.points.iter().zip(&bands[series_index]).map(|(p, &(lower, upper))| {
                        let (x, lower) = chart.plotting_area().map_coordinate(&(p.0, lower));
                        let (_, upper) = chart.plotting_area().map_coordinate(&(p.0, upper));
                        (x, lower != upper)
                    }).collect();
                    if !mapped.windows(2).any(|p| p[0].0 != p[1].0 && (p[0].1 || p[1].1)) {
                        return Err(Error::Invalid("area collapses at this pixel resolution").into());
                    }
                }
                if let Some(mut extent) = self.marker_extent(series) {
                    // Keep the established chart-wide scatter clearance policy
                    // unless the series explicitly changes marker geometry.
                    if let Marks::Scatter { radius, .. } = self.marks
                        && series.marker.is_none() && series.line_width.is_none()
                    {
                        extent = radius;
                    }
                    let (left, bottom) = chart.plotting_area().map_coordinate(&(self.x.start, self.y.start));
                    let (right, top) = chart.plotting_area().map_coordinate(&(self.x.end, self.y.end));
                    cartesian::check_markers(series.points.iter().map(|p| chart.plotting_area().map_coordinate(p)), extent, (left, bottom), (right, top))?;
                }
                if matches!(self.marks, Marks::Bars(_)) {
                    for (index, &band) in bands[series_index].iter().enumerate() {
                        let [a, b] = self.bar_corners(series_index, index, band).map(|p| chart.plotting_area().map_coordinate(&p));
                        let (category, value) = if self.horizontal { ((a.1, b.1), (a.0, b.0)) } else { ((a.0, b.0), (a.1, b.1)) };
                        if self.horizontal && series_index > 0 {
                            let previous = self.bar_corners(series_index - 1, index, band)
                                .map(|p| chart.plotting_area().map_coordinate(&p));
                            if a.1.min(b.1) <= previous[0].1.max(previous[1].1) {
                                return Err(Error::Invalid("grouped bar slots are crowded at this pixel resolution; increase chart height").into());
                            }
                        }
                        if category.0 == category.1 || cartesian::collapsed(band, value) {
                            return Err(
                                Error::Invalid("bar collapses at this pixel resolution").into()
                            );
                        }
                    }
                }
                if !matches!(self.marks, Marks::Line) {
                    continue;
                }
                cartesian::check_line(series.points.iter().map(|p| chart.plotting_area().map_coordinate(p)))?;
            }
            for interval in intervals { interval.draw(&root, &options)?; }
            frame.draw_axes(&root, &mut chart, &x_ticks, &y_ticks)?;
            let (left, bottom) = chart
                .plotting_area()
                .map_coordinate(&(x_range.start, y_range.start));
            let (right, top) = chart
                .plotting_area()
                .map_coordinate(&(x_range.end, y_range.end));
            for (series_index, (series, group)) in self.series.iter().zip(&groups).enumerate() {
                let draw = || -> Result<(), Box<dyn std::error::Error>> { match &self.marks {
                    Marks::Line => {
                        options.with_stroke_style(series.stroke_style.unwrap_or_default(), || {
                            cartesian::draw_line(&mut chart, series.points, self.series_style(series))
                        })?;
                    }
                    Marks::Area { baseline, opacity } if self.stacking == Stacking::None => chart
                        .draw_series(AreaSeries::new(
                            series.points.iter().copied(), *baseline, series.color.mix(*opacity),
                        ).border_style(series.color.stroke_width(3)))
                        .map(|_| ())?,
                    Marks::Area { opacity, .. } => {
                        let upper: Vec<_> = series.points.iter().zip(&bands[series_index]).map(|(p, band)| (p.0, band.1)).collect();
                        let mut polygon = upper.clone();
                        polygon.extend(series.points.iter().zip(&bands[series_index]).rev().map(|(p, band)| (p.0, band.0)));
                        chart.draw_series(std::iter::once(Polygon::new(polygon, series.color.mix(*opacity).filled())))?;
                        chart.draw_series(LineSeries::new(upper, series.color.stroke_width(3)))?;
                    }
                    Marks::Bars(_) => chart
                        .draw_series(bands[series_index].iter().enumerate().map(|(index, &band)| {
                            Rectangle::new(self.bar_corners(series_index, index, band), series.color.filled())
                        }))
                        .map(|_| ())?,
                    Marks::Scatter { .. } => {}
                };
                if let Some((radius, fill)) = self.series_marker(series) {
                    cartesian::draw_markers(&mut chart, series.points, radius,
                        ShapeStyle { filled: fill, ..self.series_style(series) })?;
                }
                Ok(()) };
                if self.named {
                    group.scope(draw)?;
                } else {
                    let mut draw = draw;
                    draw()?;
                }
            }
            if let Marks::Bars(labels) = &self.marks {
                use plotters_backend::text_anchor::{HPos, Pos, VPos};
                let style = TextStyle::from(("Excalifont", TICK_SIZE).into_font())
                    .pos(if self.horizontal { Pos::new(HPos::Right, VPos::Center) } else { Pos::new(HPos::Center, VPos::Top) });
                for (i, label) in labels.iter().enumerate() {
                    let point = if self.horizontal { (self.y.start, i as f64 + 0.5) } else { (i as f64 + 0.5, self.y.start) };
                    let (x, y) = chart
                        .plotting_area()
                        .map_coordinate(&point);
                    root.draw_text(label, &style, if self.horizontal { (left - 10, y) } else { (x, bottom + 10) })?;
                }
                if self.horizontal {
                    let (zero, _) = chart.plotting_area().map_coordinate(&(0.0, self.x.start));
                    if zero != left {
                        root.draw(&PathElement::new([(zero, top), (zero, bottom)], BLACK))?;
                    }
                } else {
                    let (_, zero) = chart.plotting_area().map_coordinate(&(self.x.start, 0.0));
                    if zero != bottom {
                        root.draw(&PathElement::new([(left, zero), (right, zero)], BLACK))?;
                    }
                }
            }
            for rule in rules { rule.draw(&root, &options)?; }
            for callout in callouts { root.draw(&callout)?; }
            // Legend is deliberately emitted later, re-entering the explicit group.
            if self.legend {
                let x = cartesian::legend_left(self.size.0, legend_width);
                for (index, (series, group)) in self.series.iter().zip(&groups).enumerate() {
                    let row_height = self.legend_row_height();
                    let y = (LEGEND_TOP + index as u32 * row_height) as i32;
                    let center_y = y + (row_height - 10) as i32 / 2;
                    let swatch_width = self.legend_swatch_width() as i32;
                    group.scope(|| -> Result<(), Box<dyn std::error::Error>> {
                        if matches!(self.marks, Marks::Area { .. } | Marks::Bars(_)) {
                            let opacity = match self.marks { Marks::Area { opacity, .. } => opacity, _ => 1.0 };
                            root.draw(&Rectangle::new([(x, y + 3), (x + 28, y + 17)], series.color.mix(opacity).filled()))?;
                        } else if matches!(self.marks, Marks::Line) {
                            options.with_stroke_style(series.stroke_style.unwrap_or_default(), || root.draw(&PathElement::new(
                                [(x, center_y), (x + swatch_width, center_y)],
                                self.series_style(series),
                            )))?;
                        }
                        if let Some((radius, fill)) = self.series_marker(series) {
                            root.draw(&Circle::new((x + swatch_width / 2, center_y), radius,
                                ShapeStyle { filled: fill, ..self.series_style(series) }))?;
                        }
                        let style = TextStyle::from(("Excalifont", TICK_SIZE).into_font())
                            .color(&series.color);
                        root.draw_text(series.name, &style, (x + swatch_width + self.legend_label_gap() as i32, center_y - 10))?;
                        Ok(())
                    })?;
                }
            }
            root.present()?;
            Ok(())
        })
    }
}

fn containing_range(values: impl Iterator<Item = f64>) -> Result<Range<f64>, Error> {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for value in values {
        if !value.is_finite() || value.abs() > 1e9 {
            return Err(Error::Invalid(
                "automatic linear ranges require finite values within ±1e9",
            ));
        }
        min = min.min(value);
        max = max.max(value);
    }
    if min > max {
        return Err(Error::Invalid(
            "automatic ranges need at least one observation",
        ));
    }
    let padding = if min == max {
        (min.abs() * 0.05).max(1.0)
    } else {
        ((max - min) * 0.05).max(1e-6)
    };
    let range = (min - padding).max(-1e9)..(max + padding).min(1e9);
    if range.end - range.start < 1e-6 {
        return Err(Error::Invalid(
            "cannot represent a containing automatic range with span >= 1e-6",
        ));
    }
    Ok(range)
}
