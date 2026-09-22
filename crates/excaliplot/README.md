# Excaliplot

[![crates.io](https://img.shields.io/crates/v/excaliplot?style=flat-square)](https://crates.io/crates/excaliplot)
[![docs.rs](https://img.shields.io/docsrs/excaliplot?style=flat-square)](https://docs.rs/excaliplot)

**High-level editable Excalidraw charts built on Plotters and `plotters-excalidraw`.**

Includes line, scatter, bar, area, pie/donut, band, error-bar, histogram and step/ECDF
charts; numeric and calendar axes; measured layout; native legends and annotations.
Scene, styling and export types are re-exported from `plotters-excalidraw`, so
high-level charts and direct Plotters drawings can share the same scene.

## Install

Requires Rust **1.97+**. To use a local checkout, adjust the path:

```toml
[dependencies]
excaliplot = { path = "../excaliplot.orig/crates/excaliplot", version = "0.1.0" }
```

See the [release guide](https://github.com/sagikazarmark/excaliplot.orig/blob/main/docs/releasing.md)
for distribution status and publication steps.

## Quick Start

<!-- `no_run`: the example writes into the current directory, so it is
     compiled but not executed. -->
```rust,no_run
use excaliplot::{LineChart, Overwrite};

let scene = LineChart::new(&[(1., 2.), (5., 8.), (9., 4.)], 0.0..10.0, 0.0..10.0)
    .labels("Measurements", "Time", "Value")
    .render()?;
scene.write("chart.excalidraw", Overwrite::Refuse)?;
# Ok::<(), excaliplot::Error>(())
```

## Examples

Run `cargo run -p excaliplot --example feature_gallery` from the workspace to create
`output/feature-gallery.excalidraw`. Examples create `output/` automatically and
protect existing exports. An explicit destination is used as given.

For ordinary Plotters code without the chart helpers, depend on
[`plotters-excalidraw`](https://crates.io/crates/plotters-excalidraw) instead.
See the [usage guide](https://github.com/sagikazarmark/excaliplot.orig/blob/main/docs/usage.md)
for the complete chart catalog, supported inputs, and editor compatibility.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
