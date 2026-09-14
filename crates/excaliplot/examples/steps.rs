//! Ordered supplied levels beside an ECDF of unsorted samples with ties.
use excaliplot::{Scene, StepChart, TickFormat};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("steps", || {
        let mut scene = Scene::new();
        let mut steps = StepChart::new(
            &[(1., 2.), (3., 7.), (5., 4.), (8., 6.)],
            0.0..10.0,
            0.0..10.0,
        )
        .labels("Supplied post-steps", "Time", "Level")
        .render()?;
        steps.place_at((0., 0.))?;
        scene.append(steps)?;
        let mut ecdf = StepChart::ecdf(&[8., 2., 5., 2., 6., 5.], 0.0..10.0)?
            .labels("Empirical CDF with ties", "Measurement", "Probability")
            .y_tick_format(TickFormat::FractionPercent { places: 0 })
            .render()?;
        ecdf.place_at((0., 420.))?;
        scene.append(ecdf)?;
        Ok(scene)
    })
}
