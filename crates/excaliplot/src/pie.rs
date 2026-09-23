use crate::cartesian::{LEGEND_ROW_HEIGHT, LEGEND_TOP, MARGIN};
use crate::{Error, ExcalidrawBackend, Scene, SketchStyle};
use plotters::prelude::*;
use plotters_backend::text_anchor::{HPos, Pos, VPos};
use std::f64::consts::{PI, TAU};

/// Sized for the legend, not for ticks: a pie has no axes to match.
const LEGEND_FONT: u32 = 16;
const LEGEND_LABEL_OFFSET: u32 = 25;
const LEGEND_GAP: u32 = 16;

fn legend_left(width: u32) -> u32 {
    width * 3 / 5
}

/// A positive pie value with an explicit native label and paint.
#[derive(Clone)]
pub struct NamedSlice<'a> {
    name: &'a str,
    value: f64,
    color: RGBColor,
}

impl<'a> NamedSlice<'a> {
    pub fn new(name: &'a str, value: f64, rgb: (u8, u8, u8)) -> Self {
        Self {
            name,
            value,
            color: RGBColor(rgb.0, rgb.1, rgb.2),
        }
    }
}

/// Root-coordinate wedges with a measured native legend, clockwise from 3 o'clock.
pub struct PieChart<'a> {
    slices: Vec<NamedSlice<'a>>,
    title: &'a str,
    size: (u32, u32),
    hole: f64,
    sketch: SketchStyle,
}

impl<'a> PieChart<'a> {
    pub fn new(slices: &[NamedSlice<'a>]) -> Self {
        Self {
            slices: slices.to_vec(),
            title: "Pie chart",
            size: (640, 400),
            hole: 0.0,
            sketch: SketchStyle::default(),
        }
    }
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.size = size;
        self
    }
    /// Inner radius as a fraction of the outer radius; zero makes a pie.
    pub fn donut(mut self, fraction: f64) -> Self {
        self.hole = fraction;
        self
    }
    pub fn sketch(mut self, style: SketchStyle) -> Self {
        self.sketch = style;
        self
    }
    pub fn render(&self) -> Result<Scene, Error> {
        if !(400..=16384).contains(&self.size.0) || !(300..=16384).contains(&self.size.1) {
            return Err(Error::Invalid(
                "pie size must be 400..=16384 by 300..=16384",
            ));
        }
        let center = ((self.size.0 / 3) as i32, (self.size.1 / 2 + 20) as i32);
        let radius = f64::from(
            (self.size.0 / 3 - 36)
                .min((self.size.1 - 112) / 2)
                .min(legend_left(self.size.0) - self.size.0 / 3 - LEGEND_GAP),
        );
        let total: f64 = self.slices.iter().map(|s| s.value).sum();
        self.validate(radius, total)?;
        let mut scene = Scene::new();
        let options = scene.drawing_options();
        let chart_group = options.new_group();
        chart_group
            .scope(|| -> Result<(), Box<dyn std::error::Error>> {
                {
                    let root = ExcalidrawBackend::new(&mut scene, self.size)?.into_drawing_area();
                    root.fill(&WHITE)?;
                }
                options.with_style(self.sketch, || -> Result<(), Box<dyn std::error::Error>> {
                    let title = TextStyle::from(("Excalifont", 24).into_font())
                        .pos(Pos::new(HPos::Center, VPos::Top));
                    {
                        let root =
                            ExcalidrawBackend::new(&mut scene, self.size)?.into_drawing_area();
                        root.draw_text(self.title, &title, ((self.size.0 / 2) as i32, 24))?;
                    }
                    let mut cumulative = 0.0;
                    let mut start = 0.0;
                    for (i, slice) in self.slices.iter().enumerate() {
                        cumulative += slice.value;
                        let end = if i + 1 == self.slices.len() {
                            TAU
                        } else {
                            cumulative / total * TAU
                        };
                        let points = wedge(center, radius, self.hole, start, end);
                        options.new_group().scope(
                            || -> Result<(), Box<dyn std::error::Error>> {
                                // Bypass Plotters' integer coordinate interface for
                                // arcs, preserving the same scene paint/group route.
                                scene.add_polygon(
                                    &points,
                                    (slice.color.0, slice.color.1, slice.color.2),
                                    1.,
                                )?;
                                let root = ExcalidrawBackend::new(&mut scene, self.size)?
                                    .into_drawing_area();
                                let x = legend_left(self.size.0) as i32;
                                let y = (LEGEND_TOP + i as u32 * LEGEND_ROW_HEIGHT) as i32;
                                root.draw(&Rectangle::new(
                                    [(x, y + 3), (x + 16, y + 17)],
                                    slice.color.filled(),
                                ))?;
                                root.draw_text(
                                    slice.name,
                                    &TextStyle::from(("Excalifont", LEGEND_FONT).into_font()),
                                    (x + LEGEND_LABEL_OFFSET as i32, y),
                                )?;
                                Ok(())
                            },
                        )?;
                        start = end;
                    }
                    Ok(())
                })
            })
            .map_err(|e| Error::Drawing(e.to_string()))?;
        Ok(scene)
    }

    fn validate(&self, radius: f64, total: f64) -> Result<(), Error> {
        if self.slices.is_empty()
            || !total.is_finite()
            || total <= 0.0
            || self
                .slices
                .iter()
                .any(|s| !s.value.is_finite() || s.value <= 0.0)
        {
            return Err(Error::Invalid(
                "pie values and their total must be finite and positive",
            ));
        }
        if !self.hole.is_finite()
            || !(0.0..1.0).contains(&self.hole)
            || (self.hole > 0.0
                && (self.slices.len() < 2
                    || radius * self.hole < 2.0
                    || radius * (1.0 - self.hole) < 2.0))
        {
            return Err(Error::Invalid(
                "donut needs at least two slices, a hole fraction in (0,1), and a hole/ring at least two scene units wide",
            ));
        }
        if self.slices.len() as u64 * u64::from(LEGEND_ROW_HEIGHT)
            > u64::from(self.size.1 - LEGEND_TOP - MARGIN)
        {
            return Err(Error::Invalid("pie legend exceeds chart height"));
        }
        if crate::typography::measure(self.title, 24.0)?.0 > f64::from(self.size.0 - 48) {
            return Err(Error::Invalid("pie title exceeds chart width"));
        }
        let available =
            f64::from(self.size.0 - legend_left(self.size.0) - LEGEND_LABEL_OFFSET - MARGIN);
        for slice in &self.slices {
            if slice.name.trim().is_empty()
                || crate::typography::measure(slice.name, f64::from(LEGEND_FONT))?.0 > available
            {
                return Err(Error::Invalid(
                    "pie legend labels are empty or crowded; increase chart width",
                ));
            }
            // Enforce visible separation at the smaller arc too. The complementary
            // arc matters for almost-full-circle concave sectors.
            if self.slices.len() > 1 {
                let ratio = slice.value / total;
                let arc_radius = if self.hole > 0.0 {
                    radius * self.hole
                } else {
                    radius
                };
                if ratio.min(1.0 - ratio) * TAU * arc_radius < 2.0 {
                    return Err(Error::Invalid(
                        "pie wedge collapses at this resolution; increase chart size or combine small slices",
                    ));
                }
            }
        }
        Ok(())
    }
}

// Explicitly sample in root coordinates: Plotters 0.3.7 Pie ignores translation
// and overshoots the end angle before starting its inner arc. Both arcs here
// share exact angular endpoints. This is sampling, not polygon simplification.
fn wedge(center: (i32, i32), radius: f64, hole: f64, start: f64, end: f64) -> Vec<(f64, f64)> {
    // At most two degrees and 0.25 scene units sagitta, without pixel rounding.
    let step = (PI / 90.0).min(2.0 * (1.0 - 0.25 / radius).acos());
    let steps = ((end - start) / step).ceil() as usize;
    let point = |r: f64, angle: f64| {
        // Exact cardinal endpoints keep adjacent sectors and the 2π seam aligned.
        let (sin, cos) = if angle == 0.0 || angle == TAU {
            (0.0, 1.0)
        } else if angle == PI / 2.0 {
            (1.0, 0.0)
        } else if angle == PI {
            (0.0, -1.0)
        } else if angle == 3.0 * PI / 2.0 {
            (-1.0, 0.0)
        } else {
            angle.sin_cos()
        };
        (f64::from(center.0) + r * cos, f64::from(center.1) + r * sin)
    };
    let angle = |i: usize| {
        if i == steps {
            end
        } else {
            start + (end - start) * i as f64 / steps as f64
        }
    };
    let mut points = Vec::new();
    if hole == 0.0 && end - start < TAU {
        points.push((f64::from(center.0), f64::from(center.1)));
    }
    points.extend((0..=steps).map(|i| point(radius, angle(i))));
    if hole > 0.0 {
        points.extend((0..=steps).rev().map(|i| point(radius * hole, angle(i))));
    }
    points.dedup();
    if points.first() != points.last() {
        points.push(points[0]);
    }
    points
}
