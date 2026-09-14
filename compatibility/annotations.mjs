import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection, recolorSelection } from "./interactions.mjs";

export async function verifyAnnotations(page, results, compareScenes) {
  const json = await readFile(process.env.ANNOTATIONS_FILE || path.join(import.meta.dirname, "../output/annotations.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const elements = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, elements[0].id);
    return elements;
  };
  compareScenes(raw, await load(json));
  const expected = structuredClone(raw);
  const windows = expected.filter(e => ["#ffec99", "#d0ebff"].includes(e.backgroundColor));
  const rules = expected.filter(e => e.strokeColor === "#e03131");
  assert.equal(windows.length, 2);
  assert.equal(rules.length, 2);
  const data = expected.filter(e => e.type === "line" && e.points.length === 4);
  assert.equal(data.length, 2);
  assert.deepEqual(windows.map(e => e.opacity), [35, 50]);
  assert.deepEqual(rules.map(e => e.opacity), [100, 65]);
  assert.deepEqual(rules.map(e => e.strokeStyle), ["dashed", "dotted"]);
  for (const e of windows) assert.ok(expected.indexOf(e) < expected.findIndex(e => e.type === "line"));
  for (const e of rules) assert.ok(expected.indexOf(e) > expected.indexOf(data[1]));
  const outer = rules[0].groupIds[1];
  const annotationGroups = new Set();
  for (const e of [...windows, ...rules]) {
    assert.equal(e.groupIds.length, 2);
    assert.equal(e.groupIds[1], outer);
    annotationGroups.add(e.groupIds[0]);
    assert.equal(expected.filter(other => other.groupIds[0] === e.groupIds[0]).length, 1);
  }
  assert.equal(annotationGroups.size, 4);
  const title = expected.find(e => e.text === "Threshold and event windows");
  const click = async (e, fraction = 0.5) => {
    await page.keyboard.press("Escape");
    const offset = e.type === "line"
      ? e.points[0].map((value, i) => value + (e.points.at(-1)[i] - value) * fraction)
      : [e.width * fraction, e.height * fraction];
    await page.mouse.click(100 + e.x + offset[0], 100 + e.y + offset[1]);
  };
  await click(title);
  await waitForSelection(page, expected.map(e => e.id));
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(e => window.editor.getSceneElements()[0].x === e.x, expected[0]);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Control+Shift+G");
  for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== outer);
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), outer);
  for (const rule of rules) {
    // Native dash gaps may miss a mathematical midpoint; try visible path positions.
    let selected = false;
    for (const fraction of [0.5, 0.25, 0.75, 0.1, 0.9]) {
      await click(rule, fraction);
      selected = await page.evaluate(id => !!window.editor.getAppState().selectedElementIds[id], rule.id);
      if (selected) break;
    }
    assert.ok(selected, "reference rule must be natively selectable");
    await waitForSelection(page, [rule.id]);
    await page.keyboard.press("ArrowDown");
    rule.y++;
    await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).y === e.y, rule);
    await recolorSelection(page, [rule.id], "#2f9e44");
    rule.strokeColor = "#2f9e44";
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  }
  for (const window of windows) {
    await click(window, 0.3);
    await waitForSelection(page, [window.id]);
    await page.keyboard.press("ArrowLeft");
    window.x--;
    await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).x === e.x, window);
    await page.getByRole("button", { name: "Background", exact: true }).click();
    const input = page.getByRole("textbox", { name: "Background", exact: true });
    await input.fill("b2f2bb");
    await input.press("Enter");
    await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).backgroundColor === "#b2f2bb", window.id);
    await page.mouse.click(1050, 600);
    await page.waitForFunction(() => !document.querySelector('input[aria-label="Background"]'));
    window.backgroundColor = "#b2f2bb";
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  }
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  await writeFile(path.join(results, "annotations.raw.excalidraw"), json);
  await writeFile(path.join(results, "annotations.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "annotations.edited.png") });

  // Actual clipboard composition into another editor, twice with independent IDs/groups.
  const destination = await page.context().newPage();
  await destination.goto("http://127.0.0.1:5173");
  await destination.waitForFunction(() => window.editor && window.checks);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.bringToFront();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+a");
  await waitForSelection(page, expected.map(e => e.id));
  await page.keyboard.press("Control+c");
  await page.waitForFunction(async id => {
    try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id === id; }
    catch { return false; }
  }, expected[0].id);
  await destination.bringToFront();
  await destination.evaluate(() => window.editor.updateScene({ appState: { zoom: { value: 0.5 }, scrollX: 20, scrollY: 20 } }));
  for (let i = 0; i < 2; i++) {
    await destination.mouse.click(600, 220 + i * 280);
    await destination.keyboard.press("Control+v");
    await destination.waitForFunction(n => window.editor.getSceneElements().length === n, expected.length * (i + 1));
    await destination.keyboard.press("Escape");
  }
  const composed = await destination.evaluate(() => window.editor.getSceneElements());
  assert.equal(new Set(composed.map(e => e.id)).size, expected.length * 2);
  const groups = new Set();
  for (let i = 0; i < 2; i++) {
    const copy = composed.slice(i * expected.length, (i + 1) * expected.length);
    const dx = copy[0].x - expected[0].x, dy = copy[0].y - expected[0].y;
    const remapped = new Map();
    const reference = expected.map((e, index) => {
      assert.notEqual(copy[index].id, e.id);
      assert.equal(copy[index].groupIds.length, e.groupIds.length);
      const groupIds = e.groupIds.map((id, j) => {
        const actual = copy[index].groupIds[j];
        assert.notEqual(actual, id);
        if (!remapped.has(id)) {
          assert.ok(!groups.has(actual));
          groups.add(actual);
          remapped.set(id, actual);
        }
        assert.equal(actual, remapped.get(id));
        return actual;
      });
      return { ...e, id: copy[index].id, seed: copy[index].seed, groupIds, x: e.x + dx, y: e.y + dy };
    });
    compareScenes(reference, copy);
  }
  assert.equal(groups.size, 12);
  const composedSave = await destination.evaluate(() => window.checks.save());
  compareScenes(composed, await destination.evaluate(json => window.checks.load(json), composedSave));
  await writeFile(path.join(results, "annotations.composed.excalidraw"), composedSave);
  await destination.screenshot({ path: path.join(results, "annotations.composed.png") });
  await destination.close();
  return { rules: 2, windows: 2, nativeChartMove: true, isolatedAnnotationMoves: 4, nativeStrokeRecolors: 2, nativeFillRecolors: 2, composedCopies: 2, saveReopen: true };
}
