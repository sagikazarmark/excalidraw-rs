//! Shared measured Cartesian frame, native axes and line/scatter policies.
//! Scale-specific callers supply typed coordinates and their final tick strings;
//! category, stack and legend semantics remain with the numeric chart helper.
use crate::{Error, ExcalidrawBackend};
use plotters::coord::Shift;
use plotters::coord::ranged1d::{BoldPoints, Ranged, ValueFormatter};
use plotters::prelude::*;

pub(crate) const MARGIN: u32 = 24;
pub(crate) const X_LABEL_AREA: u32 = 64;
const Y_LABEL_AREA: u32 = 88;
const TITLE_SIZE: u32 = 24;
const DESCRIPTION_SIZE: u32 = 18;
pub(crate) const TICK_SIZE: u32 = 16;

/// Top of the first right-hand legend row, and the pitch between rows.
///
/// The numeric charts and the pie legend share both numbers and place a row
/// identically from them: swatch `y + 3` to `y + 17`, label at `y`. The numeric
/// charts treat the pitch as a floor that taller marker geometry may raise; the
/// pie has no markers and uses it as given.
///
/// The band and interval legends draw their single row eight units higher, at
/// 82. That offset is load-bearing — it is the drawn coordinate their tests
/// pin — so those two stay with their own literal rather than sharing this.
pub(crate) const LEGEND_TOP: u32 = 90;
pub(crate) const LEGEND_ROW_HEIGHT: u32 = 30;

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

    /// The vertical space `DrawingArea::titled` reserves for `title`.
    ///
    /// Plotters pads the measured text by `min(height / 2, 5)` above and below.
    /// Reproduced here so layout can be computed without rendering; `build`
    /// checks it against the real allocation on every debug run.
    fn caption_height(title: &str) -> Result<u32, Error> {
        let text_height = crate::typography::measure(title, f64::from(TITLE_SIZE))?
            .1
            .ceil() as u32;
        Ok(text_height + 2 * (text_height / 2).min(5))
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
        // The caption allocation is computed, not simulated. Measuring it used to
        // mean building a whole throwaway Scene and backend per render purely to
        // ask Plotters how tall its title band is. `build` still derives the same
        // number from the real `titled` call it has to make anyway, and asserts
        // the two agree in debug builds, so this stays tethered to Plotters
        // rather than silently drifting from it.
        let plot_height = (self.size.1.saturating_sub(2 * MARGIN))
            .checked_sub(Self::caption_height(self.title)? + X_LABEL_AREA + top)
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
        // The tether: `layout` computed this number instead of rendering for it.
        // Every debug test run compares the two, so a Plotters change in caption
        // padding fails loudly here rather than shifting chart geometry quietly.
        //
        // The closed form is fallible, and it used to be compared through
        // `unwrap_or(0)`: a failed measurement silently became a comparison
        // against zero, which is the one value the real allocation can also be.
        // A measurement that fails here is an error, not a passing assertion.
        // The block is debug-only, so release builds still measure the title
        // once, in `layout`.
        #[cfg(debug_assertions)]
        {
            assert_eq!(caption_height, Self::caption_height(self.title)?);
        }
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
    validate_opacity(
        opacity,
        "scatter opacity must round to a visible value in 1..=100 percent",
    )
}

pub(crate) fn validate_radius(radius: u32) -> Result<(), Error> {
    if !(1..=20).contains(&radius) {
        return Err(Error::Invalid(
            "scatter radius must be 1..=20 scene units for fixed margins",
        ));
    }
    Ok(())
}

/// Horizontal space a right-hand legend needs for one measured label.
///
/// The three chart families that draw a legend all compute this, and all three
/// compute it the same way: the measured label, a fixed 28-unit gutter, the gap
/// between swatch and label, and the swatch itself. Only the last two vary, so
/// only the last two are arguments.
///
/// The swatch width is deliberately *not* floored here. Chart legends apply
/// their own `28.max(..)` floor before calling; error-bar legends do not, and
/// imposing one would shift their plot width and move every drawn coordinate.
pub(crate) fn legend_width(label_width: f64, gap: u32, swatch: u32) -> u32 {
    label_width.ceil() as u32 + 28 + gap + swatch
}

/// Width one single-label legend reserves, measured and bounds-checked.
///
/// The band and interval legends price their column identically: measure the
/// one label at tick size — always, so an unrenderable glyph fails whether or
/// not the legend is drawn — reject a label wider than the whole canvas, then
/// hand the measurement to `legend_width` with the shared eight-unit gap. Only
/// the swatch differs, so only the swatch is an argument.
///
/// The numeric chart does not share this. Its legend has several names to take
/// a maximum over and rejects empty ones first, and it floors its own swatch.
pub(crate) fn single_legend_width(
    label: &str,
    draw: bool,
    width: u32,
    swatch: u32,
) -> Result<u32, Error> {
    let label_width = crate::typography::measure(label, f64::from(TICK_SIZE))?.0;
    if !draw {
        return Ok(0);
    }
    if label_width > f64::from(width) {
        return Err(Error::Invalid("legend label exceeds chart width"));
    }
    Ok(legend_width(label_width, 8, swatch))
}

/// Left edge of a right-hand legend column, in root coordinates.
///
/// Every Cartesian family that draws a legend reserves `legend` units inside
/// the right margin and then indents the column by the same twelve units. The
/// interval legend adds its marker extent to this, so the shared part stops at
/// the column edge. `PieChart` has no plot area to reserve against and places
/// its column by canvas fraction instead; it deliberately does not share this.
pub(crate) fn legend_left(width: u32, legend: u32) -> i32 {
    (width - MARGIN - legend + 12) as i32
}

/// Reject an opacity that would not round to a visible native percent.
///
/// One bound, one implementation. `message` stays a caller's choice because the
/// subject is the useful half of the diagnostic: "annotation opacity" and "area
/// opacity" are different things to a caller, and collapsing them to a single
/// wording would lose the only part that says where to look.
pub(crate) fn validate_opacity(opacity: f64, message: &'static str) -> Result<(), Error> {
    if !opacity.is_finite() || !(0.005..=1.).contains(&opacity) {
        return Err(Error::Invalid(message));
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

/// Whether a span the data calls nonzero has landed on a single coordinate.
///
/// Every filled mark asks this of its value span, and all four ask it the same
/// way: a band's lower/upper pair, a histogram bin's zero-to-count height, a
/// bar's baseline-to-value height, an interval's lower/upper caps. Spans the
/// data itself makes degenerate — an empty bin, a zero-width interval — are
/// meant to land on one coordinate and stay exempt.
///
/// The diagnostic is the caller's, as in `validate_opacity`: "bar", "bin",
/// "band" and "interval" are the useful half of each message.
pub(crate) fn collapsed(span: (f64, f64), mapped: (i32, i32)) -> bool {
    span.0 != span.1 && mapped.0 == mapped.1
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

/// Layout arithmetic is crate-internal and has no public interface to test
/// through, so its measurement tests live next to it rather than in `tests/`.
///
/// These cover the closed form only. The `debug_assert_eq!` in `build` guards a
/// different thing: that the closed form still agrees with the Plotters version
/// actually linked. Neither replaces the other.
#[cfg(test)]
mod tests {
    use super::*;

    /// Plotters' title padding, restated rather than copied: half the measured
    /// text height while that stays under the five-unit cap, five units once it
    /// does not. `caption_height` writes the same rule as `min(height / 2, 5)`.
    fn padding_rule(text_height: u32) -> u32 {
        if text_height < 10 { text_height / 2 } else { 5 }
    }

    /// `Layout` is deliberately not `Debug`, so failures are read as messages.
    fn failure(result: DrawResult<Layout>) -> String {
        match result {
            Ok(_) => panic!("expected a layout failure"),
            Err(error) => error.to_string(),
        }
    }

    fn frame(title: &str, size: (u32, u32)) -> Frame<'_> {
        Frame {
            title,
            x_label: "Time (s)",
            y_label: "Value",
            size,
        }
    }

    #[test]
    fn caption_band_pads_the_measured_title_by_the_plotters_five_unit_cap_on_each_side() {
        let title = "Measured quarterly revenue";
        let text_height = crate::typography::measure(title, f64::from(TITLE_SIZE))
            .unwrap()
            .1;
        // One Excalifont line box is 1.25 em, so a 24-unit title measures 30.
        assert!((text_height - f64::from(TITLE_SIZE) * 1.25).abs() < 1e-9);
        let measured = text_height.ceil() as u32;
        assert_eq!(measured, 30);

        let band = Frame::caption_height(title).unwrap();
        // 30 of text plus 5 above and 5 below: the cap binds, since half of 30
        // is 15. Spelled out so a changed cap or a changed rule fails here.
        assert_eq!(band, 40);
        assert_eq!(band - measured, 2 * padding_rule(measured));
        assert_eq!((band - measured) % 2, 0, "padding is symmetric");
        assert_eq!((band - measured) / 2, 5);
        assert!(measured / 2 > 5, "the cap, not the half-height, applies");
    }

    #[test]
    fn caption_band_depends_on_the_title_font_size_alone_so_every_title_costs_the_same_band() {
        // The line box exists with no glyphs in it, so an absent title is still
        // charged the full band; `layout` reserves it either way.
        for title in [
            "",
            "x",
            "Measured quarterly revenue",
            "A deliberately long measured title that reaches the Y column",
        ] {
            assert_eq!(Frame::caption_height(title).unwrap(), 40, "{title:?}");
        }
        let empty = frame("", (900, 400)).layout([40., 40.], 0).unwrap();
        let titled = frame("Measured quarterly revenue", (900, 400))
            .layout([40., 40.], 0)
            .unwrap();
        assert_eq!(empty.plot_height, titled.plot_height);
    }

    #[test]
    fn title_padding_grows_with_font_size_until_it_saturates_at_five_units() {
        // `caption_height` measures at the crate's fixed TITLE_SIZE, so only one
        // height is reachable through it; the ladder records the rule that the
        // constant is fed into, and the last rung is the reachable one.
        let heights = [1_u32, 2, 9, 10, 11, 30, 100];
        let paddings: Vec<_> = heights.iter().map(|&h| padding_rule(h)).collect();
        assert_eq!(paddings, [0, 1, 4, 5, 5, 5, 5]);
        assert!(
            paddings.windows(2).all(|p| p[0] <= p[1]),
            "padding never shrinks as the title grows"
        );
        assert!(paddings.iter().all(|&p| p <= 5), "five units is the cap");
        let measured = (f64::from(TITLE_SIZE) * 1.25).ceil() as u32;
        assert_eq!(
            Frame::caption_height("Measured quarterly revenue").unwrap(),
            measured + 2 * padding_rule(measured)
        );
    }

    #[test]
    fn caption_height_surfaces_unsupported_title_glyphs_instead_of_measuring_past_them() {
        let error = Frame::caption_height("weather \u{2600}").unwrap_err();
        assert!(
            matches!(error, Error::UnsupportedGlyph('\u{2600}')),
            "{error}"
        );
    }

    #[test]
    fn layout_partitions_the_canvas_width_across_margin_label_area_plot_right_and_legend() {
        let frame = frame("Measured quarterly revenue", (900, 400));
        for legend in [0, 120] {
            let layout = frame.layout([40., 40.], legend).unwrap();
            assert_eq!(layout.legend, legend);
            assert_eq!(
                MARGIN + layout.y_area + layout.plot_width + layout.right + layout.legend,
                frame.size.0,
                "the width partition leaves no unaccounted units"
            );
            assert!(layout.plot_width >= 150, "minimum plot width holds");
        }
    }

    #[test]
    fn a_legend_allocation_shifts_the_plot_width_by_exactly_its_own_width() {
        let frame = frame("Measured quarterly revenue", (900, 400));
        let without = frame.layout([40., 40.], 0).unwrap();
        let with = frame.layout([40., 40.], 120).unwrap();
        assert_eq!(without.plot_width - with.plot_width, 120);
        // The legend takes from the plot alone; nothing else moves.
        assert_eq!(without.y_area, with.y_area);
        assert_eq!(without.right, with.right);
        assert_eq!(without.top, with.top);
        assert_eq!(without.plot_height, with.plot_height);
    }

    #[test]
    fn layout_reserves_the_caption_band_and_x_label_area_out_of_the_canvas_height() {
        for height in [400, 600] {
            let frame = frame("Measured quarterly revenue", (900, height));
            let layout = frame.layout([40., 40.], 0).unwrap();
            assert_eq!(layout.top, 0, "a centered title needs no extra top band");
            // 40 is the caption band established above, not a recomputation of it.
            assert_eq!(
                layout.plot_height,
                height - 2 * MARGIN - 40 - X_LABEL_AREA - layout.top
            );
            assert_eq!(
                2 * MARGIN + 40 + X_LABEL_AREA + layout.top + layout.plot_height,
                frame.size.1,
                "the height partition leaves no unaccounted units"
            );
        }
    }

    #[test]
    fn a_title_reaching_the_y_label_column_costs_an_extra_top_band_of_plot_height() {
        let short = frame("Measured quarterly revenue", (900, 400))
            .layout([40., 40.], 0)
            .unwrap();
        let long = frame(
            "A deliberately long measured title that reaches the Y column",
            (900, 400),
        )
        .layout([40., 40.], 0)
        .unwrap();
        // Half a tick line box plus the eight-unit gap, less the caption's own
        // five-unit bottom pad: 10 + 8 - 5.
        assert_eq!(short.top, 0);
        assert_eq!(long.top, 13);
        assert_eq!(short.plot_height - long.plot_height, 13);
        assert_eq!(short.plot_width, long.plot_width, "only height is charged");
    }

    #[test]
    fn measured_tick_and_description_widths_widen_the_label_areas_past_their_floors() {
        let frame = frame("Measured quarterly revenue", (900, 400));
        let floors = frame.layout([40., 40.], 0).unwrap();
        assert_eq!(floors.y_area, Y_LABEL_AREA, "narrow labels keep the floor");

        let wide = frame.layout([400., 200.], 0).unwrap();
        let description = crate::typography::measure("Value", f64::from(DESCRIPTION_SIZE))
            .unwrap()
            .1;
        // The Y column holds its widest tick label, the rotated description and
        // the two gaps; the gutters hold half of the widest X endpoint label.
        assert_eq!(wide.y_area, (200. + description + 8. + 10.).ceil() as u32);
        assert_eq!(wide.right, (400_f64 / 2. + 8.).ceil() as u32);
        assert!(wide.y_area > floors.y_area && wide.right > floors.right);
        assert_eq!(
            MARGIN + wide.y_area + wide.plot_width + wide.right,
            frame.size.0
        );
    }

    #[test]
    fn layout_refuses_a_canvas_whose_measured_labels_leave_less_than_the_minimum_plot_width() {
        let error =
            failure(frame("Measured quarterly revenue", (400, 400)).layout([40., 40.], 200));
        assert!(error.contains("insufficient plot width"), "{error}");
    }

    #[test]
    fn layout_refuses_a_canvas_whose_caption_and_axis_bands_leave_less_than_the_minimum_plot_height()
     {
        // 260 - 48 of margin - 40 of caption - 64 of X labels leaves 108.
        let error = failure(frame("Measured quarterly revenue", (900, 260)).layout([40., 40.], 0));
        assert!(error.contains("insufficient plot height"), "{error}");
    }

    #[test]
    fn labels_wider_than_their_measured_space_fail_rather_than_being_shrunk_or_wrapped() {
        let title = "A deliberately long measured title that reaches toward the Y label column";
        let error = failure(frame(title, (900, 400)).layout([40., 40.], 0));
        assert!(
            error.contains("label exceeds measured chart space"),
            "{error}"
        );

        let long_description = Frame {
            title: "",
            x_label: "A deliberately long measured title that reaches the Y column",
            y_label: "Value",
            size: (500, 400),
        };
        let error = failure(long_description.layout([40., 40.], 0));
        assert!(
            error.contains("label exceeds measured chart space"),
            "{error}"
        );
    }

    #[test]
    fn validate_rejects_tick_density_outside_two_to_twenty_on_either_axis() {
        let frame = frame("Measured quarterly revenue", (900, 400));
        assert!(frame.validate((2, 20)).is_ok());
        for count in [(0, 6), (1, 6), (6, 1), (21, 6), (6, 21), (usize::MAX, 6)] {
            let error = frame.validate(count).unwrap_err().to_string();
            assert!(error.contains("tick density must be 2..=20"), "{count:?}");
        }
    }

    #[test]
    fn validate_rejects_canvas_sizes_outside_four_hundred_by_three_hundred_to_sixteen_k() {
        for size in [(400, 300), (16384, 16384)] {
            assert!(frame("Title", size).validate((6, 6)).is_ok(), "{size:?}");
        }
        for size in [(399, 300), (400, 299), (16385, 300), (400, 16385), (0, 0)] {
            let error = frame("Title", size)
                .validate((6, 6))
                .unwrap_err()
                .to_string();
            assert!(error.contains("chart size must be"), "{size:?}: {error}");
        }
    }

    #[test]
    fn validate_surfaces_unsupported_glyphs_from_the_title_and_both_axis_descriptions() {
        for (title, x_label, y_label) in [
            ("weather \u{2600}", "Time (s)", "Value"),
            ("Title", "Time \u{2600}", "Value"),
            ("Title", "Time (s)", "Value \u{2600}"),
        ] {
            let frame = Frame {
                title,
                x_label,
                y_label,
                size: (900, 400),
            };
            let error = frame.validate((6, 6)).unwrap_err();
            assert!(
                matches!(error, Error::UnsupportedGlyph('\u{2600}')),
                "{error}"
            );
        }
    }

    #[test]
    fn the_three_legend_height_budgets_are_deliberately_different() {
        // Three rules, not three spellings of one. Nothing recorded the
        // difference, and swapping any two passes every other test in the
        // workspace, so state each budget by name here.
        //
        // The numeric charts reserve the X label area beneath the legend.
        assert_eq!(LEGEND_TOP + X_LABEL_AREA, 154);
        // The pie has no X label area to clear, so it reserves only the margin.
        assert_eq!(LEGEND_TOP + MARGIN, 114);
        // The interval schematic is not a row count at all: it prices one
        // fixed drawing from its own origin of 82, and clears both bands.
        assert_eq!(MARGIN + X_LABEL_AREA, 88);
    }
}
