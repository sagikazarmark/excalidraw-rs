import { chromium } from "playwright-core";
import { spawn, execFileSync } from "node:child_process";
import { readFile, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import assert from "node:assert/strict";
import path from "node:path";
import { verifyGroups } from "./groups.mjs";
import { verifyMarks } from "./marks.mjs";
import { verifyProbes } from "./probes.mjs";
import { verifyFills } from "./fills.mjs";
import { verifyComposition } from "./composition.mjs";
import { verifyLibrary } from "./library.mjs";
import { verifyLinks } from "./links.mjs";
import { verifyNotes } from "./notes.mjs";
import { verifyUnits } from "./units.mjs";
import { verifyAutoRanges } from "./auto-ranges.mjs";
import { verifyLayout } from "./layout.mjs";
import { verifyLogAxes } from "./log-axes.mjs";
import { verifyDateAxes } from "./date-axes.mjs";
import { verifyHorizontalBars } from "./horizontal-bars.mjs";
import { verifySeriesStyles } from "./series-styles.mjs";
import { verifySeriesDashes } from "./series-dashes.mjs";
import { verifyAnnotations } from "./annotations.mjs";
import { verifyCallouts } from "./callouts.mjs";
import { verifyErrorBars } from "./error-bars.mjs";
import { verifyBands } from "./bands.mjs";
import { verifyHistogram } from "./histogram.mjs";
import { verifySteps } from "./steps.mjs";

// Every suite `enabled()` guards must appear here, or its *_ONLY selector is
// silently ignored: an unlisted name leaves `focusedSuites` empty, and
// `[].every(...)` is true for everything, so asking for one suite runs them
// all. `typography` and `groups` were dispatched but unlisted, and did exactly
// that. `assertEverySuiteIsSelectable` below keeps the two lists in step.
const SUITES = ["typography", "groups", "steps", "histogram", "bands", "error_bars", "callouts", "annotations", "series_dashes", "series_styles", "horizontal_bars", "date_axes", "log_axes", "layout", "auto_ranges", "units", "notes", "links", "library", "composition", "marks", "fills"];
const focusedSuites = SUITES.filter(name => process.env[`${name.toUpperCase()}_ONLY`]);
assert.ok(focusedSuites.length <= 1, `select at most one *_ONLY suite; got ${focusedSuites.join(", ")}`);
const dispatched = new Set();
const enabled = name => {
  assert.ok(SUITES.includes(name), `suite ${name} is dispatched but not selectable; add it to SUITES`);
  dispatched.add(name);
  return focusedSuites.every(selected => selected === name);
};
// A requested suite that no `enabled()` call guards would also run everything.
const assertEverySuiteIsSelectable = () =>
  assert.deepEqual([...dispatched].sort(), [...SUITES].sort(), "every selectable suite must be dispatched");
// dagger.dang pins the browser through its Playwright image; this fallback is
// for local runs and must name the same build, or local evidence and CI
// evidence are not comparable.
const expectedBrowser = process.env.EXPECTED_CHROMIUM_VERSION || "134.0.6998.35";
assert.match(expectedBrowser, /^\d+\.\d+\.\d+\.\d+$/, "expected an exact pinned browser version");

const server = spawn(process.execPath, ["node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", "5173", "--strictPort"], { cwd: import.meta.dirname, stdio: "inherit" });
function compareScenes(expected, actual) {
  assert.deepEqual(actual.filter(e => !e.isDeleted).map(e => [e.id,e.type]), expected.map(e => [e.id,e.type]));
  for (const [index, reference] of expected.entries()) {
    const element=actual[index];
    for (const field of ["x","y","width","height","angle"]) {
      assert.ok(Math.abs(element[field]-reference[field]) <= (reference.type === "text" ? 1 : 1e-9), `${reference.id}/${field} changed`);
    }
    for (const field of ["points","text","originalText","fontFamily","fontSize","strokeColor","backgroundColor","strokeStyle","strokeWidth","opacity","groupIds","roughness","fillStyle","seed"]) {
      assert.deepEqual(element[field],reference[field],`${reference.id}/${field} changed`);
    }
  }
}
let browser;
let page;
let results;
try {
  const output = process.env.RESULTS_DIR;
  const parent = path.join(import.meta.dirname, "results");
  await mkdir(parent, { recursive: true });
  const destination = output ? path.resolve(output) : await mkdtemp(path.join(parent,"run-"));
  if (output) await mkdir(destination); // Refuse an existing evidence directory.
  results = destination;
  for (let attempt = 0; ; attempt++) {
    try { await fetch("http://127.0.0.1:5173"); break; }
    catch (error) { if (attempt === 100) throw error; await new Promise(r => setTimeout(r, 100)); }
  }
  const executablePath = process.env.CHROMIUM_PATH || execFileSync("which", ["chromium"], { encoding: "utf8" }).trim();
  browser = await chromium.launch({ executablePath, headless: true });
  const context = await browser.newContext({ viewport: { width: 1100, height: 800 } });
  page = await context.newPage();
  page.on("pageerror", error => console.error(error));
  await page.goto("http://127.0.0.1:5173");
  await page.waitForFunction(() => window.checks && window.editor);
  console.log("Browser", await browser.version());
  assert.equal(await browser.version(), expectedBrowser, "use the exact configured compatibility browser");
  const editorVersion = JSON.parse(await readFile(path.join(import.meta.dirname,"node_modules/@excalidraw/excalidraw/package.json"),"utf8")).version;
  const report = { browser: await browser.version(), expectedBrowser, focusedSuites, editor: `@excalidraw/excalidraw@${editorVersion}`, tolerance: 1, maxWidthDrift: 0, maxPositionDrift: 0, textEditChecks: 0, fixtures: {} };
   for (const name of (enabled("typography") ? ["line", "constant", "signed", "typography"] : [])) {
    const json = await readFile(path.join(process.env.GALLERY_DIR || path.join(import.meta.dirname, "../output/gallery"), `${name}.excalidraw`), "utf8");
    const raw = JSON.parse(json).elements;
    const restored = await page.evaluate(json => window.checks.load(json), json);
    assert.equal(restored.filter(e => !e.isDeleted).length, raw.length);
    assert.deepEqual(restored.map(e => e.type), raw.map(e => e.type));
    compareScenes(raw,restored);
    for (const element of raw.filter(e => e.type === "line")) {
      const actual = restored.find(e => e.id === element.id);
      for (const field of ["x", "y", "width", "height", "points"]) assert.deepEqual(actual[field], element[field]);
    }
    const measured = await page.evaluate(elements => elements.filter(e => e.type === "text").map(e => ({ id: e.id, width: window.checks.measure(e.text, e.fontSize) })), raw);
    const refreshed = await page.evaluate(elements => window.checks.refresh(elements), restored);
    for (const reference of raw.filter(e => e.type === "text")) {
      const actual = refreshed.find(e => e.id === reference.id);
      for (const width of [actual.width, measured.find(e => e.id === reference.id).width]) {
        const drift = Math.abs(width - reference.width);
        report.maxWidthDrift = Math.max(report.maxWidthDrift, drift);
        assert.ok(drift <= 1, `${name}/${reference.text}: width drift ${drift}`);
      }
      for (const field of ["x", "y", "height"]) {
        const drift = Math.abs(actual[field] - reference[field]);
        report.maxPositionDrift = Math.max(report.maxPositionDrift, drift);
        assert.ok(drift <= 1, `${name}/${reference.text}: ${field} drift ${drift}`);
      }
    }
    await page.evaluate(elements => window.editor.updateScene({ elements }), refreshed);
    await page.waitForFunction(elements => elements.every(e => window.editor.getSceneElements().find(actual => actual.id === e.id)?.width === e.width), refreshed);
    const saved = await page.evaluate(() => window.checks.save());
    const reopened = await page.evaluate(json => window.checks.load(json), saved);
    assert.equal(reopened.filter(e => !e.isDeleted).length, raw.length);
    compareScenes(refreshed,reopened);
    report.fixtures[name] = { elements: raw.length, text: measured.length };
    if (name === "typography") {
      const anchorCases=raw.filter(e => e.type === "text" && e.text === "AV gyp");
      const corpus=raw.filter(e => e.type === "text" && e.fontSize === 20);
      assert.equal(anchorCases.length,36,"all anchor cases must be exercised");
      assert.equal(corpus.length,10,"all corpus labels must be exercised");
      for (const angle of [0,Math.PI/2,Math.PI,3*Math.PI/2]) assert.equal(anchorCases.filter(e => e.angle===angle).length,9);
      // Isolate each label at the same viewport center, including all 36 anchor /
      // rotation cases. This removes UI occlusion without changing its geometry.
      for (const label of raw.filter(e => e.type === "text" && (e.text === "AV gyp" || e.fontSize === 20))) {
        await page.evaluate(async label => {
          await window.checks.load(JSON.stringify({ type: "excalidraw", version: 2, elements: [label], appState: {}, files: {} }));
          window.editor.updateScene({ appState: { scrollX: 500-label.x-label.width/2, scrollY: 300-label.y-label.height/2 } });
        }, label);
        await page.waitForFunction(id => window.editor.getSceneElements().length === 1 && window.editor.getSceneElements()[0].id === id, label.id);
        await page.mouse.dblclick(500, 300);
        const textarea = page.locator("textarea.excalidraw-wysiwyg");
        await textarea.waitFor();
        await textarea.fill(label.text + "x");
        await textarea.fill(label.text);
        await textarea.press("Escape");
        await textarea.waitFor({ state: "detached" });
        const savedLabel = JSON.parse(await page.evaluate(() => window.checks.save())).elements[0];
        assert.equal(savedLabel.text, label.text);
        assert.equal(savedLabel.angle, label.angle);
        for (const field of ["x", "y", "width", "height"]) {
          const drift = Math.abs(savedLabel[field]-label[field]);
          if (field === "width") report.maxWidthDrift = Math.max(report.maxWidthDrift, drift);
          else report.maxPositionDrift = Math.max(report.maxPositionDrift, drift);
          assert.ok(drift <= 1, `text edit ${label.text}/${label.angle}: ${field} drift ${drift}`);
        }
        report.textEditChecks++;
      }
    }
    if (name === "line") {
      await writeFile(path.join(results, "line.raw.excalidraw"), json);
      await writeFile(path.join(results, "line.saved.excalidraw"), saved);
      await page.screenshot({ path: path.join(results, "line.png") });
      // Remove chart/series groups via the editor for the single-element edit
      // regression. Grouped editing has separate coverage below.
      await page.mouse.click(120,120);
      await page.keyboard.press("Control+a");
      await page.keyboard.press("Control+Shift+G");
      await page.waitForFunction(()=>window.editor.getSceneElements().every(e=>e.groupIds.length===0));
      await page.keyboard.press("Escape");
      // Exercise the real editor's textarea and commit path, not JSON mutation.
      const title = reopened.find(e => e.text === "Six-point line");
      await page.mouse.dblclick(100 + title.x + title.width / 2, 100 + title.y + title.height / 2);
      const textarea = page.locator("textarea.excalidraw-wysiwyg");
      await textarea.waitFor();
      await textarea.fill("Edited six-point line");
      await textarea.press("Escape");
      await page.waitForFunction(() => window.editor.getSceneElements().some(e => e.text === "Edited six-point line"));
      // Use this release's Edit line button, then drag an interior vertex.
      const series = reopened.find(e => e.type === "line" && e.points.length === 6);
      const [dx, dy] = series.points[2];
      await page.mouse.click(100 + series.x + dx, 100 + series.y + dy);
      await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], series.id);
      await page.getByRole("button", { name: "Edit line", exact: true }).click();
      await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
      await page.mouse.move(100 + series.x + dx, 100 + series.y + dy);
      await page.mouse.down();
      await page.mouse.move(100 + series.x + dx + 12, 100 + series.y + dy - 12, { steps: 8 });
      await page.mouse.up();
      await page.waitForFunction(({id,points}) => JSON.stringify(window.editor.getSceneElements().find(e => e.id === id).points) !== JSON.stringify(points), {id:series.id, points:series.points});
      await page.keyboard.press("Escape");
      const edited = await page.evaluate(() => window.checks.save());
      const editedElements = JSON.parse(edited).elements;
      assert.notDeepEqual(editedElements.find(e => e.id === series.id).points, series.points, "vertex edit must change points");
      await writeFile(path.join(results, "line.edited.excalidraw"), edited);
      const editedReopened = await page.evaluate(json => window.checks.load(json), edited);
      assert.equal(editedReopened.find(e => e.id === title.id).text, "Edited six-point line");
      assert.deepEqual(editedReopened.find(e => e.id === series.id).points, editedElements.find(e => e.id === series.id).points);
      compareScenes(editedElements,editedReopened);
      await page.screenshot({ path: path.join(results, "line.edited.png") });
    }
  }
  if (enabled("auto_ranges")) report.autoRanges = await verifyAutoRanges(page, results, compareScenes);
  if (enabled("layout")) report.layout = await verifyLayout(page, results, compareScenes);
  if (enabled("log_axes")) report.logAxes = await verifyLogAxes(page, results, compareScenes);
  if (enabled("date_axes")) report.dateAxes = await verifyDateAxes(page, results, compareScenes);
  if (enabled("horizontal_bars")) report.horizontalBars = await verifyHorizontalBars(page, results, compareScenes);
  if (enabled("series_styles")) report.seriesStyles = await verifySeriesStyles(page, results, compareScenes);
  if (enabled("series_dashes")) report.seriesDashes = await verifySeriesDashes(page, results, compareScenes);
  if (enabled("annotations")) report.annotations = await verifyAnnotations(page, results, compareScenes);
  if (enabled("callouts")) report.callouts = await verifyCallouts(page, results, compareScenes);
  if (enabled("error_bars")) report.errorBars = await verifyErrorBars(page, results, compareScenes);
  if (enabled("bands")) report.bands = await verifyBands(page, results, compareScenes);
  if (enabled("histogram")) report.histogram = await verifyHistogram(page, results, compareScenes);
  if (enabled("steps")) report.steps = await verifySteps(page, results, compareScenes);
  if (enabled("groups")) {
    assert.equal(report.textEditChecks,46);
    report.groups=await verifyGroups(page,results);
  }
  if (enabled("marks")) {
    report.marks = await verifyMarks(page, results, compareScenes, process.env.MARKS_ONLY === "bars" ? ["bars"] : undefined);
    if (process.env.MARKS_ONLY !== "bars") report.probes = await verifyProbes(page, results, compareScenes);
  }
  if (enabled("fills")) report.fills = await verifyFills(page, results, compareScenes, process.env.FILLS_ONLY === "area" ? ["area"] : ["area","pie","donut","area-sketch","area-below","area-constant","pie-sketch","donut-sketch","pie-single"]);
  if (enabled("composition")) report.composition = await verifyComposition(page, results);
  if (enabled("library")) report.library = await verifyLibrary(page, results);
  if (enabled("links")) report.links = await verifyLinks(page, results);
  if (enabled("notes")) report.notes = await verifyNotes(page, results);
  if (enabled("units")) report.units = await verifyUnits(page, results, compareScenes);
  assertEverySuiteIsSelectable();
  report.suitesRun = SUITES.filter(name => enabled(name));
  await writeFile(path.join(results, "report.json"), JSON.stringify(report, null, 2));
  console.log(report);
  console.log("Evidence:", results);
} catch (error) {
  if (results) {
    await writeFile(path.join(results, "failure.txt"), error.stack || String(error));
    if (page && !page.isClosed()) {
      // Best-effort diagnostics must not replace the original assertion failure.
      await page.screenshot({path:path.join(results,"failure.png"),timeout:5000}).catch(()=>{});
      const scene = await page.evaluate(()=>window.checks && window.editor ? window.checks.save() : null).catch(()=>null);
      if (scene !== null) await writeFile(path.join(results,"failure.excalidraw"),scene);
    }
  }
  throw error;
} finally {
  await browser?.close();
  server.kill();
}
