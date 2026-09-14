# Excalifont metrics asset

`Excalifont-Regular.ttf` is a lossless WOFF2 decompression of the Latin shard
`Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2` distributed in
Excalidraw `v0.18.1`, release commit
`a2ec2889babf7d2295469c6d90ebe77fae57df84`. The Git resolution is pinned in
`dagger.lock`. This is the same Latin metrics asset originally obtained from
`@excalidraw/excalidraw@0.18.0`. The font is Version 1.000, 1000 units/em.
Decompressed TTF SHA-256:
`41dcb99eb97e75def56795d26e34fba0172a0683026b9f8bcf6421951a782d53`.

Copyright (c) 2024 by Excalidraw. All rights reserved.
Excalifont is a trademark of Excalidraw.
Design: Your Own Font Foundry (Virgil); Ján Filípek / DizajnDesign
(Excalifont modifications). https://dizajndesign.sk

The font is under SIL OFL 1.1, **not** this crate's MIT/Apache license.
See `OFL.txt`, generated from the copyright and license in the upstream font
descriptor (`packages/excalidraw/fonts/Excalifont/index.ts`), with trailing
whitespace removed. No glyphs, names, outlines, or layout tables were edited.
Rust uses the font for measurement; the scene does not embed fonts.

## Unit-glyph coverage (#27)

The existing Latin asset maps U+00B0 to `degree` and U+00B1 to `plusminus`.
No font asset or license change is needed for these two supported glyphs.
U+00B5 **micro sign is absent from all seven official 0.18.0 shards**. The Greek
shard `Excalifont-Regular-41b173a47b57366892116a575a43e2b6.woff2` maps U+03BC
to `uni03BC`; that is Greek mu, a distinct codepoint, not micro-sign coverage.
The [official shard descriptors](https://github.com/excalidraw/excalidraw/blob/817d8c553c3389650f8b4503984a6d4a5d2f0c11/packages/excalidraw/fonts/Excalifont/index.ts)
also exclude U+00B5. On 2026-09-11, upstream master still referenced these shards.

The user approved shipping degree and plus/minus while retaining explicit errors
for micro sign and Greek mu. Completing #27 requires matching upstream assets
with U+00B5 coverage in both Rust measurement and the receiving native editor,
then new measurement/edit/save/reopen acceptance. Patching only Rust metrics or
injecting a special font into the harness would not establish stock-editor support.

The OFL notice and this provenance file are required package assets.
