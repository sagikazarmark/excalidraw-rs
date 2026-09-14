import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

function references(elements) {
  const ids = new Set(elements.map(e => e.id));
  assert.equal(ids.size, elements.length);
  for (const element of elements) {
    if (element.frameId) {
      assert.equal(elements.find(e => e.id === element.frameId)?.type, "frame");
      assert.ok(elements.findIndex(e => e.id === element.frameId) > elements.indexOf(element));
    }
  }
}

export async function verifyComposition(page, results) {
  const folder = process.env.DECORATIONS_DIR || path.join(import.meta.dirname, "../output/decorations");
  const docs = {};
  const scene = () => page.evaluate(() => window.editor.getSceneElements());
  const load = async json => {
    const restored = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, restored[0].id);
    return restored;
  };
  for (const name of ["solid", "dashed", "dotted", "border-assembly", "frame-single", "frame-assembly", "border-in-frame", "frame-siblings", "border-around-frames"]) {
    const json = await readFile(path.join(folder, `${name}.excalidraw`), "utf8");
    docs[name] = json;
    const raw = JSON.parse(json).elements;
    const restored = await load(json);
    assert.equal(restored.length, raw.length);
    for (const [i, e] of restored.entries()) {
      for (const field of ["id", "type", "groupIds", "frameId", "name", "x", "y", "width", "height", "points", "strokeStyle", "strokeWidth", "roughness", "seed"]) {
        assert.deepEqual(e[field], raw[i][field], `${name}/${field}`);
      }
    }
    references(restored);
    // Real forced remeasurement: a deliberately wrong width must be repaired.
    const text = restored.find(e => e.type === "text");
    const repaired = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999 }])[0], text);
    assert.ok(Math.abs(repaired.width - text.width) < 1);
    assert.notEqual(repaired.width, 9999);
    const svg = await page.evaluate(() => window.checks.svg());
    if (["dashed", "dotted", "border-around-frames"].includes(name)) assert.match(svg, /stroke-dasharray/);
    await writeFile(path.join(results, `${name}.svg`), svg);
    for (const frame of restored.filter(e => e.type === "frame")) {
      const svg = await page.evaluate(id => window.checks.frameSvg(id), frame.id);
      assert.match(svg, /clipPath/);
      assert.match(svg, /Latency/);
      const dimensions = await page.evaluate(svg => {
        const node = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
        return [Number(node.getAttribute("width")), Number(node.getAttribute("height"))];
      }, svg);
      assert.deepEqual(dimensions, [frame.width, frame.height]);
      if (name === "border-in-frame") {
        // Measure the actual exported border path, not just the SVG envelope.
        const strokeBounds = await page.evaluate(svg => {
          const host = document.createElement("div");
          host.style.cssText = "position:absolute;left:-10000px;top:0";
          host.innerHTML = svg;
          document.body.append(host);
          try {
            const path = host.querySelector('[stroke-width="8"]');
            if (!path) return null;
            const box = path.getBBox(), matrix = path.getCTM();
            const a = new DOMPoint(box.x, box.y).matrixTransform(matrix);
            const b = new DOMPoint(box.x + box.width, box.y + box.height).matrixTransform(matrix);
            return { left: a.x - 4, top: a.y - 4, right: b.x + 4, bottom: b.y + 4 };
          } finally { host.remove(); }
        }, svg);
        assert.ok(strokeBounds, "per-frame export must contain the complete border stroke");
        assert.deepEqual(strokeBounds, { left: 16, top: 16, right: frame.width - 16, bottom: frame.height - 16 });
      }
      await writeFile(path.join(results, `${name}-${frame.id}.svg`), svg);
    }
    const saved = await page.evaluate(() => window.checks.save());
    const reopened = await load(saved);
    for (const [i, e] of reopened.entries()) {
      for (const field of ["id", "groupIds", "frameId", "name", "strokeStyle", "strokeWidth"]) assert.deepEqual(e[field], restored[i][field]);
    }
    await writeFile(path.join(results, `${name}.excalidraw`), saved);
    await page.screenshot({ path: path.join(results, `${name}.png`) });
  }

  // Border shell moves together; removing it exposes intact chart and series groups.
  const bordered = await load(docs.dashed);
  const border = bordered.at(-1);
  await page.mouse.click(100 + border.x + border.width / 2, 100 + border.y);
  await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, bordered.length);
  await page.keyboard.press("ArrowRight");
  assert.ok((await scene()).every((e, i) => e.x === bordered[i].x + 1));
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), border.groupIds[0]);
  await page.keyboard.press("Escape");
  const title = bordered.find(e => e.text === "Latency");
  await page.mouse.click(100 + title.x + title.width / 2, 100 + title.y + title.height / 2);
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), title.groupIds[0]);
  await page.keyboard.press("Escape");
  const measured = bordered.find(e => e.text === "Measured");
  await page.mouse.click(100 + measured.x + measured.width / 2, 100 + measured.y + measured.height / 2);
  await page.waitForFunction(() => Object.keys(window.editor.getAppState().selectedElementIds).length === 3);
  await page.keyboard.press("ArrowDown");
  const editedBorder = await scene();
  assert.equal(editedBorder.filter((e, i) => e.y === bordered[i].y + 1).length, 3);
  const savedBorder = await page.evaluate(() => window.checks.save());
  const reopenedBorder = await load(savedBorder);
  for (const [i, e] of reopenedBorder.entries()) {
    for (const field of ["x", "y", "groupIds", "strokeStyle", "strokeWidth"]) assert.deepEqual(e[field], editedBorder[i][field]);
  }
  await writeFile(path.join(results, "border.edited.excalidraw"), savedBorder);

  // Combined outer border + sibling frames move once as a single native group.
  const combined = await load(docs["border-around-frames"]);
  await page.evaluate(() => window.editor.updateScene({ appState: { scrollX: 320 } }));
  const outer = combined.at(-1);
  await page.mouse.click(320 + outer.x + 300, 100 + outer.y);
  await page.waitForFunction(n => Object.keys(window.editor.getAppState().selectedElementIds).length === n, combined.length);
  await page.keyboard.press("ArrowDown");
  for (const [i, e] of (await scene()).entries()) {
    assert.equal(e.y, combined[i].y + 1);
    assert.equal(e.frameId, combined[i].frameId);
  }
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), outer.groupIds[0]);
  await page.keyboard.press("Escape");
  const combinedTitle = combined.find(e => e.text === "Latency");
  await page.mouse.click(900, 650);
  await page.waitForFunction(() => !Object.values(window.editor.getAppState().selectedElementIds).some(Boolean));
  await page.mouse.click(320 + combinedTitle.x + combinedTitle.width / 2, 101 + combinedTitle.y + combinedTitle.height / 2);
  await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], combinedTitle.id);
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), combinedTitle.groupIds[0]);
  await page.keyboard.press("Escape");
  const combinedLabel = combined.find(e => e.text === "Measured");
  await page.mouse.click(320 + combinedLabel.x + combinedLabel.width / 2, 101 + combinedLabel.y + combinedLabel.height / 2);
  await page.waitForFunction(() => Object.keys(window.editor.getAppState().selectedElementIds).length === 3);
  await page.keyboard.press("ArrowDown");
  for (const [i, e] of (await scene()).entries()) assert.equal(e.frameId, combined[i].frameId);
  references(await scene());
  const combinedEdited = await page.evaluate(() => window.checks.save());
  references(await load(combinedEdited));
  await writeFile(path.join(results, "border-around-frames.edited.excalidraw"), combinedEdited);

  // Duplicate actual bordered selections, including each frame combination.
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  for (const [fixture, json] of [["border", docs.dashed], ["border-edited", savedBorder], ["border-in-frame", docs["border-in-frame"]], ["border-around-frames", combinedEdited]]) {
    const original = await load(json);
    await page.mouse.click(900, 650);
    await page.keyboard.press("Control+a");
    await page.keyboard.press("Control+c");
    await page.waitForFunction(async id => {
      try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id === id; } catch { return false; }
    }, original[0].id);
    await page.mouse.click(900, 650);
    await page.keyboard.press("Control+v");
    await page.waitForFunction(n => window.editor.getSceneElements().length === n, original.length * 2);
    const duplicated = await scene();
    references(duplicated);
    assert.equal(new Set(duplicated.flatMap(e => e.groupIds)).size, new Set(original.flatMap(e => e.groupIds)).size * 2);
    const borderStyle = e => [e.type, e.strokeStyle, e.strokeWidth, e.groupIds.length];
    assert.deepEqual(duplicated.slice(original.length).map(borderStyle), original.map(borderStyle));
    const saved = await page.evaluate(() => window.checks.save());
    const reopened = await load(saved);
    references(reopened);
    for (const [i, e] of reopened.entries()) {
      for (const field of ["groupIds", "frameId", "strokeStyle", "strokeWidth"]) assert.deepEqual(e[field], duplicated[i][field]);
    }
    await writeFile(path.join(results, `${fixture}.duplicated.excalidraw`), saved);
  }

  // A native frame selection moves its children, leaving its sibling untouched.
  const siblings = await load(docs["frame-siblings"]);
  // Keep the frame name clear of the editor's left-side properties panel.
  await page.evaluate(() => window.editor.updateScene({ appState: { scrollX: 320 } }));
  const frame = siblings.find(e => e.type === "frame");
  const name = page.locator(`[id$="-frame-name-${frame.id}"]`);
  await name.click();
  await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], frame.id);
  await page.keyboard.press("ArrowRight");
  const moved = await scene();
  for (const [i, e] of moved.entries()) assert.equal(e.x, siblings[i].x + (e.id === frame.id || e.frameId === frame.id ? 1 : 0));
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("Escape");
  await name.dblclick();
  const input = name.locator("input");
  await input.fill("Renamed panel");
  await input.press("Enter");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id)?.name === "Renamed panel", frame.id);
  // Enter and edit a child's native text without losing its membership.
  const child = siblings.find(e => e.frameId === frame.id && e.text === "Latency");
  await page.keyboard.press("Escape");
  await page.mouse.click(320 + child.x + child.width / 2, 100 + child.y + child.height / 2);
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), child.groupIds[0]);
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(320 + child.x + child.width / 2, 100 + child.y + child.height / 2);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  await textarea.fill("Edited latency");
  await textarea.press("Escape");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id)?.text === "Edited latency", child.id);
  assert.equal((await scene()).find(e => e.id === child.id).frameId, frame.id);
  const edited = await page.evaluate(() => window.checks.save());
  await load(edited);
  assert.equal((await scene()).find(e => e.id === frame.id).name, "Renamed panel");
  assert.equal((await scene()).find(e => e.id === child.id).text, "Edited latency");
  await writeFile(path.join(results, "frames.edited.excalidraw"), edited);

  // Actual clipboard insertion twice: the editor remaps groups and frame references.
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.mouse.click(900, 650);
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+c");
  await page.waitForFunction(async id => {
    try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id === id; } catch { return false; }
  }, siblings[0].id);
  const destination = await page.context().newPage();
  await destination.goto("http://127.0.0.1:5173");
  await destination.waitForFunction(() => window.editor && window.checks);
  for (let index = 0; index < 2; index++) {
    await destination.mouse.click(450, 250 + 350 * index);
    await destination.keyboard.press("Control+v");
    await destination.waitForFunction(n => window.editor.getSceneElements().length === n, siblings.length * (index + 1));
  }
  const pasted = await destination.evaluate(() => window.editor.getSceneElements());
  references(pasted);
  const frames = pasted.filter(e => e.type === "frame");
  assert.equal(frames.length, 4);
  for (const f of frames) assert.equal(pasted.filter(e => e.frameId === f.id).length, siblings.filter(e => e.frameId === frame.id).length);
  const originalGroups = new Set(JSON.parse(edited).elements.flatMap(e => e.groupIds));
  const groups = new Set(pasted.flatMap(e => e.groupIds));
  assert.equal(groups.size, originalGroups.size * 2);
  // Delete the current native selection, undo, then check all references again.
  await destination.keyboard.press("Delete");
  await destination.waitForFunction(n => window.editor.getSceneElements().length < n, pasted.length);
  references(await destination.evaluate(() => window.editor.getSceneElements()));
  await destination.keyboard.press("Control+z");
  await destination.waitForFunction(n => window.editor.getSceneElements().length === n, pasted.length);
  references(await destination.evaluate(() => window.editor.getSceneElements()));
  const saved = await destination.evaluate(() => window.checks.save());
  const reopened = await destination.evaluate(json => window.checks.load(json), saved);
  references(reopened);
  await writeFile(path.join(results, "frames.duplicated.excalidraw"), saved);
  await destination.close();

  // Composition-only example retains independent chart selections.
  const composedJson = await readFile(process.env.COMPOSITION_FILE || path.join(import.meta.dirname, "../output/composition.excalidraw"), "utf8");
  const composed = await load(composedJson);
  const heading = composed.find(e => e.text === "Latency");
  await page.mouse.click(100 + heading.x + heading.width / 2, 100 + heading.y + heading.height / 2);
  await page.keyboard.press("ArrowRight");
  const shifted = await scene();
  for (const [i, e] of shifted.entries()) assert.equal(e.x, composed[i].x + (e.groupIds.includes(heading.groupIds[0]) ? 1 : 0));
  return { fixtures: Object.keys(docs).length, duplicatedFrames: frames.length, forcedTextRefresh: true, nativeFrameEdits: true };
}
