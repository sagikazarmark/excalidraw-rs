import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyLogAxes(page, results, compareScenes) {
  const json = await readFile(process.env.LOG_AXES_FILE || path.join(import.meta.dirname, "../output/log_axes.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const elements = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, elements[0].id);
    return elements;
  };
  compareScenes(raw, await load(json));
  const refreshed = await page.evaluate(elements => window.checks.refresh(elements), raw);
  compareScenes(raw, refreshed);
  let maxWidthDrift = 0;
  for (const label of raw.filter(e => e.type === "text")) {
    const width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), label);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(width - label.width));
    assert.ok(Math.abs(width - label.width) <= 1);
  }
  const numeric = raw.find(e => e.text === "1.0e-14");
  assert.ok(numeric);
  const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], numeric);
  assert.ok(Math.abs(repaired.width - numeric.width) <= 1);
  assert.ok(Math.abs(repaired.height - numeric.height) <= 1);
  await page.evaluate(elements => window.editor.updateScene({ elements }), refreshed.map(e => e.id === repaired.id ? repaired : e));
  await page.waitForFunction(e => window.editor.getSceneElements().find(actual => actual.id === e.id)?.width === e.width, repaired);
  compareScenes(raw, await load(await page.evaluate(() => window.checks.save())));
  // Real textarea remeasurement of a tiny scientific tick, isolated to avoid UI occlusion.
  const single = { ...numeric, groupIds: [] };
  await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [single], appState: {}, files: {} }));
  await page.evaluate(e => window.editor.updateScene({ appState: { scrollX: 500 - e.x - e.width / 2, scrollY: 300 - e.y - e.height / 2 } }), single);
  await page.waitForFunction(e => window.editor.getAppState().scrollY === 300 - e.y - e.height / 2, single);
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
  let vertexEdits = 0;
  let textEdits = 1;
  for (const text of ["Log X / linear Y", "Linear X / log Y", "Log X / log Y", "Small positive log range"]) {
    const label = raw.find(e => e.text === text);
    const outer = label.groupIds.at(-1);
    const panel = raw.filter(e => e.groupIds.includes(outer));
    const line = panel.find(e => e.type === "line" && e.points.length === 4);
    const legend = panel.find(e => e.text === "Observed");
    const marks = panel.filter(e => e.type === "ellipse");
    assert.equal(Boolean(line), text !== "Small positive log range", `${text}: expected native mark type`);
    if (line) assert.deepEqual(line.groupIds, legend.groupIds);
    else {
      assert.equal(marks.length, 5); // four observations and legend swatch
      assert.ok(marks.every(e => JSON.stringify(e.groupIds) === JSON.stringify(legend.groupIds)));
    }
    const scrollX = 100, scrollY = 100 - label.y;
    await page.evaluate(({ scrollX, scrollY }) => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX, scrollY } }), { scrollX, scrollY });
    await page.waitForFunction(y => window.editor.getAppState().scrollY === y, scrollY);
    await page.mouse.click(scrollX + label.x + label.width / 2, 100 + label.height / 2);
    await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, panel.length);
    await page.keyboard.press("Control+Shift+G");
    await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, label.id);
    await page.keyboard.press("Escape");
    await page.mouse.dblclick(scrollX + label.x + label.width / 2, 100 + label.height / 2);
    await textarea.waitFor();
    await textarea.fill(`${text} edited`);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    for (const reference of expected) {
      reference.groupIds = reference.groupIds.filter(id => id !== outer);
      if (reference.id === label.id) {
        reference.text = reference.originalText = `${text} edited`;
        reference.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), reference);
      }
    }
    if (line) {
      // Ungroup the named series, including its legend, before native vertex edit.
      const [dx, dy] = line.points[2];
      await page.mouse.click(scrollX + line.x + dx, scrollY + line.y + dy);
      await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], line.id);
      await page.keyboard.press("Control+Shift+G");
      await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, line.id);
      await page.keyboard.press("Escape");
      await page.mouse.click(scrollX + line.x + dx, scrollY + line.y + dy);
      await page.getByRole("button", { name: "Edit line", exact: true }).click();
      await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
      await page.mouse.move(scrollX + line.x + dx, scrollY + line.y + dy);
      await page.mouse.down();
      await page.mouse.move(scrollX + line.x + dx + 12, scrollY + line.y + dy - 12, { steps: 8 });
      await page.mouse.up();
      await page.keyboard.press("Escape");
      for (const reference of expected) {
        reference.groupIds = reference.groupIds.filter(id => id !== line.groupIds[0]);
        if (reference.id === line.id) {
          reference.points[2] = [dx + 12, dy - 12];
          reference.width = Math.max(...reference.points.map(p => p[0])) - Math.min(...reference.points.map(p => p[0]));
          reference.height = Math.max(...reference.points.map(p => p[1])) - Math.min(...reference.points.map(p => p[1]));
        }
      }
      vertexEdits++;
    }
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(expected, await load(saved));
    await writeFile(path.join(results, `log-axes-edit-${++textEdits}.excalidraw`), saved);
  }
  await writeFile(path.join(results, "log-axes.raw.excalidraw"), json);
  await page.screenshot({ path: path.join(results, "log-axes.edited.png") });
  assert.equal(textEdits, 5);
  assert.equal(vertexEdits, 3);
  return { panels: 4, textEdits, vertexEdits, maxWidthDrift, forcedWrongDimensionsRepaired: true, saveReopen: true };
}
