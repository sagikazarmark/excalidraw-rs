//! A composed chart with one editable two-line method note, border and native frame.
use excaliplot::{BorderStyle, LineChart, Scene};
#[path = "support/output.rs"]
mod output;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    output::run("method_note", || {
        let chart = LineChart::new(&[(0., 20.), (1., 80.), (2., 30.)], 0.0..2.0, 0.0..100.0)
            .labels("Latency", "Time", "Milliseconds")
            .size((500, 340))
            .render()?;
        let mut note = Scene::new();
        note.add_note(
            "Method: synthetic samples\nOne-minute windows; no smoothing.",
            (24., 364.),
            20.,
        )?;
        let mut report = Scene::new();
        report.append(chart)?;
        report.append(note)?;
        report.add_border(12., BorderStyle::default())?;
        report.add_frame(16., Some("Measurement method"))?;
        report.place_at((0., 0.))?;
        Ok(report)
    })
}
