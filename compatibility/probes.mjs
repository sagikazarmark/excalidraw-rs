import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";

export async function verifyProbes(page, results, compareScenes) {
  const folder = process.env.PROBES_DIR || path.join(import.meta.dirname, "../output/probes");
  const generation = JSON.parse(await readFile(path.join(folder, "generation.json"), "utf8"));
  const expected = ["scatter-100", "line-100", "scatter-1000", "line-1000", "scatter-5000", "line-5000",
    "bars-positive", "bars-negative", "bars-constant", "bars-zero", "scatter-constant", "scatter-legend", "scatter-boundaries"];
  assert.deepEqual(generation.map(f => f.name).sort(), expected.sort(), "every required probe must be present exactly once");
  const report = { machine: { cpu: os.cpus()[0].model, logicalCpus: os.cpus().length, memoryGiB: os.totalmem() / 2 ** 30, platform: os.platform(), kernel: os.release(), viewport: page.viewportSize(), headless: true }, fixtures: [] };
  // Two animation frames include the editor's render before timing completes.
  const settled = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  for (const fixture of generation) {
    const json = await readFile(path.join(folder, `${fixture.name}.excalidraw`), "utf8");
    const raw = JSON.parse(json).elements;
    const counts = {};
    for (const e of raw) counts[e.type] = (counts[e.type] || 0) + 1;
    assert.deepEqual(counts, fixture.elements);
    assert.equal(raw.reduce((sum, e) => sum + (e.points?.length || 0), 0), fixture.vertices);
    assert.equal(Buffer.byteLength(json), fixture.bytes);
    if (/^(scatter|line)-\d+$/.test(fixture.name)) {
      const [kind, count] = fixture.name.split("-");
      if (kind === "scatter") assert.equal(counts.ellipse, Number(count));
      else assert.equal(raw.filter(e => e.points?.length === Number(count)).length, 1);
    }
    const loadMs = await page.evaluate(async json => {
      const start = performance.now();
      await window.checks.load(json);
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return performance.now() - start;
    }, json);
    compareScenes(raw, await page.evaluate(() => window.editor.getSceneElements()));
    await page.screenshot({ path: path.join(results, `${fixture.name}.png`) });
    const startPan = performance.now();
    const scroll = await page.evaluate(() => window.editor.getAppState().scrollX);
    await page.keyboard.down("Space");
    await page.mouse.move(800, 600);
    await page.mouse.down();
    await page.mouse.move(830, 620, { steps: 6 });
    await page.mouse.up();
    await page.keyboard.up("Space");
    await settled();
    assert.notEqual(await page.evaluate(() => window.editor.getAppState().scrollX), scroll);
    const panMs = performance.now() - startPan;
    const startSelection = performance.now();
    await page.mouse.click(150, 150);
    await page.keyboard.press("Control+a");
    await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, raw.length);
    await settled();
    const selectionMs = performance.now() - startSelection;
    await page.keyboard.press("Control+Shift+G");
    await page.keyboard.press("Escape");
    await settled();
    if (fixture.name === "scatter-legend") {
      const label = raw.find(e => e.text === "Measured");
      const state = await page.evaluate(() => { const s = window.editor.getAppState(); return { x: s.scrollX, y: s.scrollY }; });
      await page.mouse.click(state.x + label.x + label.width / 2, state.y + label.y + label.height / 2);
      await page.waitForFunction(() => Object.keys(window.editor.getAppState().selectedElementIds).length === 4);
      const selected = await page.evaluate(() => Object.keys(window.editor.getAppState().selectedElementIds).sort());
      assert.deepEqual(selected, raw.filter(e => e.groupIds[0] === label.groupIds[0]).map(e => e.id).sort());
    }
    let markSelectionMs = null;
    if (/^(scatter|line)-\d+$/.test(fixture.name)) {
      const mark = fixture.name.startsWith("scatter") ? raw.find(e => e.type === "ellipse") : raw.find(e => e.points?.length > 2);
      const state = await page.evaluate(() => { const s = window.editor.getAppState(); return { x: s.scrollX, y: s.scrollY }; });
      const x = state.x + mark.x + (mark.type === "ellipse" ? mark.width / 2 : 0);
      const y = state.y + mark.y + (mark.type === "ellipse" ? mark.height / 2 : 0);
      const start = performance.now();
      await page.mouse.click(x, y);
      await page.waitForFunction(kind => {
        const ids = Object.keys(window.editor.getAppState().selectedElementIds);
        return ids.length === 1 && window.editor.getSceneElements().find(e => e.id === ids[0])?.type === kind;
      }, mark.type);
      await settled();
      markSelectionMs = performance.now() - start;
    }
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(JSON.parse(saved).elements, await page.evaluate(json => window.checks.load(json), saved));
    await writeFile(path.join(results, `${fixture.name}.saved.excalidraw`), saved);
    report.fixtures.push({ ...fixture, loadMs, panMs, selectionMs, markSelectionMs });
  }
  return report;
}
