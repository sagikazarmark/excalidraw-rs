import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyMarks(page, results, compareScenes, names = ["bars", "scatter"]) {
  const report = {};
  for (const name of names) {
    const json = await readFile(path.join(process.env.MARKS_DIR || path.join(import.meta.dirname, "../output"), `${name}.excalidraw`), "utf8");
    const raw = JSON.parse(json).elements;
    const loaded = await page.evaluate(json => window.checks.load(json), json);
    compareScenes(raw, loaded);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, raw[0].id);
    await writeFile(path.join(results, `${name}.raw.excalidraw`), json);
    await page.screenshot({ path: path.join(results, `${name}.png`) });
    const kind = name === "bars" ? "rectangle" : "ellipse";
    const marks = raw.filter(e => e.type === kind && e.backgroundColor === "#1971c2");
    assert.equal(marks.length, name === "bars" ? 3 : 6);
    const mark = marks[0];
    await page.mouse.click(100 + mark.x + mark.width / 2, 100 + mark.y + mark.height / 2);
    await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, raw.length);
    await page.keyboard.press("Control+Shift+G");
    await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.length === 0));
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + mark.x + mark.width / 2, 100 + mark.y + mark.height / 2);
    await page.waitForFunction(id => {
      const ids = Object.keys(window.editor.getAppState().selectedElementIds);
      return ids.length === 1 && ids[0] === id;
    }, mark.id);
    await page.keyboard.press("ArrowRight");
    await page.waitForFunction(({id, x}) => window.editor.getSceneElements().find(e => e.id === id).x === x + 1, mark);
    const edited = JSON.parse(await page.evaluate(() => window.checks.save())).elements;
    for (const reference of raw) {
      const actual = edited.find(e => e.id === reference.id);
      assert.equal(actual.x, reference.x + (reference.id === mark.id ? 1 : 0));
      assert.equal(actual.y, reference.y);
    }
    const saved = await page.evaluate(() => window.checks.save());
    compareScenes(edited, await page.evaluate(json => window.checks.load(json), saved));
    await writeFile(path.join(results, `${name}.edited.excalidraw`), saved);
    report[name] = { elements: raw.length, marks: marks.length, individualSelectionAndMove: true, saveReopen: true };
  }
  return report;
}
