import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { waitForSelection, recolorSelection } from "./interactions.mjs";

export async function verifySeriesDashes(page, results, compareScenes) {
  const folder = process.env.SERIES_DASHES_DIR || path.join(import.meta.dirname, "../output/series-dashes");
  const generation = JSON.parse(await readFile(path.join(folder, "generation.json"), "utf8"));
  const expectedNames = [0, 1, 2].flatMap(r => [`comparison-${r}`, ...[100, 1000].flatMap(n => ["dashed", "dotted"].map(s => `${s}-${n}-${r}`))]);
  assert.deepEqual(generation.map(f => f.name).sort(), expectedNames.sort());
  const report = { machine: { cpu: os.cpus()[0].model, logicalCpus: os.cpus().length, memoryGiB: os.totalmem() / 2 ** 30, kernel: os.release(), viewport: page.viewportSize(), headless: true }, fixtures: [] };
  const settled = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const load = async json => {
    const restored = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, restored[0].id);
    await settled();
    return restored;
  };
  const selection = ids => waitForSelection(page, ids);
  for (const fixture of generation) {
    console.log("Native dash fixture", fixture.name);
    const json = await readFile(path.join(folder, `${fixture.name}.excalidraw`), "utf8");
    const raw = JSON.parse(json).elements;
    const expected = structuredClone(raw);
    const counts = {};
    for (const e of raw) counts[e.type] = (counts[e.type] || 0) + 1;
    assert.deepEqual(counts, fixture.elements);
    assert.equal(raw.reduce((n, e) => n + (e.points?.length || 0), 0), fixture.vertices);
    assert.equal(Buffer.byteLength(json), fixture.bytes);
    assert.equal(fixture.calls.draw_pixel, undefined);
    assert.equal(fixture.calls.blit_bitmap, undefined);
    const start = performance.now();
    compareScenes(raw, await load(json));
    const loadMs = performance.now() - start;
    const comparison = fixture.name.startsWith("comparison");
    const paths = expected.filter(e => e.points?.length > 2);
    assert.equal(paths.length, comparison ? 3 : 1);
    for (const p of paths) assert.equal(p.points.length, fixture.count);
    const svgPatterns = [];
    if (comparison) {
      for (const [i, style] of ["solid", "dashed", "dotted"].entries()) {
        const p = paths[i];
        assert.equal(p.strokeStyle, style);
        const members = expected.filter(e => e.groupIds[0] === p.groupIds[0]);
        assert.deepEqual(members.map(e => e.type), ["line", "line", "text"]);
        assert.equal(members[1].strokeStyle, style);
        for (const mark of members.slice(0, 2)) {
          const svg = await page.evaluate(id => window.checks.svg([id]), mark.id);
          const arrays = await page.evaluate(svg => [...new DOMParser().parseFromString(svg, "image/svg+xml").querySelectorAll("[stroke-dasharray]")].map(e => e.getAttribute("stroke-dasharray")), svg);
          if (style === "solid") assert.equal(arrays.length, 0);
          else assert.ok(arrays.length > 0 && arrays.every(a => /[1-9]/.test(a)));
          svgPatterns.push({ style, swatch: mark !== p, arrays });
          await writeFile(path.join(results, `${fixture.name}-${style}-${mark === p ? "path" : "legend"}.svg`), svg);
        }
        assert.deepEqual(svgPatterns.at(-1).arrays, svgPatterns.at(-2).arrays);
      }
      assert.notDeepEqual(svgPatterns[2].arrays, svgPatterns[4].arrays);
    } else assert.equal(paths[0].strokeStyle, fixture.name.split("-")[0]);
    await page.mouse.click(800, 600);
    const panStart = performance.now();
    await page.keyboard.down("Space");
    await page.mouse.move(800, 600);
    await page.mouse.down();
    await page.mouse.move(830, 620, { steps: 6 });
    await page.mouse.up();
    await page.keyboard.up("Space");
    await settled();
    assert.notEqual(await page.evaluate(() => window.editor.getAppState().scrollX), 100);
    const panMs = performance.now() - panStart;
    await load(json);
    // Native chart selection and ungrouping; subsequent selection is per series.
    const selectStart = performance.now();
    await page.mouse.click(100 + paths[0].x, 100 + paths[0].y);
    await selection(raw.map(e => e.id));
    const selectionMs = performance.now() - selectStart;
    await page.keyboard.press("Control+Shift+G");
    const outer = paths[0].groupIds.at(-1);
    for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== outer);
    await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), outer);
    let vertexEdits = 0;
    const markStart = performance.now();
    for (const p of paths) {
      await page.keyboard.press("Escape");
      await page.mouse.click(100 + p.x, 100 + p.y);
      const inner = p.groupIds[0];
      const members = expected.filter(e => e.groupIds.includes(inner));
      await selection(members.map(e => e.id));
      if (comparison) {
        await recolorSelection(page, members.map(e => e.id), "#2f9e44");
        for (const e of members) e.strokeColor = "#2f9e44";
        compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
        await page.mouse.click(100 + p.x, 100 + p.y);
        await selection(members.map(e => e.id));
      }
      // Ungroup the series before selecting its original path alone.
      if (members.length > 1) {
        await page.keyboard.press("Control+Shift+G");
        for (const e of members) e.groupIds = e.groupIds.filter(id => id !== inner);
        await page.waitForFunction(id => !window.editor.getSceneElements().find(e => e.id === id).groupIds.length, p.id);
      }
      await page.keyboard.press("Escape");
      // Rough strokes/dash gaps can miss a mathematical vertex. Try points on
      // the original path until native hit testing selects that exact element.
      const hits = comparison ? [p.points[2], p.points[1], p.points[3], [0, 0]] : [[0, 0]];
      for (const hit of hits) {
        await page.mouse.click(100 + p.x + hit[0], 100 + p.y + hit[1]);
        await settled();
        if (await page.evaluate(id => window.editor.getAppState().selectedElementIds[id], p.id)) break;
        await page.keyboard.press("Escape");
      }
      await selection([p.id]);
      if (comparison) {
        await page.getByRole("button", { name: "Edit line", exact: true }).click();
        await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
        const [x, y] = p.points[1];
        await page.mouse.move(100 + p.x + x, 100 + p.y + y);
        await page.mouse.down();
        await page.mouse.move(112 + p.x + x, 88 + p.y + y, { steps: 8 });
        await page.mouse.up();
        p.points[1] = [x + 12, y - 12];
        const ys = p.points.map(v => v[1]);
        p.height = Math.max(...ys) - Math.min(...ys);
        await page.keyboard.press("Escape");
        compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
        vertexEdits++;
      }
    }
    const markSelectionMs = performance.now() - markStart;
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
    const saveStart = performance.now();
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(expected, await load(saved));
    const saveReopenMs = performance.now() - saveStart;
    await writeFile(path.join(results, `${fixture.name}.raw.excalidraw`), json);
    await writeFile(path.join(results, `${fixture.name}.edited.excalidraw`), saved);
    await page.screenshot({ path: path.join(results, `${fixture.name}.png`) });
    report.fixtures.push({ ...fixture, loadMs, panMs, selectionMs, markSelectionMs, saveReopenMs, vertexEdits, svgPatterns });
  }
  return report;
}
