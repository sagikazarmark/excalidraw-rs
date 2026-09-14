use std::{io, path::PathBuf};

/// Keep generated examples together while respecting explicit destinations.
pub fn resolve(destination: Option<PathBuf>, name: &str) -> io::Result<PathBuf> {
    if let Some(destination) = destination {
        return Ok(destination);
    }
    let directory = PathBuf::from("output");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join(name))
}
