# plotters-excalidraw

[![crates.io](https://img.shields.io/crates/v/plotters-excalidraw?style=flat-square)](https://crates.io/crates/plotters-excalidraw)
[![docs.rs](https://img.shields.io/docsrs/plotters-excalidraw?style=flat-square)](https://docs.rs/plotters-excalidraw)

**A vector Plotters backend that exports editable native Excalidraw documents.**

## Install

Requires Rust **1.97+**.

```toml
[dependencies]
plotters-excalidraw = "0.2"
plotters = { version = "=0.3.7", default-features = false, features = ["line_series"] }
```

The library itself depends only on `plotters-backend`, not the full Plotters crate
or Chrono.

## Quick Start

<!-- `no_run`: the example writes into the current directory, so it is
     compiled but not executed. -->
```rust,no_run
use plotters::prelude::*;
use plotters_excalidraw::{ExcalidrawBackend, Overwrite, Scene};

let mut scene = Scene::new();
{
    let root = ExcalidrawBackend::new(&mut scene, (640, 400))?.into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .margin(30)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(0.0..10.0, 0.0..10.0)?;
    chart.configure_mesh().label_style(("Excalifont", 16)).draw()?;
    chart.draw_series(LineSeries::new([(1., 2.), (5., 8.), (9., 4.)], &BLUE))?;
    root.present()?;
}
scene.write("chart.excalidraw", Overwrite::Refuse)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Supported drawing

- Vector paths, filled polygons, rectangles, circles and text become native elements.
- Normal Excalifont only: printable ASCII plus `é`, `−`, `°`, and `±`.
- Pixel drawing, bitmaps and other fonts are unsupported; a drawing error prevents export.
- Native scene operations add notes, arrows, fractional-coordinate polygons, groups,
  borders, frames, links and provenance. Scenes support composition and library export.
- `text::measure` exposes the same fractional font metrics used by rendering.
- `ExcalidrawBackend::scene_mut` lets custom Plotters elements emit native scene
  operations in absolute scene coordinates.
- `present` performs no file I/O. Explicit writes protect existing files by default.
- `Scene::to_document()` returns an independent `excalidraw-document::Document`
  for general typed inspection/editing. Drawing keeps its existing supported subset.

## Shared document model

Generation uses `excalidraw-document`'s release-profile checked constructors,
typed fields, path bounds and text-content helpers. `Scene::to_document()` yields
complete records that can enter `Document::author(Profile::V0_18_1)` directly,
including multi-edit batches. Unassigned indices and absent links are explicit
nulls; live generated records carry `isDeleted: false`.

Whole-scene translation, identity/reference remapping, frame membership and library
validation use shared document operations. Framing selects `FrameOrder::Preserve`
to retain Plotters painter order and re-entered selection groups. Decorative
outer-border grouping remains a drawing-specific operation because it can span
sibling frames. Primitive emission appends checked records directly, avoiding a
whole-scene clone/validation for every mark. Font measurement, paint conversion,
bounds, drawing scopes and failed-drawing export protection remain backend-owned.

From the workspace, run `devenv shell dagger check excalidraw-rs:compatibility:charts` for native
composition, library, callout and link/provenance acceptance with freshly generated
fixtures, plus full chart acceptance. These checks also run as part of the root `dagger check` workflow.

Run `cargo run -p plotters-excalidraw --example plotters_chart` to generate
`output/plotters-chart.excalidraw` with ordinary Plotters code.

For ready-made chart helpers, use [excaliplot](https://crates.io/crates/excaliplot).
The current compatibility dependency is Excalidraw 0.18.1 (older acceptance
records used 0.18.0). Document `source` and
`customData.excaliplot` retain their existing identifiers for format compatibility.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

The bundled font has its own [SIL OFL license](assets/OFL.txt).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
