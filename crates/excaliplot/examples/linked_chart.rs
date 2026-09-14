//! A synthetic chart with one explicit source label. The URL is a placeholder,
//! not a live report or synchronization endpoint. Save editor changes separately.
use excaliplot::LineChart;
#[path = "support/output.rs"]
mod output;
#[path = "support/source_label.rs"]
mod source_label;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("linked_chart", || {
        let mut scene = LineChart::new(&[(0., 20.), (1., 80.), (2., 30.)], 0.0..2.0, 0.0..100.0)
            .labels("Synthetic latency", "Time", "Milliseconds")
            .render()?;
        source_label::add_source_label(&mut scene, (640, 460), (80, 430))?;
        Ok(scene)
    })
}
