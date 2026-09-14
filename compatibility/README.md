# Native browser checks

For the complete current fixture generation and browser checks, run
`dagger check excaliplot:compatibility:charts:full`; see [Dagger workflows](../docs/dagger.md).
The [release guide](../docs/releasing.md#verification) covers workspace/package
verification. [Phase 2](../docs/phase-2/README.md) retains historical typography evidence.

This directory is test-only; Rust export has no Node/browser runtime dependency.
The pinned editor runs locally using its own font assets. `npm ci` uses the lock
file; `shell.nix` supplies the exact browser and Node. The runner verifies browser
version and fails when any accepted geometry/type/text check fails.

Phase 3 also requires `cargo run --locked --example two_series -- <fresh-directory>`
and `SERIES_DIR=<that-directory>` (default `output/two-series`). `groups.mjs` exercises
native selection/recolor/group and browser clipboard composition. Full commands
and evidence are in [Phase 3](../docs/phase-3/README.md).

Phase 4 also requires the `bars`, `scatter`, and release-mode `probes` examples.
`marks.mjs` checks individual native mark editing; `probes.mjs` records load,
pan, selection, and save/reopen behavior alongside Rust generation diagnostics.
See [Phase 4 commands and budgets](../docs/phase-4/README.md).

Phase 5 also requires `area`, `pie`, `donut`, and `filled_gallery` examples.
`fills.mjs` exercises native filled-path edits, independent endpoint re-closing,
inner/outer donut arcs, legend selection, and official SVG fill after save/reopen.
See [Phase 5 commands and endpoint limitation](../docs/phase-5/README.md).

Do not use `docs/phase-1/line.raw.excalidraw` as a typography reference: it captures
the intentionally approximate first milestone. Reproduce current fixtures with
`cargo run --locked --example gallery -- <fresh-directory>`.

## Right-continuous steps and ECDFs

```sh
cargo run --locked --release --example steps -- target/steps-acceptance.excalidraw
STEPS_ONLY=1 STEPS_FILE=target/steps-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh destination. `steps.mjs` compares the entire imported scene, refreshes
and installs native text geometry, and saves/reopens it through the official API.
It measures all labels, selects/moves each chart independently, ungroups each and
edits both titles through the native textarea, then regroups and saves/reopens.
Each path is isolated for an unambiguous **Edit line** corner drag; the exact
12-unit X/Y corner change and all other geometry are checked, then saved/reopened
and exported to vector SVG. Native paths remain sharp (`roundness: null`).

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`compatibility/results/run-cLGzxm`. Raw/grouped scenes, two edited corner scenes,
screenshots, SVGs and report are local generated evidence. The fixture has four
generic levels and six ECDF samples with two ties: **two data paths / 18 data
vertices**, **58 total objects / 66 vertices**, **53,641 bytes**, zero pixel/bitmap
calls. Release generation including serialization was **0.729 ms**; maximum
native text-width drift was **0.000768 scene units** (tolerance one).

This is bounded acceptance for the clean two-panel example. Rust public tests also
cover exact cumulative halves/quarters/thirds, unsorted input, signed-zero ties,
singleton/all-equal samples, endpoint rules, generic order/duplicates, nonfinite
input, collapsed holds/jumps, style/layout errors and border/frame/library
composition. Manual corner edits do not preserve a step function automatically
or recalculate data. Ordinary line ordering remains nondecreasing, without sorting.

The final full native regression passed with fresh release fixtures in
`target/issue23-regression/native-results`. Rust all-target tests and doctests,
default/no-default/all-feature typechecks, formatting, Clippy, rustdoc and the
unpacked-package external consumer/all-example checks passed. Review against
`fd146b1` found no spec defects or documented-standard violations. One optional
tick-construction deduplication suggestion was deferred to retain the existing
Cartesian helper convention.

## Explicit-bin histograms

```sh
cargo run --locked --release --example histogram -- target/histogram-acceptance.excalidraw
HISTOGRAM_ONLY=1 HISTOGRAM_FILE=target/histogram-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh destination. `histogram.mjs` compares the complete imported/refreshed
scene and measured text, selects/moves the whole chart, ungroups it, and selects
and moves each of its five nonzero bins independently, including repeated counts.
It recolors and resizes one rectangle through actual native controls, edits the
title through the textarea, regroups the chart, then saves/reopens through the
official API. Complete expected geometry, order, styles, groups and text are
checked; official SVG retains the edited fill without image elements.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`compatibility/results/run-Wz5crl`. Raw/edited scenes, screenshot, SVG and report
are local generated evidence. The clean solid fixture has **six unequal signed-X
bins**, one zero count and two equal counts: **five data rectangles**, **31 total
objects / 22 path vertices**, **27,116 bytes**, and zero pixel/bitmap calls.
Release generation plus serialization measured **0.332 ms**; maximum measured
text-width drift was **0.000839 scene units** (tolerance one).
This is bounded functional acceptance for the six-bin demo. Rust tests additionally
cover fractional counts, all-zero inputs, an interior zero baseline, pixel collapse,
style/layout validation and border/frame/library composition. Manual rectangle
edits do not update counts, axes or adjacent bins.

The final full native regression passed with fresh fixtures in
`target/issue22-regression/native-results`, including installation and save/reopen
of refreshed text geometry. Rust all-target tests and doctests, feature-mode
typechecks, formatting, Clippy, rustdoc and unpacked-package consumer/all-example
checks passed. Review against `5b0aca0` found no spec defects; its native refresh
acceptance gap was fixed and confirmed on re-review. A non-blocking suggestion to
share repeated browser interaction helpers was deferred to retain suite-local
fixture conventions.

## Issue #9 follow-up acceptance

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, with fresh
release fixtures and four focused suites. Local evidence is under
`target/issue9-followup/{error-bars,bands,histogram,steps}-native`.

- Error bars and bands install refreshed elements and save/reopen them before edits.
- Bands edit a title through the native textarea, checking independently measured
  width and unchanged position/height; the edit survives save/reopen.
- Both interior band drags assert exactly the intended 12-unit X/Y displacement
  and unchanged other vertices. The closure drag returns to the original geometry.
- Histogram and both step-panel title edits assert independently measured width
  and unchanged position/height within the existing one-unit tolerance.
- All four suites passed, including the existing native groups, individual mark
  edits and save/reopen checks. Band and step SVG fill/path checks also passed.

Use the focused recipes in the respective sections with fresh fixture/results
paths to reproduce. Rust regressions cover opaque-center interval visibility,
nearby visible alternatives, and sorted/shuffled ECDF equality with duplicates.
Default/no-default/all-feature checks, all-target tests, doctests, formatting,
Clippy, rustdoc and the unpacked-package consumer/all-example checks passed.
Follow-up review found a patterned-center false rejection; the guard now applies
only to solid fills, with hachure/cross-hatch acceptance regressions.

## Caller-supplied bands

```sh
cargo run --locked --release --example bands -- target/bands-acceptance.excalidraw
BANDS_ONLY=1 BANDS_FILE=target/bands-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh output destination. `bands.mjs` checks the complete imported/refreshed
scene, actual whole-chart selection/movement and series selection through its
legend. After native ungrouping, it recolors the fill alone and verifies the
complete expected scene, including unchanged independent paths and legend.
The official native save/reopen route preserves those changes.

The fill is then isolated for unambiguous point hits, following `fills.mjs`.
Actual **Edit line** drags move an interior upper and lower vertex independently.
A third case moves the closure handle, verifies that first/last endpoints become
unequal, then manually re-closes it. Each case saves/reopens through the official
API and checks the native SVG still contains the intended fill. These are actual
editor interactions, not JSON mutation standing in for point editing.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`compatibility/results/run-szSp6z`. Raw/edited scenes, screenshot, three edited
point scenes, native SVGs and report are local generated evidence. The clean,
solid **six-sample** demo has **34 objects / 55 vertices**, including **four data
paths / 31 data vertices** (13-point closed fill plus three six-point paths).
Release generation plus serialization was **0.361 ms / 32,469 bytes**, with zero
pixel/bitmap calls. This is a bounded functional acceptance fixture, not a new
performance ceiling or an acceptance claim for sketch fills at larger sizes.

**Closure remains an editor limitation:** endpoints are independently editable,
and opening the path can remove fill until re-closed. Independent boundary and
center paths do **not** follow fill edits. The Rust public seam additionally
checks exact variable widths/mapping, optional paths, input/style/layout errors,
group/identity remapping, borders/frames/library composition and protected exports.

The final full native regression passed with fresh fixtures in
`target/issue21-regression/native-results`. Rust all-target tests and doctests,
default/no-default/all-feature typechecks, formatting, Clippy, rustdoc and the
unpacked-package external consumer/all-example checks passed. Standards review
against `db30aea` identified the missing band fixture in the complete reproduction
recipe and repeated path mapping; both were corrected and confirmed on re-review.
Spec review found no implementation defects or scope creep.

## Caller-supplied error bars

```sh
cargo run --locked --release --example error_bars -- target/error-bars-acceptance.excalidraw
ERROR_BARS_ONLY=1 ERROR_BARS_FILE=target/error-bars-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh destination. `error-bars.mjs` compares the complete native scene on
load and text refresh, selects/moves the whole chart, ungroups chart and series,
then selects and moves each of six compound observations independently. This
includes both equal-valued observations and the degenerate cap/center pair.
Two actual textarea edits change title and interpretation; official save/reopen
preserves all expected geometry, styles, identities, remaining groups and text.
The pinned editor uses five-unit Shift nudges; the test moves the top duplicate
40 units to clear its selection hit area before selecting the lower duplicate.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`target/issue20-regression/native-results` (final full regression after review).
Local raw/edited native scenes, screenshot and
report are generated evidence, reproduced by the recipe above. Native acceptance
covers this clean, solid six-observation fixture; dashed/dotted, hollow, wide-stroke,
edge validation and exact asymmetric mapping are additionally public Rust tests.

### Bounded compound performance

The fixture has **five four-object observations and one two-object degenerate
observation: 22 data objects / 32 data vertices**. Including axes, finite background,
16 text elements and the four-object legend gives **55 objects / 62 vertices**,
51,671 bytes and **zero pixel/bitmap calls**. The release-mode generator measured
generation plus serialization at **0.418 ms**. Native load was **54.0 ms**, chart
selection **30.0 ms**, six compound selection/movement workflows **465.1 ms** total,
and save/reopen **42.7 ms**. Single-run timings include automation overhead, on the
same Linux four-CPU AMD EPYC / 7.83 GiB environment and 1100×800 viewport used by
the existing harness. **Six observations is this compound workflow's tested
bound**, not a rejection limit or evidence for scatter's much larger ceilings.

Review against `3777e1f` found no documented-standard violations and one legend
occlusion defect at large marker sizes. The fix shares complete geometric extents
and gives the schematic eight units of exposed stem between each cap and marker;
a public regression covers maximum filled/hollow centers and stroke widths.
The complete native regression, Rust all-target tests/doctests, feature-mode
typechecks, formatting, Clippy, rustdoc, and unpacked-package consumer/all-example
checks passed. No dependency features changed.

## Native arrow callouts

```sh
cargo run --locked --example callouts -- target/callouts-acceptance.excalidraw
CALLOUTS_ONLY=1 CALLOUTS_FILE=target/callouts-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh destination. The combined demo includes measured markers, a dashed
model, a dotted threshold, an event window and two grouped note/arrow pairs.
`callouts.mjs` compares mapped endpoints, native kinds/heads, null bindings,
grouping and complete scene geometry. Real native selection moves the chart and
each pair; ungrouped labels undergo actual textarea edits and both endpoints of
each arrow undergo Ctrl-drags. Full comparisons preserve unrelated marks and
text. Official SVG export requires two open head legs or a filled triangle.
Save/reopen and two real clipboard pastes verify independent IDs/groups, local
points, head fields and null bindings. The library example now also includes
callouts in both bordered/framed panels and exercises its existing native route.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`compatibility/results/run-YxV3dd`: two native pair moves, two text edits, four
endpoint edits, both SVG head shapes and two clipboard copies passed. Generated
raw/edited/composed scenes, screenshot, SVGs and report are local evidence.
Typed/log mapping, input rejection and complete geometric bounds are covered at
the public Rust seams; this browser acceptance covers the clean numeric demo.

The editor can auto-bind the stationary start to nearby text/background when
entering arrow editing, even though the generated/imported arrow is unbound.
The test explicitly Ctrl-drags **both** endpoints, then verifies both bindings
and reciprocal `boundElements` remain empty after save/reopen. Unbound generation
does not disable future native editor binding behavior.

Library acceptance also passed in `compatibility/results/run-3oWCOm`. The final
full native regression passed with fresh fixtures in
`target/issue14-regression/native-results`, including the callouts and updated
library suites. Rust all-target tests/doctests, feature-mode typechecks, formatting,
Clippy, rustdoc and unpacked-package consumer/all-example checks passed.
Independent Standards/Spec review against `2345aa7` found no documented-standard
breaches and one extreme-coordinate bounds defect, fixed with a public regression
and confirmed on re-review. The optional clipboard-workflow duplication suggestion
is deferred; existing suites retain their own expected-scene comparisons.

## Chart annotations

```sh
cargo run --locked --example annotations -- target/annotations-acceptance.excalidraw
ANNOTATIONS_ONLY=1 ANNOTATIONS_FILE=target/annotations-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use a fresh fixture destination. `annotations.mjs` loads the combined measured/
model/threshold/event-window demo through the official API and compares the full
scene. It verifies two borderless windows behind axes/data, two foreground native
dashed/dotted rules, opacity, and four independent inner annotation groups.
Actual native clicks select and move the entire chart, then ungroup its outer
group. Each rule is selected, moved alone and recolored with the stroke control;
each window is selected, moved alone and recolored with the background control.
Every edit is compared with the complete expected scene, including untouched
data, axes, legends and the other annotations. The edited scene is saved/reopened,
then copied through the real clipboard into a second editor twice. Independent
element/group IDs, translated geometry, painter order, styles and save/reopen
are checked for both copies.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82**, in
`compatibility/results/run-dygnu1`: four isolated annotation moves, two stroke
recolors, two fill recolors and two clipboard copies passed. Raw/edited/composed
scenes, screenshots and report are local generated evidence. This native check
covers the clean numeric combined demo; typed/log mapping and input validation
are additionally covered through public Rust tests, without claiming independent
native acceptance for every axis/style combination.

Final full native regression passed with fresh fixtures in
`target/issue13-regression/native-results`, including the new annotation suite.
All-target Rust tests, doctests, feature-mode typechecks, formatting, Clippy,
rustdoc, and unpacked-package consumer/all-example checks passed. Independent
Standards/Spec reviews against `09cc016` found no documented-standard breaches
or spec defects; one optional duplication observation was retained deliberately
to keep each renderer's layer insertion points explicit.

## Native series dashes

Generate a fresh protected comparison/probe directory, then run the focused suite:

```sh
cargo run --locked --release --example series_dashes -- target/dash-acceptance
SERIES_DASHES_ONLY=1 SERIES_DASHES_DIR=target/dash-acceptance \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`series-dashes.mjs` accepts three same-color original paths with solid/dashed/dotted
styles in clean, roughness-1 and roughness-2 modes. Actual native selection,
ungrouping, series recoloring and one interior vertex drag per path are compared
against the complete expected scene, then saved/reopened through official APIs.
SVG exports inspect each original path and its legend independently: solid has
no dash array; dashed and dotted have distinct, matching path/swatch arrays.
For the demo's stored width 3, the pinned editor exports `8 11` and `1.5 9`;
these are observations, not configurable or cross-version spacing guarantees.
Rough strokes and dash gaps can miss an exact mathematical vertex, so native
selection tries several original vertices and still requires the exact path ID.

Verified **2026-09-11**, **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-jBBxeV`: nine native vertex edits, independent group
recolors, SVG dash inspection and save/reopen passed. Local raw/edited scenes,
SVGs, screenshots and report are generated evidence; the recipe reproduces it.

### Bounded dashed-path performance

The same release-mode generator emits **12 probes**: dashed/dotted × 100/1,000
vertices × roughness 0/1/2. Every scene has 29 elements (one data path, twelve axis/
tick lines, one finite background, fifteen text elements), original data vertices
plus 24 axis vertices, and zero pixel/bitmap calls. At 100 vertices it is 31,843
bytes; at 1,000 it is 80,730 bytes. All probes pass native load, pan, chart/path
selection and complete save/reopen comparison.

Representative **roughness-2** timings from the run above, milliseconds:

| Style / vertices | Rust generation + serialization | Native load | Pan | Chart selection | Path selection workflow | Save/reopen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Dashed / 100 | 0.135 | 48.0 | 142.2 | 27.4 | 53.5 | 41.9 |
| Dotted / 100 | 0.134 | 43.2 | 143.9 | 23.5 | 71.9 | 41.7 |
| Dashed / 1,000 | 0.242 | 56.6 | 138.6 | 24.9 | 96.6 | 50.5 |
| Dotted / 1,000 | 0.226 | 55.8 | 139.6 | 18.0 | 103.1 | 65.0 |

Machine: AMD EPYC, four logical CPUs, 7.83 GiB RAM, Linux 6.1.167, headless
1100×800 viewport. Single-run workflow timings include browser automation and
two-frame settling where applicable; they are not microbenchmarks. **1,000 data
vertices is this dash probe's tested bound**, not a library rejection limit or
evidence for the solid-path 5,000 ceiling. Dense paths still need visual review;
the comparison's vertex editing evidence is for four-point paths, while dense
probes establish load/pan/selection/save behavior.

After review, the complete native regression passed with fresh fixtures in
`target/issue12-regression/native-results`, including both styling suites using
shared native selection/recolor interactions. All-target Rust tests and doctests,
feature-mode typechecks, formatting, Clippy, rustdoc, and unpacked-package consumer/
all-example checks passed. Review clarified that dense probes establish workflow
responsiveness/persistence while four-point comparisons establish vertex editing.

## Per-series styles and line markers

Generate the protected same-color comparison before the full harness.
`SERIES_STYLES_FILE` defaults to `series_styles.excalidraw` in the repository root.
Run this slice alone with a fresh output path:

```sh
cargo run --locked --example series_styles -- target/series-styles-acceptance.excalidraw
SERIES_STYLES_ONLY=1 SERIES_STYLES_FILE=target/series-styles-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`series-styles.mjs` checks native import/refresh, every text width, exact marker
centers and painter order, matching width/opacity/fill/radius in legend swatches,
and distinct series groups. Actual editor clicks select the whole chart and one
series after ungrouping; the native stroke control recolors the hollow series.
After ungrouping that series, one circle is selected by its rim and moved alone.
Complete expected geometry, paint, groups and text are compared before and after
official save/reopen, including the untouched line vertices and other series.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-r8V1L8`: two same-color series, eight data circles,
native group selection/recolor, one isolated marker move and save/reopen passed.
Maximum text-width drift was **0.000757** scene units within the unchanged
**1-unit** tolerance. Raw/edited scenes, screenshot and report are local generated
evidence; the recipe reproduces native local-editor acceptance.

The full native regression passed after review fixes with fresh fixtures in
`target/issue11-regression/native-results`, including typography, numeric/calendar
axes, grouping/composition, individual marks, 13 performance fixtures, fills,
library items, links, notes and units. Rust all-target tests and doctests,
feature-mode typechecks, formatting, Clippy, rustdoc and unpacked-package
consumer/all-example checks passed. Review reserved space for wide legend line
caps and preserved legacy chart-wide hollow scatter clearance while applying
complete stroke extents to line markers and per-series marker/width overrides.

## Shared Cartesian review follow-up (#10)

Numeric and typed calendar charts now share measured margins, caption allocation,
final tick-string measurement/fit, native axes, and line/scatter validation and
drawing through `src/cartesian.rs`. Log coordinates remove repeated decade-boundary
key points before either measurement or mesh generation. Public regressions cover
dense single-decade X/Y axes, unique native tick labels/strokes, and common numeric/
calendar Y geometry and fit failures.

On 2026-09-11, all-target tests, doctests, no-default/all-feature typechecks,
Clippy, rustdoc and unpacked-package consumer/all-example verification passed.
The full native regression passed against **Excalidraw 0.18.0 / Chromium
152.0.7977.82** using freshly generated fixtures under `target/issue10-review`.
Evidence is in `target/issue10-review/native-results-2/report.json`, alongside
edited/reopened scenes and screenshots. All five #10 suites passed; maximum
calendar text-width drift was **0.001146**, within the unchanged **1-unit** tolerance.
Reproduce with the Phase 6 recipe using a fresh directory and the matching paths.

The first full attempt failed a cross-editor grouping comparison (`0` versus `1`);
the isolated grouping check and full rerun passed without changing chart output or
weakening assertions. The cause remains unconfirmed. Grouping comparison failures
now identify the copy, element and field to support diagnosis if it recurs.

## Horizontal bars

Generate the protected three-category/two-series fixture before the full harness.
`HORIZONTAL_BARS_FILE` defaults to `horizontal_bars.excalidraw` in the repository
root. Run this slice alone with a fresh output path:

```sh
cargo run --locked --example horizontal_bars -- target/horizontal-bars-acceptance.excalidraw
HORIZONTAL_BARS_ONLY=1 HORIZONTAL_BARS_FILE=target/horizontal-bars-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`horizontal-bars.mjs` checks native import, text measurement and forced repair of
wrong dimensions, a real long-category textarea edit/restore, whole-chart movement,
native ungrouping, independent series movement/recoloring, individual bar movement,
series/chart regrouping and official save/reopen. Two clipboard insertions into a
second editor verify complete geometry/paint/text plus fresh element and series/chart
identities, followed by another save/reopen. Native regrouping's contiguous painter
order is explicitly predicted and checked.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-0F0vYa`: three long categories, two series, five nonzero
bars, two composed copies, maximum text-width drift **0.000827** scene units within
the unchanged **1-unit** tolerance. Raw/edited/composed scenes, screenshots and report
are local generated evidence; this recipe reproduces native local-editor acceptance.

The complete native regression also passed with freshly generated fixtures in
`compatibility/results/run-ppjmpI`, including existing typography, grouped charts,
marks, performance probes, fills, composition, library, links, notes, units and
numeric/calendar axes. Rust all-target tests and doctests, feature-mode typechecks,
formatting, Clippy, rustdoc and unpacked-package consumer/all-example checks passed.

## Typed calendar and UTC axes

Generate the protected irregular-observation example before the full harness.
`DATE_AXES_FILE` defaults to `date_axes.excalidraw` in the repository root. Run
this slice alone with a fresh output path:

```sh
cargo run --locked --example date_axes -- target/date-axes-acceptance.excalidraw
DATE_AXES_ONLY=1 DATE_AXES_FILE=target/date-axes-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`date-axes.mjs` proves unequal elapsed-day spacing across leap day and a month
boundary, UTC year-boundary labels, and repeated-timestamp ellipse positions.
It verifies native import, every text width, forced repair of wrong date/UTC text
dimensions, two real tick textarea change-and-restores, native chart ungrouping,
a persistent title edit and an interior line-vertex drag, followed by official
save/reopen and complete expected geometry/text/paint comparison of both panels.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-5iBmnj`: two panels, three text edits, one vertex edit,
maximum text-width drift **0.001146** scene units within the unchanged **1-unit**
tolerance. Raw/edited scenes, screenshot and report are local generated evidence;
the recipe reproduces native local-editor acceptance.

The complete native regression passed with freshly generated fixtures in
`compatibility/results/run-PpU4TI`, including all existing typography, grouping,
marks, performance, fills, composition, library, links, notes, units, automatic
ranges, measured layout and log-axis checks. Rust all-target tests and doctests,
feature-mode typechecks, Clippy, rustdoc, and
unpacked-package consumer/all-example verification also passed. The resolved
feature tree adds calendar support and transitive Chrono defaults, with no
system-font or raster dependencies.

## Positive logarithmic axes

Generate the protected four-panel fixture before the full harness. `LOG_AXES_FILE`
defaults to `log_axes.excalidraw` in the repo root. Run this slice alone with a
fresh output path:

```sh
cargo run --locked --example log_axes -- target/log-axes-acceptance.excalidraw
LOG_AXES_ONLY=1 LOG_AXES_FILE=target/log-axes-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`log-axes.mjs` checks the three line-axis combinations and tiny-log scatter through
native import, measured text refresh, repair of deliberately incorrect dimensions,
five actual textarea edits (including a scientific tick), three interior vertex
drags, and official save/reopen. It verifies series/legend grouping, native
ellipses, and complete expected geometry/text/paint of edited and neighboring panels.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-rTbVAG`: four panels, five text edits, three vertex
edits, maximum width drift **0.000784** scene units within the unchanged **1-unit**
tolerance. Raw scenes, edited saves, screenshot and report are local generated
evidence; the recipe reproduces this native local-editor acceptance.

The complete native regression passed in `compatibility/results/run-BrTlZ4` with
freshly generated fixtures, including all existing typography, groups, marks,
performance, fills, composition, library, links, notes, units, automatic-range and
layout checks. Rust all-target tests, doctests, feature-mode typechecks, Clippy,
rustdoc and unpacked-package consumer/examples passed. The dependency feature tree
is unchanged: no system-font discovery, bitmap or raster backend was added.

## Measured Cartesian layout

Generate the protected explicit-range comparison before the full harness.
`LAYOUT_FILE` defaults to `layout.excalidraw` in the repo root. Run this slice alone
using a fresh output path:

```sh
cargo run --locked --example layout -- target/layout-acceptance.excalidraw
LAYOUT_ONLY=1 LAYOUT_FILE=target/layout-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

The example compares decimal and compact scientific ticks with right/off legends,
fraction-to-percent Y labels, and identical explicit bounds. It reports a deliberate
impossible-fit error. `layout.mjs` checks native import, all text widths, forced
dimension refresh (including repair of an intentionally incorrect width/height),
four actual textarea change-and-restore operations on numeric/legend labels and
three persistent title edits after native ungrouping. Complete geometry, paints,
text and remaining series identities are compared after every panel save/reopen.
Refresh retains the required `repairBindings: true` prerequisite for 0.18.0.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-Jvxgei`: three panels, seven actual text edits, maximum
width drift **0.000734** scene units within the unchanged **1-unit** tolerance.
Local raw/edited scenes, screenshot and report are generated evidence; rerun the
recipe to reproduce. This records local native-editor acceptance.

The complete native regression passed in `compatibility/results/run-OCTPfS`,
including refreshed/repaired-scene save/reopen, the seven layout text edits,
existing 46 typography edits, 13 performance fixtures, groups, marks, nine filled
fixtures, composition, library, links, notes and units. The note/units/layout
textarea checks wait for native scroll state before clicking to avoid creating
new text during asynchronous repositioning. Rust all-target tests, doctests,
feature-mode typechecks, Clippy, rustdoc and unpacked-package consumer/examples
also passed. Review fixes preserve typed category glyph errors and reserve
caption/top-Y-tick clearance without introducing a second X axis.

## Automatic linear ranges

Generate the protected three-panel comparison before running the full harness.
`AUTO_RANGES_FILE` defaults to `auto_ranges.excalidraw` in the repo root. Run this
slice alone using a fresh output path:

```sh
cargo run --locked --example auto_ranges -- target/auto-ranges-acceptance.excalidraw
AUTO_RANGES_ONLY=1 AUTO_RANGES_FILE=target/auto-ranges-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`auto-ranges.mjs` checks irregular numeric X, constant Y and an explicit-range
counterpart through native import, measured text refresh (including intentionally
wrong dimensions), ungrouping, three actual textarea edits, three interior vertex
drags and save/reopen after each panel edit. It compares unchanged neighboring
panels and vertices as well as expected edited positions and text.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82**.
Focused evidence is in `compatibility/results/run-gIUJj3` (local generated report,
raw scene, three edited saves and screenshot; rerun to reproduce).
The complete native regression also passed in `compatibility/results/run-P5IRHy`,
including existing typography, grouping, marks, fills, composition, library,
links, notes and units checks alongside the three new comparison panels.

## Scene composition and decoration

Generate `composition` and `decorations` examples before running the full harness.
`COMPOSITION_FILE` and `DECORATIONS_DIR` default to `composition.excalidraw` and
`decorations` in the repo root. To run this slice alone:

```sh
cargo run --locked --example composition -- target/composition.excalidraw
cargo run --locked --example decorations -- target/decorations
COMPOSITION_ONLY=1 COMPOSITION_FILE=target/composition.excalidraw \
DECORATIONS_DIR=target/decorations \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use fresh output paths. Nine decoration fixtures cover all border patterns and
clean/rough chart appearances, border/series movement, native frame movement and
renaming, child editing, sibling membership, duplicate clipboard insertion,
deletion/undo, save/reopen and official clipped per-frame SVG export. The two-chart
fixture checks independent movement. Screenshots, SVGs and saved native scenes
are retained in the printed results directory.

Text refresh explicitly enables `repairBindings`, a prerequisite for dimension
refresh in Excalidraw 0.18.0; the check repairs an intentionally incorrect width
before accepting the measurement result.

Verified on 2026-09-11 against Excalidraw 0.18.0 and Chromium 152.0.7977.82:
the complete native regression passed, including 46 text edits, 13 performance
fixtures, nine filled fixtures, and the nine decoration fixtures. Additional
checks covered edited-border persistence, duplicate insertion of borders and
both frame combinations, outer-border/inner-series movement without reparenting,
and an exported 8-unit border with 16-unit clearance on all four frame edges.
The local report and generated evidence are in
`target/composition-acceptance/native-results`; regenerate with the recipes above.

## Native library items

Generate the protected `library` example before the full harness. `LIBRARY_DIR`
defaults to `library` in the repo root. Run this slice alone with fresh paths:

```sh
cargo run --locked --example library -- target/library-acceptance
LIBRARY_ONLY=1 LIBRARY_DIR=target/library-acceptance \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`library.mjs` passes each generated file as a Blob to the official `updateLibrary`
API (which uses `loadLibraryFromBlob`), then clicks actual library-panel tiles.
It verifies that library import leaves the canvas empty, inserts the diagram
twice and a second item once, and compares every supported source field against
inserted content. Only editor-managed revision/seed fields and remapped identities
and insertion offsets are exempted; geometry, styles and relationships are checked.

The diagram includes two bordered sibling frames plus an outer border; the second
item includes scatter ellipses. Checks prove independent element/group identities,
five local frame memberships, native frame-name editing isolated to one instance,
and persistence after official scene save/reopen. On a green receiving canvas,
five white backgrounds remain finite 500×340 rectangles. Official transparent
SVG/PNG exports retain them: SVG dimensions and sampled PNG/canvas pixels verify
white interiors, transparent export margins and exposed green canvas.

Verified on 2026-09-11 against **Excalidraw 0.18.0 / Chromium 152.0.7977.82**.
The focused run's report, saved scene, canvas screenshot and official exports are
in `compatibility/results/run-IlSEQh` (local generated evidence; rerun to reproduce).
The complete regression also passed in `compatibility/results/run-hU80Sk`, including
46 text edits, 13 performance fixtures, nine filled fixtures, grouping, marks,
composition and the library route. Rust all-target tests, doctests, feature-mode
typechecks, formatting, Clippy, rustdoc and the unpacked-package consumer/examples
passed in the same implementation session.
This is actual local native-library acceptance; hosted-editor manual acceptance
has not been observed. No clipboard route is used by these library checks.

The protected example now includes arrows and replays this same
import/insertion/edit/save route. The comparison intentionally
examines all source fields, so supported new content cannot be silently omitted.
The Rust serializer must also extend reference validation if new reference-bearing
kinds are introduced. Those later slices are not prerequisites for library export.

## Source links and provenance

The protected `linked_chart` example adds one explicit source label to a synthetic
chart. Its `example.org` report URL is a placeholder. Generate it before the full
harness; `LINKED_FILE` defaults to `linked_chart.excalidraw` in the repo root.
The library fixture now has one linked label per latency panel; regenerate it too.

```sh
cargo run --locked --example linked_chart -- target/linked-acceptance.excalidraw
cargo run --locked --example library -- target/linked-library-acceptance
LINKS_ONLY=1 LINKED_FILE=target/linked-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
LIBRARY_ONLY=1 LIBRARY_DIR=target/linked-library-acceptance \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

Use fresh paths. `links.mjs` selects the actual label, checks the native hyperlink
popup, clicks its anchor and asserts the resulting tab's exact destination. Only
the placeholder HTTP response is intercepted; the link interaction is native.
It edits the linked label through the real textarea, saves/reopens with official
APIs, then copies the chart and pastes it twice through the browser clipboard.
Links and complete custom data survive; element/group identities are independent
while source keys repeat. The duplicate scene is saved/reopened as well.

The library regression compares links and all custom data through official import,
two diagram insertions, native frame editing and save/reopen. Four source labels
survive in the two inserted diagrams. Existing geometry, frames, backgrounds and
official export checks also run.

Verified on 2026-09-11 with **Excalidraw 0.18.0 / Chromium 152.0.7977.82**:
focused links acceptance passed in `compatibility/results/run-uWHe52`, and library
preservation passed in `compatibility/results/run-SacLoE`. These are local generated
reports/scenes/screenshots; rerun to reproduce. Hosted-editor manual acceptance
has not been observed for this feature.

The complete native regression also passed in `compatibility/results/run-TrRtTq`,
including 46 text edits, 13 performance fixtures, grouping, marks, nine filled
fixtures, composition, linked library items and linked-label navigation/edit/copy.
Rust feature-mode typechecks, all-target tests, doctests, formatting, Clippy,
rustdoc and unpacked-package consumer/example verification passed in this session.

## Multiline notes

Generate the protected method-note demo and typography corpus before the full
harness. `NOTE_FILE` and `NOTE_GALLERY_FILE` default to `method_note.excalidraw`
and `note_gallery.excalidraw` in the repository root. Use fresh output paths:

```sh
cargo run --locked --example method_note -- target/method-note-acceptance.excalidraw
cargo run --locked --example note_gallery -- target/note-gallery-acceptance.excalidraw
cargo run --locked --example library -- target/note-library-acceptance
NOTES_ONLY=1 NOTE_FILE=target/method-note-acceptance.excalidraw \
NOTE_GALLERY_FILE=target/note-gallery-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
LIBRARY_ONLY=1 LIBRARY_DIR=target/note-library-acceptance \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`notes.mjs` loads eight Rust-generated corpus elements through the official API:
unequal line lengths, leading/middle/trailing blanks, normalized CRLF, whitespace,
and a short line whose width is smaller than the blank-line space. It checks
browser canvas measurements and native forced refresh with deliberately incorrect
width **and** height. `repairBindings: true` is required alongside
`refreshDimensions: true` in 0.18.0; omitting it makes refresh a no-op. This existing
prerequisite is preserved and the multiline checks prove dimensions really change.

Seven visible corpus notes enter the real textarea, are changed and restored,
then saved/reopened. The whitespace-only note survives import/refresh/save; native
editing may delete whitespace-only text. An empty string is rejected by Rust
because the pinned editor marks it deleted on import. Leading/trailing blanks
around visible text retain their text and line boxes through editing.

The composed method-note demo is moved via its actual native frame selection,
exported through the official clipped frame SVG route, and edited as one two-line
textarea before save/reopen. Frame membership remains intact. The library example
now includes a method note per panel; library import, repeated insertion, isolated
note editing and save/reopen run alongside the existing frame, link, background
and official export checks.

Verified on 2026-09-11 with **Excalidraw 0.18.0 / Chromium 152.0.7977.82**:
focused notes passed in `compatibility/results/run-YzKZKI` (eight real textarea
edits including the demo; maximum refresh width drift below 0.000079 scene units),
and library-note acceptance passed in `compatibility/results/run-yF9ao7`.
These local generated reports, saved scenes and images are reproducible with the
commands above. The existing one-scene-unit typography tolerance is unchanged.

The complete native regression passed in `compatibility/results/run-Ld7FKm`,
including the existing 46 text edits, 13 performance fixtures, grouping, marks,
nine filled fixtures, composition, links, library notes and the new note corpus.
Rust all-target tests, doctests, feature-mode typechecks, formatting, Clippy,
rustdoc and unpacked-package consumer/example verification also passed.

## Scientific unit labels

The agreed partial delivery of #27 supports degree U+00B0 and plus/minus U+00B1.
Micro sign U+00B5 remains an explicit error because it is absent from all seven
pinned editor shards; Greek mu U+03BC is distinct and also unsupported. See the
[reproducible font generation and blocker](../crates/plotters-excalidraw/assets/README.md#unit-glyph-coverage-27).
The harness uses the unmodified pinned Excalifont and editor assets.

Generate the protected chart-and-note example and corpus before the full harness.
`UNITS_FILE` defaults to `units.excalidraw`; the corpus lives in `GALLERY_DIR`.
Run this slice alone with fresh paths:

```sh
cargo run --locked --example gallery -- target/units-gallery
cargo run --locked --example units -- target/units-acceptance.excalidraw
UNITS_ONLY=1 GALLERY_DIR=target/units-gallery \
UNITS_FILE=target/units-acceptance.excalidraw \
nix-shell compatibility/shell.nix --run 'node compatibility/verify.mjs'
```

`units.mjs` checks eight corpus strings (isolated glyphs, adjacent combinations,
signs, decimals, punctuation, existing accented text and leading/trailing spaces),
the chart title, rotated Y description and two-line units note. Each undergoes
canvas measurement, native refresh, a real textarea change-and-restore, and
official save/reopen. A forced refresh repairs deliberately wrong width/height on
an unrotated copy; intact rotated geometry is refreshed and edited separately
because native resizing compensates the origin of a rotated box. The full chart
is also imported, its note changed to a new units string, and saved/reopened.

Verified on 2026-09-11 with **Excalidraw 0.18.0 / Chromium 152.0.7977.82** in
`compatibility/results/run-0ZGLvB`: 12 actual text edits, maximum width drift
**0.000944**, maximum position drift **0.000472** scene units, within the unchanged
1-unit tolerance. Local saved scenes, screenshot and report are reproducible using
the commands above. This is native local acceptance for the two supported glyphs;
it does not claim hosted-editor or micro-sign acceptance.

The complete native regression passed in `compatibility/results/run-DeJBiD`,
including the existing 46 corpus/anchor edits, grouping, 13 performance fixtures,
nine filled fixtures, composition, library, links, notes and the new units checks.
Rust all-target tests, doctests, feature-mode typechecks, formatting, Clippy,
rustdoc and the unpacked-package consumer/examples passed in the same session.
Review tightened units round-trip checks to compare complete chart geometry,
identity, paint and text, plus native text layout fields. The focused rerun passed
in `compatibility/results/run-8re4py` with the same drift bounds.
