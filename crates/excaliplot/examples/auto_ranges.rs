//! Irregular numeric X and constant Y with inferred ranges, beside fixed bounds.
use excaliplot::{LineChart, Scene};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("auto_ranges", || {
        let points = [
            (0., -5.),
            (1., 2.),
            (3., 8.),
            (8., 3.),
            (13., 10.),
            (20., 5.),
        ];
        let constant = points.map(|(x, _)| (x, 5.));
        let mut comparison = Scene::new();
        for (index, chart) in [
            LineChart::auto(&points).labels("Automatic irregular X", "Time (s)", "Value"),
            LineChart::auto(&constant).labels("Automatic constant Y", "Time (s)", "Value"),
            LineChart::new(&points, -5.0..25.0, -10.0..15.0).labels(
                "Explicit comparison",
                "Time (s)",
                "Value",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut panel = chart.render()?;
            panel.place_at((0., index as f64 * 440.))?;
            comparison.append(panel)?;
        }
        Ok(comparison)
    })
}
