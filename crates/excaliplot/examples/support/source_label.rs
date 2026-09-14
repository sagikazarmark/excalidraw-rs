use excaliplot::{Error, ExcalidrawBackend, Provenance, ReportUrl, Scene, SourceLink};
use plotters::prelude::*;

pub fn add_source_label(
    scene: &mut Scene,
    size: (u32, u32),
    position: (i32, i32),
) -> Result<(), Error> {
    let source = SourceLink::new(ReportUrl::new(
        "https://example.org/reports/latency#summary",
    )?)
    .with_provenance(Provenance::new(
        "report-label",
        Some("synthetic-latency"),
        serde_json::json!({"synthetic": true, "period": "2026-09"}),
    )?);
    let options = scene.drawing_options();
    let root = ExcalidrawBackend::new(scene, size)?.into_drawing_area();
    options.with_source(Some(&source), || {
        root.draw(&Text::new(
            "Source report",
            position,
            ("Excalifont", 18).into_font().color(&BLUE),
        ))
        .map_err(|error| Error::Drawing(error.to_string()))
    })
}
