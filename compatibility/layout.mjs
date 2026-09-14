import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyLayout(page, results, compareScenes) {
  const json = await readFile(process.env.LAYOUT_FILE || path.join(import.meta.dirname, "../output/layout.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const elements = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, elements[0].id);
    return elements;
  };
  compareScenes(raw, await load(json));
  const refreshed = await page.evaluate(elements => window.checks.refresh(elements), raw);
  compareScenes(raw, refreshed);
  await page.evaluate(elements => window.editor.updateScene({ elements }), refreshed);
  await page.waitForFunction(elements => elements.every(e => window.editor.getSceneElements().find(actual => actual.id === e.id)?.width === e.width), refreshed);
  compareScenes(raw, await load(await page.evaluate(() => window.checks.save())));
  let maxWidthDrift = 0;
  for (const label of raw.filter(e => e.type === "text")) {
    const width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), label);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(width - label.width));
    assert.ok(Math.abs(width - label.width) <= 1);
  }
  const numeric = raw.find(e => e.text === "1.0e9");
  const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], numeric);
  assert.ok(Math.abs(repaired.width - numeric.width) <= 1);
  assert.ok(Math.abs(repaired.height - numeric.height) <= 1);
  const repairedScene = refreshed.map(e => e.id === repaired.id ? repaired : e);
  await page.evaluate(elements => window.editor.updateScene({ elements }), repairedScene);
  await page.waitForFunction(e => window.editor.getSceneElements().find(actual => actual.id === e.id)?.width === e.width, repaired);
  compareScenes(raw, await load(await page.evaluate(() => window.checks.save())));
  let textEdits = 0;
  // Native textarea remeasurement of each format and the long legend, isolated
  // only to keep the editor toolbar from occluding endpoint labels.
  for (const text of ["1000000000", "1.0e9", "20%", "Secondary observation"]) {
    const label = raw.find(e => e.text === text);
    assert.ok(label, text);
    const single = { ...label, groupIds: [] };
    await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [single], appState: {}, files: {} }));
    await page.evaluate(e => window.editor.updateScene({ appState: { scrollX: 500 - e.x - e.width / 2, scrollY: 300 - e.y - e.height / 2 } }), label);
    await page.waitForFunction(e => window.editor.getAppState().scrollX === 500 - e.x - e.width / 2 && window.editor.getAppState().scrollY === 300 - e.y - e.height / 2, label);
    await page.mouse.dblclick(500, 300);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    assert.equal(await textarea.inputValue(), text);
    await textarea.fill(`${text}x`);
    await textarea.fill(text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes([single], await load(saved));
    textEdits++;
  }
  await load(json);
  const expected = structuredClone(raw);
  for (const title of ["Decimal comparison", "Compact right legend", "Compact legend off"]) {
    const label = raw.find(e => e.text === title);
    const outer = label.groupIds.at(-1);
    const panel = raw.filter(e => e.groupIds.includes(outer));
    const marks = panel.filter(e => e.type === "line" && e.points.length === 4);
    assert.equal(marks.length, 2);
    assert.notEqual(marks[0].groupIds[0], marks[1].groupIds[0]);
    const legends = panel.filter(e => ["Primary observation", "Secondary observation"].includes(e.text));
    assert.equal(legends.length, title === "Compact legend off" ? 0 : 2);
    const scrollX = 30, scrollY = 100 - label.y;
    await page.evaluate(({ scrollX, scrollY }) => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX, scrollY } }), { scrollX, scrollY });
    await page.waitForFunction(y => window.editor.getAppState().scrollY === y, scrollY);
    await page.mouse.click(scrollX + label.x + label.width / 2, scrollY + label.y + label.height / 2);
    await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, panel.length);
    await page.keyboard.press("Control+Shift+G");
    await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, label.id);
    await page.keyboard.press("Escape");
    await page.mouse.dblclick(scrollX + label.x + label.width / 2, scrollY + label.y + label.height / 2);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    await textarea.fill(`${title} edited`);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    for (const reference of expected) {
      reference.groupIds = reference.groupIds.filter(id => id !== outer);
      if (reference.id === label.id) {
        reference.text = reference.originalText = `${title} edited`;
        reference.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), reference);
      }
    }
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(expected, await load(saved));
    await writeFile(path.join(results, `layout-edit-${++textEdits}.excalidraw`), saved);
  }
  await writeFile(path.join(results, "layout.raw.excalidraw"), json);
  await page.screenshot({ path: path.join(results, "layout.edited.png") });
  return { panels: 3, textEdits, maxWidthDrift, forcedWrongDimensionsRepaired: true, hiddenSeriesGroupsPreserved: true, saveReopen: true };
}
