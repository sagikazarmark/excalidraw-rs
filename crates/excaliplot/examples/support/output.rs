use excaliplot::{Overwrite, Scene};
use std::{ffi::OsStr, path::PathBuf, time::Instant};

#[path = "destination.rs"]
mod destination;

pub fn run(
    name: &str,
    draw: impl FnOnce() -> Result<Scene, excaliplot::Error>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut overwrite = Overwrite::Refuse;
    let mut destination = None;
    for arg in std::env::args_os().skip(1) {
        if arg == OsStr::new("--overwrite") {
            overwrite = Overwrite::Allow;
        } else if arg == OsStr::new("--help") {
            println!(
                "Usage: cargo run --example {name} -- [--overwrite] [destination.excalidraw]\nDefault: output/{name}.excalidraw"
            );
            return Ok(());
        } else if arg.to_string_lossy().starts_with('-') || destination.is_some() {
            return Err("expected [--overwrite] [destination.excalidraw]".into());
        } else {
            destination = Some(PathBuf::from(arg));
        }
    }
    let destination = destination::resolve(destination, &format!("{name}.excalidraw"))?;
    let start = Instant::now();
    let scene = draw()?;
    let bytes = scene.to_bytes()?.len();
    let generation = start.elapsed();
    scene.write(&destination, overwrite)?;
    println!(
        "Wrote {} ({bytes} bytes, {generation:?} generation including serialization)\n{:#?}",
        destination.display(),
        scene.diagnostics()
    );
    Ok(())
}
