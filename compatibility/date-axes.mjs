import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyDateAxes(page, results, compareScenes) {
  const json = await readFile(process.env.DATE_AXES_FILE || path.join(import.meta.dirname, "../output/date_axes.excalidraw"), "utf8");
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
  // Actual calendar labels must remeasure correctly, including repair of bad boxes.
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  let textEdits = 0;
  for (const text of ["2024-02-28", "2025-01-01 00:00:00Z"]) {
    const label = raw.find(e => e.text === text);
    assert.ok(label, text);
    const single = { ...label, groupIds: [] };
    const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], single);
    compareScenes([single], [repaired]);
    await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [repaired], appState: {}, files: {} }));
    await page.evaluate(e => window.editor.updateScene({ appState: { scrollX: 500 - e.x - e.width / 2, scrollY: 300 - e.y - e.height / 2 } }), repaired);
    await page.waitForFunction(e => window.editor.getAppState().scrollY === 300 - e.y - e.height / 2, repaired);
    await page.mouse.dblclick(500, 300);
    await textarea.waitFor();
    assert.equal(await textarea.inputValue(), text);
    await textarea.fill(`${text}x`);
    await textarea.fill(text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    compareScenes([single], await load(await page.evaluate(() => window.checks.save())));
    textEdits++;
  }
  await load(json);
  const expected = structuredClone(raw);
  const label = raw.find(e => e.text === "Irregular daily observations");
  const outer = label.groupIds.at(-1);
  const panel = raw.filter(e => e.groupIds.includes(outer));
  const line = panel.find(e => e.type === "line" && e.strokeColor === "#1971c2");
  assert.equal(line.points.length, 4);
  // Elapsed days are 0, 1, 4, 8: observation indexes would fail this check.
  const xs = line.points.map(p => p[0]);
  assert.ok(Math.abs(xs[1] - xs[3] / 8) <= 1);
  assert.ok(Math.abs(xs[2] - xs[3] / 2) <= 1);
  const utc = raw.find(e => e.text === "UTC observations across midnight");
  const marks = raw.filter(e => e.type === "ellipse" && e.groupIds.includes(utc.groupIds.at(-1)));
  assert.equal(marks.length, 4);
  assert.equal(marks[1].x, marks[2].x);
  assert.notEqual(marks[1].y, marks[2].y);
  await page.evaluate(() => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX: 100, scrollY: 100 } }));
  await page.waitForFunction(() => window.editor.getAppState().scrollY === 100);
  await page.mouse.click(100 + label.x + label.width / 2, 100 + label.y + label.height / 2);
  await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, panel.length);
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, label.id);
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(100 + label.x + label.width / 2, 100 + label.y + label.height / 2);
  await textarea.waitFor();
  await textarea.fill("Irregular observations edited");
  await textarea.press("Escape");
  await textarea.waitFor({ state: "detached" });
  textEdits++;
  for (const e of expected) {
    e.groupIds = e.groupIds.filter(id => id !== outer);
    if (e.id === label.id) {
      e.text = e.originalText = "Irregular observations edited";
      e.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), e);
    }
  }
  const [dx, dy] = line.points[2];
  await page.mouse.click(100 + line.x + dx, 100 + line.y + dy);
  await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], line.id);
  await page.getByRole("button", { name: "Edit line", exact: true }).click();
  await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
  await page.mouse.move(100 + line.x + dx, 100 + line.y + dy);
  await page.mouse.down();
  await page.mouse.move(112 + line.x + dx, 88 + line.y + dy, { steps: 8 });
  await page.mouse.up();
  await page.keyboard.press("Escape");
  const editedLine = expected.find(e => e.id === line.id);
  editedLine.points[2] = [dx + 12, dy - 12];
  editedLine.width = Math.max(...editedLine.points.map(p => p[0])) - Math.min(...editedLine.points.map(p => p[0]));
  editedLine.height = Math.max(...editedLine.points.map(p => p[1])) - Math.min(...editedLine.points.map(p => p[1]));
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  await writeFile(path.join(results, "date-axes.raw.excalidraw"), json);
  await writeFile(path.join(results, "date-axes.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "date-axes.edited.png") });
  return { panels: 2, textEdits, vertexEdits: 1, maxWidthDrift, elapsedTimeSpacing: true, forcedWrongDimensionsRepaired: true, saveReopen: true };
}
