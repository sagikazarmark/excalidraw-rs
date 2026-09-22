# Pinned fixtures

| File | Source | Retrieved | SHA-256 |
| --- | --- | --- | --- |
| `openapi.json` | `https://api.excalidraw.com/docs/json` | 2026-09-21 | `3be56e9cc74d64bf24deb51f1d278b8c2eae506750d9d10c0d0f64c86a2d21af` |

`openapi.json` is the published artifact every contract in this crate was
transcribed from, stored byte-for-byte. `tests/openapi_snapshot.rs` asserts the
operation table and the two schema gaps against it.

A drift check should re-fetch the URL and **report** a difference. It must not
rewrite this file: the crate's types were written against these bytes, and a
silent update would hide a breaking change in a public-beta API.

The scene fixtures are hand-built to exercise documented and undocumented shapes
together. They are not captured responses from the live service.
