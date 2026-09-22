//! What `UnguardedCheckThenWrite` catches — and, deliberately, what it does not.
//!
//! Half of these tests assert a *negative*. The type is named for its weakness
//! and its rustdoc records two facts measured against the live service on
//! 2026-09-21: `sceneVersion` does not track geometry, and `contentEpoch` counts
//! only authoritative replacements. The `does_not_detect` tests pin that
//! weakness in the suite instead of leaving it to prose — if one of them starts
//! failing, the helper has quietly grown a guarantee its name, its rustdoc and
//! `docs/excalidraw-api-spec.md` all disclaim, which is a contract change and
//! not a bug fix.
//!
//! No test here performs I/O.
use excalidraw_api::{Error, SceneVersion, model, scene_content::UnguardedCheckThenWrite};
use serde_json::{Value, json};

/// Scene metadata as `GET /scenes/{id}` returns it, with `edit` applied first.
///
/// Built by decoding a response body rather than by struct literal: the only
/// `SceneMetadata` a caller ever holds is one the server sent.
fn metadata(edit: impl FnOnce(&mut Value)) -> model::SceneMetadata {
    let mut body = json!({
        "id": "s-1", "name": "Diagram", "workspace": "w-1", "collection": "col-1",
        "created": "2026-09-01T10:00:00Z", "updated": "2026-09-01T10:00:00Z",
        "creator": "u-1", "updater": "u-1",
        "sceneVersion": "v1", "contentEpoch": 7,
        "linkSharing": 1, "previewUrl": null, "previewFilename": null,
        "previewBackground": "#ffffff", "isDeleted": false, "isPrivate": false,
        "pinned": false, "updateCount": 12, "revisionCount": 3,
        "totalElements": 40, "deletedElements": 2, "lastRevision": null
    });
    edit(&mut body);
    serde_json::from_value(body).expect("fixture decodes")
}

/// Metadata exactly as it was when the caller recorded its expectation.
fn unchanged() -> model::SceneMetadata {
    metadata(|_| {})
}

fn version(value: &str) -> SceneVersion {
    SceneVersion::new(value)
}

/// The caller's expectation, taken from a scene it has just read.
fn intent() -> UnguardedCheckThenWrite {
    UnguardedCheckThenWrite::new(version("v1"))
}

fn detail(error: Error) -> String {
    match error {
        Error::Invalid { what, detail } => {
            assert_eq!(what, "scene changed");
            detail
        }
        other => panic!("expected a caller-side Invalid, got {other:?}"),
    }
}

// ------------------------------------------------------------------ it catches

#[test]
fn an_unchanged_scene_lets_the_write_proceed() {
    intent().check(&unchanged()).expect("nothing moved");
}

#[test]
fn an_unchanged_scene_lets_the_write_proceed_with_the_epoch_expected() {
    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    intent.check(&unchanged()).expect("nothing moved");
}

#[test]
fn an_advanced_scene_version_is_detected() {
    let found = metadata(|body| body["sceneVersion"] = json!("v2"));

    let detail = detail(intent().check(&found).expect_err("the scene moved"));
    assert!(
        detail.contains("v1") && detail.contains("v2"),
        "the message names what was expected and what was found: {detail}"
    );
}

#[test]
fn a_changed_content_epoch_is_detected_when_the_caller_asked_for_it() {
    // A concurrent PUT: `contentEpoch` advances even when the replacement body
    // reproduces the same `sceneVersion`.
    let found = metadata(|body| body["contentEpoch"] = json!(8));

    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    let detail = detail(intent.check(&found).expect_err("a replacement landed"));
    assert!(
        detail.contains("contentEpoch") && detail.contains('7') && detail.contains('8'),
        "the message names the epoch it compared: {detail}"
    );
}

#[test]
fn an_epoch_that_went_backwards_is_a_change_like_any_other() {
    // Equality, not ordering: `SceneVersion` supports only equality, and the
    // epoch is compared the same way. A rewind is not "no change".
    let found = metadata(|body| body["contentEpoch"] = json!(6));

    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    intent.check(&found).expect_err("6 is not 7");
}

#[test]
fn the_scene_version_is_reported_even_when_the_epoch_still_matches() {
    let found = metadata(|body| body["sceneVersion"] = json!("v2"));

    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    let detail = detail(intent.check(&found).expect_err("the scene moved"));
    assert!(
        detail.contains("sceneVersion"),
        "the first mismatch found is the one reported: {detail}"
    );
}

// -------------------------------------------------------------- it does not catch
//
// Every test below asserts `Ok` on a scene that *did* change under the caller.

#[test]
fn check_does_not_detect_a_geometry_only_edit() {
    // Measured: moving an element's `x`, or raising an element's `version`, left
    // `sceneVersion` untouched. Another writer can reposition the whole scene —
    // bumping `updated`, `updater` and the counters on the way — and the check
    // still passes, because it reads neither the content nor any of those.
    let moved = metadata(|body| {
        body["updated"] = json!("2026-09-01T11:00:00Z");
        body["updater"] = json!("someone-else");
        body["updateCount"] = json!(13);
        body["revisionCount"] = json!(4);
    });

    intent()
        .check(&moved)
        .expect("documented blind spot: sceneVersion does not track geometry");
}

#[test]
fn check_does_not_detect_a_concurrent_patch_even_with_the_epoch_expected() {
    // Measured: `contentEpoch` never advanced on `PATCH`. Combined with the
    // blind spot above, a concurrent `PATCH` of element geometry moves neither
    // value, so the strongest form of this check still passes over it.
    let patched = metadata(|body| {
        body["updated"] = json!("2026-09-01T11:00:00Z");
        body["totalElements"] = json!(41);
    });

    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    intent
        .check(&patched)
        .expect("documented blind spot: contentEpoch is blind to PATCH");
}

#[test]
fn check_does_not_detect_a_concurrent_metadata_only_update() {
    // Measured: a metadata-only update does not advance `contentEpoch` either.
    // A rename or a sharing change under the caller's feet is invisible here.
    let renamed = metadata(|body| {
        body["name"] = json!("Renamed by someone else");
        body["linkSharing"] = json!(3);
        body["isPrivate"] = json!(true);
    });

    let intent = UnguardedCheckThenWrite {
        expected: version("v1"),
        also_expect_epoch: Some(7),
    };
    intent
        .check(&renamed)
        .expect("documented blind spot: metadata updates move neither value");
}

#[test]
fn new_does_not_compare_the_epoch_so_an_identical_concurrent_put_is_missed() {
    // `new` leaves the epoch out: comparing it is opt-in. Measured: `PUT`
    // advances `contentEpoch` even when the body is identical, and `PUT`
    // recomputes `sceneVersion` from that body — so a concurrent republication
    // of the same content is a real authoritative write that this intent, as
    // constructed by `new`, cannot see.
    let republished = metadata(|body| body["contentEpoch"] = json!(8));

    let intent = intent();
    assert_eq!(intent.also_expect_epoch, None, "epoch comparison is opt-in");
    intent
        .check(&republished)
        .expect("documented blind spot: new() does not compare the epoch");
}

#[test]
fn check_does_not_bind_the_write_that_follows() {
    // Not a compare-and-swap: `check` is a point-in-time observation against
    // metadata the caller happens to hold, it takes `&self`, and it returns
    // nothing the write has to carry. The same intent passing now says nothing
    // about the scene a moment later, and nothing stops the write either way.
    let intent = intent();
    intent.check(&unchanged()).expect("passes now");

    let later = metadata(|body| body["sceneVersion"] = json!("v2"));
    intent.check(&later).expect_err("and not a moment later");

    // Still usable, still unchanged by either call: it guards nothing.
    intent.check(&unchanged()).expect("passes again");
    assert_eq!(intent, UnguardedCheckThenWrite::new(version("v1")));
}
