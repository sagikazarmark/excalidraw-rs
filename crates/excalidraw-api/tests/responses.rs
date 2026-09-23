//! Response decoding and status mapping. No I/O.
use excalidraw_api::{Error, NoHeaders, Operation, SceneId, SliceHeaders, UserId, model, op};
use serde_json::json;

fn scene_record() -> serde_json::Value {
    json!({
        "metadata": {
            "workspace": "w-1", "name": "Diagram", "created": "2026-09-01T10:00:00Z",
            "updated": null, "previewUrl": null, "previewFilename": null,
            "previewBackground": "#ffffff", "isDeleted": false, "isPrivate": false,
            "creator": "u-1", "updater": null, "sceneVersion": "abc123",
            "contentEpoch": 7, "linkSharing": 1, "collection": "col-1",
            "updateCount": 12, "revisionCount": 3, "lastRevision": null,
            "totalElements": 40, "deletedElements": 2, "pinned": false, "id": "s-1"
        },
        "readOnlyLinks": [],
        "sharedSlidesLinks": []
    })
}

fn decode<O: Operation>(op: O, body: serde_json::Value) -> O::Output {
    op.decode(200, &NoHeaders, &serde_json::to_vec(&body).unwrap())
        .expect("decodes")
}

#[test]
fn scene_metadata_decodes() {
    let record = decode(
        op::GetScene {
            scene: SceneId::new("s-1").unwrap(),
        },
        scene_record(),
    );
    assert_eq!(record.metadata.id, "s-1");
    assert_eq!(record.metadata.scene_version.as_str(), "abc123");
    assert_eq!(record.metadata.content_epoch, 7);
    assert_eq!(record.metadata.link_sharing, model::LinkSharing::ReadOnly);
    assert_eq!(record.metadata.total_elements, 40);
    assert_eq!(record.metadata.updated, None);
    assert!(record.metadata.extra.is_empty());
}

#[test]
fn unknown_response_fields_are_retained_not_rejected() {
    let mut body = scene_record();
    body["metadata"]["somethingNew"] = json!({ "nested": true });
    body["alsoNew"] = json!(1);

    let record = decode(
        op::GetScene {
            scene: SceneId::new("s-1").unwrap(),
        },
        body,
    );
    assert_eq!(
        record.metadata.extra.get("somethingNew"),
        Some(&json!({ "nested": true })),
        "a new server field must not break a reader"
    );
    assert_eq!(record.extra.get("alsoNew"), Some(&json!(1)));
}

#[test]
fn nullable_fields_decode_from_null_and_from_absence_alike() {
    let with_null = json!({
        "id": "c-1", "name": "Design", "workspace": "w-1",
        "created": "2026-09-01T10:00:00Z",
        "updated": null, "creator": null, "teams": null
    });
    let collection: model::Collection = serde_json::from_value(with_null).expect("null decodes");
    assert_eq!(collection.creator, None);
    assert_eq!(collection.teams, None);

    // Documented limitation: serde treats a missing `Option` field as `None`,
    // so a field the server stops sending is indistinguishable from an explicit
    // null. Detecting that removal is the drift check's job, not decoding's.
    let mut absent = json!({
        "id": "c-1", "name": "Design", "workspace": "w-1",
        "created": "2026-09-01T10:00:00Z",
        "updated": null, "creator": null, "teams": null
    });
    absent.as_object_mut().unwrap().remove("creator");
    let collection: model::Collection =
        serde_json::from_value(absent).expect("absence decodes as None too");
    assert_eq!(collection.creator, None);
}

#[test]
fn a_missing_non_nullable_field_is_still_a_decode_error() {
    // The guarantee that does hold: a required field with a non-Option type
    // cannot go missing unnoticed.
    let mut body = json!({
        "id": "c-1", "name": "Design", "workspace": "w-1",
        "created": "2026-09-01T10:00:00Z",
        "updated": null, "creator": null, "teams": null
    });
    body.as_object_mut().unwrap().remove("workspace");
    assert!(serde_json::from_value::<model::Collection>(body).is_err());
}

#[test]
fn counts_typed_as_number_accept_integral_values_only() {
    let mut body = scene_record();
    body["metadata"]["updateCount"] = json!(12.0);
    let record = decode(
        op::GetScene {
            scene: SceneId::new("s-1").unwrap(),
        },
        body.clone(),
    );
    assert_eq!(record.metadata.update_count, 12);

    body["metadata"]["updateCount"] = json!(12.5);
    let refused = op::GetScene {
        scene: SceneId::new("s-1").unwrap(),
    }
    .decode(200, &NoHeaders, &serde_json::to_vec(&body).unwrap());
    assert!(
        matches!(refused, Err(Error::Decode { .. })),
        "a fractional count is not silently truncated"
    );
}

#[test]
fn content_epoch_is_refused_past_the_published_maximum() {
    let mut body = scene_record();
    body["metadata"]["contentEpoch"] = json!(9_007_199_254_740_992u64);
    let refused = op::GetScene {
        scene: SceneId::new("s-1").unwrap(),
    }
    .decode(200, &NoHeaders, &serde_json::to_vec(&body).unwrap());
    assert!(
        matches!(refused, Err(Error::Decode { .. })),
        "contentEpoch is published as 0..=9007199254740991"
    );
}

#[test]
fn an_unlisted_link_sharing_value_survives() {
    let mut body = scene_record();
    body["metadata"]["linkSharing"] = json!(2);
    let record = decode(
        op::GetScene {
            scene: SceneId::new("s-1").unwrap(),
        },
        body,
    );
    assert_eq!(record.metadata.link_sharing, model::LinkSharing::Unknown(2));
}

#[test]
fn log_entries_decode_from_snake_case_keys() {
    let page = decode(
        op::GetLogs::default(),
        json!({
            "logs": [{
                "id": "0191b5e0-0000-7000-8000-000000000000",
                "action": "workspace", "operation": "update",
                "details": { "field": "name" },
                "created_at": "2026-09-01T10:00:00Z",
                "ip_address": null, "user_id": "u-1", "workspace_id": "w-1",
                "app_source": "excalidraw-plus", "source_type": "api",
                "source_id": null, "status": 200,
                "user_email": "a@example.com"
            }],
            "nextCursor": "next-1",
            "hasMore": true,
            "availableActions": ["workspace", "scene"]
        }),
    );
    let entry = &page.logs[0];
    assert_eq!(entry.user_id.as_deref(), Some("u-1"));
    assert_eq!(entry.app_source, model::AppSource::ExcalidrawPlus);
    assert_eq!(entry.source_type, model::SourceType::Api);
    assert_eq!(entry.status, 200);
    // Page-mode counts are absent outside page mode, despite being marked
    // required by the published schema.
    assert_eq!(page.total_count, None);
    assert_eq!(page.current_page, None);
    assert!(page.has_more);
}

#[test]
fn log_page_counts_decode_in_page_mode() {
    let page = decode(
        op::GetLogs::default(),
        json!({
            "logs": [], "nextCursor": null, "hasMore": false,
            "availableActions": [],
            "totalCount": 120, "totalPages": 12, "currentPage": 1
        }),
    );
    assert_eq!(page.total_count, Some(120));
    assert_eq!(page.total_pages, Some(12));
}

#[test]
fn max_uses_decodes_both_published_arms() {
    let invite = |max: serde_json::Value| -> model::Invite {
        serde_json::from_value(json!({
            "id": "i-1", "created": "2026-09-01T10:00:00Z", "type": "link",
            "status": "pending", "email": null, "role": "member",
            "resolvedAt": null, "redeemedBy": null, "maxUses": max
        }))
        .expect("decodes")
    };
    assert_eq!(invite(json!(5)).max_uses, Some(model::MaxUses::Limited(5)));
    assert_eq!(
        invite(json!("unlimited")).max_uses,
        Some(model::MaxUses::Unlimited)
    );

    let refused: Result<model::Invite, _> = serde_json::from_value(json!({
        "id": "i-1", "created": "x", "type": "link", "status": "pending",
        "email": null, "role": "member", "resolvedAt": null, "redeemedBy": null,
        "maxUses": "many"
    }));
    assert!(
        refused.is_err(),
        "an unlisted string is not silently accepted"
    );
}

#[test]
fn a_delete_retains_whatever_the_server_returned() {
    let ack = decode(
        op::DeleteScene {
            scene: SceneId::new("s-1").unwrap(),
        },
        json!({}),
    );
    assert!(ack.is_empty());

    let ack = decode(
        op::DeleteUser {
            user: UserId::new("u-1").unwrap(),
        },
        json!({ "queued": true }),
    );
    assert_eq!(ack.0.get("queued"), Some(&json!(true)));
}

// ------------------------------------------------------------- status mapping

fn fail(status: u16, body: &str) -> Error {
    op::GetWorkspace
        .decode(status, &NoHeaders, body.as_bytes())
        .expect_err("not a success")
}

#[test]
fn documented_error_envelopes_map_to_api_errors() {
    for (status, kind) in [
        (400u16, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
    ] {
        let body = json!({ "statusCode": status, "error": kind, "message": "nope" });
        match fail(status, &body.to_string()) {
            Error::Api {
                status: got,
                kind: got_kind,
                message,
            } => {
                assert_eq!(got, status);
                assert_eq!(got_kind, kind);
                assert_eq!(message, "nope");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }
}

#[test]
fn a_documented_status_with_an_undocumented_body_is_not_given_a_fake_message() {
    match fail(401, "<html>gateway</html>") {
        Error::Unexpected { status, body } => {
            assert_eq!(status, 401);
            assert_eq!(body, b"<html>gateway</html>");
        }
        other => panic!("expected Unexpected, got {other:?}"),
    }
}

#[test]
fn rate_limiting_is_mapped_from_headers_though_no_operation_declares_it() {
    let headers = SliceHeaders(&[
        ("X-RateLimit-Limit", "600"),
        ("x-ratelimit-remaining", "0"),
        ("X-RateLimit-Reset", "1789000000"),
    ]);
    let error = op::GetWorkspace
        .decode(429, &headers, b"{}")
        .expect_err("429 is an error");
    match error {
        Error::RateLimited(limit) => {
            assert_eq!(limit.limit, Some(600));
            assert_eq!(limit.remaining, Some(0));
            assert_eq!(limit.reset, Some(1789000000));
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
    assert!(
        excalidraw_api::Error::RateLimited(Default::default()).is_retryable(),
        "429 is retryable"
    );
}

#[test]
fn server_errors_are_unexpected_and_retryable() {
    let error = fail(500, "internal");
    assert_eq!(error.status(), Some(500));
    assert!(error.is_retryable());

    let error = fail(
        404,
        r#"{"statusCode":404,"error":"Not Found","message":"gone"}"#,
    );
    assert!(!error.is_retryable(), "a 404 must not be retried");
}

#[test]
fn a_malformed_success_body_reports_a_decode_error() {
    let error = op::GetWorkspace
        .decode(200, &NoHeaders, b"{\"id\": 5}")
        .expect_err("id is a string");
    assert!(matches!(error, Error::Decode { .. }));
}

#[test]
fn undeclared_statuses_carrying_the_envelope_are_typed_errors() {
    // 413 is reachable: a dataURL over the request body limit returns
    // FST_ERR_CTP_BODY_TOO_LARGE (observed 2026-09-21), and the artifact
    // declares 413 nowhere.
    let body = r#"{"code":"FST_ERR_CTP_BODY_TOO_LARGE","error":"Payload Too Large","message":"Request body is too large","statusCode":413}"#;
    match fail(413, body) {
        Error::Api {
            status,
            kind,
            message,
        } => {
            assert_eq!(status, 413);
            assert_eq!(kind, "Payload Too Large");
            assert_eq!(message, "Request body is too large");
        }
        other => panic!("expected a typed error, got {other:?}"),
    }
    assert!(
        !fail(413, body).is_retryable(),
        "a too-large body will not shrink"
    );
}

#[test]
fn a_server_error_is_retryable_whatever_shape_its_body_has() {
    // 503 was observed as a transient during the live run.
    let enveloped = fail(
        503,
        r#"{"statusCode":503,"error":"Service Unavailable","message":"try again"}"#,
    );
    assert!(matches!(enveloped, Error::Api { status: 503, .. }));
    assert!(enveloped.is_retryable());

    let bare = fail(503, "<html>503</html>");
    assert!(matches!(bare, Error::Unexpected { status: 503, .. }));
    assert!(bare.is_retryable());
}
