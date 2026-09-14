use crate::cartesian::{self, Frame, Ticks};
use crate::{Error, ExcalidrawBackend, Scene, SketchStyle, TickFormat};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use plotters::coord::ranged1d::Ranged;
use plotters::coord::types::RangedDate;
use plotters::prelude::*;
use std::ops::Range;

/// Calendar-date line with explicit containing bounds and linear numeric Y.
/// Dates use elapsed-day spacing, not observation indexes. Input must be in
/// nondecreasing X order; duplicate dates and exact repeated points are retained
/// subject to native path normalization. At least two distinct mapped points are
/// required. Bounds are inclusive for containment despite Rust's `Range` syntax.
pub struct DateLineChart<'a> {
    chart: CalendarChart<'a, NaiveDate>,
}

/// Calendar-date scatter. Each observation emits one native ellipse, including
/// duplicates, in input order. See [`DateLineChart`] for bounds and date semantics.
pub struct DateScatterChart<'a> {
    chart: CalendarChart<'a, NaiveDate>,
}

/// UTC-instant line with explicit containing bounds and linear numeric Y.
/// No local-time or DST inference is performed. See [`DateLineChart`] for ordering.
/// Subsecond inputs are accepted, but scene coordinates have integer resolution.
/// Leap-second representations are rejected; UTC labels use a literal `Z`.
pub struct UtcLineChart<'a> {
    chart: CalendarChart<'a, DateTime<Utc>>,
}

/// UTC-instant scatter preserving unordered and duplicate observations as native
/// ellipses. See [`UtcLineChart`] for time and resolution semantics.
pub struct UtcScatterChart<'a> {
    chart: CalendarChart<'a, DateTime<Utc>>,
}

macro_rules! calendar_chart {
    ($name:ident, $value:ty, $scatter:expr) => {
        impl<'a> $name<'a> {
            /// Bounds must increase, contain every point and lie in years 1..=9999.
            /// Spans must fit Chrono's signed nanoseconds (about 292 years).
            /// Y bounds follow the numeric linear contract: ±1e9, span >= 1e-6.
            pub fn new(points: &'a [($value, f64)], x: Range<$value>, y: Range<f64>) -> Self {
                Self {
                    chart: CalendarChart {
                        points,
                        x,
                        y,
                        scatter: $scatter,
                        radius: 5,
                        fill: true,
                        opacity: 1.,
                        title: if $scatter {
                            "Calendar scatter"
                        } else {
                            "Calendar line"
                        },
                        x_label: "Date",
                        y_label: "Y",
                        size: (900, 400),
                        sketch: SketchStyle::default(),
                        tick_count: (6, 6),
                        y_format: TickFormat::Auto,
                        rules: vec![],
                        callouts: vec![],
                        intervals: vec![],
                    },
                }
            }
            pub fn labels(mut self, title: &'a str, x: &'a str, y: &'a str) -> Self {
                self.chart.title = title;
                self.chart.x_label = x;
                self.chart.y_label = y;
                self
            }
            pub fn size(mut self, size: (u32, u32)) -> Self {
                self.chart.size = size;
                self
            }
            pub fn sketch(mut self, style: SketchStyle) -> Self {
                self.chart.sketch = style;
                self
            }
            /// Desired major ticks per axis, 2..=20. Plotters selects calendar
            /// key points; reduce density or enlarge the chart if labels crowd.
            pub fn tick_density(mut self, x: usize, y: usize) -> Self {
                self.chart.tick_count = (x, y);
                self
            }
            pub fn y_tick_format(mut self, format: TickFormat) -> Self {
                self.chart.y_format = format;
                self
            }
            /// Add a full-span typed rule after data, with its own inner group.
            pub fn reference_rule(mut self, rule: crate::ReferenceRule<$value>) -> Self {
                self.chart.rules.push(rule);
                self
            }
            /// Add a typed, noncollapsed window before axes/data, in input order.
            pub fn shaded_interval(mut self, interval: crate::ShadedInterval<$value>) -> Self {
                self.chart.intervals.push(interval);
                self
            }
            /// Add a typed anchor with an explicitly offset note/arrow pair after rules.
            pub fn callout(mut self, callout: crate::Callout<$value>) -> Self {
                self.chart.callouts.push(callout);
                self
            }
            pub fn render(&self) -> Result<Scene, Error> {
                self.chart.render()
            }
        }
    };
}
calendar_chart!(DateLineChart, NaiveDate, false);
calendar_chart!(DateScatterChart, NaiveDate, true);
calendar_chart!(UtcLineChart, DateTime<Utc>, false);
calendar_chart!(UtcScatterChart, DateTime<Utc>, true);

macro_rules! scatter_options {
    ($name:ident) => {
        impl $name<'_> {
            /// Native marker radius 1..=20; full markers must fit the plot's
            /// five-unit halo. Widen bounds for large endpoint markers.
            pub fn marker(mut self, radius: u32, fill: bool) -> Self {
                self.chart.radius = radius;
                self.chart.fill = fill;
                self
            }
            /// Visible opacity 0.005..=1.0.
            pub fn opacity(mut self, opacity: f64) -> Self {
                self.chart.opacity = opacity;
                self
            }
        }
    };
}
scatter_options!(DateScatterChart);
scatter_options!(UtcScatterChart);

trait CalendarValue:
    crate::annotations::CoordinateValue + Copy + Ord + std::fmt::Debug + 'static
{
    type Coord: Ranged<ValueType = Self, FormatOption = plotters::coord::ranged1d::DefaultFormatting>;
    fn coordinate(range: Range<Self>) -> Self::Coord;
    fn valid(self) -> bool;
    fn span(start: Self, end: Self) -> Option<i64>;
    fn label(self, span: i64) -> String;
}

impl CalendarValue for NaiveDate {
    type Coord = RangedDate<Self>;
    fn coordinate(range: Range<Self>) -> Self::Coord {
        range.into()
    }
    fn valid(self) -> bool {
        (1..=9999).contains(&self.year())
    }
    fn span(start: Self, end: Self) -> Option<i64> {
        (end - start).num_nanoseconds()
    }
    fn label(self, _: i64) -> String {
        self.format("%Y-%m-%d").to_string()
    }
}

impl crate::annotations::CoordinateValue for NaiveDate {
    fn valid(&self) -> bool {
        CalendarValue::valid(*self)
    }
}

impl crate::annotations::CoordinateValue for DateTime<Utc> {
    fn valid(&self) -> bool {
        CalendarValue::valid(*self)
    }
}

impl CalendarValue for DateTime<Utc> {
    type Coord = plotters::coord::types::RangedDateTime<Self>;
    fn coordinate(range: Range<Self>) -> Self::Coord {
        range.into()
    }
    fn valid(self) -> bool {
        (1..=9999).contains(&self.year()) && self.timestamp_subsec_nanos() < 1_000_000_000
    }
    fn span(start: Self, end: Self) -> Option<i64> {
        (end - start).num_nanoseconds()
    }
    fn label(self, span: i64) -> String {
        // Include the calendar date even on sub-day ranges crossing midnight.
        let format = if span >= 2 * 86_400_000_000_000 {
            "%Y-%m-%dZ"
        } else if span >= 60_000_000_000 {
            "%Y-%m-%d %H:%M:%SZ"
        } else {
            "%Y-%m-%d %H:%M:%S%.9fZ"
        };
        self.format(format).to_string()
    }
}

struct CalendarChart<'a, X> {
    rules: Vec<crate::ReferenceRule<X>>,
    callouts: Vec<crate::Callout<X>>,
    intervals: Vec<crate::ShadedInterval<X>>,
    points: &'a [(X, f64)],
    x: Range<X>,
    y: Range<f64>,
    scatter: bool,
    radius: u32,
    fill: bool,
    opacity: f64,
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    size: (u32, u32),
    sketch: SketchStyle,
    tick_count: (usize, usize),
    y_format: TickFormat,
}

impl<X: CalendarValue> CalendarChart<'_, X> {
    fn frame(&self) -> Frame<'_> {
        Frame {
            title: self.title,
            x_label: self.x_label,
            y_label: self.y_label,
            size: self.size,
        }
    }

    fn validate(&self) -> Result<i64, Error> {
        self.frame().validate(self.tick_count)?;
        self.y_format.validate()?;
        if !self.x.start.valid() || !self.x.end.valid() || self.x.start >= self.x.end {
            return Err(Error::Invalid(
                "calendar bounds must increase within years 1..=9999; leap seconds are unsupported",
            ));
        }
        let span = X::span(self.x.start, self.x.end)
            .filter(|&span| span > 0)
            .ok_or(Error::Invalid(
                "calendar span must fit positive signed nanoseconds (about 292 years)",
            ))?;
        crate::AxisScale::Linear.validate(&self.y)?;
        if self.points.is_empty() {
            return Err(Error::Invalid("series must not be empty"));
        }
        if self.points.iter().any(|&(x, y)| {
            !x.valid()
                || x < self.x.start
                || x > self.x.end
                || !y.is_finite()
                || y < self.y.start
                || y > self.y.end
        }) {
            return Err(Error::Invalid(
                "all points must be representable and inside the explicit bounds",
            ));
        }
        if !self.scatter {
            cartesian::validate_line(self.points)?;
            cartesian::validate_order(self.points)?;
        } else {
            cartesian::validate_scatter(self.radius, self.opacity)?;
        }
        Ok(span)
    }

    fn render(&self) -> Result<Scene, Error> {
        let span = self.validate()?;
        let x_coord = X::coordinate(self.x.clone());
        let y_coord = crate::AxisScale::Linear.coordinate(self.y.clone());
        let x_ticks = Ticks::select(&x_coord, self.tick_count.0);
        let y_ticks = Ticks::select(&y_coord, self.tick_count.1);
        if x_ticks.is_empty() || y_ticks.is_empty() {
            return Err(Error::Invalid(
                "calendar axis has no visible ticks; widen bounds",
            ));
        }
        // A multi-day range can still receive sub-day key points. Choose label
        // precision from actual tick spacing, not only the enclosing range.
        let span = x_ticks
            .windows(2)
            .filter_map(|p| X::span(p[0], p[1]))
            .min()
            .map_or(span, |spacing| span.min(spacing));
        let x_ticks = Ticks::new(x_ticks, self.tick_count.0, |x| x.label(span))?;
        let y_ticks = Ticks::new(y_ticks, self.tick_count.1, |y| self.y_format.label(*y))?;
        let frame = self.frame();
        let mut scene = Scene::new();
        let options = scene.drawing_options();
        let group = options.new_group();
        let draw = || -> Result<(), Box<dyn std::error::Error>> {
            let layout = frame.layout([x_ticks.max_width(), y_ticks.max_width()], 0)?;
            let root = ExcalidrawBackend::new(&mut scene, self.size)?.into_drawing_area();
            root.fill(&WHITE)?;
            let mut chart = frame.build(&root, &layout, x_coord, y_coord)?;
            frame.check_ticks(&chart, &layout, &x_ticks, &y_ticks)?;
            let rules = self
                .rules
                .iter()
                .map(|rule| rule.map(&chart))
                .collect::<Result<Vec<_>, _>>()?;
            let intervals = self
                .intervals
                .iter()
                .map(|interval| interval.map(&chart))
                .collect::<Result<Vec<_>, _>>()?;
            let callouts = self
                .callouts
                .iter()
                .map(|callout| callout.map(&chart, self.size))
                .collect::<Result<Vec<_>, _>>()?;
            let map = |point: &(X, f64)| chart.plotting_area().map_coordinate(point);
            if self.scatter {
                cartesian::check_markers(
                    self.points.iter().map(map),
                    self.radius,
                    map(&(self.x.start, self.y.start)),
                    map(&(self.x.end, self.y.end)),
                )?;
            } else {
                cartesian::check_line(self.points.iter().map(map))?;
            }
            for interval in intervals {
                interval.draw(&root, &options)?;
            }
            frame.draw_axes(&root, &mut chart, &x_ticks, &y_ticks)?;
            let color = RGBColor(25, 113, 194);
            if self.scatter {
                cartesian::draw_scatter(
                    &mut chart,
                    self.points,
                    color,
                    self.radius,
                    self.fill,
                    self.opacity,
                )?;
            } else {
                cartesian::draw_line(&mut chart, self.points, color.stroke_width(3))?;
            }
            for rule in rules {
                rule.draw(&root, &options)?;
            }
            for callout in callouts {
                root.draw(&callout)?;
            }
            root.present()?;
            Ok(())
        };
        group
            .scope(|| options.with_style(self.sketch, draw))
            .map_err(|e| Error::Drawing(e.to_string()))?;
        Ok(scene)
    }
}
