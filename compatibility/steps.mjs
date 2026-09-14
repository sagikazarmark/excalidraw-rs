import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection } from "./interactions.mjs";

export async function verifySteps(page, results, compareScenes) {
  const json = await readFile(process.env.STEPS_FILE || path.join(import.meta.dirname, "../output/steps.excalidraw"), "utf8");
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
  const paths = expected.filter(e => e.strokeColor === "#1971c2");
  assert.deepEqual(paths.map(e => e.points.length), [8, 10]);
  for (const line of paths) {
    assert.equal(line.type, "line");
    assert.equal(line.roundness, null);
    for (let i = 1; i < line.points.length; i++) {
      const a = line.points[i - 1], b = line.points[i];
      assert.ok((a[0] === b[0]) !== (a[1] === b[1]), "original sharp horizontal/vertical legs");
    }
    const group = line.groupIds[0];
    const panel = expected.filter(e => e.groupIds.includes(group));
    const title = panel.find(e => e.type === "text" && e.fontSize === 24);
    await page.keyboard.press("Escape");
    // Scroll each panel into the same viewport for actual selection and editing.
    const scrollY = 100 - panel[0].y;
    await page.evaluate(scrollY => window.editor.updateScene({ appState: { scrollY } }), scrollY);
    await page.mouse.click(100 + title.x + title.width / 2, scrollY + title.y + title.height / 2);
    await waitForSelection(page, panel.map(e => e.id));
    await page.keyboard.press("ArrowRight");
    for (const e of panel) e.x++;
    await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).x === e.x, line);
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
    await page.keyboard.press("Control+Shift+G");
    for (const e of panel) e.groupIds = [];
    await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), group);
    await page.keyboard.press("Escape");
    await page.mouse.dblclick(100 + title.x + title.width / 2, scrollY + title.y + title.height / 2);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    title.text = `Edited ${title.text}`;
    title.originalText = title.text;
    title.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), title);
    await textarea.fill(title.text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  }
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+g");
  await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.length === 1));
  const group = await page.evaluate(() => window.editor.getSceneElements()[0].groupIds[0]);
  for (const e of expected) e.groupIds = [group];
  const saved = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(saved));
  await writeFile(path.join(results, "steps.raw.excalidraw"), json);
  await writeFile(path.join(results, "steps.grouped.excalidraw"), saved);

  // Isolate each path for unambiguous native corner hits, following fills/bands.
  for (const [index, line] of paths.entries()) {
    line.groupIds = [];
    await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [line], appState: {}, files: {} }));
    const [dx, dy] = line.points[2];
    await page.evaluate(({ line, dx, dy }) => window.editor.updateScene({ appState: {
      zoom: { value: 8 }, scrollX: 75 - line.x - dx, scrollY: 50 - line.y - dy,
    } }), { line, dx, dy });
    await page.mouse.click(950, 700);
    await page.keyboard.press("Control+a");
    await page.getByRole("button", { name: "Edit line", exact: true }).click();
    await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
    await page.mouse.move(600, 400);
    await page.mouse.down();
    await page.mouse.move(696, 496, { steps: 8 });
    await page.mouse.up();
    await page.keyboard.press("Escape");
    const edited = await page.evaluate(() => window.checks.save());
    const es = JSON.parse(edited).elements;
    const wanted = structuredClone(line);
    wanted.points[2] = [dx + 12, dy + 12];
    compareScenes([wanted], es);
    assert.equal(es[0].roundness, null);
    compareScenes(es, await load(edited));
    const svg = await page.evaluate(() => window.checks.svg());
    assert.ok(svg.includes('stroke="#1971c2"'));
    assert.ok(!svg.includes("<image"));
    await writeFile(path.join(results, `steps-corner-${index}.edited.excalidraw`), edited);
    await writeFile(path.join(results, `steps-corner-${index}.svg`), svg);
    await page.screenshot({ path: path.join(results, `steps-corner-${index}.png`) });
  }
  return { elements: raw.length, paths: 2, dataVertices: 18, maxWidthDrift,
    chartMoves: 2, titleEdits: 2, nativeRegroup: true, exactCornerEdits: 2,
    saveReopen: true, sharpPaths: true, nativeSvg: true };
}
