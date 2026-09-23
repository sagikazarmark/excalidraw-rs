//! The download/convert/upload pipeline, with no network and no API key.
//!
//! Runs against the committed fixture, so it demonstrates exactly what the
//! client would send without contacting the service:
//!
//! ```sh
//! cargo run -p excalidraw-api --example offline_pipeline
//! ```
use excalidraw_api::{
    Error, Operation, SceneId, op,
    plus::SceneContent,
    scene_content::{self, ElementIds, Embedding, PatchFields},
};
use excalidraw_document::{Number, element};

const DOWNLOADED: &[u8] = include_bytes!("../tests/fixtures/scene_content.json");
const PARTIAL: &[u8] = include_bytes!("../tests/fixtures/scene_content_partial.json");

fn main() -> Result<(), Error> {
    let scene = SceneId::new("scene-1")?;

    // 1. What GET /scenes/{id}/content returns.
    let downloaded = SceneContent::from_slice(DOWNLOADED).map_err(Error::Content)?;
    println!("downloaded sceneVersion {}", downloaded.scene_version());
    println!(
        "  elements: {}",
        downloaded
            .document()
            .as_object()
            .get("elements")
            .and_then(|e| e.as_array())
            .map_or(0, Vec::len)
    );

    // 2. Convert. The payload is an ordinary preserving document.
    let mut document = downloaded.document().clone();
    document
        .edit_element(0, |shape| shape.set(element::X, Number::from_f64(42.5)?))
        .map_err(Error::Content)?;

    // 3a. Authoritative replacement.
    let (body, changes) = scene_content::replacement_from_document(
        &document,
        &scene_content::PlusProjection::strict(),
    )?;
    println!("  projection changes: {}", changes.len());
    let request = op::ReplaceSceneContent {
        scene: scene.clone(),
        body,
    }
    .request()?;
    println!(
        "PUT {} ({} bytes)",
        request.relative_url(),
        request.body.as_ref().map_or(0, Vec::len)
    );

    // 3b. Or a merge that leaves other elements alone.
    // The download's ids are the service's own, so this merge updates rather
    // than inserting duplicates.
    let patch = scene_content::patch_from(
        &document,
        PatchFields::elements(),
        ElementIds::RequireCanonical,
    )?;
    let request = op::PatchSceneContent { scene, body: patch }.request()?;
    println!(
        "PATCH {} ({} bytes)",
        request.relative_url(),
        request.body.as_ref().map_or(0, Vec::len)
    );

    // The guard: a download that could not embed every referenced file would
    // silently delete images if replayed through PUT.
    let partial = SceneContent::from_slice(PARTIAL).map_err(Error::Content)?;
    match scene_content::into_replacement(partial, Embedding::RequireComplete) {
        Err(error) => println!("\nguard refused an incomplete download:\n  {error}"),
        Ok(_) => unreachable!("the fixture reports a failed embedding"),
    }

    Ok(())
}
