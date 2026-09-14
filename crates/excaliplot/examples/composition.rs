//! Two independently editable charts placed by their complete bounds.
use excaliplot::{BarChart, LineChart, Scene};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("composition", compose)
}

fn compose() -> Result<Scene, excaliplot::Error> {
    let mut report = Scene::new();
    let mut latency = LineChart::new(&[(0., 20.), (1., 80.), (2., 30.)], 0.0..2.0, 0.0..100.0)
        .labels("Latency", "Time", "Milliseconds")
        .size((400, 300))
        .render()?;
    latency.place_at((0., 0.))?;
    report.append(latency)?;
    let mut volume = BarChart::new(&[("Read", 80.), ("Write", 40.)], 0.0..100.0)
        .labels("Requests", "Operation", "Count")
        .size((400, 300))
        .render()?;
    volume.place_at((440., 0.))?;
    report.append(volume)?;
    Ok(report)
}
