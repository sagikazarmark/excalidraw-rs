# Document codec fuzz target

Install `cargo-fuzz` with stable Rust (`cargo install cargo-fuzz --version 0.13.1
--locked`), and install nightly `nightly-2026-09-10`. Its instrumented build uses
AddressSanitizer and coverage guidance by default. From the repository root:

```sh
python3 fuzz/seed-corpus.py /tmp/document-corpus
cargo +nightly-2026-09-10 fuzz run document /tmp/document-corpus -- -max_total_time=90 -max_len=16384 -rss_limit_mb=2048 -seed=12345 -print_final_stats=1
```

libFuzzer writes new cases to its corpus directory. The seed script keeps repository
fixtures untouched and adds malformed/legacy/reference/geometry cases. The target
checks preserving scene/library encoding, all scene validation purposes, both
profiles, export admission and successful projection round trips. It also exercises
ID remapping, profile migration, clipboard conversion, Plus parsing and PNG/SVG
extraction/insertion. The fuzz crate enables the optional embedded feature and the
seed script includes valid compressed PNG/SVG payloads. Fuzz campaigns are opt-in;
ordinary workspace checks do not install nightly or run an unbounded campaign.

The 2026-09-12 local campaigns executed 499,916 and 367,377 inputs without a
crash. See [historical verification](../docs/excalidraw-document-verification-history.md) for environment, limitations
and the associated performance baseline. Install cargo-fuzz using stable: its
locked older rustix dependency fails to compile under this particular nightly.
Additional feature-surface campaigns executed 1,627,263 and 721,383 inputs; see
`docs/excalidraw-document-followups.md` for the separate review and transport
regressions that complement those no-crash runs.
