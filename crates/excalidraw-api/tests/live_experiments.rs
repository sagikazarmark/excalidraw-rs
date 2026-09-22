//! Opt-in experiments that settle the spec's open questions against the live
//! service. **Never run in CI**: every test is `#[ignore]`d and additionally
//! refuses to run unless `EXCALIDRAW_API_LIVE=1`.
//!
//! These mutate a workspace: they create a scratch scene, write to it, and trash
//! it afterwards. Point them at a throwaway workspace.
//!
//! Key hygiene, since running these means putting a live key on disk:
//!
//! - Use a scratch workspace, not one holding work you care about.
//! - A read-scoped key is enough for the observational experiments (q6, q7, q16)
//!   and cannot touch anything; the rest need write access.
//! - Rotate the key afterwards. `.env` is gitignored, but a key that has been on
//!   disk and exercised by a test harness should not stay valid indefinitely.
//! - Never give CI a key. These are `#[ignore]`d *and* env-gated precisely so
//!   running them stays a deliberate local act.
//!
//! ```sh
//! export EXCALIDRAW_API_KEY=sk-...
//! export EXCALIDRAW_API_LIVE=1
//! export EXCALIDRAW_API_COLLECTION=private   # optional; personal keys only
//! cargo test -p excalidraw-api --features blocking --test live_experiments \
//!   -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Each test prints an ANSWER line. Record the answers in
//! `docs/excalidraw-api-spec.md` under "Open questions" and delete the ones that
//! are resolved.
#![cfg(feature = "blocking")]

use excalidraw_api::{
    ApiKey, CollectionId, Error, HeaderLookup, Method, Operation, Request, SceneId, blocking,
    model, op, plus,
};
use serde_json::{Value, json};

/// A request this crate's typed operations deliberately cannot express.
///
/// Several open questions are about what the server does with an *illegal* body,
/// so the experiment has to bypass the typed layer. That it takes only this much
/// code is the point of the `Operation` seam.
struct Raw {
    method: Method,
    path: String,
    body: Option<Value>,
}

impl Operation for Raw {
    type Output = (u16, Value);
    fn request(&self) -> Result<Request, Error> {
        let mut request = Request {
            method: self.method,
            path: self.path.clone(),
            query: Vec::new(),
            body: None,
        };
        if let Some(body) = &self.body {
            request.body = Some(serde_json::to_vec(body).expect("serialises"));
        }
        Ok(request)
    }
    fn decode(
        &self,
        status: u16,
        _: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error> {
        let parsed = serde_json::from_slice(body)
            .unwrap_or_else(|_| json!({ "<non-json>": String::from_utf8_lossy(body) }));
        Ok((status, parsed))
    }
}

fn enabled() -> bool {
    if std::env::var("EXCALIDRAW_API_LIVE").as_deref() != Ok("1") {
        eprintln!("skipped: set EXCALIDRAW_API_LIVE=1 to run live experiments");
        return false;
    }
    true
}

fn client() -> blocking::Client {
    blocking::Client::new(ApiKey::from_env().expect("EXCALIDRAW_API_KEY")).expect("client builds")
}

fn collection() -> CollectionId {
    CollectionId::new(
        std::env::var("EXCALIDRAW_API_COLLECTION").unwrap_or_else(|_| "private".to_owned()),
    )
    .expect("collection id")
}

/// A scratch scene, trashed when the guard drops.
struct Scratch {
    client: blocking::Client,
    scene: SceneId,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let client = client();
        let record = client
            .send(op::CreateScene {
                scene: model::NewScene::new(
                    format!("excalidraw-api experiment: {label}"),
                    collection(),
                ),
            })
            .expect("scratch scene created");
        let scene = SceneId::new(record.metadata.id.clone()).expect("scene id");
        eprintln!("  scratch scene {scene}");
        Self { client, scene }
    }
    fn seed(&self) -> plus::SceneContent {
        let document = excalidraw_document::Document::from_value(json!({
            "type": "excalidraw", "version": 2, "source": "excalidraw-api experiment",
            "elements": [{
                "id": "seed-rect", "type": "rectangle",
                "x": 0, "y": 0, "width": 100, "height": 50, "angle": 0,
                "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
                "roundness": null, "roughness": 1, "opacity": 100,
                "seed": 1, "version": 1, "versionNonce": 1, "index": "a0",
                "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
                "groupIds": [], "frameId": null, "boundElements": null, "link": null
            }],
            "appState": { "viewBackgroundColor": "#ffffff" },
            "files": {}
        }))
        .unwrap();
        let body = plus::ReplaceSceneContent::new(document).unwrap();
        self.client
            .send(op::ReplaceSceneContent {
                scene: self.scene.clone(),
                body,
            })
            .expect("seeded")
    }
    /// Replace the scene with exactly these elements.
    fn put(&self, elements: Value) -> (u16, Value) {
        self.put_with(elements, json!({}))
    }
    fn put_with(&self, elements: Value, files: Value) -> (u16, Value) {
        self.raw(
            Method::Put,
            "/content",
            Some(json!({
                "type": "excalidraw", "version": 2, "source": "excalidraw-api experiment",
                "appState": { "viewBackgroundColor": "#ffffff" },
                "elements": elements,
                "files": files
            })),
        )
    }
    fn patch(&self, body: Value) -> (u16, Value) {
        self.raw(Method::Patch, "/content", Some(body))
    }
    fn raw(&self, method: Method, suffix: &str, body: Option<Value>) -> (u16, Value) {
        self.client
            .send(Raw {
                method,
                path: format!("/scenes/{}{suffix}", self.scene),
                body,
            })
            .expect("raw request completes")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        match self.client.send(op::DeleteScene {
            scene: self.scene.clone(),
        }) {
            Ok(_) => eprintln!("  scratch scene trashed"),
            Err(error) => eprintln!("  WARNING: could not trash {}: {error}", self.scene),
        }
    }
}

/// Open question: does PATCH strip or reject a full GET envelope?
#[test]
#[ignore = "live"]
fn q1_patch_with_a_full_get_envelope() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q1 patch envelope");
    let seeded = scratch.seed();

    let mut envelope = seeded.document().as_object().clone();
    envelope.insert("sceneVersion".into(), json!(seeded.scene_version()));

    let (status, body) = scratch.raw(Method::Patch, "/content", Some(Value::Object(envelope)));
    println!("ANSWER q1: PATCH with a full GET envelope -> {status}");
    println!("  body: {}", truncate(&body));
    println!(
        "  verdict: {}",
        match status {
            200 => "ACCEPTED (extra root fields are tolerated or stripped)",
            400 => "REJECTED (the closed root is enforced)",
            _ => "UNEXPECTED, record verbatim",
        }
    );
}

/// Open question: are `stickynote` and a common `created` field accepted on write?
#[test]
#[ignore = "live"]
fn q2_undocumented_element_shapes() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q2 undocumented elements");
    scratch.seed();

    for (label, element) in [
        (
            "stickynote",
            json!({
                "id": "note-1", "type": "stickynote",
                "x": 0, "y": 0, "width": 100, "height": 100, "angle": 0,
                "strokeColor": "#1e1e1e", "backgroundColor": "#ffec99",
                "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
                "roundness": null, "roughness": 0, "opacity": 100,
                "seed": 2, "version": 1, "versionNonce": 2, "index": "a1",
                "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
                "groupIds": [], "frameId": null, "boundElements": null, "link": null
            }),
        ),
        (
            "rectangle with `created`",
            json!({
                "id": "rect-created", "type": "rectangle",
                "x": 0, "y": 0, "width": 10, "height": 10, "angle": 0,
                "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
                "roundness": null, "roughness": 0, "opacity": 100,
                "seed": 3, "version": 1, "versionNonce": 3, "index": "a2",
                "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
                "groupIds": [], "frameId": null, "boundElements": null, "link": null,
                "created": 1789000000000_i64
            }),
        ),
    ] {
        let (status, body) = scratch.raw(
            Method::Patch,
            "/content",
            Some(json!({ "elements": [element] })),
        );
        println!("ANSWER q2 [{label}]: write -> {status}");
        if status == 200 {
            let kept = body["elements"]
                .as_array()
                .map(|elements| elements.len())
                .unwrap_or(0);
            println!("  accepted; scene now has {kept} element(s)");
            println!("  round-tripped: {}", truncate(&body["elements"]));
        } else {
            println!("  rejected: {}", truncate(&body));
        }
    }
}

/// Open question: may a PATCH element be a partial record?
#[test]
#[ignore = "live"]
fn q3_partial_element_records() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q3 partial element");
    scratch.seed();

    let (status, body) = scratch.raw(
        Method::Patch,
        "/content",
        Some(json!({ "elements": [{ "id": "seed-rect", "x": 999 }] })),
    );
    println!("ANSWER q3: PATCH with a partial element record -> {status}");
    if status == 200 {
        let moved = body["elements"]
            .as_array()
            .and_then(|elements| elements.iter().find(|e| e["id"] == "seed-rect"))
            .map(|e| e["x"].clone());
        println!("  seed-rect.x is now {moved:?} (was 0)");
        println!(
            "  verdict: partial records are {} merged",
            if moved == Some(json!(999)) { "" } else { "NOT" }
        );
    } else {
        println!("  rejected: {}", truncate(&body));
    }
}

/// Open question: how are provisional string/numeric authoring ids normalised?
#[test]
#[ignore = "live"]
fn q4_authoring_id_normalisation() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q4 authoring ids");
    scratch.seed();

    let (status, body) = scratch.raw(
        Method::Patch,
        "/content",
        Some(json!({ "elements": [{
            "id": 12345, "type": "ellipse",
            "x": 5, "y": 5, "width": 20, "height": 20, "angle": 0,
            "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 1, "strokeStyle": "solid",
            "roundness": null, "roughness": 0, "opacity": 100,
            "seed": 4, "version": 1, "versionNonce": 4, "index": "a3",
            "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
            "groupIds": [], "frameId": null, "boundElements": null, "link": null
        }] })),
    );
    println!("ANSWER q4: numeric element id on write -> {status}");
    if status == 200 {
        let ids: Vec<Value> = body["elements"]
            .as_array()
            .map(|elements| elements.iter().map(|e| e["id"].clone()).collect())
            .unwrap_or_default();
        println!("  persisted ids: {ids:?}");
        println!(
            "  verdict: the server {} the numeric id",
            if ids.iter().any(|id| id == &json!(12345)) {
                "kept"
            } else {
                "normalised"
            }
        );
    } else {
        println!("  rejected: {}", truncate(&body));
    }
}

/// Open question: does PUT really ignore a supplied sceneVersion?
#[test]
#[ignore = "live"]
fn q5_put_recomputes_scene_version() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q5 scene version");
    let seeded = scratch.seed();
    let before = seeded.scene_version().to_owned();

    let mut envelope = seeded.document().as_object().clone();
    envelope.insert(
        "sceneVersion".into(),
        json!("definitely-not-a-real-version"),
    );

    let (status, body) = scratch.raw(Method::Put, "/content", Some(Value::Object(envelope)));
    println!("ANSWER q5: PUT with a bogus sceneVersion -> {status}");
    println!("  before:   {before}");
    println!("  supplied: definitely-not-a-real-version");
    println!("  returned: {}", body["sceneVersion"]);
    println!(
        "  verdict: supplied value was {}",
        if body["sceneVersion"] == json!("definitely-not-a-real-version") {
            "HONOURED (contradicts the documentation)"
        } else {
            "ignored and recomputed, as documented"
        }
    );
}

/// Open question: are `cursor` and `page` mutually exclusive on `GET /logs`?
#[test]
#[ignore = "live"]
fn q6_log_pagination_modes() {
    if !enabled() {
        return;
    }
    let client = client();

    let page_mode = client
        .send(op::GetLogs {
            query: excalidraw_api::LogQuery {
                page: Some("1".into()),
                limit: Some(1),
                ..Default::default()
            },
        })
        .expect("page mode");
    println!(
        "ANSWER q6a: page mode -> totalCount={:?} totalPages={:?} currentPage={:?} nextCursor={:?}",
        page_mode.total_count, page_mode.total_pages, page_mode.current_page, page_mode.next_cursor
    );

    let cursor_mode = client
        .send(op::GetLogs {
            query: excalidraw_api::LogQuery {
                limit: Some(1),
                ..Default::default()
            },
        })
        .expect("cursor mode");
    println!(
        "ANSWER q6b: cursor mode -> totalCount={:?} nextCursor={:?} hasMore={}",
        cursor_mode.total_count, cursor_mode.next_cursor, cursor_mode.has_more
    );

    let both = client.send(Raw {
        method: Method::Get,
        path: "/logs?page=1&cursor=abc&limit=1".into(),
        body: None,
    });
    match both {
        Ok((status, _)) => println!("ANSWER q6c: both parameters -> {status} (not exclusive)"),
        Err(error) => println!("ANSWER q6c: both parameters -> rejected: {error}"),
    }
}

/// Observational: which linkSharing values actually occur, and does the
/// documented-but-absent `2` ever appear?
#[test]
#[ignore = "live"]
fn q7_observed_link_sharing_values() {
    if !enabled() {
        return;
    }
    let client = client();
    let scenes: Vec<model::SceneRecord> = client
        .collect(
            |page| op::ListScenes {
                page,
                collection: None,
            },
            excalidraw_api::PageRequest::new().limit(100),
            500,
        )
        .expect("scene listing");

    let mut seen: Vec<i64> = scenes
        .iter()
        .map(|scene| scene.metadata.link_sharing.as_i64())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    println!(
        "ANSWER q7: {} scene(s), observed linkSharing values {seen:?}",
        scenes.len()
    );
    let unknown: Vec<&model::SceneRecord> = scenes
        .iter()
        .filter(|s| matches!(s.metadata.link_sharing, model::LinkSharing::Unknown(_)))
        .collect();
    println!("  unlisted values: {}", unknown.len());

    let extras: std::collections::BTreeSet<&String> = scenes
        .iter()
        .flat_map(|scene| scene.metadata.extra.keys())
        .collect();
    println!("ANSWER q7b: undocumented metadata fields observed: {extras:?}");
}

/// Does the server rewrite element ids, and does it rewrite the references that
/// point at them?
///
/// This matters for publishing generated scenes: `plotters-excalidraw` mints
/// UUID-style ids, which are not Excalidraw's 21-character form. If ids are
/// rewritten but `containerId` and `boundElements` are not, every bound label
/// silently detaches.
#[test]
#[ignore = "live"]
fn q8_reference_rewriting_under_id_normalisation() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q8 reference rewriting");

    // A container with a bound label, using UUID-style ids like the backend mints.
    let box_id = "3f2a1c44-0000-4000-8000-000000000001";
    let label_id = "3f2a1c44-0000-4000-8000-000000000002";
    let common = |id: &str, kind: &str, index: &str| {
        json!({
            "id": id, "type": kind, "angle": 0,
            "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
            "roundness": null, "roughness": 1, "opacity": 100,
            "seed": 1, "version": 1, "versionNonce": 1, "index": index,
            "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
            "groupIds": [], "frameId": null, "link": null,
            // The server requires this key to be present; null is accepted but
            // omitting it is a 400. The public reference does not say so.
            "boundElements": null
        })
    };
    let mut container = common(box_id, "rectangle", "a0");
    container["x"] = json!(0);
    container["y"] = json!(0);
    container["width"] = json!(200);
    container["height"] = json!(100);
    container["boundElements"] = json!([{ "id": label_id, "type": "text" }]);
    container["customData"] = json!({ "excaliplot": { "series": "measured" } });

    let mut label = common(label_id, "text", "a1");
    label["x"] = json!(20);
    label["y"] = json!(40);
    label["width"] = json!(84);
    label["height"] = json!(25);
    label["fontSize"] = json!(20);
    label["fontFamily"] = json!(5);
    label["text"] = json!("Hello");
    label["textAlign"] = json!("center");
    label["verticalAlign"] = json!("middle");
    label["containerId"] = json!(box_id);
    label["originalText"] = json!("Hello");
    label["autoResize"] = json!(true);
    label["lineHeight"] = json!(1.25);

    let (status, body) = scratch.raw(
        Method::Put,
        "/content",
        Some(json!({
            "type": "excalidraw", "version": 2, "source": "excalidraw-api experiment",
            "appState": { "viewBackgroundColor": "#ffffff" },
            "elements": [container, label],
            "files": {}
        })),
    );
    println!("ANSWER q8: PUT with UUID-style ids -> {status}");
    if status != 200 {
        println!("  rejected: {}", truncate(&body));
        return;
    }

    let elements = body["elements"].as_array().cloned().unwrap_or_default();
    let find = |kind: &str| elements.iter().find(|e| e["type"] == kind).cloned();
    let container = find("rectangle").expect("container survives");
    let label = find("text").expect("label survives");

    let new_box = container["id"].as_str().unwrap_or_default().to_owned();
    let new_label = label["id"].as_str().unwrap_or_default().to_owned();
    println!("  container id: {box_id} -> {new_box}");
    println!("  label id:     {label_id} -> {new_label}");
    println!("  rewritten: {}", new_box != box_id);

    let container_ref = label["containerId"].as_str().unwrap_or_default();
    let bound = container["boundElements"][0]["id"]
        .as_str()
        .unwrap_or_default();
    println!("  label.containerId  -> {container_ref}");
    println!("  container.bound[0] -> {bound}");
    println!(
        "  VERDICT: references {}",
        if container_ref == new_box && bound == new_label {
            "were rewritten consistently; bound labels survive publishing"
        } else {
            "were NOT rewritten consistently; BOUND LABELS BREAK on publish"
        }
    );
    println!(
        "  customData preserved: {}",
        container["customData"] != Value::Null
    );
    println!("  customData: {}", truncate(&container["customData"]));
}

/// A complete persisted element, which is what every write schema demands.
fn element(id: &str, kind: &str, index: &str) -> Value {
    json!({
        "id": id, "type": kind, "index": index,
        "x": 0, "y": 0, "width": 100, "height": 50, "angle": 0,
        "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
        "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
        "roundness": null, "roughness": 1, "opacity": 100,
        "seed": 1, "version": 1, "versionNonce": 1,
        "updated": 1789000000000_i64, "isDeleted": false, "locked": false,
        "groupIds": [], "frameId": null, "boundElements": null, "link": null
    })
}

/// Find the single element of a kind in a content body.
fn only(body: &Value, kind: &str) -> Option<Value> {
    body["elements"]
        .as_array()?
        .iter()
        .find(|e| e["type"] == kind)
        .cloned()
}

fn truncate(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= 400 {
        text
    } else {
        format!("{}… ({} bytes)", &text[..400], text.len())
    }
}

/// Open question: which way does the `versionNonce` tie-break go, and what
/// happens on equal version *and* equal nonce?
///
/// No concurrency is needed: `PATCH` merges the submitted fragment against the
/// stored scene, so sequential requests exercise the same reconciliation.
#[test]
#[ignore = "live"]
fn q9_version_and_nonce_reconciliation() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q9 reconciliation");

    // The stored baseline every case starts from.
    let baseline = |x: i64| {
        let mut e = element("recon-1", "rectangle", "a0");
        e["version"] = json!(5);
        e["versionNonce"] = json!(100);
        e["x"] = json!(x);
        e
    };

    let mut cases: Vec<(&str, i64, i64, i64)> = vec![
        // label, submitted version, submitted nonce, submitted x
        ("higher version", 6, 100, 601),
        ("lower version", 4, 100, 401),
        ("equal version, higher nonce", 5, 200, 502),
        ("equal version, lower nonce", 5, 50, 503),
        ("equal version, equal nonce", 5, 100, 504),
    ];
    cases.sort_by_key(|c| c.0);

    for (label, version, nonce, x) in cases {
        // Reset, then take the id the SERVER assigned. Submitting our own id
        // again would be normalised to a fresh one, so the merge would never
        // match and the element would be inserted instead of merged.
        let (status, stored) = scratch.put(json!([baseline(0)]));
        assert_eq!(status, 200, "baseline restored");
        let stored_element = only(&stored, "rectangle").expect("baseline element");
        let canonical_id = stored_element["id"].as_str().expect("id").to_owned();
        let stored_version = stored_element["version"].clone();
        let stored_nonce = stored_element["versionNonce"].clone();

        let mut submitted = baseline(x);
        submitted["id"] = json!(canonical_id);
        submitted["version"] = json!(version);
        submitted["versionNonce"] = json!(nonce);
        let (status, body) = scratch.patch(json!({ "elements": [submitted] }));

        let count = body["elements"].as_array().map_or(0, Vec::len);
        let after = body["elements"]
            .as_array()
            .and_then(|elements| elements.iter().find(|e| e["id"] == json!(canonical_id)))
            .cloned()
            .unwrap_or(Value::Null);
        let stored_x = after["x"].clone();
        let won = stored_x == json!(x);
        println!(
            "ANSWER q9 [{label}]: stored(v={stored_version}, nonce={stored_nonce}) \
             vs submitted(v={version}, nonce={nonce}) -> {status}, x={stored_x} => submitted {}",
            if won { "WON" } else { "was discarded" }
        );
        println!(
            "    elements now: {count} (merge, not insert: {}); result v={} nonce={}",
            count == 1,
            after["version"],
            after["versionNonce"]
        );
    }
    println!("  (baseline is always version 5, nonce 100, x 0)");
}

/// Open question: what does `contentEpoch` do, and is it monotone per scene?
#[test]
#[ignore = "live"]
fn q10_content_epoch_semantics() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q10 content epoch");
    let client = client();

    let read = |label: &str| {
        let record = client
            .send(op::GetScene {
                scene: scratch.scene.clone(),
            })
            .expect("metadata");
        println!(
            "ANSWER q10 [{label}]: contentEpoch={} sceneVersion={} updateCount={} revisionCount={} totalElements={}",
            record.metadata.content_epoch,
            record.metadata.scene_version,
            record.metadata.update_count,
            record.metadata.revision_count,
            record.metadata.total_elements
        );
        record.metadata.content_epoch
    };

    let after_create = read("after create");
    scratch.put(json!([element("epoch-1", "rectangle", "a0")]));
    let after_put = read("after PUT");
    scratch.put(json!([element("epoch-1", "rectangle", "a0")]));
    let after_same_put = read("after identical PUT");

    let mut moved = element("epoch-2", "ellipse", "a1");
    moved["version"] = json!(9);
    scratch.patch(json!({ "elements": [moved] }));
    let after_patch = read("after PATCH");

    client
        .send(op::UpdateScene {
            scene: scratch.scene.clone(),
            patch: model::ScenePatch::new().name("q10 renamed"),
        })
        .expect("rename");
    let after_rename = read("after metadata-only PATCH");

    let series = [
        after_create,
        after_put,
        after_same_put,
        after_patch,
        after_rename,
    ];
    println!("  series: {series:?}");
    println!(
        "  monotone non-decreasing: {}",
        series.windows(2).all(|w| w[1] >= w[0])
    );
    println!(
        "  changed on content write: {}; on identical write: {}; on rename: {}",
        after_put != after_create,
        after_same_put != after_put,
        after_rename != after_patch
    );
}

/// Open question: what is `sceneVersion` derived from?
///
/// Known to be content-derived and stable for identical content. These probes
/// narrow *which* content, without claiming to recover the function.
#[test]
#[ignore = "live"]
fn q11_scene_version_inputs() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q11 scene version");

    let version_of =
        |(_, body): (u16, Value)| body["sceneVersion"].as_str().unwrap_or_default().to_owned();

    let base = element("sv-1", "rectangle", "a0");
    let a = version_of(scratch.put(json!([base.clone()])));

    let mut moved = base.clone();
    moved["x"] = json!(50);
    let b = version_of(scratch.put(json!([moved])));

    let mut bumped = base.clone();
    bumped["version"] = json!(77);
    let c = version_of(scratch.put(json!([bumped.clone()])));

    let mut renonced = bumped.clone();
    renonced["versionNonce"] = json!(4242);
    let d = version_of(scratch.put(json!([renonced])));

    // Same elements, different stored appState.
    let e = version_of(scratch.raw(
        Method::Put,
        "/content",
        Some(json!({
            "type": "excalidraw", "version": 2, "source": "excalidraw-api experiment",
            "appState": { "viewBackgroundColor": "#ff0000" },
            "elements": [base.clone()], "files": {}
        })),
    ));

    let f = version_of(scratch.put(json!([base.clone()])));

    println!("ANSWER q11: sceneVersion probes");
    println!("  A baseline                     = {a}");
    println!(
        "  B moved x                      = {b}  (differs from A: {})",
        b != a
    );
    println!(
        "  C element version 1 -> 77      = {c}  (differs from A: {})",
        c != a
    );
    println!(
        "  D nonce changed too            = {d}  (differs from C: {})",
        d != c
    );
    println!(
        "  E baseline + red background    = {e}  (differs from A: {})",
        e != a
    );
    println!(
        "  F baseline again               = {f}  (equals A: {})",
        f == a
    );
    println!(
        "  length: {} chars, hex: {}",
        a.len(),
        a.chars().all(|c| c.is_ascii_hexdigit())
    );
    println!(
        "  => depends on geometry: {}; on element version: {}; on nonce: {}; on appState: {}",
        b != a,
        c != a,
        d != c,
        e != a
    );
}

/// Open question: does `PATCH` validate references against the merged scene, or
/// only against the submitted fragment?
#[test]
#[ignore = "live"]
fn q12_patch_reference_validation_scope() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q12 reference scope");

    // Store a container on its own, with no label.
    let mut container = element("q12-box", "rectangle", "a0");
    container["width"] = json!(200);
    container["height"] = json!(100);
    let (status, stored) = scratch.put(json!([container]));
    assert_eq!(status, 200);
    let stored_box = only(&stored, "rectangle")
        .and_then(|e| e["id"].as_str().map(str::to_owned))
        .expect("container id");

    // Now PATCH in only a label, pointing at an element that exists in the
    // stored scene but not in this fragment.
    let mut label = element("q12-label", "text", "a1");
    label["containerId"] = json!(stored_box);
    label["fontSize"] = json!(20);
    label["fontFamily"] = json!(5);
    label["text"] = json!("Bound");
    label["textAlign"] = json!("center");
    label["verticalAlign"] = json!("middle");
    label["originalText"] = json!("Bound");
    label["autoResize"] = json!(true);
    label["lineHeight"] = json!(1.25);

    let (status, body) = scratch.patch(json!({ "elements": [label] }));
    println!("ANSWER q12a: PATCH a label referencing a stored-only container -> {status}");
    if status == 200 {
        println!("  verdict: validated against the MERGED scene");
        let bound = only(&body, "rectangle").map(|e| e["boundElements"].clone());
        println!(
            "  container.boundElements now: {}",
            truncate(&bound.unwrap_or(Value::Null))
        );
    } else {
        println!("  verdict: validated against the FRAGMENT only");
        println!("  {}", truncate(&body));
    }

    // Control: a reference to an id that exists nowhere at all.
    let mut orphan = label.clone();
    orphan["id"] = json!("q12-orphan");
    orphan["containerId"] = json!("definitely-not-a-real-element");
    let (status, body) = scratch.patch(json!({ "elements": [orphan] }));
    println!("ANSWER q12b: PATCH a label referencing nothing -> {status}");
    if status != 200 {
        println!("  dangling references are rejected: {}", truncate(&body));
    } else {
        println!("  WARNING: a dangling containerId was accepted");
    }
}

/// Open question: which other upstream-only element fields does the write schema
/// accept? `created` and `stickynote` are already known to pass.
#[test]
#[ignore = "live"]
fn q13_other_upstream_only_fields() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q13 upstream fields");

    let text = || {
        let mut e = element("q13-text", "text", "a0");
        e["fontSize"] = json!(20);
        e["fontFamily"] = json!(5);
        e["text"] = json!("Probe");
        e["textAlign"] = json!("center");
        e["verticalAlign"] = json!("middle");
        e["containerId"] = Value::Null;
        e["originalText"] = json!("Probe");
        e["autoResize"] = json!(true);
        e["lineHeight"] = json!(1.25);
        e
    };
    let freedraw = || {
        let mut e = element("q13-draw", "freedraw", "a0");
        e["points"] = json!([[0, 0], [10, 10]]);
        e["pressures"] = json!([0.5, 0.5]);
        e["simulatePressure"] = json!(true);
        e
    };
    let stickynote = |base_height: Value| {
        let mut e = element("q13-note", "stickynote", "a0");
        e["width"] = json!(100);
        e["height"] = json!(100);
        e["baseHeight"] = base_height;
        e
    };

    let mut with_base_font = text();
    with_base_font["baseFontSize"] = json!(16);
    let mut with_label_position = text();
    with_label_position["labelPosition"] = json!(0.5);
    let mut with_stroke_options = freedraw();
    with_stroke_options["strokeOptions"] = json!({ "variability": "variable", "streamline": 0.5 });

    for (label, elements) in [
        ("text.baseFontSize", json!([with_base_font])),
        ("text.labelPosition", json!([with_label_position])),
        ("freedraw.strokeOptions", json!([with_stroke_options])),
        ("freedraw (control)", json!([freedraw()])),
        ("stickynote + baseHeight", json!([stickynote(json!(100))])),
    ] {
        let (status, body) = scratch.put(elements);
        print!("ANSWER q13 [{label}]: write -> {status}");
        if status == 200 {
            let kept = body["elements"][0].as_object().is_some_and(|e| {
                [
                    "baseFontSize",
                    "labelPosition",
                    "strokeOptions",
                    "baseHeight",
                ]
                .iter()
                .any(|k| e.contains_key(*k))
            });
            println!(" accepted; probe field round-tripped: {kept}");
        } else {
            println!();
            println!("  rejected: {}", truncate(&body));
        }
    }
}

/// Open question: are unreferenced files retained, and does dropping a file from
/// `files` remove it?
#[test]
#[ignore = "live"]
fn q14_file_retention() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q14 file retention");

    let file_id = "q14testfile001";
    let files = json!({
        file_id: {
            "id": file_id,
            "mimeType": "image/png",
            "created": 1789000000000_i64,
            "dataURL": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg=="
        }
    });
    let mut image = element("q14-image", "image", "a0");
    image["fileId"] = json!(file_id);
    image["status"] = json!("saved");
    image["scale"] = json!([1, 1]);
    image["crop"] = Value::Null;

    let (status, body) = scratch.put_with(json!([image.clone()]), files.clone());
    println!("ANSWER q14a: PUT an image with its file -> {status}");
    if status != 200 {
        println!("  rejected: {}", truncate(&body));
        return;
    }
    println!(
        "  files returned: {:?}",
        body["files"]
            .as_object()
            .map(|f| f.keys().collect::<Vec<_>>())
    );

    // Drop the element but keep the file: is an unreferenced file retained?
    let (status, body) = scratch.put_with(
        json!([element("q14-rect", "rectangle", "a0")]),
        files.clone(),
    );
    println!("ANSWER q14b: PUT without the image, files still supplied -> {status}");
    println!(
        "  files returned: {:?}",
        body["files"]
            .as_object()
            .map(|f| f.keys().collect::<Vec<_>>())
    );
    println!(
        "  unreferenced file retained: {}",
        body["files"][file_id] != Value::Null
    );

    // Drop the file from the body entirely.
    let (status, body) =
        scratch.put_with(json!([element("q14-rect", "rectangle", "a0")]), json!({}));
    println!("ANSWER q14c: PUT with files: {{}} -> {status}");
    println!(
        "  files returned: {:?}",
        body["files"]
            .as_object()
            .map(|f| f.keys().collect::<Vec<_>>())
    );
    println!(
        "  file removed by omission: {}",
        body["files"][file_id] == Value::Null
    );

    // Does an image element referencing a now-absent file still validate?
    let (status, body) = scratch.put_with(json!([image]), json!({}));
    println!("ANSWER q14d: PUT an image whose file is absent -> {status}");
    if status != 200 {
        println!("  reference integrity enforced: {}", truncate(&body));
    } else {
        println!(
            "  accepted with a dangling fileId; filesFailedToEmbed={}",
            body["filesFailedToEmbed"]
        );
    }
}

/// Open question: what is the request size limit?
///
/// The only published bound is the 20,971,520-character `dataURL` string. This
/// brackets practical usage and checks whether that bound is enforced.
#[test]
#[ignore = "live"]
fn q15_size_limits() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q15 size limits");
    const PUBLISHED_MAX: usize = 20_971_520;

    let probe = |label: &str, chars: usize| {
        let file_id = "q15testfile001";
        let data = format!("data:image/png;base64,{}", "A".repeat(chars));
        let files = json!({
            file_id: {
                "id": file_id, "mimeType": "image/png",
                "created": 1789000000000_i64, "dataURL": data
            }
        });
        let mut image = element("q15-image", "image", "a0");
        image["fileId"] = json!(file_id);
        image["status"] = json!("saved");
        image["scale"] = json!([1, 1]);
        image["crop"] = Value::Null;

        let (status, body) = scratch.put_with(json!([image]), files);
        println!("ANSWER q15 [{label}]: dataURL of {chars} chars -> {status}");
        if status != 200 {
            println!("  {}", truncate(&body));
        }
        status
    };

    probe("1 MB", 1_000_000);
    probe("just over the published maximum", PUBLISHED_MAX + 1);

    // Many elements rather than one large file.
    let many: Vec<Value> = (0..5_000)
        .map(|i| element(&format!("q15-{i}"), "rectangle", &format!("a{i}")))
        .collect();
    let (status, body) = scratch.put(json!(many));
    println!("ANSWER q15 [5000 elements]: -> {status}");
    if status != 200 {
        println!("  {}", truncate(&body));
    } else {
        println!(
            "  stored: {} element(s)",
            body["elements"].as_array().map_or(0, Vec::len)
        );
    }
}

/// Open question: what does `workspace.type` hold?
#[test]
#[ignore = "live"]
fn q16_workspace_type_and_shape() {
    if !enabled() {
        return;
    }
    let client = client();
    let workspace = client.send(op::GetWorkspace).expect("workspace");
    println!("ANSWER q16: workspace.type = {:?}", workspace.r#type);
    println!(
        "  apiVersion={:?} flags={:?}",
        workspace.api_version, workspace.flags
    );
    println!("  preferences present: {}", workspace.preferences.is_some());
    println!(
        "  undocumented fields: {:?}",
        workspace.extra.keys().collect::<Vec<_>>()
    );

    let users = client
        .send(op::ListUsers {
            page: excalidraw_api::PageRequest::new().limit(100),
        })
        .expect("users");
    let extras: std::collections::BTreeSet<&String> =
        users.data.iter().flat_map(|u| u.extra.keys()).collect();
    println!(
        "  users: {}; undocumented user fields: {extras:?}",
        users.data.len()
    );
}

/// Does the file-reference check cover deleted elements?
///
/// This decides how `Embedding::RequireComplete` should be described: if the
/// server rejects every dangling `fileId`, the guard prevents a rejected request;
/// if tombstones are exempt, real data can still be dropped.
#[test]
#[ignore = "live"]
fn q17_file_reference_check_and_tombstones() {
    if !enabled() {
        return;
    }
    let scratch = Scratch::new("q17 tombstone file refs");

    let mut image = element("q17-image", "image", "a0");
    image["fileId"] = json!("q17missingfile1");
    image["status"] = json!("saved");
    image["scale"] = json!([1, 1]);
    image["crop"] = Value::Null;

    let (status, body) = scratch.put_with(json!([image.clone()]), json!({}));
    println!("ANSWER q17a: live image, missing file -> {status}");
    if status != 200 {
        println!("  rejected: {}", truncate(&body["message"]));
    }

    let mut tombstone = image.clone();
    tombstone["isDeleted"] = json!(true);
    let (status, body) = scratch.put_with(json!([tombstone]), json!({}));
    println!("ANSWER q17b: TOMBSTONED image, missing file -> {status}");
    if status == 200 {
        println!("  ACCEPTED: the check exempts deleted elements, so a partial");
        println!("  download replayed through PUT can drop a file a tombstone needs");
        println!(
            "  stored elements: {}",
            body["elements"].as_array().map_or(0, Vec::len)
        );
    } else {
        println!("  rejected too: {}", truncate(&body["message"]));
        println!("  => every dangling fileId is refused; the guard prevents a 400,");
        println!("     not silent image loss");
    }
}
