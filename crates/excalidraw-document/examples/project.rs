//! JSON stdin/stdout adapter for the independent browser serializer oracle.
use excalidraw_document::{Document, ExportMode, Profile};
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let profile = match args.first().map(String::as_str) {
        Some("release") => Profile::V0_18_1,
        Some("snapshot") => Profile::SnapshotAfa3a653,
        _ => return Err("expected release|snapshot local|database".into()),
    };
    let mode = match args.get(1).map(String::as_str) {
        Some("local") => ExportMode::Local,
        Some("database") => ExportMode::Database,
        _ => return Err("expected local|database".into()),
    };
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes)?;
    let document = Document::from_slice(&bytes)?;
    std::io::stdout().write_all(
        &document
            .project_native(profile, mode, "document-oracle")?
            .document
            .to_vec()?,
    )?;
    Ok(())
}
