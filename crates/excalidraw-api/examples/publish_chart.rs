//! Render a chart and publish it to an Excalidraw Plus scene.
//!
//! This is the workspace's end-to-end story: `excaliplot` generates native
//! elements, `excalidraw-document` holds them, and this crate moves them to a
//! hosted scene.
//!
//! ```sh
//! export EXCALIDRAW_API_KEY=sk-...
//! cargo run -p excalidraw-api --features blocking --example publish_chart -- <sceneId> [sceneVersion]
//! ```
//!
//! Without a key or a scene id it stops after building the request and prints
//! what it would have sent, so it is safe to run unattended.
//!
//! Publishing is a replacement, so a chart that is republished on a schedule
//! overwrites whatever is in the scene. Passing the `sceneVersion` the previous
//! run printed makes this run look before it writes. That check is advisory —
//! see [`UnguardedCheckThenWrite`] for the two things it cannot see.
use excalidraw_api::{
    ApiKey, Operation, SceneId, SceneVersion, op,
    scene_content::{self, PlusProjection, UnguardedCheckThenWrite},
};
use excaliplot::LineChart;

type Failure = Box<dyn std::error::Error>;

fn main() -> Result<(), Failure> {
    // 1. Generate a chart as native editable elements.
    let points = [(1.0, 2.0), (2.0, 5.0), (3.0, 4.0), (4.0, 7.0)];
    let scene = LineChart::new(&points, 0.0..5.0, 0.0..8.0)
        .labels("Throughput", "Hour", "Requests/s")
        .render()?;

    let document = scene.to_document()?;

    // 2. Project it onto the Plus transport profile. A generated document is
    //    already legal, so a strict policy reports no changes.
    let (body, changes) =
        scene_content::replacement_from_document(&document, &PlusProjection::strict())?;
    println!(
        "rendered {} elements, {} projection change(s)",
        document
            .as_object()
            .get("elements")
            .and_then(|e| e.as_array())
            .map_or(0, Vec::len),
        changes.len()
    );
    for change in &changes {
        println!(
            "  {} {:?} -> {:?}",
            change.path, change.before, change.after
        );
    }

    // 3. Publish, if we were given somewhere to publish to.
    let Some(target) = std::env::args().nth(1) else {
        let request = op::ReplaceSceneContent {
            scene: SceneId::new("SCENE_ID")?,
            body,
        }
        .request()?;
        println!(
            "\nno scene id given; would send PUT {} ({} bytes)",
            request.relative_url(),
            request.body.as_ref().map_or(0, Vec::len)
        );
        println!("pass a scene id and set EXCALIDRAW_API_KEY to publish for real");
        return Ok(());
    };

    let scene_id = SceneId::new(target)?;
    let client = excalidraw_api::blocking::Client::new(ApiKey::from_env()?)?;

    // If the caller said which version of the scene they are replacing, read the
    // metadata back and stop if it no longer matches. The API publishes no
    // `If-Match`, so this is a cheap sanity check and nothing more: it cannot
    // see a concurrent geometry edit or a concurrent PATCH, and the scene may
    // change again between this read and the PUT below.
    if let Some(expected) = std::env::args().nth(2) {
        let intent = UnguardedCheckThenWrite::new(SceneVersion::new(expected));
        let record = client.send(op::GetScene {
            scene: scene_id.clone(),
        })?;
        intent.check(&record.metadata)?;
        println!(
            "scene still at sceneVersion {}; publishing",
            record.metadata.scene_version
        );
    }

    // Replacement is authoritative: everything currently in the scene is
    // replaced, and open editors are forced to reload.
    let replaced = client.send(op::ReplaceSceneContent {
        scene: scene_id.clone(),
        body,
    })?;
    println!("\npublished to {scene_id}");
    println!("  new sceneVersion: {}", replaced.scene_version());
    println!("  pass that back as the second argument to check before the next publish");
    if let Some(limit) = client.last_rate_limit() {
        println!("  rate limit remaining: {:?}", limit.remaining);
    }
    Ok(())
}
