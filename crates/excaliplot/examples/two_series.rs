use excaliplot::{
    ExcalidrawBackend, FillStyle, LineChart, NamedSeries, Overwrite, Scene, SketchStyle,
};
use plotters::prelude::*;
use std::path::PathBuf;

#[path = "support/destination.rs"]
mod destination;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(PathBuf::from);
    let destination = destination::resolve(destination, "two-series")?;
    std::fs::create_dir_all(&destination)?;
    let measured = [
        (1.0, 2.0),
        (2.0, 5.0),
        (4.0, 3.0),
        (6.0, 8.0),
        (7.0, 6.0),
        (9.0, 9.0),
    ];
    let reference = [
        (1.0, 3.0),
        (2.0, 4.0),
        (4.0, 4.0),
        (6.0, 6.0),
        (7.0, 7.0),
        (9.0, 8.0),
    ];
    for (name, roughness) in [("clean", 0), ("sketch-1", 1), ("sketch-2", 2)] {
        let scene = LineChart::from_series(
            &[
                NamedSeries::new("Measured", &measured, (25, 113, 194)),
                NamedSeries::new("Reference", &reference, (230, 119, 0)),
            ],
            0.0..10.0,
            0.0..10.0,
        )
        .labels("Measured and reference", "Time (s)", "Value")
        .size((800, 440))
        .sketch(SketchStyle::new(roughness, FillStyle::Solid)?)
        .render()?;
        scene.write(
            destination.join(format!("{name}.excalidraw")),
            Overwrite::Refuse,
        )?;
        println!("{name}: {:?}", scene.diagnostics());
    }
    // Composition probe for finite extents, fill patterns, alpha and paint order.
    let mut scene = Scene::new();
    let options = scene.drawing_options();
    let group = options.new_group();
    {
        let root = ExcalidrawBackend::new(&mut scene, (360, 160))?.into_drawing_area();
        group.scope(|| -> Result<(), Box<dyn std::error::Error>> {
            root.fill(&WHITE)?;
            for (index, fill) in [FillStyle::Solid, FillStyle::Hachure, FillStyle::CrossHatch]
                .into_iter()
                .enumerate()
            {
                let x = 20 + index as i32 * 110;
                options.with_style(SketchStyle::new(1, fill)?, || {
                    root.draw(&Rectangle::new(
                        [(x, 30), (x + 90, 110)],
                        BLUE.mix(0.5).filled(),
                    ))
                })?;
            }
            root.draw(&Rectangle::new(
                [(50, 70), (290, 130)],
                RED.mix(0.5).filled(),
            ))?;
            root.draw(&Text::new(
                "Solid / Hachure / Cross-hatch",
                (25, 135),
                ("Excalifont", 16),
            ))?;
            Ok(())
        })?;
    }
    scene.write(destination.join("paints.excalidraw"), Overwrite::Refuse)?;
    Ok(())
}
