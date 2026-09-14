import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection } from "./interactions.mjs";

export async function verifyErrorBars(page, results, compareScenes) {
  const json = await readFile(process.env.ERROR_BARS_FILE || path.join(import.meta.dirname, "../output/error_bars.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const timings = {};
  const timed = async (name, action) => {
    const start = performance.now();
    const result = await action();
    timings[name] = performance.now() - start;
    return result;
  };
  const load = async json => {
    const es = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, es[0].id);
    return es;
  };
  compareScenes(raw, await timed("loadMs", () => load(json)));
  const refreshed = await page.evaluate(es => window.checks.refresh(es), raw);
  compareScenes(raw, refreshed);
  await page.evaluate(elements => window.editor.updateScene({ elements }), refreshed);
  await page.waitForFunction(es => es.every(e => window.editor.getSceneElements().find(a => a.id === e.id)?.width === e.width), refreshed);
  compareScenes(refreshed, await load(await page.evaluate(() => window.checks.save())));
  const expected = structuredClone(raw);
  const centers = expected.filter(e => e.type === "ellipse" && e.groupIds.length === 3);
  assert.equal(centers.length, 6);
  assert.equal(new Set(centers.map(e => e.groupIds[0])).size, 6);
  const compounds = centers.map(c => expected.filter(e => e.groupIds[0] === c.groupIds[0]));
  assert.deepEqual(compounds.map(es => es.map(e => e.type)), [
    ["line", "line", "line", "ellipse"], ["line", "line", "line", "ellipse"],
    ["line", "line", "line", "ellipse"], ["line", "line", "line", "ellipse"],
    ["line", "ellipse"], ["line", "line", "line", "ellipse"],
  ]);
  assert.equal(centers[1].x, centers[5].x);
  assert.equal(centers[1].y, centers[5].y);
  const title = expected.find(e => e.text === "Six supplied intervals");
  const legend = expected.find(e => e.text === "Supplied 95% CI");
  const click = async e => {
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + e.x + e.width / 2, 100 + e.y + e.height / 2);
  };
  const ungroup = async id => {
    await page.keyboard.press("Control+Shift+G");
    for (const e of expected) e.groupIds = e.groupIds.filter(g => g !== id);
    await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), id);
  };
  await timed("chartSelectionMs", async () => {
    await click(title);
    await waitForSelection(page, expected.map(e => e.id));
  });
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(x => window.editor.getSceneElements()[0].x === x, expected[0].x);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await ungroup(title.groupIds.at(-1));
  await click(legend);
  const series = expected.filter(e => e.groupIds.includes(legend.groupIds[0]));
  await waitForSelection(page, series.map(e => e.id));
  await ungroup(legend.groupIds[0]);
  // Move the topmost duplicate first, exposing the other equal-valued compound.
  await timed("sixCompoundMovesMs", async () => {
    for (const index of [5, 1, 0, 2, 3, 4]) {
      await click(centers[index]);
      await waitForSelection(page, compounds[index].map(e => e.id));
      // The pinned editor uses five-unit Shift nudges. Clear its hit-test halo
      // as well as the visible cap before selecting the underlying duplicate.
      for (let n = 0; n < 8; n++) await page.keyboard.press("Shift+ArrowRight");
      for (const e of compounds[index]) e.x += 40;
      await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).x === e.x, centers[index]);
      compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
    }
  });
  for (const label of [title, legend]) {
    await page.keyboard.press("Escape");
    await page.mouse.dblclick(100 + label.x + label.width / 2, 100 + label.y + label.height / 2);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    label.text = label === title ? "Edited intervals" : "Edited 95% CI";
    label.originalText = label.text;
    label.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), label);
    await textarea.fill(label.text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  }
  const saved = await timed("saveReopenMs", async () => {
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(expected, await load(saved));
    return saved;
  });
  await writeFile(path.join(results, "error-bars.raw.excalidraw"), json);
  await writeFile(path.join(results, "error-bars.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "error-bars.edited.png") });
  return { observations: 6, dataElements: 22, elements: raw.length,
    vertices: raw.reduce((n, e) => n + (e.points?.length || 0), 0),
    bytes: Buffer.byteLength(json), nativeCompoundMoves: 6, nativeTextEdits: 2,
    saveReopen: true, timings };
}
