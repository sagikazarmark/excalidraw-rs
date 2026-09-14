import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyHorizontalBars(page, results, compareScenes) {
  const json = await readFile(process.env.HORIZONTAL_BARS_FILE || path.join(import.meta.dirname, "../output/horizontal_bars.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const elements = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, elements[0].id);
    return elements;
  };
  compareScenes(raw, await load(json));
  compareScenes(raw, await page.evaluate(elements => window.checks.refresh(elements), raw));
  let maxWidthDrift = 0;
  for (const label of raw.filter(e => e.type === "text")) {
    const width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), label);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(width - label.width));
    assert.ok(Math.abs(width - label.width) <= 1);
  }
  const longLabel = raw.find(e => e.text === "International customer support");
  assert.equal(longLabel.angle, 0);
  const single = { ...longLabel, groupIds: [] };
  const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], single);
  compareScenes([single], [repaired]);
  await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [repaired], appState: {}, files: {} }));
  await page.evaluate(e => window.editor.updateScene({ appState: { scrollX: 500 - e.x - e.width / 2, scrollY: 300 - e.y - e.height / 2 } }), repaired);
  await page.waitForFunction(e => window.editor.getAppState().scrollY === 300 - e.y - e.height / 2, repaired);
  await page.mouse.dblclick(500, 300);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  assert.equal(await textarea.inputValue(), single.text);
  await textarea.fill(`${single.text}x`);
  await textarea.fill(single.text);
  await textarea.press("Escape");
  await textarea.waitFor({ state: "detached" });
  compareScenes([single], await load(await page.evaluate(() => window.checks.save())));

  await load(json);
  const expected = structuredClone(raw);
  const before = raw.find(e => e.text === "Before");
  const inner = before.groupIds[0], outer = before.groupIds[1];
  const members = raw.filter(e => e.groupIds.includes(inner));
  const bars = members.filter(e => e.type === "rectangle" && e.height !== 14);
  assert.equal(members.length, 4, "two nonzero bars, swatch and name");
  assert.equal(bars.length, 2);
  assert.equal(raw.filter(e => e.type === "rectangle").length, 8);
  assert.equal(bars[0].x, bars[1].x + bars[1].width);
  assert.ok(bars[0].y + bars[0].height < bars[1].y);
  const click = async e => {
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + e.x + e.width / 2, 100 + e.y + e.height / 2);
  };
  const selection = async ids => page.waitForFunction(ids => {
    const selected = Object.keys(window.editor.getAppState().selectedElementIds).filter(id => window.editor.getAppState().selectedElementIds[id]);
    return selected.length === ids.length && ids.every(id => selected.includes(id));
  }, ids);
  await click(before);
  await selection(raw.map(e => e.id));
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(e => window.editor.getSceneElements()[0].x === e.x, expected[0]);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Control+Shift+G");
  for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== outer);
  await page.waitForFunction(outer => window.editor.getSceneElements().every(e => !e.groupIds.includes(outer)), outer);
  await click(expected.find(e => e.id === before.id));
  await selection(members.map(e => e.id));
  await page.keyboard.press("ArrowDown");
  for (const e of expected.filter(e => e.groupIds.includes(inner))) e.y++;
  await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).y === e.y, expected.find(e => e.id === before.id));
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.getByRole("button", { name: "Stroke", exact: true }).click();
  await page.getByRole("textbox", { name: "Stroke", exact: true }).fill("2f9e44");
  await page.getByRole("textbox", { name: "Stroke", exact: true }).press("Enter");
  await page.waitForFunction(ids => window.editor.getSceneElements().filter(e => ids.includes(e.id)).every(e => e.strokeColor === "#2f9e44"), members.map(e => e.id));
  await page.mouse.click(1050, 600);
  await page.waitForFunction(() => !document.querySelector('input[aria-label="Stroke"]'));
  for (const e of expected.filter(e => e.groupIds.includes(inner))) e.strokeColor = "#2f9e44";
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await click(expected.find(e => e.id === before.id));
  await selection(members.map(e => e.id));
  await page.keyboard.press("Control+Shift+G");
  for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== inner);
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, bars[0].id);
  const bar = expected.find(e => e.id === bars[0].id);
  await click(bar);
  await selection([bar.id]);
  await page.keyboard.press("ArrowRight");
  bar.x++;
  await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).x === e.x, bar);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  // Restore the series group through native multi-selection, then group the chart.
  await page.keyboard.press("Escape");
  await page.evaluate(ids => window.editor.updateScene({ appState: { selectedElementIds: Object.fromEntries(ids.map(id => [id, true])) } }), members.map(e => e.id));
  await selection(members.map(e => e.id));
  await page.keyboard.press("Control+g");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 1, bar.id);
  const newInner = await page.evaluate(id => window.editor.getSceneElements().find(e => e.id === id).groupIds[0], bar.id);
  for (const e of expected.filter(e => members.some(m => m.id === e.id))) e.groupIds = [newInner];
  // Native grouping makes the selected elements contiguous at their topmost
  // member's position in painter order.
  const grouped = expected.filter(e => e.groupIds.includes(newInner));
  const lastMember = expected.findLastIndex(e => e.groupIds.includes(newInner));
  const reordered = [
    ...expected.slice(0, lastMember + 1).filter(e => !e.groupIds.includes(newInner)),
    ...grouped,
    ...expected.slice(lastMember + 1),
  ];
  expected.splice(0, expected.length, ...reordered);
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+g");
  await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.at(-1) === window.editor.getSceneElements()[0].groupIds.at(-1)));
  const newOuter = await page.evaluate(() => window.editor.getSceneElements()[0].groupIds.at(-1));
  for (const e of expected) e.groupIds.push(newOuter);
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  await writeFile(path.join(results, "horizontal-bars.raw.excalidraw"), json);
  await writeFile(path.join(results, "horizontal-bars.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "horizontal-bars.edited.png") });

  // Actual clipboard composition: duplicate the edited chart into a second editor.
  const destination = await page.context().newPage();
  await destination.goto("http://127.0.0.1:5173");
  await destination.waitForFunction(() => window.editor && window.checks);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.bringToFront();
  await click(expected.find(e => e.id === before.id));
  await selection(expected.map(e => e.id));
  await page.keyboard.press("Control+c");
  await page.waitForFunction(async id => {
    try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id === id; }
    catch { return false; }
  }, expected[0].id);
  await destination.bringToFront();
  await destination.evaluate(() => window.editor.updateScene({ appState: { zoom: { value: 0.5 }, scrollX: 20, scrollY: 20, viewBackgroundColor: "#d3f9d8" } }));
  for (let i = 0; i < 2; i++) {
    await destination.mouse.click(500, 250 + i * 280);
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
          assert.ok(!groups.has(actual), "groups must not leak across series or copies");
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
  assert.equal(groups.size, 6);
  const composedSave = await destination.evaluate(() => window.checks.save());
  compareScenes(composed, await destination.evaluate(json => window.checks.load(json), composedSave));
  await writeFile(path.join(results, "horizontal-bars.composed.excalidraw"), composedSave);
  await destination.screenshot({ path: path.join(results, "horizontal-bars.composed.png") });
  await destination.close();
  return { categories: 3, series: 2, nonzeroBars: 5, textEdits: 1, maxWidthDrift, individualBarMove: true, seriesMoveAndRecolor: true, nativeRegroup: true, composedCopies: 2, saveReopen: true };
}
