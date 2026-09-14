use excaliplot::{AreaChart, FillStyle, NamedSlice, Overwrite, PieChart, SketchStyle};
use std::path::PathBuf;

#[path = "support/destination.rs"]
mod destination;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args_os().nth(1).map(PathBuf::from);
    let destination = destination::resolve(destination, "filled-gallery")?;
    std::fs::create_dir(&destination)?;
    let style = SketchStyle::new(1, FillStyle::Solid)?;
    let slices = [
        NamedSlice::new("Major", 75.0, (25, 113, 194)),
        NamedSlice::new("Narrow", 1.0, (230, 119, 0)),
        NamedSlice::new("Rest", 24.0, (47, 158, 68)),
    ];
    let scenes = [
        (
            "area-sketch",
            AreaChart::new(
                &[(1.0, 2.0), (3.0, 6.0), (5.0, 3.0), (9.0, 8.0)],
                0.0..10.0,
                0.0..10.0,
            )
            .sketch(style)
            .render()?,
        ),
        (
            "area-below",
            AreaChart::new(
                &[(1.0, -2.0), (3.0, -4.0), (6.0, -1.0), (9.0, -3.0)],
                0.0..10.0,
                -5.0..5.0,
            )
            .render()?,
        ),
        (
            "area-constant",
            AreaChart::new(&[(1.0, 2.0), (1.0, 2.0), (9.0, 2.0)], 0.0..10.0, 0.0..10.0).render()?,
        ),
        ("pie-sketch", PieChart::new(&slices).sketch(style).render()?),
        (
            "donut-sketch",
            PieChart::new(&slices).donut(0.5).sketch(style).render()?,
        ),
        (
            "pie-single",
            PieChart::new(&[NamedSlice::new("All", 1.0, (25, 113, 194))]).render()?,
        ),
    ];
    for (name, scene) in scenes {
        scene.write(
            destination.join(format!("{name}.excalidraw")),
            Overwrite::Refuse,
        )?;
        println!("{name}: {:?}", scene.diagnostics());
    }
    Ok(())
}
