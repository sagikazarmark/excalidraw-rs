import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyAutoRanges(page, results, compareScenes) {
  const json = await readFile(process.env.AUTO_RANGES_FILE || path.join(import.meta.dirname, "../output/auto_ranges.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const elements = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, elements[0].id);
    return elements;
  };
  compareScenes(raw, await load(json));
  const refreshed = await page.evaluate(elements => window.checks.refresh(elements), raw);
  compareScenes(raw, refreshed);
  const title = raw.find(e => e.text === "Automatic irregular X");
  const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], title);
  assert.ok(Math.abs(repaired.width - title.width) <= 1);
  assert.ok(Math.abs(repaired.height - title.height) <= 1);
  await writeFile(path.join(results, "auto-ranges.raw.excalidraw"), json);
  const expected = structuredClone(raw);
  let edits = 0;
  for (const text of ["Automatic irregular X", "Automatic constant Y", "Explicit comparison"]) {
    const label = raw.find(e => e.text === text);
    const panel = raw.filter(e => e.groupIds.includes(label.groupIds[0]));
    const line = panel.find(e => e.type === "line" && e.strokeColor === "#1971c2");
    assert.equal(line.points.length, 6);
    if (text === "Automatic constant Y") assert.ok(line.points.every(p => p[1] === 0));
    else assert.ok(line.points[5][0] > 19 * line.points[1][0], "irregular X spacing retained");
    // Center this panel, then ungroup through the real editor before editing.
    const scrollX = 100, scrollY = 100 - (label.y - title.y);
    await page.evaluate(({ scrollX, scrollY }) => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX, scrollY } }), { scrollX, scrollY });
    await page.waitForFunction(y => window.editor.getAppState().scrollY === y, scrollY);
    await page.mouse.click(scrollX + label.x + label.width / 2, scrollY + label.y + label.height / 2);
    await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, panel.length);
    await page.keyboard.press("Control+Shift+G");
    await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, line.id);
    await page.keyboard.press("Escape");
    await page.mouse.dblclick(scrollX + label.x + label.width / 2, scrollY + label.y + label.height / 2);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    await textarea.fill(`${text} edited`);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    const [dx, dy] = line.points[2];
    await page.mouse.click(scrollX + line.x + dx, scrollY + line.y + dy);
    await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], line.id);
    await page.getByRole("button", { name: "Edit line", exact: true }).click();
    await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
    await page.mouse.move(scrollX + line.x + dx, scrollY + line.y + dy);
    await page.mouse.down();
    await page.mouse.move(scrollX + line.x + dx + 12, scrollY + line.y + dy - 12, { steps: 8 });
    await page.mouse.up();
    await page.keyboard.press("Escape");
    const edited = await page.evaluate(() => window.editor.getSceneElements());
    const editedLine = edited.find(e => e.id === line.id);
    assert.ok(Math.abs(editedLine.points[2][0] - dx - 12) <= 1);
    assert.ok(Math.abs(editedLine.points[2][1] - dy + 12) <= 1);
    for (const [i, point] of line.points.entries()) if (i !== 2) assert.deepEqual(editedLine.points[i], point);
    const editedLabel = edited.find(e => e.id === label.id);
    assert.equal(editedLabel.text, `${text} edited`);
    assert.equal(editedLabel.originalText, `${text} edited`);
    for (const reference of expected) {
      if (panel.some(e => e.id === reference.id)) reference.groupIds = [];
      if (reference.id === line.id) {
        reference.points[2] = [dx + 12, dy - 12];
        reference.width = Math.max(...reference.points.map(p => p[0])) - Math.min(...reference.points.map(p => p[0]));
        reference.height = Math.max(...reference.points.map(p => p[1])) - Math.min(...reference.points.map(p => p[1]));
      }
      if (reference.id === label.id) {
        reference.text = reference.originalText = `${text} edited`;
        reference.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), reference);
      }
    }
    compareScenes(expected, edited);
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(expected, await load(saved));
    await writeFile(path.join(results, `auto-ranges-edit-${++edits}.excalidraw`), saved);
  }
  await page.screenshot({ path: path.join(results, "auto-ranges.edited.png") });
  return { panels: edits, textEdits: edits, vertexEdits: edits, forcedWrongDimensionsRepaired: true, saveReopen: true };
}
