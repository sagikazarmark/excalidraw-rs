//! Explicit multi-decade line/scatter axes and a tiny positive log range.
use excaliplot::{AxisScale, LineChart, NamedSeries, ScatterChart, Scene, TickFormat};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("log_axes", || {
        let points = [(1., 1.), (10., 10.), (100., 100.), (1000., 1000.)];
        let names = [NamedSeries::new("Observed", &points, (25, 113, 194))];
        let mut scene = Scene::new();
        for (index, (title, x, y)) in [
            ("Log X / linear Y", AxisScale::Log10, AxisScale::Linear),
            ("Linear X / log Y", AxisScale::Linear, AxisScale::Log10),
            ("Log X / log Y", AxisScale::Log10, AxisScale::Log10),
        ]
        .into_iter()
        .enumerate()
        {
            let mut panel = LineChart::from_series(&names, 1.0..1000.0, 1.0..1000.0)
                .x_scale(x)
                .y_scale(y)
                .labels(title, "Input", "Response")
                .size((800, 400))
                .render()?;
            panel.place_at((0., index as f64 * 440.))?;
            scene.append(panel)?;
        }
        let tiny = [(1e-15, 1.), (1e-14, 10.), (1e-13, 100.), (1e-12, 1000.)];
        let names = [NamedSeries::new("Observed", &tiny, (25, 113, 194))];
        let mut panel = ScatterChart::from_series(&names, 1e-15..1e-12, 1.0..1000.0)
            .x_scale(AxisScale::Log10)
            .y_scale(AxisScale::Log10)
            .x_tick_format(TickFormat::Scientific { places: 1 })
            .labels("Small positive log range", "Input (s)", "Response")
            .size((800, 400))
            .render()?;
        panel.place_at((0., 1320.))?;
        scene.append(panel)?;
        Ok(scene)
    })
}
