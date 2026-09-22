# excalidraw-api

**Typed client for the public [Excalidraw Plus REST API](https://plus.excalidraw.com/docs/api), built on
[`excalidraw-document`](../excalidraw-document/README.md).**

All 27 documented operations. Scene payloads are `excalidraw_document::plus`
types, unchanged, so a download and an upload are the same preserving document
model the rest of this workspace uses.

## No I/O by default

The default build constructs requests and decodes responses as bytes:

<!-- Not compiled: `status`, `headers` and `body` come from the caller's own
     HTTP stack, so this sketch has no self-contained form. -->
```rust,ignore
use excalidraw_api::{Operation, SceneId, op};

let op = op::GetSceneContent { scene: SceneId::new("abc123")? };
let request = op.request()?;
assert_eq!(request.relative_url(), "/scenes/abc123/content");

// Send it with whatever HTTP stack you already have, then hand the result back:
let output = op.decode(status, &headers, &body)?;
```

Enable `client` for an async client, or `blocking` for a synchronous one. Both
are thin wrappers over the same `Operation` implementations, so error mapping is
identical whichever transport you use.

| Feature | Adds |
| --- | --- |
| *(default)* | request building and response decoding only |
| `client` | `reqwest` (async) plus a `ring` cryptography provider |
| `blocking` | `reqwest`'s blocking client; implies `client` |
| `retry` | bounded backoff for `429` and `5xx` on replayable operations |
| `client-core` | transport without a provider, for callers who install their own |

`reqwest` is built with `rustls-no-provider`, so `aws-lc-sys` and its cmake build
stay out of the tree; `client` then installs `ring`, which needs only a C
compiler. The client works with no setup.

Choosing your own provider means depending on `client-core` instead and calling
something like `rustls::crypto::aws_lc_rs::default_provider().install_default()`
before constructing a client. Nothing installs one for you in that
configuration, and reqwest panics on the first request without one.

## Download, convert, upload

<!-- Not compiled: needs the `blocking` feature, a key and a live server. The
     `offline_pipeline` example below is the executable form of this flow. -->
```rust,ignore
use excalidraw_api::{ApiKey, SceneId, op, scene_content::{self, Embedding}};

let client = excalidraw_api::blocking::Client::new(ApiKey::from_env()?)?;
let scene = SceneId::new("abc123")?;

let downloaded = client.send(op::GetSceneContent { scene: scene.clone() })?;
let body = scene_content::into_replacement(downloaded, Embedding::RequireComplete)?;
let replaced = client.send(op::ReplaceSceneContent { scene, body })?;
println!("{}", replaced.scene_version());
```

`cargo run -p excalidraw-api --example offline_pipeline` shows the same flow
against a committed fixture, with no network and no key.

## What the conversion policy protects you from

- **A partial download is not a legal upload.** A `GET` that could not embed
  every referenced file reports them in `filesFailedToEmbed`; replaying it through
  `PUT` sends a `files` map missing records that elements still reference, and the
  service rejects that with `400 Referenced file ... is missing from scene files`
  (tombstones included). `Embedding::RequireComplete` fails locally instead, before
  uploading a body that cannot succeed, and names the files;
  `Embedding::AllowMissing` sends it anyway.
- **A `PATCH` reusing a client-side id inserts a duplicate.** The service rewrites
  any element id not already in its 21-character form, so merging by an id you
  chose yourself silently creates a second element instead of updating the one you
  meant — with a `200`. `scene_content::patch_from` takes an
  `ElementIds::RequireCanonical` or `AllowProvisional` decision: the first rejects
  ids the service would rewrite, the second says you are inserting. The guard is
  absent from `PUT`, where provisional ids are correct because the whole scene and
  all its references are rewritten together.
- **A `GET` body is not a legal `PATCH` body.** The merge root is closed to
  `elements`, `appState` and `files`. Build merge requests with
  `scene_content::patch_from`; there is deliberately no way to forward a response
  verbatim.
- **Native exports carry `appState` the transport does not.** `gridSize`,
  `gridStep` and `gridModeEnabled` are rejected rather than silently stripped.
  `scene_content::replacement_from_document` prunes them only when the policy
  says so, and reports every dropped key as a `Change`.

A document produced by this workspace's own generators already satisfies the
transport profile and projects with zero changes.

## Deliberate limits

- **No element model.** The published element schema is literally `{}` for both
  reads and writes, so elements stay preserving records.
  `unconfirmed_element_paths()` reports documentation gaps, not server rejections.
- **No compare-and-swap.** No content operation publishes an `ETag`, `If-Match`,
  `409` or `412`, and `PUT` recomputes `sceneVersion` from the submitted body.
  `SceneVersion` supports equality only, and the read-then-write helper is called
  `UnguardedCheckThenWrite`. It is weaker than even that name suggests:
  `sceneVersion` does not change when element geometry does, so another writer can
  move a whole scene while the check still passes.
- **No automatic retries** unless you enable `retry` and pass a policy. `POST` is
  never retried: the API publishes no idempotency key. Nor is a full content
  replacement (`PUT /scenes/{id}/content`): a replay after an ambiguous `5xx` is a
  second authoritative write that bumps `contentEpoch`, reloads connected editors,
  and can erase a collaborator's edit made during the backoff.
- **Responses keep unknown fields** in an `extra` map. A field the server *stops*
  sending decodes as `None`, because serde cannot distinguish absent from null
  for an `Option`; catching removals is the drift check's job.
- No MCP client, no scene history, no key provisioning, no local reconciliation.

The API is public beta and its own documentation reserves the right to change
payloads and response shapes. Every contract here was transcribed from a pinned
artifact recorded in `OPENAPI_SNAPSHOT` and committed under `tests/fixtures/`;
`tests/openapi_snapshot.rs` asserts the crate against it.

Run `cargo test -p excalidraw-api --all-features`. No test needs a network or an
API key.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
