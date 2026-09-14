import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyUnits(page, results, compareScenes) {
  const load = async json => {
    const restored = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, restored[0].id);
    return restored;
  };
  let maxWidthDrift = 0, maxPositionDrift = 0, textEdits = 0;
  const geometry = (actual, expected) => {
    for (const field of ["x", "y", "width", "height"]) {
      const drift = Math.abs(actual[field] - expected[field]);
      assert.ok(drift <= 1, `${expected.text}/${field}: ${drift}`);
      if (field === "width") maxWidthDrift = Math.max(maxWidthDrift, drift);
      else maxPositionDrift = Math.max(maxPositionDrift, drift);
    }
    for (const field of ["id", "type", "text", "originalText", "fontFamily", "fontSize", "lineHeight", "angle", "textAlign", "verticalAlign", "groupIds", "frameId", "containerId", "autoResize", "strokeColor", "backgroundColor", "opacity", "roughness", "fillStyle"]) {
      assert.deepEqual(actual[field], expected[field], `${expected.text}/${field}`);
    }
  };
  const gallery = process.env.GALLERY_DIR || path.join(import.meta.dirname, "../output/gallery");
  const corpus = JSON.parse(await readFile(path.join(gallery, "units-corpus.excalidraw"), "utf8")).elements;
  assert.deepEqual(corpus.map(e => e.text), ["°", "±", "20°C", "20 ± 2 °C", "−5.0°C ±0.2", "Café: 20°C (±2)", " ° ± ", "°±°"]);
  const json = await readFile(process.env.UNITS_FILE || path.join(import.meta.dirname, "../output/units.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const labels = raw.filter(e => e.text?.match(/[°±]/));
  assert.equal(labels.length, 3); // Title, rotated Y description, multiline note.
  assert.ok(labels.some(e => e.angle !== 0));
  for (const label of [...corpus, ...labels]) {
    const restored = await load(JSON.stringify({ type: "excalidraw", version: 2, elements: [label], appState: {}, files: {} }));
    geometry(restored[0], label);
    // Corrupt dimensions on an unrotated copy: the editor compensates origins
    // when resizing rotated text, so a corrupt rotated box has a different anchor.
    const unrotated = { ...label, angle: 0 };
    const refreshed = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], unrotated);
    assert.notEqual(refreshed.width, 9999);
    assert.notEqual(refreshed.height, 9999);
    geometry(refreshed, unrotated);
    geometry(await page.evaluate(e => window.checks.refresh([e])[0], label), label);
    const width = await page.evaluate(e => Math.max(...e.text.split("\n").map(line => window.checks.measure(line || " ", e.fontSize))), label);
    assert.ok(Math.abs(width - label.width) <= 1);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(width - label.width));
    await page.evaluate(e => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX: 500 - e.x - e.width / 2, scrollY: 300 - e.y - e.height / 2 } }), label);
    await page.waitForFunction(e => window.editor.getAppState().scrollX === 500 - e.x - e.width / 2 && window.editor.getAppState().scrollY === 300 - e.y - e.height / 2, label);
    await page.mouse.dblclick(500, 300);
    const textarea = page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    assert.equal(await textarea.inputValue(), label.text);
    await textarea.fill(label.text + "x");
    await textarea.fill(label.text);
    await textarea.press("Escape");
    await textarea.waitFor({ state: "detached" });
    const saved = await page.evaluate(() => window.checks.save());
    geometry((await load(saved))[0], label);
    await writeFile(path.join(results, `units-label-${textEdits++}.excalidraw`), saved);
  }
  const restored = await load(json);
  compareScenes(raw, restored);
  for (const label of labels) geometry(restored.find(e => e.id === label.id), label);
  const note = labels.find(e => e.text.includes("\n"));
  await page.evaluate(e => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX: 500 - e.x, scrollY: 250 - e.y } }), note);
  await page.waitForFunction(y => window.editor.getAppState().scrollY === y, 250 - note.y);
  await page.mouse.dblclick(510, 262);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  const editedText = "Method: edited samples\nUncertainty: ±1°C; interval: 2 s.";
  await textarea.fill(editedText);
  await textarea.press("Escape");
  await textarea.waitFor({ state: "detached" });
  const saved = await page.evaluate(() => window.checks.save());
  const edited = JSON.parse(saved).elements.find(e => e.id === note.id);
  assert.equal(edited.text, editedText);
  assert.equal(edited.originalText, editedText);
  assert.equal(edited.fontFamily, 5);
  assert.equal(edited.height, 50);
  assert.ok(Math.abs(edited.x - note.x) <= 1 && Math.abs(edited.y - note.y) <= 1);
  const width = await page.evaluate(e => Math.max(...e.text.split("\n").map(line => window.checks.measure(line, e.fontSize))), edited);
  geometry(edited, { ...note, text: editedText, originalText: editedText, width, height: 50 });
  const reopened = await load(saved);
  compareScenes(raw.map(e => e.id === note.id ? { ...note, text: editedText, originalText: editedText, width, height: 50 } : e), reopened);
  geometry(reopened.find(e => e.id === note.id), edited);
  for (const label of labels.filter(e => e.id !== note.id)) geometry(reopened.find(e => e.id === label.id), label);
  await writeFile(path.join(results, "units.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "units.edited.png") });
  return { corpus: corpus.length, textEdits: textEdits + 1, maxWidthDrift, maxPositionDrift, forcedWrongDimensionsRepaired: true };
}
