# excalidraw-rs

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/excalidraw-rs/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/excalidraw-rs/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/excalidraw-rs/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/excalidraw-rs)
[![crates.io](https://img.shields.io/crates/v/excaliplot?style=flat-square)](https://crates.io/crates/excaliplot)
[![docs.rs](https://img.shields.io/docsrs/excaliplot?style=flat-square)](https://docs.rs/excaliplot)

**Editable native [Excalidraw](https://excalidraw.com) charts and documents in Rust.**

Generate charts as native shapes and text, then open them in Excalidraw to move,
recolor, and annotate them. Generation runs entirely in Rust.

## Choose a crate

| Crate | Use it for |
| --- | --- |
| [`excaliplot`](crates/excaliplot/README.md) | Ready-made charts with validated data, measured layout, legends, and annotations. |
| [`plotters-excalidraw`](crates/plotters-excalidraw/README.md) | An Excalidraw backend for ordinary Plotters code, plus scene composition and export. |
| [`excalidraw-document`](crates/excalidraw-document/README.md) | Preserving, inspecting, editing, validating, and authoring scene/library JSON with typed fields and explicit editor profiles. |
| [`excalidraw-api`](crates/excalidraw-api/README.md) | Reading and writing scenes in an Excalidraw Plus workspace over the public REST API. |

Chart helpers include lines, scatter, bars, areas, pie/donut, supplied bands and
error bars, histograms, and steps/ECDFs. They support numeric and calendar axes,
sketch styling, grouping, borders, frames, and native library export.

The workspace is at **0.2.0** with a pre-1.0 API. The instructions below run
the examples from a checkout.

## Run the first chart

Requires Rust **1.97+** and Cargo. From a checkout:

```sh
cargo run --locked -p excaliplot --example line
```

Open `output/line.excalidraw` at **excalidraw.com**. Examples create `output/`
automatically and refuse to replace existing files. Pass `-- --overwrite` to
explicitly replace generated output; save manual edits to a separate path.

## Use your own data

Add it as a dependency:

```toml
[dependencies]
excaliplot = "0.2"
```

```rust
use excaliplot::{LineChart, Overwrite};

fn main() -> Result<(), excaliplot::Error> {
    let points = [(1.0, 2.0), (2.0, 5.0), (4.0, 3.0)];
    let scene = LineChart::new(&points, 0.0..5.0, 0.0..6.0)
        .labels("Measurements", "Time (s)", "Value")
        .render()?;
    scene.write("measurements.excalidraw", Overwrite::Refuse)?;
    Ok(())
}
```

## Explore the examples

```sh
# A 30-panel feature tour
cargo run --locked -p excaliplot --example feature_gallery
# Draw directly with Plotters
cargo run --locked -p plotters-excalidraw --example plotters_chart
```

Generate `output/operations-gallery.excalidraw` with
`cargo run --locked --example operations_gallery`: 18 synthetic operational
scenarios, each in three sketch styles.
The [usage guide](docs/usage.md) covers individual charts, styling, composition,
library items, and source links with runnable examples.

## Support boundary

- Chart/backend text uses normal Excalifont: printable ASCII plus `é`, `−`, `°`, and `±`.
- Invalid data, unsupported drawing, and layouts that cannot fit return errors.
  Pixel drawing and bitmap output are unsupported.
- Editing is one-way: native edits do not update Rust data or survive regeneration.
- Compatibility is editor-version-specific. Current browser checks use Excalidraw
  **0.18.1**, plus a pinned source snapshot for `excalidraw-document`; older chart
  acceptance records used **0.18.0**. See the [browser harness](compatibility/README.md).

## Documentation and development

- [Documentation index](docs/README.md) — usage, architecture, compatibility, and design records.
- API reference: [excaliplot](https://docs.rs/excaliplot),
  [plotters-excalidraw](https://docs.rs/plotters-excalidraw),
  [excalidraw-document](https://docs.rs/excalidraw-document).
  Build locally with `cargo doc --locked --workspace --no-deps --open`.
- [Dagger workflows](docs/dagger.md) — run the full checks with `devenv shell dagger check`.
- [Releasing](docs/releasing.md) — version policy, verification, and publication order.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

The bundled font is licensed under [SIL OFL 1.1](crates/plotters-excalidraw/assets/OFL.txt).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
