use crate::{DrawingOptions, Error, ExcalidrawBackend, StrokeStyle};
use plotters::coord::Shift;
use plotters::prelude::*;
use std::ops::Range;

/// An explicit single-line note and straight unbound arrow pointing to a data anchor.
/// `offset` places the note's top-left relative to the mapped anchor in scene units.
/// The leader starts at the point on the note's measured edge nearest the anchor.
/// The anchor must lie outside the note. There is no routing around other elements,
/// attachment or layout reservation. Complete note/head/stroke bounds
/// must fit the chart canvas. Callouts paint after rules, before legends, in input
/// order; each arrow then note pair shares one independent inner chart group.
#[derive(Clone)]
pub struct Callout<X = f64> {
    anchor: (X, f64),
    text: String,
    offset: (f64, f64),
    font_size: f64,
    arrow: crate::ArrowStyle,
}

impl<X> Callout<X> {
    /// Default: 20-unit Excalifont note and opaque dark width-2 open arrowhead.
    pub fn new(anchor: (X, f64), text: impl Into<String>, offset: (f64, f64)) -> Self {
        Self {
            anchor,
            text: text.into(),
            offset,
            font_size: 20.,
            arrow: crate::ArrowStyle::default(),
        }
    }

    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self
    }

    /// Override leader paint/head; text remains opaque dark Excalifont.
    pub fn arrow_style(mut self, style: crate::ArrowStyle) -> Self {
        self.arrow = style;
        self
    }

    pub(crate) fn map<A: Ranged<ValueType = X>, B: Ranged<ValueType = f64>>(
        &self,
        chart: &ChartContext<'_, ExcalidrawBackend<'_>, Cartesian2d<A, B>>,
        canvas: (u32, u32),
    ) -> Result<MappedCallout, Error>
    where
        X: CoordinateValue,
    {
        let anchor = map_point(chart, &self.anchor)?;
        if !self.offset.0.is_finite() || !self.offset.1.is_finite() {
            return Err(Error::Invalid("callout offset must be finite"));
        }
        let size = crate::typography::measure(&self.text, self.font_size)?;
        let mapped = MappedCallout {
            anchor: (f64::from(anchor.0), f64::from(anchor.1)),
            position: (
                f64::from(anchor.0) + self.offset.0,
                f64::from(anchor.1) + self.offset.1,
            ),
            text: self.text.clone(),
            font_size: self.font_size,
            width: size.0,
            height: size.1,
            arrow: self.arrow,
        };
        // Validate through the same scene emission route before publishing chart output.
        let mut probe = crate::Scene::with_namespace("callout-validation");
        mapped.emit(&mut probe)?;
        let b = probe.bounds()?.unwrap();
        if b.x < 0.
            || b.y < 0.
            || b.x + b.width > f64::from(canvas.0)
            || b.y + b.height > f64::from(canvas.1)
        {
            return Err(Error::Invalid(
                "complete callout label and arrow must fit the chart canvas",
            ));
        }
        Ok(mapped)
    }
}

pub(crate) struct MappedCallout {
    anchor: (f64, f64),
    position: (f64, f64),
    text: String,
    font_size: f64,
    width: f64,
    height: f64,
    arrow: crate::ArrowStyle,
}

impl MappedCallout {
    pub(crate) fn emit(&self, scene: &mut crate::Scene) -> Result<(), Error> {
        // Project onto the measured text rectangle so the leader leaves the
        // label toward the anchor rather than passing through its interior.
        let start = (
            self.anchor
                .0
                .clamp(self.position.0, self.position.0 + self.width),
            self.anchor
                .1
                .clamp(self.position.1, self.position.1 + self.height),
        );
        scene.drawing_options().new_group().scope(|| {
            scene.add_arrow(start, self.anchor, self.arrow)?;
            scene.add_note(&self.text, self.position, self.font_size)
        })
    }
}

impl<'a> plotters::element::PointCollection<'a, (i32, i32)> for &'a MappedCallout {
    type Point = &'a (i32, i32);
    type IntoIter = std::iter::Empty<Self::Point>;
    fn point_iter(self) -> Self::IntoIter {
        std::iter::empty()
    }
}

impl plotters::element::Drawable<ExcalidrawBackend<'_>> for MappedCallout {
    fn draw<I: Iterator<Item = (i32, i32)>>(
        &self,
        _: I,
        backend: &mut ExcalidrawBackend<'_>,
        _: (u32, u32),
    ) -> Result<(), plotters_backend::DrawingErrorKind<Error>> {
        self.emit(backend.scene_mut())
            .map_err(plotters_backend::DrawingErrorKind::DrawingError)
    }
}

/// A borderless full-height X window or full-width Y window behind axes/data.
/// Endpoints must increase, fit chart bounds, and map to nonzero native area.
/// Available on numeric line/scatter/area and date/UTC helpers. `X` is their X
/// coordinate type. Both endpoints are included. Windows never expand automatic
/// bounds or reserve layout space. Each has an independent inner chart group and
/// inherits the chart's sketch/fill style; overlapping windows paint in input order.
#[derive(Clone)]
pub struct ShadedInterval<X = f64> {
    position: IntervalPosition<X>,
    rgb: (u8, u8, u8),
    opacity: f64,
}

#[derive(Clone)]
enum IntervalPosition<X> {
    X(Range<X>),
    Y(Range<f64>),
}

impl<X> ShadedInterval<X> {
    /// Shade this X interval across all Y values. Default alpha: 0.2.
    pub fn x(range: Range<X>, rgb: (u8, u8, u8)) -> Self {
        Self {
            position: IntervalPosition::X(range),
            rgb,
            opacity: 0.2,
        }
    }

    /// Shade this Y interval across all X values. Default alpha: 0.2.
    pub fn y(range: Range<f64>, rgb: (u8, u8, u8)) -> Self {
        Self {
            position: IntervalPosition::Y(range),
            rgb,
            opacity: 0.2,
        }
    }

    /// Alpha in 0.005..=1.0, rounded to native integer percent.
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    pub(crate) fn map<A: Ranged<ValueType = X>, B: Ranged<ValueType = f64>>(
        &self,
        chart: &ChartContext<'_, ExcalidrawBackend<'_>, Cartesian2d<A, B>>,
    ) -> Result<MappedInterval, Error>
    where
        X: CoordinateValue,
    {
        crate::cartesian::validate_opacity(
            self.opacity,
            "annotation opacity must be 0.005..=1.0 (visible native percent)",
        )?;
        let x = chart.as_coord_spec().x_spec().range();
        let y = chart.as_coord_spec().y_spec().range();
        let (x, y) = match &self.position {
            IntervalPosition::X(range) => (range.clone(), y),
            IntervalPosition::Y(range) => (x, range.clone()),
        };
        if x.start.partial_cmp(&x.end) != Some(std::cmp::Ordering::Less)
            || y.start.partial_cmp(&y.end) != Some(std::cmp::Ordering::Less)
        {
            return Err(Error::Invalid(
                "annotation interval endpoints must be ordered and distinct",
            ));
        }
        let points = [
            map_point(chart, &(x.start, y.start))?,
            map_point(chart, &(x.end, y.end))?,
        ];
        if points[0].0 == points[1].0 || points[0].1 == points[1].1 {
            return Err(Error::Invalid(
                "annotation interval collapses at this pixel resolution",
            ));
        }
        Ok(MappedInterval {
            points,
            paint: RGBColor(self.rgb.0, self.rgb.1, self.rgb.2)
                .mix(self.opacity)
                .filled(),
        })
    }
}

pub(crate) struct MappedInterval {
    points: [(i32, i32); 2],
    paint: ShapeStyle,
}

impl MappedInterval {
    pub(crate) fn draw(
        &self,
        root: &DrawingArea<ExcalidrawBackend<'_>, Shift>,
        options: &DrawingOptions,
    ) -> Result<(), Box<dyn std::error::Error>> {
        options
            .new_group()
            .scope(|| root.draw(&Rectangle::new(self.points, self.paint)))?;
        Ok(())
    }
}

/// A full-span data-coordinate reference line. `X` is the chart's X value type.
/// Coordinates include both range endpoints. Paint is checked at render time.
/// Available on numeric line/scatter/area and date/UTC helpers. Rules paint in
/// input order after data and before legends, each in its own inner chart group.
/// They never expand automatic bounds or reserve layout space. The centerline
/// spans mapped bounds; its stroke can extend half the width beyond those bounds.
/// Sketch roughness applies. Editing is one-way, with no data binding or labels.
#[derive(Clone)]
pub struct ReferenceRule<X = f64> {
    position: RulePosition<X>,
    rgb: (u8, u8, u8),
    opacity: f64,
    width: u32,
    stroke_style: StrokeStyle,
}

#[derive(Clone)]
enum RulePosition<X> {
    Horizontal(f64),
    Vertical(X),
}

impl<X> ReferenceRule<X> {
    /// Span X at this Y value. Defaults: opaque, solid, width 2.
    pub fn horizontal(y: f64, rgb: (u8, u8, u8)) -> Self {
        Self::new(RulePosition::Horizontal(y), rgb)
    }

    /// Span Y at this X value. Defaults: opaque, solid, width 2.
    pub fn vertical(x: X, rgb: (u8, u8, u8)) -> Self {
        Self::new(RulePosition::Vertical(x), rgb)
    }

    fn new(position: RulePosition<X>, rgb: (u8, u8, u8)) -> Self {
        Self {
            position,
            rgb,
            opacity: 1.,
            width: 2,
            stroke_style: StrokeStyle::Solid,
        }
    }

    /// Width in 1..=20 scene units.
    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Alpha in 0.005..=1.0, rounded to native integer percent.
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    /// Native solid/dashed/dotted path, with editor-defined spacing.
    pub fn stroke_style(mut self, style: StrokeStyle) -> Self {
        self.stroke_style = style;
        self
    }
}

pub(crate) trait CoordinateValue: Clone + PartialOrd {
    fn valid(&self) -> bool;
}

impl CoordinateValue for f64 {
    fn valid(&self) -> bool {
        self.is_finite()
    }
}

/// Validate and map while the final chart coordinate specification is alive.
/// Later data-anchor annotations can reuse this route without inspecting output.
pub(crate) fn map_point<X, Y>(
    chart: &ChartContext<'_, ExcalidrawBackend<'_>, Cartesian2d<X, Y>>,
    point: &(X::ValueType, f64),
) -> Result<(i32, i32), Error>
where
    X: Ranged,
    X::ValueType: CoordinateValue,
    Y: Ranged<ValueType = f64>,
{
    let x = chart.as_coord_spec().x_spec().range();
    let y = chart.as_coord_spec().y_spec().range();
    if !point.0.valid()
        || !point.1.is_finite()
        || point.0 < x.start
        || point.0 > x.end
        || point.1 < y.start
        || point.1 > y.end
    {
        return Err(Error::Invalid(
            "annotation coordinates must be finite, representable and inside chart bounds",
        ));
    }
    Ok(chart.plotting_area().map_coordinate(point))
}

pub(crate) struct MappedRule {
    points: [(i32, i32); 2],
    paint: ShapeStyle,
    stroke_style: StrokeStyle,
}

impl<X> ReferenceRule<X> {
    pub(crate) fn map<A: Ranged<ValueType = X>, B: Ranged<ValueType = f64>>(
        &self,
        chart: &ChartContext<'_, ExcalidrawBackend<'_>, Cartesian2d<A, B>>,
    ) -> Result<MappedRule, Error>
    where
        X: CoordinateValue,
    {
        if !(1..=20).contains(&self.width) {
            return Err(Error::Invalid(
                "annotation rule width must be 1..=20 scene units",
            ));
        }
        crate::cartesian::validate_opacity(
            self.opacity,
            "annotation opacity must be 0.005..=1.0 (visible native percent)",
        )?;
        let x = chart.as_coord_spec().x_spec().range();
        let y = chart.as_coord_spec().y_spec().range();
        let points = match &self.position {
            RulePosition::Horizontal(value) => [(x.start, *value), (x.end, *value)],
            RulePosition::Vertical(value) => [(value.clone(), y.start), (value.clone(), y.end)],
        };
        Ok(MappedRule {
            points: [map_point(chart, &points[0])?, map_point(chart, &points[1])?],
            paint: RGBColor(self.rgb.0, self.rgb.1, self.rgb.2)
                .mix(self.opacity)
                .stroke_width(self.width),
            stroke_style: self.stroke_style,
        })
    }
}

impl MappedRule {
    pub(crate) fn draw(
        &self,
        root: &DrawingArea<ExcalidrawBackend<'_>, Shift>,
        options: &DrawingOptions,
    ) -> Result<(), Box<dyn std::error::Error>> {
        options.new_group().scope(|| {
            options.with_stroke_style(self.stroke_style, || {
                root.draw(&PathElement::new(self.points, self.paint))
            })
        })?;
        Ok(())
    }
}
