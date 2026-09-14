import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { waitForSelection } from "./interactions.mjs";

export async function verifyCallouts(page, results, compareScenes) {
  const json = await readFile(process.env.CALLOUTS_FILE || path.join(import.meta.dirname, "../output/callouts.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const compare = (expected, actual) => {
    compareScenes(expected, actual);
    for (const e of expected) {
      const a = actual.find(a => a.id === e.id);
      for (const key of ["startBinding", "endBinding", "startArrowhead", "endArrowhead", "elbowed", "containerId", "frameId", "boundElements"]) {
        if (key in e) assert.deepEqual(a[key], e[key], `${e.id}/${key}`);
      }
    }
  };
  const load = async json => {
    const es = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, es[0].id);
    return es;
  };
  compare(raw, await load(json));
  const expected = structuredClone(raw);
  const arrows = expected.filter(e => e.type === "arrow");
  assert.deepEqual(arrows.map(e => e.endArrowhead), ["arrow", "triangle"]);
  const measured = expected.find(e => e.type === "line" && e.points.length === 4);
  for (const [index, arrow] of arrows.entries()) {
    const note = expected[expected.indexOf(arrow) + 1];
    const vertex = measured.points[index + 2];
    assert.equal(arrow.x + arrow.points[1][0], measured.x + vertex[0]);
    assert.equal(arrow.y + arrow.points[1][1], measured.y + vertex[1]);
    assert.deepEqual(arrow.groupIds, note.groupIds);
    assert.equal(note.containerId, null);
    assert.equal(arrow.startBinding, null);
    assert.equal(arrow.endBinding, null);
  }
  // A real native selection moves the complete chart, then the callout pair alone.
  const title = expected.find(e => e.text === "Explaining observations");
  await page.mouse.click(100 + title.x + title.width / 2, 100 + title.y + title.height / 2);
  await waitForSelection(page, expected.map(e => e.id));
  await page.keyboard.press("ArrowRight");
  for (const e of expected) e.x++;
  await page.waitForFunction(x => window.editor.getSceneElements()[0].x === x, expected[0].x);
  compare(expected, await page.evaluate(() => window.editor.getSceneElements()));
  await page.keyboard.press("Control+Shift+G");
  const outer = arrows[0].groupIds.at(-1);
  for (const e of expected) e.groupIds = e.groupIds.filter(id => id !== outer);
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), outer);
  const svgHeads = [];
  for (const arrow of arrows) {
    const note = expected[expected.indexOf(arrow) + 1];
    await page.keyboard.press("Escape");
    await page.mouse.click(100 + note.x + note.width / 2, 100 + note.y + note.height / 2);
    await waitForSelection(page, [arrow.id, note.id]);
    await page.keyboard.press("ArrowDown");
    arrow.y++; note.y++;
    await page.waitForFunction(e => window.editor.getSceneElements().find(a => a.id === e.id).y === e.y, note);
    compare(expected, await page.evaluate(() => window.editor.getSceneElements()));
    await page.keyboard.press("Control+Shift+G");
    arrow.groupIds = []; note.groupIds = [];
    await page.waitForFunction(id => !window.editor.getSceneElements().find(e => e.id === id).groupIds.length, arrow.id);
    await page.keyboard.press("Escape");
    // Edit the note through the actual textarea; the unbound arrow must stay put.
    await page.mouse.dblclick(100 + note.x + note.width / 2, 100 + note.y + note.height / 2);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    note.text += " edited"; note.originalText = note.text;
    note.width = await page.evaluate(e => window.checks.measure(e.text, e.fontSize), note);
    await textarea.fill(note.text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    compare(expected, await page.evaluate(() => window.editor.getSceneElements()));
    await page.keyboard.press("Escape");
    const [dx, dy] = arrow.points[1];
    await page.mouse.click(100 + arrow.x + dx / 2, 100 + arrow.y + dy / 2);
    await waitForSelection(page, [arrow.id]);
    await page.getByRole("button", { name: "Edit arrow", exact: true }).click();
    await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
    await page.keyboard.down("Control");
    await page.mouse.move(100 + arrow.x + dx, 100 + arrow.y + dy);
    await page.mouse.down();
    await page.mouse.move(112 + arrow.x + dx, 88 + arrow.y + dy, { steps: 8 });
    await page.mouse.up();
    // Edit the start too: 0.18.0 may auto-bind the stationary endpoint on
    // entering edit mode. Explicit Ctrl-drags of both endpoints leave both free.
    await page.mouse.move(100 + arrow.x, 100 + arrow.y);
    await page.mouse.down();
    await page.mouse.move(108 + arrow.x, 108 + arrow.y, { steps: 8 });
    await page.mouse.up();
    arrow.x += 8; arrow.y += 8;
    arrow.points[1] = [dx + 4, dy - 20];
    arrow.width = Math.abs(dx + 4); arrow.height = Math.abs(dy - 20);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await page.keyboard.up("Control");
    compare(expected, await page.evaluate(() => window.editor.getSceneElements()));
    const svg = await page.evaluate(id => window.checks.svg([id]), arrow.id);
    const paths = await page.evaluate(svg => [...new DOMParser().parseFromString(svg, "image/svg+xml").querySelectorAll("path")].map(p => ({ d: p.getAttribute("d"), fill: p.getAttribute("fill"), stroke: p.getAttribute("stroke") })), svg);
    // Official renderer must draw a shaft plus two open head legs, or a filled triangle.
    if (arrow.endArrowhead === "arrow") {
      assert.equal(paths.length, 3);
      assert.ok(paths.every(p => p.fill === "none" && p.stroke === arrow.strokeColor));
    } else {
      assert.ok(paths.length >= 2);
      assert.ok(paths.some(p => p.fill === arrow.strokeColor));
    }
    svgHeads.push({ head: arrow.endArrowhead, paths });
    await writeFile(path.join(results, `callout-${arrow.endArrowhead}.svg`), svg);
  }
  const saved = await page.evaluate(() => window.checks.save());
  compare(expected, await load(saved));
  await writeFile(path.join(results, "callouts.raw.excalidraw"), json);
  await writeFile(path.join(results, "callouts.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "callouts.edited.png") });
  // Copy the grouped original twice, retaining intentional callout groups.
  await load(json);
  const destination = await page.context().newPage();
  await destination.goto("http://127.0.0.1:5173");
  await destination.waitForFunction(() => window.editor && window.checks);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.bringToFront();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+a");
  await waitForSelection(page, raw.map(e => e.id));
  await page.keyboard.press("Control+c");
  await page.waitForFunction(async id => {
    try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id === id; }
    catch { return false; }
  }, raw[0].id);
  await destination.bringToFront();
  await destination.evaluate(() => window.editor.updateScene({ appState: { zoom: { value: 0.5 }, scrollX: 20, scrollY: 20 } }));
  for (let i = 0; i < 2; i++) {
    await destination.mouse.click(600, 220 + i * 280);
    await destination.keyboard.press("Control+v");
    await destination.waitForFunction(n => window.editor.getSceneElements().length === n, raw.length * (i + 1));
    await destination.keyboard.press("Escape");
  }
  const composed = await destination.evaluate(() => window.editor.getSceneElements());
  assert.equal(new Set(composed.map(e => e.id)).size, raw.length * 2);
  const seenGroups = new Set();
  for (let i = 0; i < 2; i++) {
    const copy = composed.slice(i * raw.length, (i + 1) * raw.length);
    const dx = copy[0].x - raw[0].x, dy = copy[0].y - raw[0].y;
    const groups = new Map();
    const reference = raw.map((e, j) => ({ ...e, id: copy[j].id, seed: copy[j].seed, x: e.x + dx, y: e.y + dy,
      groupIds: e.groupIds.map((id, k) => {
        const actual = copy[j].groupIds[k];
        assert.notEqual(actual, id);
        if (!groups.has(id)) { assert.ok(!seenGroups.has(actual)); seenGroups.add(actual); groups.set(id, actual); }
        assert.equal(actual, groups.get(id));
        return actual;
      }),
    }));
    compare(reference, copy);
  }
  const composedSave = await destination.evaluate(() => window.checks.save());
  compare(composed, await destination.evaluate(json => window.checks.load(json), composedSave));
  await writeFile(path.join(results, "callouts.composed.excalidraw"), composedSave);
  await destination.close();
  return { nativePairMoves: 2, nativeTextEdits: 2, nativeEndpointEdits: 4, svgHeads, composedCopies: 2, saveReopen: true };
}
