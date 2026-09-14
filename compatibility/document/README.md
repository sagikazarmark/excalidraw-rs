# Full document oracle

This suite uses the editor's public exports in real Chromium. It compares direct
Rust/JS local and database projections, preserves an independently constructed
corpus through Rust, loads every known persisted kind, decodes a real image,
mounts ordinary kinds, performs a native rectangle drag and saves/reopens files.
It also moves a bound-text container, edits its label through the native text
editor, checks both binding generations and elbow metadata after save/reopen,
and checks snapshot sticky-note pair metadata.
AI/embed types receive public-loader/data coverage, not remote plugin activation.

The normal suite also runs the Rust-only `authored_diagram` example for each
profile. It verifies authored relationship/path/order fields before mounting,
moves a labeled box and its bound arrow through native input, verifies the Rust
text replacement in the native text editor, edits it, and saves/reopens it.
Reports include `authored` checks and `authored.json` / `authored-edited.json`.
The only reverse-reference comparison normalization is null versus an empty array;
other asserted relationship fields must survive loading unchanged.

The normal suite additionally runs the Rust-only `authored_gallery` example. It
loads all 12/13 persisted kinds, validates image decode and native flip/crop,
freehand resizing, multiline editing, elbow target routing with a retained fixed
segment, frame/group movement, duplicate/library-copy identity isolation, actual
library blob loading, and snapshot sticky growth/shrink. Authored model fields,
including text geometry, are compared exactly before editing and after save/reopen
(apart from null/empty reverse-reference equivalence). No text-metric exemption is
used. Historical deleted labels are tested separately: public element restoration
retains historical edges without resurrecting live reverse bindings; release blob
loading/saving prunes tombstones while snapshot retains them.

Reports include `authoredGallery`; `authored-gallery-*.json` artifacts capture
Rust input, load, mount, edits and reopen results. AI/embed kinds have loader/data
coverage; remote activation is not part of these native-edit checks.

From the workspace:

```sh
nix-shell compatibility/shell.nix --run 'npm ci --prefix compatibility'
nix-shell compatibility/shell.nix --run 'node compatibility/document/run.mjs'
```

The released target resolves from the lockfile to **0.18.1**. For the snapshot,
clone Excalidraw separately and check out exactly
`afa3a653fc5d2b742adcbd5a6063187b056d2419`, fetch tag `v0.18.1`, then install its
dependencies with `npx --yes yarn@1.22.22 install --frozen-lockfile --ignore-scripts`.
Use the Node environment from `compatibility/shell.nix` for installation and runs:

```sh
nix-shell compatibility/shell.nix --run 'EXCALIDRAW_SOURCE=/tmp/opencode/excalidraw-source node compatibility/document/audit.mjs'
nix-shell compatibility/shell.nix --run 'EXCALIDRAW_SOURCE=/tmp/opencode/excalidraw-source node compatibility/document/run.mjs'
```

The snapshot adapter resolves common/element/math/utils/fractional-indexing/
laser-pointer/editor and React from that checkout. Vite serves the public source
entrypoint, styles and font imports. No registry version is substituted for the
snapshot. `audit.mjs` checks declared fields independently using TypeScript's AST,
including line/elbow refinement aliases. Rust public-interface tests exercise each
descriptor with nondefault values, presence, malformed/unknown values and edits.

Evidence goes to a new `compatibility/results/document-*` directory: browser,
Node, source/package identity, dependency-lock hash, full projections, corpus,
edited document and screenshot. `DOCUMENT_RESULTS` selects an output directory.
The projection fixture is synthetic; its deliberately nondecodable asset tests
serializer pruning only. The separate browser corpus contains a decodable PNG.

Baseline verified during implementation: Node 24.19.0, Chromium 152.0.7977.82,
0.18.1 and snapshot afa3a653. For direct Node runs, set `DOCUMENT_EXTENDED=1` for the older JS-authored
freehand/image/elbow/sticky interaction corpus and bidirectional Rust/editor
PNG/SVG transport checks, in addition to the normal Rust-authored gallery. These checks
exercise real pointer/keyboard/text-editor operations. They do not claim every
possible routing or editing permutation.

```sh
nix-shell compatibility/shell.nix --run 'DOCUMENT_EXTENDED=1 node compatibility/document/run.mjs'
nix-shell compatibility/shell.nix --run 'DOCUMENT_EXTENDED=1 EXCALIDRAW_SOURCE=/tmp/opencode/excalidraw-source node compatibility/document/run.mjs'
```

The [Dagger workflows](../../docs/dagger.md) use the official
`mcr.microsoft.com/playwright:v1.51.1-noble` image with Chromium **134.0.6998.35**,
not the Nix Chromium above. They build static Rust helpers separately and select
them with `DOCUMENT_EXAMPLES_DIR`; direct local runs fall back to Cargo. Historical
acceptance runs on this browser are recorded in [Dagger history](../../docs/dagger-history.md).

Dagger runs both profiles **with extended mode enabled**, including bidirectional
PNG/SVG transport and the JS-authored editing corpus. Harness assertions and process
exit status determine success. Diagnostic evidence exports also default to extended mode.
Numbered `extended-*.json` artifacts retain native input/save/reopen stages and
Rust/editor PNG/SVG payloads and extracted documents. Extended failures retain the
error, completed stages and best-effort scene/screenshot evidence.

```sh
devenv shell dagger check excaliplot:compatibility:document
```
