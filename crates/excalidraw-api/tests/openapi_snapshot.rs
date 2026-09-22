//! Assertions against the pinned public artifact every contract was
//! transcribed from. This test reads a committed file; it performs no network
//! access and needs no API key.
//!
//! A drift check re-fetches the URL and reports a difference. It must never
//! rewrite the fixture: the crate's types were written against these bytes, and
//! silently adopting a change would hide a breaking change in a beta API.
use excalidraw_api::{API_BASE_URL, OPENAPI_SNAPSHOT};
use serde_json::Value;

const ARTIFACT: &[u8] = include_bytes!("fixtures/openapi.json");

fn document() -> Value {
    serde_json::from_slice(ARTIFACT).expect("the pinned artifact parses")
}

/// The 27 documented operations, exactly as published.
const OPERATIONS: &[(&str, &str)] = &[
    ("get", "/api/v1/collections"),
    ("post", "/api/v1/collections"),
    ("get", "/api/v1/collections/{collectionId}"),
    ("patch", "/api/v1/collections/{collectionId}"),
    ("delete", "/api/v1/collections/{collectionId}"),
    ("get", "/api/v1/collections/{collectionId}/scenes"),
    ("post", "/api/v1/collections/{collectionId}/scenes"),
    ("get", "/api/v1/scenes"),
    ("post", "/api/v1/scenes"),
    ("get", "/api/v1/scenes/{sceneId}"),
    ("patch", "/api/v1/scenes/{sceneId}"),
    ("delete", "/api/v1/scenes/{sceneId}"),
    ("get", "/api/v1/scenes/{sceneId}/content"),
    ("put", "/api/v1/scenes/{sceneId}/content"),
    ("patch", "/api/v1/scenes/{sceneId}/content"),
    ("get", "/api/v1/workspaces"),
    ("patch", "/api/v1/workspaces"),
    ("get", "/api/v1/workspaces/users"),
    ("get", "/api/v1/workspaces/users/{userId}"),
    ("patch", "/api/v1/workspaces/users/{userId}"),
    ("delete", "/api/v1/workspaces/users/{userId}"),
    ("get", "/api/v1/workspaces/invites"),
    ("post", "/api/v1/workspaces/invites"),
    ("get", "/api/v1/workspaces/invites/{inviteId}"),
    ("patch", "/api/v1/workspaces/invites/{inviteId}"),
    ("delete", "/api/v1/workspaces/invites/{inviteId}"),
    ("get", "/api/v1/logs"),
];

#[test]
fn the_fixture_is_the_bytes_the_snapshot_names() {
    assert_eq!(ARTIFACT.len() as u64, OPENAPI_SNAPSHOT.bytes);
    assert_eq!(OPENAPI_SNAPSHOT.url, "https://api.excalidraw.com/docs/json");
    // The digest itself is recorded in tests/fixtures/README.md and verified by
    // the drift check, which owns the hashing dependency.
    assert_eq!(OPENAPI_SNAPSHOT.sha256.len(), 64);
    assert!(
        OPENAPI_SNAPSHOT
            .sha256
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );
}

#[test]
fn the_artifact_publishes_exactly_the_operations_this_crate_implements() {
    let document = document();
    let paths = document["paths"].as_object().expect("paths object");

    let mut published: Vec<(String, String)> = Vec::new();
    for (path, item) in paths {
        for method in item.as_object().expect("path item").keys() {
            if ["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                published.push((method.clone(), path.clone()));
            }
        }
    }
    published.sort();

    let mut expected: Vec<(String, String)> = OPERATIONS
        .iter()
        .map(|(m, p)| ((*m).to_owned(), (*p).to_owned()))
        .collect();
    expected.sort();

    assert_eq!(
        published, expected,
        "the published operation set changed; review before updating the crate"
    );
    assert_eq!(published.len(), 27);
}

#[test]
fn every_operation_is_bearer_authenticated() {
    let document = document();
    for (method, path) in OPERATIONS {
        let security = &document["paths"][path][method]["security"];
        assert_eq!(
            security,
            &serde_json::json!([{ "apiKey": [] }]),
            "{method} {path}"
        );
    }
    let scheme = &document["components"]["securitySchemes"]["apiKey"];
    assert_eq!(scheme["type"], "http", "named apiKey but an HTTP scheme");
    assert_eq!(scheme["scheme"], "bearer");
}

#[test]
fn the_base_url_matches_the_published_server_and_route_prefix() {
    let document = document();
    assert_eq!(document["servers"][0]["url"], "https://api.excalidraw.com");
    assert_eq!(API_BASE_URL, "https://api.excalidraw.com/api/v1");
    for (_, path) in OPERATIONS {
        assert!(path.starts_with("/api/v1/"), "{path}");
    }
}

#[test]
fn the_artifact_still_publishes_no_element_schema() {
    let document = document();

    // Reads: the item schema is literally `{}`.
    let items = &document["paths"]["/api/v1/scenes/{sceneId}/content"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"]["properties"]["elements"]["items"];
    assert_eq!(
        items,
        &serde_json::json!({}),
        "an element schema appeared; the crate's \"no element model\" stance should be revisited"
    );

    // Writes: the whole elements schema is `{}` on both write operations.
    for method in ["put", "patch"] {
        let elements = &document["paths"]["/api/v1/scenes/{sceneId}/content"][method]["requestBody"]
            ["content"]["application/json"]["schema"]["properties"]["elements"];
        assert_eq!(elements, &serde_json::json!({}), "{method} elements schema");
    }
}

#[test]
fn the_artifact_still_names_no_reusable_schemas() {
    let document = document();
    assert_eq!(
        document["components"]["schemas"],
        serde_json::json!({}),
        "named schemas appeared; code generation may now be viable"
    );
    for (method, path) in OPERATIONS {
        assert!(
            document["paths"][path][method]["operationId"].is_null(),
            "{method} {path} gained an operationId"
        );
    }
}

#[test]
fn rate_limiting_and_server_errors_remain_prose_only() {
    let document = document();
    for (method, path) in OPERATIONS {
        let responses = document["paths"][path][method]["responses"]
            .as_object()
            .expect("responses");
        // 413 and 503 are also reachable in practice, observed 2026-09-21.
        for status in ["413", "429", "500", "503"] {
            assert!(
                !responses.contains_key(status),
                "{method} {path} now declares {status}; error mapping can stop relying on prose"
            );
        }
        assert!(
            responses.contains_key("200") && responses.contains_key("401"),
            "{method} {path} lost a documented status"
        );
    }
}

#[test]
fn the_patch_content_root_is_closed_to_three_fields() {
    let document = document();
    let schema = &document["paths"]["/api/v1/scenes/{sceneId}/content"]["patch"]["requestBody"]["content"]
        ["application/json"]["schema"];

    let mut properties: Vec<&String> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .collect();
    properties.sort();
    assert_eq!(
        properties,
        ["appState", "elements", "files", "filesFailedToEmbed"],
        "PATCH gained or lost a root field; scene_content::patch_from must match"
    );
    assert_eq!(
        schema["additionalProperties"],
        serde_json::json!(false),
        "a full GET envelope is not a legal PATCH body"
    );
}

#[test]
fn the_stored_app_state_profile_is_still_two_fields() {
    let document = document();
    let schema = &document["paths"]["/api/v1/scenes/{sceneId}/content"]["get"]["responses"]["200"]
        ["content"]["application/json"]["schema"]["properties"]["appState"];

    let mut properties: Vec<&String> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .collect();
    properties.sort();
    assert_eq!(properties, ["lockedMultiSelections", "viewBackgroundColor"]);
    assert_eq!(schema["additionalProperties"], serde_json::json!(false));

    let mut ours = excalidraw_api::scene_content::PLUS_APP_STATE_KEYS;
    ours.sort_unstable();
    assert_eq!(
        ours.as_slice(),
        ["lockedMultiSelections", "viewBackgroundColor"],
        "the pruning allow-list must track the published profile"
    );
}

#[test]
fn pagination_bounds_match_the_published_parameters() {
    let document = document();
    let parameters = document["paths"]["/api/v1/scenes"]["get"]["parameters"]
        .as_array()
        .expect("parameters");

    let limit = parameters
        .iter()
        .find(|p| p["name"] == "limit")
        .expect("limit parameter");
    assert_eq!(limit["schema"]["minimum"], 1);
    assert_eq!(limit["schema"]["maximum"], 100);

    let offset = parameters
        .iter()
        .find(|p| p["name"] == "offset")
        .expect("offset parameter");
    assert_eq!(offset["schema"]["minimum"], 0);
    assert_eq!(offset["schema"]["maximum"], 9_007_199_254_740_991u64);
}

#[test]
fn the_file_record_limits_match_what_the_document_crate_enforces() {
    let document = document();
    let file = &document["paths"]["/api/v1/scenes/{sceneId}/content"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"]["properties"]["files"]["additionalProperties"];
    assert_eq!(
        file["properties"]["id"]["pattern"],
        "^[A-Za-z0-9_-]{1,128}$"
    );
    assert_eq!(file["properties"]["dataURL"]["maxLength"], 20_971_520u64);
    assert_eq!(file["additionalProperties"], serde_json::json!(false));
}

// ------------------------------------------------------------------ the join
//
// `OPERATIONS` above is what the artifact publishes. `all_ops::all()` is what
// the crate builds. Until these met, both lists were hand-written and nothing
// connected them: an op could target a path the artifact does not document, or
// the crate could stop covering one, and every test still passed.

mod all_ops;

#[test]
fn every_implemented_operation_is_one_the_artifact_publishes() {
    let mut implemented: Vec<(String, String)> = all_ops::all()
        .iter()
        .map(|built| (built.method.as_str().to_lowercase(), built.template()))
        .collect();
    implemented.sort();

    let mut published: Vec<(String, String)> = OPERATIONS
        .iter()
        .map(|&(method, path)| (method.to_owned(), path.to_owned()))
        .collect();
    published.sort();

    assert_eq!(
        implemented, published,
        "the operations this crate builds and the operations the artifact \
         publishes have diverged"
    );
}

#[test]
fn no_two_operations_claim_the_same_method_and_path() {
    let built = all_ops::all();
    let mut seen: Vec<(String, String, &str)> = built
        .iter()
        .map(|b| (b.method.as_str().to_owned(), b.template(), b.name))
        .collect();
    seen.sort();
    for pair in seen.windows(2) {
        assert!(
            pair[0].0 != pair[1].0 || pair[0].1 != pair[1].1,
            "{} and {} both build {} {}",
            pair[0].2,
            pair[1].2,
            pair[0].0,
            pair[0].1
        );
    }
    assert_eq!(built.len(), OPERATIONS.len());
}
