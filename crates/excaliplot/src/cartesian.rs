//! Shared measured Cartesian frame, native axes and line/scatter policies.
//! Scale-specific callers supply typed coordinates and their final tick strings;
//! category, stack and legend semantics remain with the numeric chart helper.
use crate::{Error, ExcalidrawBackend, Scene};
use plotters::coord::Shift;
use plotters::coord::ranged1d::{BoldPoints, Ranged, ValueFormatter};
use plotters::prelude::*;

pub(crate) const MARGIN: u32 = 24;
pub(crate) const X_LABEL_AREA: u32 = 64;
const Y_LABEL_AREA: u32 = 88;
const TITLE_SIZE: u32 = 24;
const DESCRIPTION_SIZE: u32 = 18;
pub(crate) const TICK_SIZE: u32 = 16;

type DrawResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type NativeChart<'a, 'scene, X, Y> = ChartContext<'a, ExcalidrawBackend<'scene>, Cartesian2d<X, Y>>;
type NativeRoot<'scene> = DrawingArea<ExcalidrawBackend<'scene>, Shift>;

pub(crate) struct Frame<'a> {
    pub title: &'a str,
    pub x_label: &'a str,
    pub y_label: &'a str,
    pub size: (u32, u32),
}

pub(crate) struct Layout {
    pub legend: u32,
    y_area: u32,
    right: u32,
    top: u32,
    pub plot_width: u32,
    pub plot_height: u32,
}

/// Final strings are measured once and reused for fit checks and mesh labels.
pub(crate) struct Ticks<T> {
    values: Vec<T>,
    labels: Vec<String>,
    dimensions: Vec<(f64, f64)>,
    count: usize,
}

impl<T: PartialEq> Ticks<T> {
    pub fn select(coord: &impl Ranged<ValueType = T>, count: usize) -> Vec<T> {
        if count == 0 {
            vec![]
        } else {
            coord.key_points(BoldPoints(count))
        }
    }

    pub fn new(values: Vec<T>, count: usize, format: impl Fn(&T) -> String) -> Result<Self, Error> {
        let labels: Vec<_> = values.iter().map(format).collect();
        let mut seen = std::collections::HashSet::new();
        let mut dimensions = Vec::with_capacity(labels.len());
        for label in &labels {
            if !seen.insert(label) {
                return Err(Error::Invalid(
                    "visible ticks need distinct labels; increase precision or reduce tick density",
                ));
            }
            dimensions.push(crate::typography::measure(label, f64::from(TICK_SIZE))?);
        }
        Ok(Self {
            values,
            labels,
            dimensions,
            count,
        })
    }

    pub fn max_width(&self) -> f64 {
        self.dimensions.iter().map(|d| d.0).fold(0., f64::max)
    }

    fn label(&self, value: &T) -> String {
        self.values
            .iter()
            .position(|v| v == value)
            .map(|i| self.labels[i].clone())
            .unwrap_or_default()
    }
}

impl Frame<'_> {
    pub fn validate(&self, tick_count: (usize, usize)) -> Result<(), Error> {
        // Preserve typed glyph failures before entering the drawing route.
        for (text, size) in [
            (self.title, TITLE_SIZE),
            (self.x_label, DESCRIPTION_SIZE),
            (self.y_label, DESCRIPTION_SIZE),
        ] {
            crate::typography::measure(text, f64::from(size))?;
        }
        if !(2..=20).contains(&tick_count.0) || !(2..=20).contains(&tick_count.1) {
            return Err(Error::Invalid(
                "desired tick density must be 2..=20 per axis",
            ));
        }
        if !(400..=16384).contains(&self.size.0) || !(300..=16384).contains(&self.size.1) {
            return Err(Error::Invalid(
                "chart size must be 400..=16384 by 300..=16384",
            ));
        }
        Ok(())
    }

    /// One bounded measurement pass, followed by one final coordinate build.
    pub fn layout(&self, widths: [f64; 2], legend: u32) -> DrawResult<Layout> {
        let [x_width, y_width] = widths;
        let description_height =
            crate::typography::measure(self.y_label, f64::from(DESCRIPTION_SIZE))?.1;
        let y_area = Y_LABEL_AREA
            .max((y_width + description_height + 8. + 10.).ceil() as u32)
            .max((x_width / 2.).ceil() as u32);
        let right = MARGIN.max((x_width / 2. + 8.).ceil() as u32);
        let plot_width = self.size.0.checked_sub(MARGIN + y_area + right + legend)
            .filter(|&w| w >= 150)
            .ok_or(Error::Invalid("measured labels and legend leave insufficient plot width (minimum 150); increase chart size"))?;
        let (title_width, _) = crate::typography::measure(self.title, f64::from(TITLE_SIZE))?;
        let title_left =
            f64::from(MARGIN + (self.size.0 - MARGIN - right - legend) / 2) - title_width / 2.;
        let top = if title_left < f64::from(MARGIN + y_area - 10) + 8. {
            (f64::from(TICK_SIZE) * 1.25 / 2. + 8. - 5.).ceil() as u32
        } else {
            0
        };
        // Ask Plotters for its actual caption allocation rather than duplicating
        // its font rounding and padding policy. This scratch scene is never exported.
        let mut scratch = Scene::new();
        let root = ExcalidrawBackend::new(&mut scratch, self.size)?.into_drawing_area();
        let caption = root.margin(MARGIN, MARGIN, MARGIN, right + legend);
        let below = caption.titled(self.title, ("Excalifont", TITLE_SIZE))?;
        let plot_height = below
            .dim_in_pixel()
            .1
            .checked_sub(X_LABEL_AREA + top)
            .filter(|&h| h >= 120)
            .ok_or(Error::Invalid(
                "measured labels leave insufficient plot height (minimum 120)",
            ))?;
        for (text, size, available) in [
            (
                self.title,
                TITLE_SIZE,
                self.size.0 - MARGIN - right - legend,
            ),
            (self.x_label, DESCRIPTION_SIZE, plot_width),
            (self.y_label, DESCRIPTION_SIZE, plot_height),
        ] {
            if crate::typography::measure(text, f64::from(size))?.0 > f64::from(available) {
                return Err(Error::Invalid(
                    "label exceeds measured chart space; increase chart size",
                )
                .into());
            }
        }
        Ok(Layout {
            legend,
            y_area,
            right,
            top,
            plot_width,
            plot_height,
        })
    }

    pub fn build<'a, 'scene, X: Ranged, Y: Ranged>(
        &self,
        root: &'a NativeRoot<'scene>,
        layout: &Layout,
        x: X,
        y: Y,
    ) -> DrawResult<NativeChart<'a, 'scene, X, Y>> {
        // A top label area would create a duplicate X axis in Plotters 0.3.7.
        let caption = root.margin(MARGIN, MARGIN, MARGIN, layout.right + layout.legend);
        let below = caption.titled(self.title, ("Excalifont", TITLE_SIZE))?;
        let caption_height = caption.dim_in_pixel().1 - below.dim_in_pixel().1;
        let chart = ChartBuilder::on(root)
            .margin(MARGIN)
            .margin_top(MARGIN + caption_height + layout.top)
            .margin_right(layout.right + layout.legend)
            .x_label_area_size(X_LABEL_AREA)
            .y_label_area_size(layout.y_area)
            .build_cartesian_2d(x, y)?;
        debug_assert_eq!(
            chart.plotting_area().dim_in_pixel(),
            (layout.plot_width, layout.plot_height)
        );
        Ok(chart)
    }

    pub fn check_ticks<X: Ranged, Y: Ranged>(
        &self,
        chart: &NativeChart<'_, '_, X, Y>,
        layout: &Layout,
        x_ticks: &Ticks<X::ValueType>,
        y_ticks: &Ticks<Y::ValueType>,
    ) -> Result<(), Error>
    where
        X::ValueType: Clone,
        Y::ValueType: Clone,
    {
        let (title_width, title_height) =
            crate::typography::measure(self.title, f64::from(TITLE_SIZE))?;
        let title_left =
            f64::from(MARGIN + (self.size.0 - MARGIN - layout.right - layout.legend) / 2)
                - title_width / 2.;
        let title_bottom =
            f64::from(MARGIN) + (title_height.ceil() / 2.).floor().min(5.) + title_height;
        let x_start = chart.as_coord_spec().x_spec().range().start;
        let y_start = chart.as_coord_spec().y_spec().range().start;
        let mapped_x = x_ticks
            .values
            .iter()
            .zip(&x_ticks.dimensions)
            .map(|(x, &dimensions)| {
                (
                    chart
                        .plotting_area()
                        .map_coordinate(&(x.clone(), y_start.clone())),
                    dimensions,
                )
            });
        let mapped_y = y_ticks
            .values
            .iter()
            .zip(&y_ticks.dimensions)
            .map(|(y, &dimensions)| {
                (
                    chart
                        .plotting_area()
                        .map_coordinate(&(x_start.clone(), y.clone())),
                    dimensions,
                )
            });
        for horizontal in [true, false] {
            let mut previous: Option<(i32, f64)> = None;
            let ticks: Box<dyn Iterator<Item = _>> = if horizontal {
                Box::new(mapped_x.clone())
            } else {
                Box::new(mapped_y.clone())
            };
            for ((x, y), (width, height)) in ticks {
                let (position, extent) = if horizontal {
                    if f64::from(x) - width / 2. < 0.
                        || f64::from(x) + width / 2. > f64::from(self.size.0)
                    {
                        return Err(Error::Invalid("endpoint tick label exceeds canvas bounds"));
                    }
                    (x, width)
                } else {
                    let tick_right = f64::from(x - 10);
                    if title_left < tick_right + 8.
                        && title_left + title_width + 8. > tick_right - width
                        && f64::from(y) - height / 2. < title_bottom + 8.
                    {
                        return Err(Error::Invalid(
                            "title crowds Y tick labels; shorten title or increase chart width",
                        ));
                    }
                    (y, height)
                };
                if let Some((last, last_extent)) = previous
                    && f64::from((position - last).abs()) < (extent + last_extent) / 2. + 8.
                {
                    return Err(Error::Invalid(
                        "tick labels are crowded; reduce density or increase chart size",
                    ));
                }
                previous = Some((position, extent));
            }
        }
        Ok(())
    }

    pub fn draw_axes<X, Y>(
        &self,
        root: &NativeRoot<'_>,
        chart: &mut NativeChart<'_, '_, X, Y>,
        x_ticks: &Ticks<X::ValueType>,
        y_ticks: &Ticks<Y::ValueType>,
    ) -> DrawResult
    where
        X: Ranged + ValueFormatter<X::ValueType>,
        Y: Ranged + ValueFormatter<Y::ValueType>,
        X::ValueType: Clone + PartialEq,
        Y::ValueType: Clone + PartialEq,
    {
        chart
            .configure_mesh()
            .disable_mesh()
            // Invisible strokes retain Plotters' label offsets. Native strokes
            // below give each axis one independently editable path.
            .axis_style(TRANSPARENT)
            .x_labels(x_ticks.count)
            .y_labels(y_ticks.count)
            .x_label_formatter(&|v| x_ticks.label(v))
            .y_label_formatter(&|v| y_ticks.label(v))
            .label_style(("Excalifont", TICK_SIZE))
            .axis_desc_style(("Excalifont", DESCRIPTION_SIZE))
            .x_desc(self.x_label)
            .y_desc(self.y_label)
            .draw()?;
        let x = chart.as_coord_spec().x_spec().range();
        let y = chart.as_coord_spec().y_spec().range();
        let (left, bottom) = chart
            .plotting_area()
            .map_coordinate(&(x.start.clone(), y.start.clone()));
        let (right, top) = chart.plotting_area().map_coordinate(&(x.end, y.end));
        root.draw(&PathElement::new(
            [(left - 10, bottom), (right, bottom)],
            BLACK,
        ))?;
        root.draw(&PathElement::new([(left, top), (left, bottom + 10)], BLACK))?;
        for tick in &x_ticks.values {
            let (position, _) = chart
                .plotting_area()
                .map_coordinate(&(tick.clone(), y.start.clone()));
            if position != left {
                root.draw(&PathElement::new(
                    [(position, bottom), (position, bottom + 5)],
                    BLACK,
                ))?;
            }
        }
        for tick in &y_ticks.values {
            let (_, position) = chart
                .plotting_area()
                .map_coordinate(&(x.start.clone(), tick.clone()));
            if position != bottom {
                root.draw(&PathElement::new(
                    [(left - 5, position), (left, position)],
                    BLACK,
                ))?;
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_line<T: PartialEq>(points: &[T]) -> Result<(), Error> {
    if points.len() < 2 || !points.windows(2).any(|p| p[0] != p[1]) {
        return Err(Error::Invalid(
            "a marker-free line needs at least two distinct points",
        ));
    }
    Ok(())
}

pub(crate) fn validate_order<X: PartialOrd, Y>(points: &[(X, Y)]) -> Result<(), Error> {
    if points.windows(2).any(|p| p[0].0 > p[1].0) {
        return Err(Error::Invalid(
            "points must be in nondecreasing X order; input is never sorted",
        ));
    }
    Ok(())
}

pub(crate) fn validate_scatter(radius: u32, opacity: f64) -> Result<(), Error> {
    validate_radius(radius)?;
    validate_opacity(opacity)
}

pub(crate) fn validate_radius(radius: u32) -> Result<(), Error> {
    if !(1..=20).contains(&radius) {
        return Err(Error::Invalid(
            "scatter radius must be 1..=20 scene units for fixed margins",
        ));
    }
    Ok(())
}

pub(crate) fn validate_opacity(opacity: f64) -> Result<(), Error> {
    if !opacity.is_finite() || !(0.005..=1.).contains(&opacity) {
        return Err(Error::Invalid(
            "scatter opacity must round to a visible value in 1..=100 percent",
        ));
    }
    Ok(())
}

pub(crate) fn check_line(points: impl Iterator<Item = (i32, i32)>) -> Result<(), Error> {
    let mut points = points;
    let first = points.next();
    if points.all(|p| Some(p) == first) {
        return Err(Error::Invalid(
            "series collapses at this pixel resolution; narrow bounds or increase size",
        ));
    }
    Ok(())
}

pub(crate) fn check_markers(
    points: impl Iterator<Item = (i32, i32)>,
    radius: u32,
    (left, bottom): (i32, i32),
    (right, top): (i32, i32),
) -> Result<(), Error> {
    // The five-unit halo protects title/ticks/legend, including hollow strokes
    // and opt-in roughness. Range padding alone cannot guarantee this clearance.
    let radius = radius as i32;
    for (x, y) in points {
        if x - radius < left - 5
            || x + radius > right + 5
            || y - radius < top - 5
            || y + radius > bottom + 5
        {
            return Err(Error::Invalid(
                "scatter marker crowds plot-edge labels; reduce radius or widen bounds",
            ));
        }
    }
    Ok(())
}

pub(crate) fn draw_line<X: Ranged, Y: Ranged>(
    chart: &mut NativeChart<'_, '_, X, Y>,
    points: &[(X::ValueType, Y::ValueType)],
    style: impl Into<ShapeStyle>,
) -> DrawResult
where
    X::ValueType: Clone + 'static,
    Y::ValueType: Clone + 'static,
{
    chart.draw_series(LineSeries::new(points.iter().cloned(), style))?;
    Ok(())
}

pub(crate) fn draw_scatter<X: Ranged, Y: Ranged>(
    chart: &mut NativeChart<'_, '_, X, Y>,
    points: &[(X::ValueType, Y::ValueType)],
    color: RGBColor,
    radius: u32,
    fill: bool,
    opacity: f64,
) -> DrawResult
where
    X::ValueType: Clone,
    Y::ValueType: Clone,
{
    draw_markers(
        chart,
        points,
        radius,
        ShapeStyle {
            color: color.mix(opacity),
            filled: fill,
            stroke_width: 2,
        },
    )
}

pub(crate) fn draw_markers<X: Ranged, Y: Ranged>(
    chart: &mut NativeChart<'_, '_, X, Y>,
    points: &[(X::ValueType, Y::ValueType)],
    radius: u32,
    style: ShapeStyle,
) -> DrawResult
where
    X::ValueType: Clone,
    Y::ValueType: Clone,
{
    chart.draw_series(
        points
            .iter()
            .cloned()
            .map(|point| Circle::new(point, radius, style)),
    )?;
    Ok(())
}
