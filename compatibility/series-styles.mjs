import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection, recolorSelection } from "./interactions.mjs";

export async function verifySeriesStyles(page, results, compareScenes) {
  const json = await readFile(process.env.SERIES_STYLES_FILE || path.join(import.meta.dirname, "../output/series_styles.excalidraw"), "utf8");
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
  const before = raw.find(e => e.text === "Before"), after = raw.find(e => e.text === "After");
  assert.notEqual(before.groupIds[0], after.groupIds[0]);
  assert.equal(before.groupIds[1], after.groupIds[1]);
  for (const [label, width, opacity, fill, radius] of [[before, 2, 100, true, 5], [after, 6, 65, false, 7]]) {
    const members = raw.filter(e => e.groupIds[0] === label.groupIds[0]);
    assert.deepEqual(members.map(e => e.type), ["line", "ellipse", "ellipse", "ellipse", "ellipse", "line", "ellipse", "text"]);
    for (const mark of members.slice(0, -1)) {
      assert.equal(mark.strokeWidth, width);
      assert.equal(mark.opacity, opacity);
      assert.equal(mark.strokeColor, mark.type === "ellipse" && fill ? "transparent" : "#1971c2");
      assert.equal(mark.backgroundColor, mark.type === "ellipse" && fill ? "#1971c2" : "transparent");
      if (mark.type === "ellipse") assert.equal(mark.width, radius * 2);
    }
    members.slice(1, 5).forEach((marker, i) => {
      assert.equal(marker.x + radius, members[0].x + members[0].points[i][0]);
      assert.equal(marker.y + radius, members[0].y + members[0].points[i][1]);
    });
  }
  const expected = structuredClone(raw);
  const click = async e => {
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + e.x + e.width / 2, 100 + e.y + e.height / 2);
  };
  const selection = ids => waitForSelection(page, ids);
  await click(after);
  await selection(raw.map(e => e.id));
  await page.keyboard.press("Control+Shift+G");
  const outer = after.groupIds[1], inner = after.groupIds[0];
  for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== outer);
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), outer);
  const members = expected.filter(e => e.groupIds.includes(inner));
  await click(after);
  await selection(members.map(e => e.id));
  await recolorSelection(page, members.map(e => e.id), "#2f9e44");
  for (const e of members) e.strokeColor = "#2f9e44";
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await click(after);
  await selection(members.map(e => e.id));
  await page.keyboard.press("Control+Shift+G");
  for (const e of members) e.groupIds = [];
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).groupIds.length === 0, after.id);
  // Click the hollow circle's rim, away from its underlying line segment.
  const marker = members.find(e => e.type === "ellipse");
  await page.keyboard.press("Escape");
  await page.mouse.click(100 + marker.x + marker.width / 2, 100 + marker.y);
  await selection([marker.id]);
  await page.keyboard.press("ArrowRight");
  marker.x++;
  await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).x === e.x, marker);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  await writeFile(path.join(results, "series-styles.raw.excalidraw"), json);
  await writeFile(path.join(results, "series-styles.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "series-styles.edited.png") });
  return { series: 2, dataMarkers: 8, maxWidthDrift, nativeGroupSelection: true, nativeRecolor: true, isolatedMarkerMove: true, saveReopen: true };
}
