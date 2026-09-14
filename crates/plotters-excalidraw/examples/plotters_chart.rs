//! Ordinary Plotters drawing, with native scene decoration after rendering.
use plotters::prelude::*;
use plotters_excalidraw::{BorderStyle, ExcalidrawBackend, Overwrite, Scene};
use std::{ffi::OsStr, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut destination = None;
    let mut overwrite = Overwrite::Refuse;
    for arg in std::env::args_os().skip(1) {
        if arg == OsStr::new("--overwrite") {
            overwrite = Overwrite::Allow;
        } else if arg == OsStr::new("--help") {
            println!(
                "Usage: cargo run -p plotters-excalidraw --example plotters_chart -- [--overwrite] [destination.excalidraw]\nDefault: output/plotters-chart.excalidraw"
            );
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') || destination.is_some() {
            return Err("expected [--overwrite] [destination.excalidraw]".into());
        } else {
            destination = Some(PathBuf::from(arg));
        }
    }
    let destination = match destination {
        Some(path) => path,
        None => {
            std::fs::create_dir_all("output")?;
            PathBuf::from("output/plotters-chart.excalidraw")
        }
    };
    let mut scene = Scene::new();
    let group = scene.drawing_options().new_group();
    group.scope(|| -> Result<(), Box<dyn std::error::Error>> {
        let root = ExcalidrawBackend::new(&mut scene, (640, 400))?.into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .caption("Plotters to Excalidraw", ("Excalifont", 24))
            .margin(30)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(0.0..10.0, 0.0..10.0)?;
        chart
            .configure_mesh()
            .disable_mesh()
            .label_style(("Excalifont", 16))
            .axis_desc_style(("Excalifont", 18))
            .x_desc("Time")
            .y_desc("Value")
            .draw()?;
        chart.draw_series(LineSeries::new(
            [(1., 2.), (5., 8.), (9., 4.)],
            BLUE.stroke_width(2),
        ))?;
        root.present()?;
        Ok(())
    })?;
    scene.add_note("Editable native elements", (24., 420.), 20.)?;
    scene.add_border(12., BorderStyle::default())?;
    scene.write(&destination, overwrite)?;
    println!("Wrote {}", destination.display());
    Ok(())
}
