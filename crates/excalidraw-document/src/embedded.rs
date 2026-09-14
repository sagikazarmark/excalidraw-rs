//! Native PNG tEXt / SVG metadata scene transport. No rasterization or image
//! decoding. Extraction has an explicit uncompressed-byte limit.
use crate::{Document, Error, wire};
use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use serde_json::{Value, json};
use std::io::{Read, Write};

const MIME: &str = "application/vnd.excalidraw+json";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
fn failure(message: impl Into<String>) -> Error {
    Error::at("/embedded", message)
}
fn encode(document: &Document) -> Result<String, Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&document.to_vec()?)
        .map_err(|e| failure(e.to_string()))?;
    let bytes = encoder.finish().map_err(|e| failure(e.to_string()))?;
    let byte_string: String = bytes.into_iter().map(char::from).collect();
    Ok(serde_json::to_string(
        &json!({"version":"1","encoding":"bstring","compressed":true,"encoded":byte_string}),
    )?)
}
fn decode(text: &str, limit: usize) -> Result<Document, Error> {
    if text.len() > limit.saturating_mul(8).saturating_add(1024) {
        return Err(failure("encoded payload exceeds limit"));
    }
    let value = wire::parse(text.as_bytes())?;
    if value.get("type").and_then(Value::as_str) == Some("excalidraw") {
        if text.len() > limit {
            return Err(failure("scene exceeds limit"));
        }
        return Document::from_value(value);
    }
    if value.get("encoding").and_then(Value::as_str) != Some("bstring")
        || value.get("version").is_some_and(|v| v != &json!("1"))
    {
        return Err(failure("unsupported payload encoding/version"));
    }
    let encoded = value
        .get("encoded")
        .and_then(Value::as_str)
        .ok_or_else(|| failure("missing encoded string"))?;
    let bytes: Vec<u8> = encoded
        .chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| failure("invalid byte string")))
        .collect::<Result<_, _>>()?;
    let compressed = value
        .get("compressed")
        .and_then(Value::as_bool)
        .ok_or_else(|| failure("missing compression flag"))?;
    let bytes = if compressed {
        let mut result = Vec::new();
        ZlibDecoder::new(bytes.as_slice())
            .take(limit.saturating_add(1) as u64)
            .read_to_end(&mut result)
            .map_err(|e| failure(e.to_string()))?;
        result
    } else {
        bytes
    };
    if bytes.len() > limit {
        return Err(failure("scene exceeds limit"));
    }
    Document::from_slice(&bytes)
}
struct Chunk<'a> {
    kind: &'a [u8],
    data: &'a [u8],
    raw: &'a [u8],
}
fn chunks(png: &[u8]) -> Result<Vec<Chunk<'_>>, Error> {
    if !png.starts_with(PNG) {
        return Err(failure("invalid PNG signature"));
    }
    let mut offset = 8;
    let mut result = Vec::new();
    let mut ended = false;
    let mut image_data = false;
    let mut image_data_ended = false;
    while offset < png.len() {
        let header = png
            .get(offset..offset + 8)
            .ok_or_else(|| failure("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(length)
            .and_then(|n| n.checked_add(12))
            .ok_or_else(|| failure("PNG length overflow"))?;
        let raw = png
            .get(offset..end)
            .ok_or_else(|| failure("truncated PNG data"))?;
        let kind = &raw[4..8];
        let data = &raw[8..8 + length];
        let crc = u32::from_be_bytes(raw[8 + length..].try_into().unwrap());
        if crc32fast::hash(&raw[4..8 + length]) != crc {
            return Err(failure("PNG checksum mismatch"));
        }
        if result.is_empty() && (kind != b"IHDR" || length != 13) {
            return Err(failure("PNG must begin with IHDR"));
        }
        if kind == b"IHDR" && !result.is_empty() {
            return Err(failure("duplicate PNG IHDR"));
        }
        if kind == b"IDAT" {
            if image_data_ended {
                return Err(failure("nonconsecutive PNG IDAT"));
            }
            image_data = true;
        } else if image_data {
            image_data_ended = true;
        }
        result.push(Chunk { kind, data, raw });
        offset = end;
        if kind == b"IEND" {
            if length != 0 || offset != png.len() {
                return Err(failure("invalid PNG end"));
            }
            ended = true;
            break;
        }
    }
    if !ended || !image_data {
        return Err(failure("missing PNG IEND or IDAT"));
    }
    Ok(result)
}
fn scene_text(chunk: &Chunk<'_>) -> bool {
    chunk.kind == b"tEXt"
        && chunk.data.starts_with(MIME.as_bytes())
        && chunk.data.get(MIME.len()) == Some(&0)
}
fn chunk(kind: &[u8; 4], data: &[u8]) -> Result<Vec<u8>, Error> {
    let length = u32::try_from(data.len()).map_err(|_| failure("PNG metadata too large"))?;
    let mut bytes = Vec::new();
    bytes.extend(length.to_be_bytes());
    bytes.extend(kind);
    bytes.extend(data);
    bytes.extend(crc32fast::hash(&bytes[4..]).to_be_bytes());
    Ok(bytes)
}
pub fn embed_png(png: &[u8], document: &Document) -> Result<Vec<u8>, Error> {
    let chunks = chunks(png)?;
    let text = encode(document)?;
    // PNG tEXt stores Latin-1. JSON punctuation escapes controls; compressed byte
    // string characters above ASCII must be encoded as single bytes, not UTF-8.
    let mut data = MIME.as_bytes().to_vec();
    data.push(0);
    data.extend(
        text.chars()
            .map(|c| u8::try_from(c as u32).map_err(|_| failure("non-Latin1 encoded payload")))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let metadata = chunk(b"tEXt", &data)?;
    let mut result = PNG.to_vec();
    for c in chunks {
        result.extend(c.raw);
        if c.kind == b"IHDR" {
            result.extend(&metadata);
        }
        if scene_text(&c) {
            result.truncate(result.len() - c.raw.len());
        }
    }
    Ok(result)
}
pub fn extract_png(png: &[u8], limit: usize) -> Result<Document, Error> {
    let chunks = chunks(png)?;
    let mut payloads = chunks.iter().filter(|c| scene_text(c));
    let c = payloads
        .next()
        .ok_or_else(|| failure("PNG has no scene metadata"))?;
    if payloads.next().is_some() {
        return Err(failure("ambiguous PNG scene metadata"));
    }
    let text: String = c.data[MIME.len() + 1..]
        .iter()
        .copied()
        .map(char::from)
        .collect();
    decode(&text, limit)
}
fn svg_document(svg: &str) -> Result<roxmltree::Document<'_>, Error> {
    let doc = roxmltree::Document::parse(svg).map_err(|e| failure(e.to_string()))?;
    if doc.root_element().tag_name().name() != "svg" {
        return Err(failure("expected SVG root"));
    }
    Ok(doc)
}
fn scene_metadata(node: roxmltree::Node<'_, '_>) -> bool {
    node.is_element()
        && matches!(node.tag_name().name(), "metadata" | "svg")
        && node.children().any(|n| {
            n.is_comment()
                && n.text()
                    .is_some_and(|v| v.trim() == format!("payload-type:{MIME}"))
        })
}
pub fn embed_svg(svg: &str, document: &Document) -> Result<String, Error> {
    let tree = svg_document(svg)?;
    let root = tree.root_element();
    if root
        .descendants()
        .any(|node| node != root && node.tag_name().name() == "svg" && scene_metadata(node))
    {
        return Err(failure(
            "nested SVG legacy payload must be extracted explicitly before embedding",
        ));
    }
    let encoded = encode(document)?;
    let bytes: Vec<u8> = encoded
        .chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| failure("non-byte payload")))
        .collect::<Result<_, _>>()?;
    let metadata = format!(
        "<metadata xmlns=\"http://www.w3.org/2000/svg\"><!-- payload-type:{MIME} --><!-- payload-version:2 --><!-- payload-start -->{}<!-- payload-end --></metadata>",
        STANDARD.encode(bytes)
    );
    let mut output = svg.to_owned();
    let mut ranges: Vec<_> = root
        .descendants()
        .filter(|n| n.tag_name().name() == "metadata" && scene_metadata(*n))
        .map(|n| n.range())
        .collect();
    // Legacy files place the payload comments/text directly beneath the SVG root.
    if scene_metadata(root) {
        let mut active = false;
        let mut finished = false;
        for child in root.children() {
            if child.is_comment() {
                let text = child.text().unwrap_or("").trim();
                if text == "payload-start" {
                    if active || finished {
                        return Err(failure("ambiguous payload markers"));
                    }
                    active = true;
                    ranges.push(child.range());
                    continue;
                }
                if text == "payload-end" {
                    if !active {
                        return Err(failure("invalid payload markers"));
                    }
                    active = false;
                    finished = true;
                    ranges.push(child.range());
                    continue;
                }
                if text.starts_with("payload-type:") || text.starts_with("payload-version:") {
                    ranges.push(child.range());
                }
            } else if active {
                if !child.is_text() {
                    return Err(failure("unexpected element in SVG payload"));
                }
                ranges.push(child.range());
            }
        }
        if active || !finished {
            return Err(failure("incomplete legacy SVG payload"));
        }
    }
    ranges.sort_by_key(|r| r.start);
    if ranges.windows(2).any(|w| w[0].end > w[1].start) {
        return Err(failure("overlapping SVG scene metadata"));
    }
    ranges.reverse();
    for range in ranges {
        output.replace_range(range, "");
    }
    let parsed = svg_document(&output)?;
    let range = parsed.root_element().range();
    let root_text = &output[range.clone()];
    if root_text.trim_end().ends_with("/>") {
        let end = range.end - 2;
        let qualified = root_text[1..]
            .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .next()
            .ok_or_else(|| failure("missing SVG name"))?
            .to_owned();
        output.replace_range(end..range.end, &format!(">{metadata}</{qualified}>"));
    } else {
        let close = range.start
            + root_text
                .rfind("</")
                .ok_or_else(|| failure("missing SVG close tag"))?;
        output.insert_str(close, &metadata);
    }
    Ok(output)
}
pub fn extract_svg(svg: &str, limit: usize) -> Result<Document, Error> {
    let tree = svg_document(svg)?;
    let nodes: Vec<_> = tree.descendants().filter(|n| scene_metadata(*n)).collect();
    if nodes.len() != 1 {
        return Err(failure("missing or ambiguous SVG scene metadata"));
    }
    let node = nodes[0];
    let mut version = "1";
    let mut active = false;
    let mut payload = String::new();
    let mut finished = false;
    for child in node.children() {
        if child.is_comment() {
            let comment = child.text().unwrap_or("").trim();
            if let Some(v) = comment.strip_prefix("payload-version:") {
                version = v;
            }
            if comment == "payload-start" {
                if active || finished {
                    return Err(failure("ambiguous payload markers"));
                }
                active = true;
            }
            if comment == "payload-end" {
                if !active {
                    return Err(failure("invalid payload markers"));
                }
                active = false;
                finished = true;
            }
        } else if active && child.is_text() {
            payload.push_str(child.text().unwrap_or(""));
        }
    }
    if !finished || active {
        return Err(failure("missing SVG payload markers"));
    }
    if payload.len() > limit.saturating_mul(12).saturating_add(2048) {
        return Err(failure("encoded payload exceeds limit"));
    }
    let bytes = STANDARD
        .decode(payload.split_whitespace().collect::<String>())
        .map_err(|e| failure(e.to_string()))?;
    let text = match version {
        "1" => String::from_utf8(bytes).map_err(|e| failure(e.to_string()))?,
        "2" => bytes.into_iter().map(char::from).collect(),
        _ => return Err(failure("unsupported SVG payload version")),
    };
    decode(&text, limit)
}
