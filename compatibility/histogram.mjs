import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection } from "./interactions.mjs";

export async function verifyHistogram(page, results, compareScenes) {
  const json = await readFile(process.env.HISTOGRAM_FILE || path.join(import.meta.dirname, "../output/histogram.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const load = async json => {
    const es = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, es[0].id);
    return es;
  };
  compareScenes(raw, await load(json));
  const refreshed = await page.evaluate(es => window.checks.refresh(es), raw);
  compareScenes(raw, refreshed);
  await page.evaluate(elements => window.editor.updateScene({ elements }), refreshed);
  await page.waitForFunction(es => es.every(e => window.editor.getSceneElements().find(a => a.id === e.id)?.width === e.width), refreshed);
  compareScenes(refreshed, await load(await page.evaluate(() => window.checks.save())));
  let maxWidthDrift = 0;
  for (const label of raw.filter(e => e.type === "text")) {
    const width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), label);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(width - label.width));
    assert.ok(Math.abs(width - label.width) <= 1);
  }
  const expected = structuredClone(raw);
  const bins = expected.filter(e => e.backgroundColor === "#1971c2");
  assert.equal(bins.length, 5);
  assert.ok(bins.every(e => e.type === "rectangle" && e.y + e.height === bins[0].y + bins[0].height));
  assert.ok(bins[1].width > bins[0].width);
  assert.equal(bins[1].height, bins[3].height);
  assert.ok(bins[2].x > bins[1].x + bins[1].width, "zero bin retains its gap");
  const title = expected.find(e => e.text === "Unequal-bin frequencies");
  const click = async e => {
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + e.x + e.width / 2, 100 + e.y + e.height / 2);
  };
  await click(title);
  await waitForSelection(page, expected.map(e => e.id));
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(x => window.editor.getSceneElements()[0].x === x, expected[0].x);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Control+Shift+G");
  for (const e of expected) e.groupIds = [];
  await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.length === 0));
  for (const bin of bins) {
    await click(bin);
    await waitForSelection(page, [bin.id]);
    await page.keyboard.press("ArrowUp");
    bin.y--;
    await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).y === e.y, bin);
  }
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  const bin = bins.at(-1);
  await page.getByRole("button", { name: "Background", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Background", exact: true });
  await input.fill("b2f2bb");
  await input.press("Enter");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).backgroundColor === "#b2f2bb", bin.id);
  await page.mouse.click(1050, 600);
  bin.backgroundColor = "#b2f2bb";
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  // Resize with the native bottom-right handle, rather than mutating JSON.
  await click(bin);
  await waitForSelection(page, [bin.id]);
  const right = 100 + bin.x + bin.width, bottom = 100 + bin.y + bin.height;
  await page.mouse.move(right + 4, bottom + 4);
  await page.mouse.down();
  await page.mouse.move(right + 24, bottom + 14, { steps: 8 });
  await page.mouse.up();
  const resized = await page.evaluate(id => window.editor.getSceneElements().find(e => e.id === id), bin.id);
  assert.ok(resized.width > bin.width && resized.height > bin.height, "native resize changes bin dimensions");
  Object.assign(bin, { x: resized.x, y: resized.y, width: resized.width, height: resized.height });
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(100 + title.x + title.width / 2, 100 + title.y + title.height / 2);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  title.text = "Edited frequencies";
  title.originalText = title.text;
  title.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), title);
  await textarea.fill(title.text);
  await textarea.press("Escape");
  await textarea.waitFor({ state: "detached" });
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+g");
  await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.length === 1));
  const group = await page.evaluate(() => window.editor.getSceneElements()[0].groupIds[0]);
  for (const e of expected) e.groupIds = [group];
  await click(title);
  await waitForSelection(page, expected.map(e => e.id));
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  const svg = await page.evaluate(() => window.checks.svg());
  assert.ok(svg.includes('fill="#b2f2bb"'));
  assert.ok(!svg.includes("<image"));
  await writeFile(path.join(results, "histogram.raw.excalidraw"), json);
  await writeFile(path.join(results, "histogram.edited.excalidraw"), saved);
  await writeFile(path.join(results, "histogram.edited.svg"), svg);
  await page.screenshot({ path: path.join(results, "histogram.edited.png") });
  return { bins: 6, nativeRectangles: 5, maxWidthDrift, chartMove: true,
    individualBinMoves: 5, binResize: true, binRecolor: true, titleEdit: true,
    nativeRegroup: true, saveReopen: true, nativeSvgFill: true };
}
