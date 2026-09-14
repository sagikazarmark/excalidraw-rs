import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection } from "./interactions.mjs";

export async function verifyBands(page, results, compareScenes) {
  const json = await readFile(process.env.BANDS_FILE || path.join(import.meta.dirname, "../output/bands.excalidraw"), "utf8");
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
  const expected = structuredClone(raw);
  const fill = expected.find(e => e.type === "line" && e.backgroundColor === "#1971c2");
  const paths = expected.filter(e => e.type === "line" && e.groupIds.length === 2);
  assert.equal(paths.length, 4);
  assert.deepEqual(paths.map(e => e.points.length), [13, 6, 6, 6]);
  assert.deepEqual(paths.map(e => e.opacity), [35, 100, 100, 100]);
  assert.deepEqual(fill.points[0], fill.points.at(-1));
  const title = expected.find(e => e.text === "Ordered supplied envelope");
  const legend = expected.find(e => e.text === "Supplied range");
  const click = async e => {
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + e.x + e.width / 2, 100 + e.y + e.height / 2);
  };
  const ungroup = async id => {
    await page.keyboard.press("Control+Shift+G");
    for (const e of expected) e.groupIds = e.groupIds.filter(g => g !== id);
    await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), id);
  };
  await click(title);
  await waitForSelection(page, expected.map(e => e.id));
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(x => window.editor.getSceneElements()[0].x === x, expected[0].x);
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await ungroup(title.groupIds.at(-1));
  await click(legend);
  await waitForSelection(page, expected.filter(e => e.groupIds.includes(legend.groupIds[0])).map(e => e.id));
  await ungroup(legend.groupIds[0]);
  // Pick inside the envelope, away from its independent center and boundaries.
  await page.keyboard.press("Escape");
  await page.mouse.click(100 + fill.x + 12, 100 + fill.y + 12);
  await waitForSelection(page, [fill.id]);
  await page.getByRole("button", { name: "Background", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Background", exact: true });
  await input.fill("b2f2bb");
  await input.press("Enter");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id).backgroundColor === "#b2f2bb", fill.id);
  await page.mouse.click(1050, 600);
  fill.backgroundColor = "#b2f2bb";
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(100 + title.x + title.width / 2, 100 + title.y + title.height / 2);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  title.text = "Edited supplied envelope";
  title.originalText = title.text;
  title.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), title);
  await textarea.fill(title.text);
  await textarea.press("Escape");
  await textarea.waitFor({ state: "detached" });
  compareScenes(expected, await page.evaluate(() => window.editor.getSceneElements()));
  const whole = await page.evaluate(() => window.checks.save());
  compareScenes(expected, await load(whole));
  await writeFile(path.join(results, "bands.raw.excalidraw"), json);
  await writeFile(path.join(results, "bands.edited.excalidraw"), whole);
  await page.screenshot({ path: path.join(results, "bands.edited.png") });

  // Isolate the fill for point hits, as in fills.mjs. Separate paths are not bound
  // to it and are deliberately excluded from any follow-the-fill editing claim.
  for (const index of [2, 9, 0]) {
    const [dx, dy] = fill.points[index];
    await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [fill], appState: {}, files: {} }));
    await page.evaluate(({ fill, dx, dy }) => window.editor.updateScene({ appState: {
      zoom: { value: 8 }, scrollX: 75 - fill.x - dx, scrollY: 50 - fill.y - dy,
    } }), { fill, dx, dy });
    await page.mouse.click(950, 700);
    await page.keyboard.press("Control+a");
    await page.getByRole("button", { name: "Edit line", exact: true }).click();
    await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
    await page.mouse.move(600, 400);
    await page.mouse.down();
    await page.mouse.move(696, 304, { steps: 8 });
    await page.mouse.up();
    if (index === 0) {
      const opened = await page.evaluate(() => window.editor.getSceneElements()[0]);
      assert.notDeepEqual(opened.points[0], opened.points.at(-1));
      await page.mouse.move(696, 304);
      await page.mouse.down();
      await page.mouse.move(600, 400, { steps: 8 });
      await page.mouse.up();
    }
    await page.keyboard.press("Escape");
    const saved = await page.evaluate(() => window.checks.save());
    const edited = JSON.parse(saved).elements;
    const wanted = structuredClone(fill);
    if (index !== 0) wanted.points[index] = [dx + 12, dy - 12];
    compareScenes([wanted], edited);
    assert.deepEqual(edited[0].points[0], edited[0].points.at(-1));
    assert.equal(edited[0].backgroundColor, "#b2f2bb");
    compareScenes(edited, await load(saved));
    await page.waitForFunction(points => JSON.stringify(window.editor.getSceneElements()[0].points) === JSON.stringify(points), edited[0].points);
    const svg = await page.evaluate(() => window.checks.svg());
    assert.ok(svg.includes('fill="#b2f2bb"'));
    await writeFile(path.join(results, `bands-vertex-${index}.edited.excalidraw`), saved);
    await writeFile(path.join(results, `bands-vertex-${index}.svg`), svg);
  }
  return { elements: raw.length, dataPaths: 4, dataVertices: 31, samples: 6,
    chartMove: true, legendSelection: true, nativeFillRecolor: true, nativeTextEdits: 1,
    upperLowerPointEdits: true, independentEndpointsReclosed: true,
    nativeSvgFill: true, saveReopen: true };
}
