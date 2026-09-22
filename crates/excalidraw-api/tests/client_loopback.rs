//! Offline coverage for the HTTP client, over a loopback origin.
//!
//! The crate's other suites stop at the `Operation` seam: they assert what
//! `request()` builds and what `decode()` accepts. Everything between those two
//! points — base-URL normalisation, the `Authorization` header, when a
//! `Content-Type` is attached, the rate-limit latch, the pagination walk and the
//! retry loop — lives on the concrete client and had no coverage at all.
//!
//! A real `TcpListener` on 127.0.0.1 covers it without a mock-server dependency:
//! the client under test is the shipping one, and the assertions are made
//! against bytes that actually crossed a socket. `Connection: close` on every
//! canned response keeps the accept loop one-request-per-connection, so the
//! recorded order is the request order.
//!
//! Scope note: this models transport mechanics only. Per
//! `docs/excalidraw-api-crate-plan.md` ("Verification plan", item 7), a local
//! origin must not model an invented nonce tie-break, server ID normalisation
//! or compare-and-swap, and nothing here does. Results prove the client's
//! behaviour, never Excalidraw Plus's.
//!
//! The loopback origin itself lives in `common`, shared with
//! `client_loopback_async.rs` so both clients are proved against exactly the
//! same server.
#![cfg(feature = "blocking")]

use excalidraw_api::{
    ApiKey, ClientConfig, PageRequest, SceneId,
    blocking::Client,
    model::NewCollection,
    op::{CreateCollection, ListCollections, ReplaceSceneContent},
};

mod common;

use common::{JSON, origin, page};

fn client(base: &str) -> Client {
    Client::with_config(
        ApiKey::new("test-key").expect("key"),
        ClientConfig::default().base_url(base.to_owned()),
    )
    .expect("client")
}

#[test]
fn a_trailing_slash_on_the_base_url_does_not_double_the_separator() {
    let (base, handle) = origin(vec![(
        200,
        vec![JSON],
        r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#,
    )]);
    let client = Client::with_config(
        ApiKey::new("test-key").expect("key"),
        // The documented normalisation: a trailing slash is trimmed away.
        ClientConfig::default().base_url(format!("{base}/")),
    )
    .expect("client");

    client
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect("send");

    let seen = handle.join().expect("origin thread");
    assert_eq!(seen[0].start_line, "GET /collections HTTP/1.1");
}

#[test]
fn the_api_key_is_sent_as_a_bearer_token_and_never_logged_in_the_path() {
    let (base, handle) = origin(vec![(
        200,
        vec![JSON],
        r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#,
    )]);

    client(&base)
        .send(ListCollections {
            page: PageRequest::new().limit(25).offset(50),
        })
        .expect("send");

    let seen = handle.join().expect("origin thread");
    assert_eq!(seen[0].header("authorization"), Some("Bearer test-key"));
    // Query parameters are emitted in declaration order.
    assert_eq!(
        seen[0].start_line,
        "GET /collections?limit=25&offset=50 HTTP/1.1"
    );
    assert!(!seen[0].start_line.contains("test-key"));
}

#[test]
fn a_bodyless_request_carries_no_content_type_and_a_json_body_carries_one() {
    let listing = r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#;
    let created =
        r#"{"id":"c1","name":"Team","workspace":"w","created":"2026-01-01T00:00:00.000Z"}"#;
    let (base, handle) = origin(vec![(200, vec![JSON], listing), (200, vec![JSON], created)]);
    let client = client(&base);

    client
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect("list");
    client
        .send(CreateCollection {
            collection: NewCollection {
                name: "Team".to_owned(),
            },
        })
        .expect("create");

    let seen = handle.join().expect("origin thread");
    assert_eq!(seen[0].header("content-type"), None);
    assert_eq!(seen[1].header("content-type"), Some("application/json"));
    // The body is sent verbatim, not re-serialised by the transport.
    assert_eq!(seen[1].body, r#"{"name":"Team"}"#);
    assert_eq!(seen[1].start_line, "POST /collections HTTP/1.1");
}

#[test]
fn the_user_agent_carries_the_crate_version_and_the_configured_suffix() {
    let (base, handle) = origin(vec![(
        200,
        vec![JSON],
        r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#,
    )]);
    let client = Client::with_config(
        ApiKey::new("test-key").expect("key"),
        ClientConfig::default()
            .base_url(base.clone())
            .user_agent_suffix("excaliplot-tests"),
    )
    .expect("client");

    client
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect("send");

    let seen = handle.join().expect("origin thread");
    let agent = seen[0].header("user-agent").expect("user-agent");
    assert!(agent.starts_with("excalidraw-api/"), "{agent}");
    assert!(agent.ends_with(" excaliplot-tests"), "{agent}");
}

#[test]
fn the_rate_limit_latch_records_headers_and_a_later_bare_response_does_not_clear_it() {
    let listing = r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#;
    let (base, handle) = origin(vec![
        (
            200,
            vec![
                JSON,
                ("X-RateLimit-Limit", "600"),
                ("X-RateLimit-Remaining", "599"),
                ("X-RateLimit-Reset", "1790000000"),
            ],
            listing,
        ),
        (200, vec![JSON], listing),
    ]);
    let client = client(&base);

    assert!(client.last_rate_limit().is_none(), "nothing sent yet");

    client
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect("first");
    let seen_limit = client.last_rate_limit().expect("latched");
    assert_eq!(seen_limit.limit, Some(600));
    assert_eq!(seen_limit.remaining, Some(599));
    assert_eq!(seen_limit.reset, Some(1_790_000_000));

    client
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect("second");
    // A response with no rate-limit headers must not clobber a good reading.
    let still = client.last_rate_limit().expect("retained");
    assert_eq!(still.remaining, Some(599));

    handle.join().expect("origin thread");
}

#[test]
fn collect_stops_requesting_once_the_budget_is_spent() {
    // Three pages are available and each carries one item, but the caller asked
    // for two. A third request would be wasted: its page cannot be used, and it
    // still costs rate-limit budget. Regression for the over-fetch fixed in
    // `page::absorb`.
    let (base, handle) = origin(vec![
        (
            200,
            vec![JSON],
            Box::leak(page(&["a"], true, 0).into_boxed_str()),
        ),
        (
            200,
            vec![JSON],
            Box::leak(page(&["b"], true, 1).into_boxed_str()),
        ),
        (
            200,
            vec![JSON],
            Box::leak(page(&["c"], true, 2).into_boxed_str()),
        ),
    ]);

    let collected = client(&base)
        .collect(
            |page| ListCollections { page },
            PageRequest::new().limit(1),
            2,
        )
        .expect("collect");

    let seen = handle.join().expect("origin thread");
    assert_eq!(collected.len(), 2);
    assert_eq!(
        seen.len(),
        2,
        "asked for 2 items at 1 per page; a third request is an over-fetch"
    );
    assert_eq!(
        seen[1].start_line,
        "GET /collections?limit=1&offset=1 HTTP/1.1"
    );
}

#[test]
fn collect_with_a_zero_budget_sends_nothing() {
    // A walk that can keep nothing must not spend a request, or rate-limit
    // budget, on a page it would discard.
    let (base, handle) = origin(vec![(
        200,
        vec![JSON],
        Box::leak(page(&["a"], true, 0).into_boxed_str()),
    )]);

    let collected = client(&base)
        .collect(
            |page| ListCollections { page },
            PageRequest::new().limit(1),
            0,
        )
        .expect("collect");

    assert!(collected.is_empty());
    assert!(
        handle.join().expect("origin thread").is_empty(),
        "a zero budget must not fetch"
    );
}

#[test]
fn collect_stops_on_an_empty_page_that_still_reports_a_next() {
    // The page after an empty one is at the same offset: following it would
    // request that page forever, since nothing ever counts toward the budget.
    let (base, handle) = origin(vec![
        (
            200,
            vec![JSON],
            Box::leak(page(&[], true, 0).into_boxed_str()),
        ),
        (
            200,
            vec![JSON],
            Box::leak(page(&[], true, 0).into_boxed_str()),
        ),
    ]);

    let collected = client(&base)
        .collect(
            |page| ListCollections { page },
            PageRequest::new().limit(1),
            100,
        )
        .expect("collect");

    assert!(collected.is_empty());
    assert_eq!(
        handle.join().expect("origin thread").len(),
        1,
        "an empty page cannot advance the offset"
    );
}

#[test]
fn collect_walks_every_page_until_the_server_reports_no_next() {
    let (base, handle) = origin(vec![
        (
            200,
            vec![JSON],
            Box::leak(page(&["a"], true, 0).into_boxed_str()),
        ),
        (
            200,
            vec![JSON],
            Box::leak(page(&["b"], false, 1).into_boxed_str()),
        ),
    ]);

    let collected = client(&base)
        .collect(
            |page| ListCollections { page },
            PageRequest::new().limit(1),
            100,
        )
        .expect("collect");

    let seen = handle.join().expect("origin thread");
    assert_eq!(collected.len(), 2);
    assert_eq!(
        seen.len(),
        2,
        "the walk ends on hasNextPage=false, not on a wasted probe"
    );
}

#[test]
fn a_documented_error_envelope_becomes_an_api_error_with_its_status() {
    let (base, handle) = origin(vec![(
        404,
        vec![JSON],
        r#"{"statusCode":404,"error":"Not Found","message":"no such collection"}"#,
    )]);

    let error = client(&base)
        .send(ListCollections {
            page: PageRequest::new(),
        })
        .expect_err("404 is an error");

    handle.join().expect("origin thread");
    match error {
        excalidraw_api::Error::Api {
            status,
            kind,
            message,
        } => {
            assert_eq!(status, 404);
            assert_eq!(kind, "Not Found");
            assert_eq!(message, "no such collection");
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

#[cfg(feature = "retry")]
mod retry {
    use super::*;
    use excalidraw_api::{Backoff, RetryPolicy};
    use std::time::Duration;

    fn immediate(max_retries: u32) -> RetryPolicy {
        RetryPolicy {
            max_retries,
            backoff: Backoff {
                initial: Duration::from_millis(1),
                max: Duration::from_millis(1),
                factor_percent: 100,
            },
            respect_reset: false,
        }
    }

    #[test]
    fn a_retried_get_makes_exactly_one_attempt_more_than_the_retry_budget() {
        let unavailable = r#"{"statusCode":503,"error":"Service Unavailable","message":"down"}"#;
        let (base, handle) = origin(vec![
            (503, vec![JSON], unavailable),
            (503, vec![JSON], unavailable),
            (503, vec![JSON], unavailable),
        ]);

        let error = client(&base)
            .send_retrying(
                ListCollections {
                    page: PageRequest::new(),
                },
                immediate(2),
            )
            .expect_err("still failing after the budget");

        let seen = handle.join().expect("origin thread");
        assert_eq!(seen.len(), 3, "1 initial attempt + 2 retries");
        // A 5xx carrying the documented envelope is a typed `Api` error, not
        // `Unexpected` — and it is still retryable.
        assert!(
            matches!(error, excalidraw_api::Error::Api { status: 503, .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_retried_get_succeeds_on_a_later_attempt_without_replaying_the_success() {
        let unavailable = r#"{"statusCode":503,"error":"Service Unavailable","message":"down"}"#;
        let listing = r#"{"limit":10,"offset":0,"hasNextPage":false,"data":[]}"#;
        let (base, handle) = origin(vec![
            (503, vec![JSON], unavailable),
            (200, vec![JSON], listing),
        ]);

        client(&base)
            .send_retrying(
                ListCollections {
                    page: PageRequest::new(),
                },
                immediate(3),
            )
            .expect("second attempt succeeds");

        let seen = handle.join().expect("origin thread");
        assert_eq!(seen.len(), 2, "the walk stops at the first success");
    }

    #[test]
    fn a_full_content_replacement_is_never_retried_even_though_put_is_idempotent() {
        // The first attempt may have landed behind the 503. Replaying it would be
        // a second authoritative replacement: another `contentEpoch`, another
        // forced reload, and any collaborator edit made during the backoff lost.
        let unavailable = r#"{"statusCode":503,"error":"Service Unavailable","message":"down"}"#;
        let (base, handle) = origin(vec![
            (503, vec![JSON], unavailable),
            (503, vec![JSON], unavailable),
        ]);
        let document = excalidraw_document::Document::from_value(serde_json::json!({
            "type": "excalidraw", "version": 2, "source": "test",
            "elements": [], "appState": {"viewBackgroundColor": "#ffffff"}, "files": {}
        }))
        .unwrap();

        client(&base)
            .send_retrying(
                ReplaceSceneContent {
                    scene: SceneId::new("scene-1").unwrap(),
                    body: excalidraw_api::plus::ReplaceSceneContent::new(document).unwrap(),
                },
                immediate(3),
            )
            .expect_err("PUT content fails without retrying");

        let seen = handle.join().expect("origin thread");
        assert_eq!(seen.len(), 1, "a replay is a new authoritative write");
    }

    #[test]
    fn a_post_is_never_retried_even_when_the_status_is_retryable() {
        let unavailable = r#"{"statusCode":503,"error":"Service Unavailable","message":"down"}"#;
        let (base, handle) = origin(vec![(503, vec![JSON], unavailable)]);

        client(&base)
            .send_retrying(
                CreateCollection {
                    collection: NewCollection {
                        name: "Team".to_owned(),
                    },
                },
                immediate(3),
            )
            .expect_err("POST fails without retrying");

        let seen = handle.join().expect("origin thread");
        assert_eq!(seen.len(), 1, "POST has no published idempotency key");
    }
}
