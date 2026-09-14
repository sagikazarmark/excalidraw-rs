//! JSON stdin/stdout bridge for independent browser transport checks.
#[cfg(feature = "embedded")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use excalidraw_document::{Document, embedded::*};
    use std::io::{Read, Write};
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: serde_json::Value = serde_json::from_str(&input)?;
    let operation = request["operation"].as_str().ok_or("operation required")?;
    let output = match operation {
        "embed" => {
            let doc = Document::from_value(request["document"].clone())?;
            let png = STANDARD.decode(request["png"].as_str().ok_or("png required")?)?;
            serde_json::json!({"png":STANDARD.encode(embed_png(&png,&doc)?),"svg":embed_svg(request["svg"].as_str().ok_or("svg required")?,&doc)?})
        }
        "extract" => {
            let png = STANDARD.decode(request["png"].as_str().ok_or("png required")?)?;
            serde_json::json!({"png":extract_png(&png,16*1024*1024)?.into_value(),"svg":extract_svg(request["svg"].as_str().ok_or("svg required")?,16*1024*1024)?.into_value()})
        }
        _ => return Err("unknown operation".into()),
    };
    std::io::stdout().write_all(&serde_json::to_vec(&output)?)?;
    Ok(())
}
#[cfg(not(feature = "embedded"))]
fn main() {
    eprintln!("Enable --features embedded to run this transport bridge");
}
