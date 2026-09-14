//! Preserving JSON stdin/stdout codec for independent compatibility fixtures.
use excalidraw_document::Document;
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes)?;
    let document = Document::from_slice(&bytes)?;
    std::io::stdout().write_all(&document.to_vec()?)?;
    Ok(())
}
