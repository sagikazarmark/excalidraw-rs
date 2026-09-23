//! Typed client for the public Excalidraw Plus REST API, built on
//! [`excalidraw_document`].
//!
//! The default build performs **no I/O**. [`Operation`] turns each documented
//! operation into a [`Request`] and decodes a status, headers and body back into
//! a typed result, so a caller with their own HTTP stack needs no transport from
//! this crate. Enable `client` for an async `Client`, or `blocking` for a
//! synchronous one. Neither type exists in the default build, so neither is
//! linked here.
//!
//! # Download, convert, upload
//!
//! ```no_run
//! use excalidraw_api::{ApiKey, SceneId, op, scene_content::{self, Embedding}};
//! # #[cfg(feature = "blocking")]
//! # fn main() -> Result<(), excalidraw_api::Error> {
//! let client = excalidraw_api::blocking::Client::new(ApiKey::from_env()?)?;
//! let scene = SceneId::new("abc123")?;
//!
//! let downloaded = client.send(op::GetSceneContent { scene: scene.clone() })?;
//! let document = downloaded.document().clone();
//!
//! let body = scene_content::into_replacement(downloaded, Embedding::RequireComplete)?;
//! let replaced = client.send(op::ReplaceSceneContent { scene, body })?;
//! println!("{}", replaced.scene_version());
//! # let _ = document;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "blocking"))]
//! # fn main() {}
//! ```
//!
//! Scene payloads are [`excalidraw_document::plus`] types, unchanged. Those
//! adapters enforce the closed content root, the two-field `appState` profile,
//! file-record constraints and JavaScript-safe numbers. This crate adds the
//! transport and the conversion policy in [`scene_content`], not a second scene
//! validator.
//!
//! # Confirmed server behaviour
//!
//! Observed against the live service on 2026-09-21, resolving questions the
//! published documentation leaves open:
//!
//! - **Element ids are rewritten on write** unless they are already in
//!   Excalidraw's 21-character form. References (`containerId`,
//!   `boundElements`) are rewritten consistently, so bound labels survive, and
//!   `customData` is preserved. Consume the returned content as canonical.
//!   A `PATCH` reusing a client-side id therefore *inserts a duplicate* rather
//!   than updating; [`scene_content::ElementIds`] makes that a decision instead
//!   of an accident.
//! - **`PATCH` rejects a full `GET` envelope** with `400 unrecognized_keys`, so
//!   the closed merge root is enforced rather than tolerated.
//! - **Elements must be complete records.** A partial `{id, x}` patch fails with
//!   `invalid_union / No matching discriminator`.
//! - **`boundElements` must be present**, though `null` is accepted; omitting it
//!   is a `400`.
//! - **`stickynote` is accepted** by the write schema and requires a numeric
//!   `baseHeight`, even though the public element reference omits the type.
//! - **Reference integrity is enforced on write.** A dangling `fileId` is a
//!   `400`, tombstones included, and a dangling `containerId` likewise. A
//!   partial download replayed through `PUT` is therefore refused rather than
//!   silently dropping images.
//! - **`sceneVersion` is not a content hash.** It is stable for identical
//!   content, but changing element geometry or an element's `version` leaves it
//!   unchanged; only `versionNonce` and `appState` moved it in testing. Two
//!   scenes with different geometry can share a value, so it cannot be used to
//!   detect that content changed.
//! - **`contentEpoch` counts authoritative replacements.** It advanced on every
//!   `PUT`, including an identical one, and did not move on `PATCH` or on a
//!   metadata-only update.
//! - **Merge order at equal `version` favours the lower `versionNonce`**, and a
//!   full tie keeps the submitted element. Reproduced three times.
//! - **`freedraw.strokeOptions` is accepted and then stripped.** The write
//!   succeeds, reports no error, and the field is absent from the response. It
//!   is the one observed case where a round trip loses data without saying so:
//!   compare the returned content rather than assuming the upload was stored
//!   verbatim.
//! - **Upstream-only element fields survive.** `created` on any element, and
//!   `baseFontSize` and `labelPosition` on text, were accepted and round-tripped
//!   unchanged, despite being absent from the published element schema.
//!   [`plus::ReplaceSceneContent::unconfirmed_element_paths`] still reports them,
//!   deliberately: one workspace on one date is not a server contract, and the
//!   conservative signal is the useful one when the alternative is silent loss.
//!
//! # What this crate will not do
//!
//! - It does not model or validate elements. The published artifact's element
//!   schema is literally `{}`; preserving records plus
//!   [`plus::ReplaceSceneContent::unconfirmed_element_paths`] is the honest
//!   answer.
//! - It implements no compare-and-swap. No content operation publishes an
//!   `ETag`, `If-Match`, `409` or `412`, and `PUT` recomputes `sceneVersion`
//!   from the submitted body. See
//!   [`scene_content::UnguardedCheckThenWrite`].
//! - It performs no local reconciliation, revision bumping, text measurement or
//!   arrow routing, and it is not an MCP client.
//! - It never retries automatically. A `429` is returned as
//!   [`Error::RateLimited`] carrying the reset timestamp.
//!
//! The API is public beta and its own documentation reserves the right to change
//! payloads and response shapes. Response models therefore retain unknown fields
//! in an `extra` map rather than failing, while request bodies stay exact.
#![forbid(unsafe_code)]

// The README's example is a contract too, and nothing compiled it.
// `cfg(doctest)` compiles its fences without prepending the README to the
// rendered crate documentation.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}

mod auth;
mod de;
mod error;
mod ids;
mod page;
mod request;

pub mod model;
pub mod op;
pub mod scene_content;

#[cfg(feature = "client-core")]
mod client;

#[cfg(feature = "blocking")]
pub use client::blocking;
#[cfg(feature = "retry")]
pub use client::retry::{Backoff, RetryPolicy};
#[cfg(feature = "client-core")]
pub use client::{Client, ClientConfig};

pub use auth::ApiKey;
pub use de::Timestamp;
pub use error::{Error, HeaderLookup, NoHeaders, RateLimit, SliceHeaders};
pub use ids::{CollectionId, InviteId, SceneId, SceneVersion, UserId};
pub use page::{LogOperation, LogQuery, Page, PageRequest};
pub use request::{JsonOperation, Method, Operation, Request};

/// Re-exported so callers need not depend on `excalidraw-document` directly to
/// name a request or response body.
pub use excalidraw_document::plus;

/// Default API root. Every [`Request`] path is relative to this.
pub const API_BASE_URL: &str = "https://api.excalidraw.com/api/v1";

/// Documented rate limit, per IP, per minute.
pub const RATE_LIMIT_PER_MINUTE: u64 = 600;

/// The pinned public artifact this crate was written against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub url: &'static str,
    pub retrieved: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

/// Identity of the OpenAPI document every contract here was transcribed from.
///
/// The API is public beta; a drift check should compare a fresh fetch against
/// this and report, not silently adopt, a difference.
pub const OPENAPI_SNAPSHOT: Snapshot = Snapshot {
    url: "https://api.excalidraw.com/docs/json",
    retrieved: "2026-09-21",
    sha256: "3be56e9cc74d64bf24deb51f1d278b8c2eae506750d9d10c0d0f64c86a2d21af",
    bytes: 139_904,
};
