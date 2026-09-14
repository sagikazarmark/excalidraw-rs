//! Shared explicit ranges: measured decimals, compact right legend, legend off.
use excaliplot::{LegendPosition, LineChart, NamedSeries, Scene, TickFormat};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("layout", || {
        let a = [(0., 0.2), (2e8, 0.5), (5e8, 0.3), (1e9, 0.8)];
        let b = [(0., 0.4), (2e8, 0.3), (5e8, 0.6), (1e9, 0.9)];
        let series = [
            NamedSeries::new("Primary observation", &a, (25, 113, 194)),
            NamedSeries::new("Secondary observation", &b, (224, 49, 49)),
        ];
        let mut comparison = Scene::new();
        for (index, (title, format, legend)) in [
            (
                "Decimal comparison",
                TickFormat::Auto,
                LegendPosition::Right,
            ),
            (
                "Compact right legend",
                TickFormat::Scientific { places: 1 },
                LegendPosition::Right,
            ),
            (
                "Compact legend off",
                TickFormat::Scientific { places: 1 },
                LegendPosition::Off,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut panel = LineChart::from_series(&series, 0.0..1e9, 0.0..1.0)
                .labels(title, "Requests", "Success (%)")
                .x_tick_format(format)
                .y_tick_format(TickFormat::FractionPercent { places: 0 })
                .legend(legend)
                .size((1000, 400))
                .render()?;
            panel.place_at((0., index as f64 * 440.))?;
            comparison.append(panel)?;
        }
        let error = LineChart::from_series(&series, 0.0..1e9, 0.0..1.0)
            .size((400, 300))
            .render()
            .err()
            .ok_or(excaliplot::Error::Invalid(
                "impossible layout unexpectedly fit",
            ))?;
        eprintln!("Expected impossible fit: {error}");
        Ok(comparison)
    })
}
