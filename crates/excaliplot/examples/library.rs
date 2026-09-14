//! Import these files from Excalidraw's library panel, then click an item to insert.
use excaliplot::{
    BorderStyle, Callout, LineChart, NamedSeries, Overwrite, ScatterChart, Scene, StrokeStyle,
};
#[path = "support/destination.rs"]
mod destination;
#[path = "support/source_label.rs"]
mod source_label;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let destination = destination::resolve(destination, "library")?;
    std::fs::create_dir(&destination)?;
    let mut diagram = Scene::new();
    for (index, name) in ["Before", "After"].into_iter().enumerate() {
        let mut panel = LineChart::from_series(
            &[
                NamedSeries::new(
                    "Measured",
                    &[(0., 20.), (1., 80.), (2., 30.)],
                    (25, 113, 194),
                ),
                NamedSeries::new("Target", &[(0., 40.), (1., 40.), (2., 40.)], (224, 49, 49)),
            ],
            0.0..2.0,
            0.0..100.0,
        )
        .labels("Latency", "Time", "Milliseconds")
        .size((500, 340))
        .callout(Callout::new((1., 80.), "Peak", (30., -40.)).font_size(16.))
        .render()?;
        source_label::add_source_label(&mut panel, (500, 340), (350, 310))?;
        panel.add_note(
            "Method: synthetic samples\nOne-minute windows.",
            (24., 364.),
            20.,
        )?;
        panel.add_border(
            12.,
            BorderStyle {
                width: 4,
                stroke: StrokeStyle::Dashed,
                ..BorderStyle::default()
            },
        )?;
        panel.add_frame(16., Some(name))?;
        panel.place_at((index as f64 * 600., 0.))?;
        diagram.append(panel)?;
    }
    diagram.add_border(24., BorderStyle::default())?;
    diagram.write_library(destination.join("diagram.excalidrawlib"), Overwrite::Refuse)?;
    let mut other = ScatterChart::new(&[(1., 2.), (2., 4.), (4., 3.)], 0.0..5.0, 0.0..5.0)
        .labels("Utilization", "CPU", "Latency")
        .size((500, 340))
        .render()?;
    other.add_frame(16., Some("Independent item"))?;
    other.write_library(destination.join("other.excalidrawlib"), Overwrite::Refuse)?;
    Ok(())
}
