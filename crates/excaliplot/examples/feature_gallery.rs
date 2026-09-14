//! A tour of the public chart, styling, annotation, backend and scene features.
//! Each panel appears once; all data is synthetic. See the README coverage table.
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use excaliplot::{
    AreaChart, ArrowHead, ArrowStyle, AxisScale, BandChart, BarChart, BorderStyle, Callout,
    DateLineChart, DateScatterChart, ErrorBarChart, ExcalidrawBackend, FillStyle, HistogramChart,
    LegendPosition, LineChart, NamedBarSeries, NamedSeries, NamedSlice, Overwrite, PieChart,
    Provenance, ReferenceRule, ReportUrl, ScatterChart, Scene, ShadedInterval, SketchStyle,
    SourceLink, StepChart, StrokeStyle, TickFormat, UtcLineChart, UtcScatterChart,
    VerticalInterval,
};
use plotters::prelude::*;
use std::{ffi::OsStr, path::PathBuf, time::Instant};

#[path = "support/destination.rs"]
mod destination;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SIZE: (u32, u32) = (1040, 480);
const BLUE: (u8, u8, u8) = (25, 113, 194);
const RED: (u8, u8, u8) = (224, 49, 49);
const GREEN: (u8, u8, u8) = (47, 158, 68);
const ORANGE: (u8, u8, u8) = (230, 119, 0);

struct Panel {
    scene: Scene,
    note: &'static str,
}

impl Panel {
    fn new(scene: Scene, note: &'static str) -> Self {
        Self { scene, note }
    }
}

fn numeric_charts() -> Result<Vec<Panel>> {
    let observed = [(1., 2.), (3., 5.), (6., 8.), (9., 4.)];
    let model = [(1., 3.), (3., 4.), (6., 5.), (9., 6.)];
    let target = [(1., 6.), (3., 6.), (6., 6.), (9., 6.)];
    let lines = [
        NamedSeries::new("Measured", &observed, BLUE)
            .line_width(2)
            .marker(4, true),
        NamedSeries::new("Model", &model, BLUE)
            .stroke_style(StrokeStyle::Dashed)
            .line_width(4)
            .opacity(0.65)
            .marker(6, false),
        NamedSeries::new("Target", &target, GREEN).stroke_style(StrokeStyle::Dotted),
    ];
    let scatter = [
        NamedSeries::new("Before", &[(8., 7.), (2., 3.), (5., 6.), (5., 6.)], RED)
            .marker(7, false)
            .line_width(3),
        NamedSeries::new("After", &[(2., 2.), (8., 4.), (5., 3.)], GREEN)
            .marker(5, true)
            .opacity(0.6),
    ];
    let sketch = SketchStyle::new(1, FillStyle::Solid)?;
    let hachure = SketchStyle::new(1, FillStyle::Hachure)?;
    let cross_hatch = SketchStyle::new(2, FillStyle::CrossHatch)?;
    let categories = ["API", "Storage", "Workers"];
    let costs = [
        NamedBarSeries::new("Compute", &[30., 12., 24.], BLUE),
        NamedBarSeries::new("Network", &[10., 8., 6.], ORANGE),
    ];
    let changes = [
        NamedBarSeries::new("Before", &[4., -2., 3.], BLUE),
        NamedBarSeries::new("After", &[2., -1., 0.], GREEN),
    ];
    let areas = [
        NamedSeries::new("API", &[(0., 3.), (2., 6.), (5., 4.), (8., 7.)], BLUE),
        NamedSeries::new("Workers", &[(0., 2.), (2., 3.), (5., 5.), (8., 2.)], GREEN),
    ];
    let slices = [
        NamedSlice::new("Compute: 60 (60%)", 60., BLUE),
        NamedSlice::new("Storage: 25 (25%)", 25., GREEN),
        NamedSlice::new("Network: 15 (15%)", 15., ORANGE),
    ];

    Ok(vec![
        Panel::new(
            LineChart::from_series(&lines, 0.0..10.0, 0.0..10.0)
                .size(SIZE)
                .labels("01 / Annotated multi-series lines", "Time (s)", "Value")
                .shaded_interval(ShadedInterval::x(2.0..4.0, (255, 236, 153)).opacity(0.35))
                .shaded_interval(ShadedInterval::y(0.5..1.5, GREEN).opacity(0.12))
                .reference_rule(
                    ReferenceRule::horizontal(7., RED).stroke_style(StrokeStyle::Dotted),
                )
                .reference_rule(ReferenceRule::vertical(4., ORANGE).width(3).opacity(0.6))
                .callout(Callout::new((6., 8.), "Peak", (40., -55.)))
                .callout(
                    Callout::new((9., 4.), "Recovery", (-150., 45.)).arrow_style(ArrowStyle {
                        head: ArrowHead::Triangle,
                        color: GREEN,
                        ..ArrowStyle::default()
                    }),
                )
                .render()?,
            "Solid / dashed / dotted paths; widths, alpha, filled / hollow markers.\nX/Y windows and rules; both arrowheads; grouped series and right legend.",
        ),
        Panel::new(
            ScatterChart::from_series(&scatter, 0.0..10.0, 0.0..10.0)
                .size(SIZE)
                .labels("02 / Styled scatter", "CPU load", "Latency")
                .marker(4, true)
                .opacity(0.8)
                .sketch(sketch)
                .render()?,
            "Unordered and repeated observations retain independent native circles.\nPer-series settings override chart defaults; roughness 1, solid fill.",
        ),
        Panel::new(
            LineChart::auto_from_series(&[
                NamedSeries::new(
                    "Observed",
                    &[(0., -5.), (1., 2.), (3., 8.), (20., 5.)],
                    BLUE,
                ),
                NamedSeries::new("Constant", &[(0., 3.), (20., 3.)], GREEN),
            ])
            .size(SIZE)
            .labels(
                "03 / Automatic linear ranges",
                "Irregular X",
                "Signed value",
            )
            .legend(LegendPosition::Off)
            .y_tick_format(TickFormat::Decimal { places: 1 })
            .tick_density(5, 5)
            .render()?,
            "Automatic bounds contain all series, including a constant-Y line.\nDecimal ticks, chosen density and hidden legend; groups are retained.",
        ),
        Panel::new(
            BarChart::new(&[("API", 4.), ("Storage", -2.), ("Workers", 0.)], -4.0..6.0)
                .size(SIZE)
                .labels(
                    "04 / Signed vertical bars",
                    "Service",
                    "Cost change (USD k)",
                )
                .sketch(hachure)
                .render()?,
            "Positive, negative and zero values; explicit bounds include zero.\nZero keeps its category without a rectangle; roughness 1, hachure fill.",
        ),
        Panel::new(
            BarChart::from_series(&categories, &changes, -4.0..6.0)
                .size(SIZE)
                .labels("05 / Grouped vertical bars", "Service", "Change (hours)")
                .render()?,
            "Named series occupy separate slots in each category.\nEach series groups its rectangles with its legend swatch and label.",
        ),
        Panel::new(
            BarChart::auto(&[
                ("International support", 4.),
                ("Domestic support", -2.),
                ("Automation", 0.),
            ])
            .horizontal()
            .size(SIZE)
            .labels("06 / Horizontal bars", "Team", "Change (hours)")
            .y_tick_format(TickFormat::Decimal { places: 1 })
            .render()?,
            "Measured long category labels, top-to-bottom order and automatic bounds.\nThe logical Y formatter controls the horizontal numeric value axis.",
        ),
        Panel::new(
            BarChart::auto_from_series(
                &["International support", "Domestic support", "Automation"],
                &changes,
            )
            .horizontal()
            .size(SIZE)
            .labels("07 / Grouped horizontal bars", "Team", "Change (hours)")
            .render()?,
            "Grouped signed horizontal bars with automatic bounds.\nSource order controls categories and the series slots within each category.",
        ),
        Panel::new(
            BarChart::from_series(&categories, &costs, 0.0..50.0)
                .stacked()
                .size(SIZE)
                .labels("08 / Stacked bars", "Service", "Cost (USD k)")
                .sketch(cross_hatch)
                .render()?,
            "Nonnegative series accumulate from zero; separate editable segments.\nRoughness 2 and cross-hatch demonstrate the third sketch/fill choices.",
        ),
        Panel::new(
            BarChart::auto_from_series(&categories, &costs)
                .percent_stacked()
                .size(SIZE)
                .labels("09 / Percent-stacked bars", "Service", "Cost share")
                .y_tick_format(TickFormat::Percent { places: 0 })
                .render()?,
            "Raw costs normalize per category; automatic ranges contain 0..100.\nPercent appends a sign to values that are already percentages.",
        ),
        Panel::new(
            AreaChart::auto(&[(0., 8.), (2., 5.), (5., 7.), (8., 3.)])
                .baseline(10.)
                .size(SIZE)
                .labels("10 / Area below a custom baseline", "Time (h)", "Capacity")
                .opacity(0.4)
                .render()?,
            "A single filled area below a nonzero baseline with its data-edge border.\nAutomatic ranges include both observations and the baseline.",
        ),
        Panel::new(
            AreaChart::from_series(&areas, 0.0..8.0, 0.0..10.0)
                .size(SIZE)
                .labels("11 / Overlapping areas", "Time (h)", "Throughput (GB/s)")
                .opacity(0.3)
                .sketch(hachure)
                .reference_rule(
                    ReferenceRule::horizontal(8., RED).stroke_style(StrokeStyle::Dashed),
                )
                .render()?,
            "Translucent areas overlap in input painter order; native closed paths.\nAn area reference rule paints in front of the fills and data borders.",
        ),
        Panel::new(
            AreaChart::auto_from_series(&areas)
                .stacked()
                .size(SIZE)
                .labels("12 / Stacked areas", "Time (h)", "Requests (thousands)")
                .opacity(0.65)
                .render()?,
            "Aligned X samples accumulate into bands; automatic bounds contain totals.\nEach band's thickness represents its original supplied value.",
        ),
        Panel::new(
            AreaChart::from_series(&areas, 0.0..8.0, 0.0..100.0)
                .percent_stacked()
                .size(SIZE)
                .labels("13 / Percent-stacked areas", "Time (h)", "Request share")
                .opacity(0.65)
                .y_tick_format(TickFormat::Percent { places: 0 })
                .render()?,
            "The same raw samples normalize to 100 at each X coordinate.\nIndependent series groups retain their matching fill legend entries.",
        ),
        Panel::new(
            PieChart::new(&slices)
                .size(SIZE)
                .title("14 / Pie: cloud cost mix")
                .render()?,
            "Positive slice values become editable native wedge polygons.\nLegend values and percentages here are explicit caller-supplied text.",
        ),
        Panel::new(
            PieChart::new(&slices)
                .donut(0.55)
                .size(SIZE)
                .title("15 / Donut: cloud cost mix")
                .sketch(sketch)
                .render()?,
            "A configurable inner-radius fraction produces native ring sectors.\nSlice geometry and each slice's legend entry share an inner group.",
        ),
        Panel::new(
            ErrorBarChart::new(
                &[
                    VerticalInterval::new(1., 2., 3., 6.),
                    VerticalInterval::new(3., 3., 5., 8.),
                    VerticalInterval::new(5., 4., 4., 4.),
                    VerticalInterval::new(7., 2., 6., 7.),
                ],
                "Supplied min/max",
                0.0..8.0,
                0.0..10.0,
            )
            .size(SIZE)
            .labels("16 / Asymmetric error bars", "Observation", "Estimate")
            .color(BLUE)
            .line_width(3)
            .opacity(0.8)
            .stroke_style(StrokeStyle::Dashed)
            .cap_half_width(10)
            .marker(5, false)
            .render()?,
            "Caller-supplied limits, including a zero-length interval; no statistics computed.\nEach cap/stem/center compound is grouped; the legend matches its styling.",
        ),
        Panel::new(
            BandChart::new(
                &[1., 3., 5., 7., 9.],
                &[3., 2., 4., 3., 5.],
                &[3., 7., 8., 6., 5.],
                "Supplied range",
                0.0..10.0,
                0.0..10.0,
            )
            .center(&[3., 4., 6., 5., 5.])
            .boundary_lines(true)
            .color(GREEN)
            .opacity(0.3)
            .line_width(2)
            .size(SIZE)
            .labels("17 / Varying envelope", "Time (s)", "Value")
            .render()?,
            "Supplied lower/upper limits taper to zero width at both endpoints.\nOptional center and boundary lines are independent editable paths.",
        ),
        Panel::new(
            HistogramChart::new(
                &[-10., -8., -4., 0., 2., 10.],
                &[4., 0., 8., 6., 4.],
                -10.0..10.0,
                -2.0..10.0,
            )
            .size(SIZE)
            .labels("18 / Explicit-bin histogram", "Measurement", "Count")
            .color(ORANGE)
            .opacity(0.7)
            .render()?,
            "Unequal numeric bin widths, a zero-count bin and an interior zero baseline.\nSupplied counts are heights, not density; no raw-sample binning is performed.",
        ),
        Panel::new(
            StepChart::new(
                &[(1., 2.), (3., 7.), (5., 7.), (7., 4.)],
                0.0..10.0,
                0.0..10.0,
            )
            .size(SIZE)
            .labels("19 / Right-continuous steps", "Time (s)", "Level")
            .color(ORANGE)
            .line_width(4)
            .sketch(sketch)
            .render()?,
            "Strictly increasing X; each level holds until the next sample.\nThe final hold reaches the right bound; equal successive levels are retained.",
        ),
        Panel::new(
            StepChart::ecdf(&[6., 2., 4., 2., 8., 5.], 0.0..10.0)?
                .size(SIZE)
                .labels("20 / Empirical CDF", "Sample value", "Cumulative share")
                .y_tick_format(TickFormat::FractionPercent { places: 0 })
                .render()?,
            "ECDF explicitly sorts samples and aggregates ties as count(sample <= x) / n.\nBounds expose both tails; FractionPercent displays 0..1 as 0%..100%.",
        ),
        Panel::new(
            LineChart::new(
                &[(1., 1.), (10., 10.), (100., 100.), (1000., 1000.)],
                1.0..1000.0,
                1.0..1000.0,
            )
            .x_scale(AxisScale::Log10)
            .y_scale(AxisScale::Log10)
            .size(SIZE)
            .labels("21 / Log-log axes", "Input", "Response")
            .render()?,
            "Independent positive base-10 X and Y scales with explicit containing bounds.\nSuccessive decades occupy equal distances on both axes.",
        ),
        Panel::new(
            ScatterChart::new(
                &[(1e-14, 2.), (1e-13, 6.), (1e-12, 4.)],
                1e-15..1e-11,
                0.0..8.0,
            )
            .x_scale(AxisScale::Log10)
            .size(SIZE)
            .marker(6, false)
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .labels("22 / Log-X scatter", "Concentration", "Response")
            .render()?,
            "Logarithmic X with linear Y and hollow scatter markers.\nScientific tick formatting keeps tiny positive values distinguishable.",
        ),
        Panel::new(
            LineChart::new(
                &[(0., 1.), (2., 10.), (4., 100.), (6., 1000.)],
                0.0..6.0,
                1.0..1000.0,
            )
            .y_scale(AxisScale::Log10)
            .size(SIZE)
            .labels("23 / Log-Y line", "Time (h)", "Population")
            .render()?,
            "Linear X with logarithmic Y; source observations remain unchanged.\nDefault Auto numeric formatting is used here and on the log-log panel.",
        ),
    ])
}

fn calendar_charts() -> Result<Vec<Panel>> {
    let date = |month, day| NaiveDate::from_ymd_opt(2024, month, day).unwrap();
    let dates = [
        (date(2, 28), 2.),
        (date(2, 29), 5.),
        (date(3, 3), 3.),
        (date(3, 7), 6.),
    ];
    let start = Utc.with_ymd_and_hms(2024, 12, 31, 23, 0, 0).unwrap();
    let end = start + Duration::hours(2);
    let instants = [
        (start, 2.),
        (start + Duration::minutes(30), 5.),
        (start + Duration::minutes(90), 3.),
        (end, 6.),
    ];
    Ok(vec![
        Panel::new(
            DateLineChart::new(&dates, date(2, 28)..date(3, 7), 0.0..8.0)
                .size(SIZE)
                .tick_density(3, 5)
                .labels("24 / Calendar-date line", "Calendar date", "Value")
                .shaded_interval(ShadedInterval::x(date(2, 29)..date(3, 3), (255, 236, 153)))
                .reference_rule(ReferenceRule::vertical(date(3, 3), ORANGE))
                .callout(Callout::new((date(2, 29), 5.), "Leap day", (30., -55.)))
                .render()?,
            "Typed NaiveDate values cross leap day and a month boundary with elapsed spacing.\nDate-typed windows, reference rules and callouts use that same mapping.",
        ),
        Panel::new(
            DateScatterChart::new(
                &[(date(3, 3), 3.), (date(2, 29), 5.), (date(2, 29), 2.)],
                date(2, 28)..date(3, 7),
                0.0..8.0,
            )
            .size(SIZE)
            .tick_density(3, 5)
            .marker(6, false)
            .opacity(0.7)
            .labels("25 / Calendar-date scatter", "Calendar date", "Value")
            .render()?,
            "Unordered observations and repeated dates are valid scatter input.\nISO date labels are measured; marker size, fill and opacity are configurable.",
        ),
        Panel::new(
            UtcLineChart::new(&instants, start..end, 0.0..8.0)
                .size(SIZE)
                .tick_density(2, 5)
                .labels("26 / UTC line across midnight", "UTC instant", "Value")
                .reference_rule(
                    ReferenceRule::vertical(start + Duration::hours(1), RED)
                        .stroke_style(StrokeStyle::Dashed),
                )
                .render()?,
            "Typed DateTime<Utc> values span a year boundary with elapsed-time spacing.\nMeasured labels include the date and literal Z; a UTC rule marks midnight.",
        ),
        Panel::new(
            UtcScatterChart::new(
                &[
                    instants[2],
                    instants[0],
                    instants[1],
                    instants[1],
                    instants[3],
                ],
                start..end,
                0.0..8.0,
            )
            .size(SIZE)
            .tick_density(2, 5)
            .marker(4, true)
            .opacity(0.65)
            .labels("27 / UTC scatter", "UTC instant", "Value")
            .render()?,
            "Unordered UTC samples and exact duplicate observations remain separate ellipses.\nLower tick density leaves room for full date/time labels.",
        ),
    ])
}

fn backend_panel() -> Result<Scene> {
    let mut scene = Scene::new();
    let options = scene.drawing_options();
    options.new_group().scope(|| -> Result<()> {
        let root = ExcalidrawBackend::new(&mut scene, SIZE)?.into_drawing_area();
        root.fill(&WHITE)?;
        root.draw(&Text::new(
            "29 / Direct Plotters backend",
            (24, 24),
            ("Excalifont", 24).into_font(),
        ))?;
        // The drawing area must be dropped before scene operations or serialization.
        let (left, right) = root.split_horizontally(620);
        let mut chart = ChartBuilder::on(&left)
            .margin_top(90)
            .margin_right(30)
            .margin_bottom(30)
            .margin_left(30)
            .x_label_area_size(45)
            .y_label_area_size(55)
            .build_cartesian_2d(0.0..10.0, 0.0..10.0)?;
        chart
            .configure_mesh()
            .disable_mesh()
            .x_labels(4)
            .y_labels(4)
            .label_style(("Excalifont", 16))
            .draw()?;
        options.with_stroke_style(StrokeStyle::Dashed, || {
            chart.draw_series(LineSeries::new(
                [(1., 2.), (3., 6.), (6., 4.), (9., 8.)],
                BLUE_COLOR.stroke_width(3),
            ))
        })?;
        options.with_style(
            SketchStyle::new(1, FillStyle::CrossHatch)?,
            || -> Result<()> {
                right.draw(&Rectangle::new(
                    [(30, 100), (150, 180)],
                    BLUE_COLOR.mix(0.6).filled(),
                ))?;
                right.draw(&Circle::new(
                    (240, 140),
                    40,
                    RGBColor(47, 158, 68).stroke_width(3),
                ))?;
                right.draw(&Polygon::new(
                    vec![(40, 320), (120, 220), (190, 340), (40, 320)],
                    RGBColor(230, 119, 0).mix(0.6).filled(),
                ))?;
                Ok(())
            },
        )?;
        right.draw(&Text::new(
            "Native primitives",
            (30, 380),
            ("Excalifont", 20).into_font(),
        ))?;
        right.draw(&Text::new(
            "Rotated text",
            (350, 340),
            ("Excalifont", 18)
                .into_font()
                .transform(FontTransform::Rotate270),
        ))?;
        root.present()?;
        Ok(())
    })?;
    Ok(scene)
}

const BLUE_COLOR: RGBColor = RGBColor(BLUE.0, BLUE.1, BLUE.2);

fn scene_panel() -> Result<Scene> {
    let mut scene = Scene::new();
    let options = scene.drawing_options();
    let source = SourceLink::new(ReportUrl::new(
        "https://example.org/reports/synthetic#method",
    )?)
    .with_provenance(Provenance::new(
        "method-note",
        Some("feature-gallery"),
        serde_json::json!({"synthetic": true, "units": "scene coordinates"}),
    )?);
    options.new_group().scope(|| -> Result<()> {
        scene.add_note("30 / Native notes, arrows and source links", (24., 24.), 24.)?;
        options.with_source(Some(&source), || {
            scene.add_note("Source report (placeholder HTTPS link)\nMethod: synthetic data, no live refresh.\nTypography: café, 20°C, ±2, −3.",
                (80., 120.), 22.)
        })?;
        scene.add_note("Select the source note to open its link.\nStructured provenance lives in customData.excaliplot.",
            (80., 300.), 20.)?;
        options.with_stroke_style(StrokeStyle::Dotted, || {
            scene.add_arrow((720., 280.), (550., 210.), ArrowStyle {
                head: ArrowHead::Triangle, color: GREEN, width: 3, opacity: 0.8,
            })
        })?;
        Ok(())
    })?;
    // Offset translation is distinct from the bounds-aligned placement used below.
    scene.translate((12., 8.))?;
    Ok(scene)
}

fn gallery() -> Result<Scene> {
    let mut panels = numeric_charts()?;
    panels.extend(calendar_charts()?);
    panels.push(Panel::new(
        ScatterChart::auto(&[(42., 7.)]).size(SIZE).marker(8, true)
            .labels("28 / Singleton automatic scatter", "Input", "Response").render()?,
        "A singleton is valid scatter data; constant axes receive containing padding.\nThis panel also demonstrates a dashed border inside a named native frame.",
    ));
    panels.push(Panel::new(backend_panel()?,
        "Real ChartBuilder, mesh and LineSeries calls beside vector primitives.\nScoped grouping, sketch and stroke styles; a dotted border inside a sibling frame."));
    panels.push(Panel::new(scene_panel()?,
        "Scene-only multiline notes and arrows, with an opt-in source link and provenance.\nNotes are added before borders/frames so decoration bounds include their full boxes."));

    let mut gallery = Scene::new();
    gallery.add_note("Excaliplot / Feature gallery", (40., 24.), 36.)?;
    gallery.add_note(
        "30 panels, one rendering each. All data is synthetic. Read left to right, then down.\nClean, roughness 1 and roughness 2 appear across distinct panels, with solid, hachure and cross-hatch fills.\nSelect a panel to move it; ungroup to edit series and individual marks. Native edits do not recalculate data.",
        (40., 85.), 22.,
    )?;
    let sections = [
        "Lines, scatter, annotations and automatic layout",
        "Single and grouped bars, signed data and horizontal orientation",
        "Grouped horizontal bars, absolute stacks and percentage stacks",
        "Custom baselines, overlapping areas and stacked areas",
        "Normalized areas, pie wedges and donut sectors",
        "Supplied uncertainty and pre-binned distributions",
        "Steps, empirical distributions and logarithmic axes",
        "Mixed logarithmic scales and calendar dates",
        "Calendar scatter and UTC instants",
        "Automatic singleton ranges, custom Plotters drawing and native scene features",
    ];
    let mut row_top = 220.;
    // Bounds-driven rows leave room for measured multiline notes and frame padding.
    for (row, chunk) in panels.chunks_mut(3).enumerate() {
        gallery.add_note(sections[row], (40., row_top), 26.)?;
        let mut next_x = 40.;
        let mut row_bottom = row_top;
        for (column, panel) in chunk.iter_mut().enumerate() {
            let mut scene = std::mem::take(&mut panel.scene);
            scene.add_note(panel.note, (24., f64::from(SIZE.1) + 16.), 18.)?;
            let stroke = if row == 9 {
                [StrokeStyle::Dashed, StrokeStyle::Dotted, StrokeStyle::Solid][column]
            } else {
                StrokeStyle::Solid
            };
            scene.add_border(
                12.,
                BorderStyle {
                    color: (173, 181, 189),
                    width: 2,
                    stroke,
                },
            )?;
            if row == 9 {
                scene.add_frame(
                    16.,
                    Some(["Automatic ranges", "Plotters backend", "Source and method"][column]),
                )?;
            }
            scene.place_at((next_x, row_top + 60.))?;
            let bounds = scene.bounds()?.expect("a rendered panel is nonempty");
            next_x = bounds.x + bounds.width + 40.;
            row_bottom = row_bottom.max(bounds.y + bounds.height);
            gallery.append(scene)?;
        }
        row_top = row_bottom + 55.;
    }
    Ok(gallery)
}

fn main() -> Result<()> {
    let mut destination = None;
    let mut overwrite = Overwrite::Refuse;
    let mut library = false;
    for arg in std::env::args_os().skip(1) {
        if arg == OsStr::new("--overwrite") {
            overwrite = Overwrite::Allow;
        } else if arg == OsStr::new("--library") {
            library = true;
        } else if arg == OsStr::new("--help") {
            println!(
                "Usage: cargo run --example feature_gallery -- [--overwrite] [--library] [destination.excalidraw]\n\
                 --library also exports a single native library item beside the scene.\n\
                 Default: output/feature-gallery.excalidraw"
            );
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') || destination.is_some() {
            return Err("expected [--overwrite] [--library] [destination.excalidraw]".into());
        } else {
            destination = Some(PathBuf::from(arg));
        }
    }
    let destination = destination::resolve(destination, "feature-gallery.excalidraw")?;
    let library_path = destination.with_extension("excalidrawlib");
    if library && destination == library_path {
        return Err("scene destination must differ from the companion .excalidrawlib path".into());
    }
    let start = Instant::now();
    let scene = gallery()?;
    scene.write(&destination, overwrite)?;
    println!(
        "Wrote {} (30 panels, {:?})\n{:#?}",
        destination.display(),
        start.elapsed(),
        scene.diagnostics()
    );
    if library {
        scene.write_library(&library_path, overwrite)?;
        println!(
            "Wrote {} (one item containing the complete gallery)",
            library_path.display()
        );
    }
    Ok(())
}
