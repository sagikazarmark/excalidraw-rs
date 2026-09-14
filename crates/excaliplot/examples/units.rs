//! Scientific labels using the verified degree and plus/minus glyphs.
//! Micro sign U+00B5 is absent from the pinned editor font and remains unsupported.
use excaliplot::LineChart;
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("units", || {
        let mut scene = LineChart::new(
            &[(0., 20.), (1., 22.), (2., 19.), (3., 23.)],
            0.0..3.0,
            0.0..30.0,
        )
        .labels("Temperature ±2°C", "Time (s)", "Temperature (°C)")
        .render()?;
        scene.add_note(
            "Method: synthetic samples\nUncertainty: ±2°C; interval: 1 s.",
            (24., 424.),
            20.,
        )?;
        Ok(scene)
    })
}
