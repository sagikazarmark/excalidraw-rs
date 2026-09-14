import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyNotes(page, results) {
  const scene = () => page.evaluate(() => window.editor.getSceneElements());
  const load = async json => {
    const restored = await page.evaluate(json => window.checks.load(json), json);
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, restored[0].id);
    return restored;
  };
  const geometry = (actual, expected) => {
    for (const field of ["x", "y", "width", "height"]) {
      assert.ok(Math.abs(actual[field] - expected[field]) <= 1, `${expected.text}/${field}: ${actual[field]} vs ${expected[field]}`);
    }
    for (const field of ["text", "originalText", "fontFamily", "fontSize", "lineHeight", "angle", "containerId", "autoResize", "groupIds", "frameId"]) {
      assert.deepEqual(actual[field], expected[field], `${expected.text}/${field}`);
    }
  };
  const corpus = JSON.parse(await readFile(process.env.NOTE_GALLERY_FILE || path.join(import.meta.dirname, "../output/note_gallery.excalidraw"), "utf8")).elements;
  assert.equal(corpus.length, 8);
  let textEdits = 0, maxWidthDrift = 0;
  for (const note of corpus) {
    console.log("Note corpus", JSON.stringify(note.text));
    const json = JSON.stringify({ type: "excalidraw", version: 2, elements: [note], appState: {}, files: {} });
    const restored = await load(json);
    assert.equal(restored.length, 1);
    geometry(restored[0], note);
    // The actual native refresh path must repair deliberately corrupt dimensions.
    const refreshed = await page.evaluate(e => window.checks.refresh([{ ...e, width: 9999, height: 9999 }])[0], note);
    assert.notEqual(refreshed.width, 9999);
    assert.notEqual(refreshed.height, 9999);
    geometry(refreshed, note);
    maxWidthDrift = Math.max(maxWidthDrift, Math.abs(refreshed.width - note.width));
    const canvasWidth = await page.evaluate(e => Math.max(...e.text.split("\n").map(line => window.checks.measure(line || " ", e.fontSize))), note);
    assert.ok(Math.abs(canvasWidth - note.width) <= 1);
    // Blank notes survive native load/refresh/save. The editor deletes them on
    // commit, so real edit round trips use the visible corpus entries.
    if (note.text.trim()) {
      await page.evaluate(e => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX: 500 - e.x, scrollY: 250 - e.y } }), note);
      await page.waitForFunction(e => window.editor.getAppState().scrollX === 500 - e.x && window.editor.getAppState().scrollY === 250 - e.y, note);
      const line = note.text.split("\n").findIndex(line => line.trim());
      await page.mouse.dblclick(505, 250 + (line + 0.5) * 25);
      const textarea = page.locator("textarea.excalidraw-wysiwyg");
      await textarea.waitFor();
      assert.equal(await textarea.inputValue(), note.text);
      await textarea.fill(note.text + "x");
      await textarea.fill(note.text);
      await textarea.press("Escape");
      await textarea.waitFor({ state: "detached" });
      geometry((await scene())[0], note);
      textEdits++;
    }
    const saved = await page.evaluate(() => window.checks.save());
    geometry((await load(saved))[0], note);
    await writeFile(path.join(results, `note-${textEdits}-${corpus.indexOf(note)}.excalidraw`), saved);
  }

  const json = await readFile(process.env.NOTE_FILE || path.join(import.meta.dirname, "../output/method_note.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const restored = await load(json);
  const note = restored.find(e => e.text?.includes("\n"));
  assert.ok(note);
  assert.equal(note.text, "Method: synthetic samples\nOne-minute windows; no smoothing.");
  geometry(note, raw.find(e => e.id === note.id));
  const frame = restored.find(e => e.id === note.frameId);
  assert.equal(frame.type, "frame");
  assert.ok(note.y + note.height < frame.y + frame.height);
  const refreshed = await page.evaluate(elements => window.checks.refresh(elements.map(e => e.text?.includes("\n") ? { ...e, width: 9999 } : e)), restored);
  geometry(refreshed.find(e => e.id === note.id), note);
  const svg = await page.evaluate(id => window.checks.frameSvg(id), frame.id);
  assert.match(svg, /Method: synthetic samples/);
  assert.match(svg, /One-minute windows; no smoothing\./);
  await writeFile(path.join(results, "method-note.svg"), svg);
  await page.evaluate(frame => window.editor.updateScene({ appState: { selectedElementIds: {}, scrollX: 350 - frame.x, scrollY: 100 - frame.y } }), frame);
  await page.locator(`[id$="-frame-name-${frame.id}"]`).click();
  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(({ id, x }) => window.editor.getSceneElements().find(e => e.id === id)?.x === x + 1, frame);
  for (const [i, e] of (await scene()).entries()) assert.equal(e.x, restored[i].x + 1);
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("Escape");
  // Remove the outer border selection group through the actual editor, then edit
  // the free text inside its native frame without changing frame membership.
  await page.mouse.click(350 + note.x + 10, 100 + note.y + 12);
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), note.groupIds.at(-1));
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(350 + note.x + 10, 100 + note.y + 12);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  const editedText = "Method: edited samples\nFive-minute windows.";
  await textarea.fill(editedText);
  await textarea.press("Escape");
  await page.waitForFunction(({ id, text }) => window.editor.getSceneElements().find(e => e.id === id)?.text === text, { id: note.id, text: editedText });
  const edited = (await scene()).find(e => e.id === note.id);
  assert.equal(edited.originalText, editedText);
  assert.equal(edited.height, 50);
  assert.notEqual(edited.width, note.width);
  assert.equal(edited.frameId, frame.id);
  assert.equal(edited.containerId, null);
  const saved = await page.evaluate(() => window.checks.save());
  geometry((await load(saved)).find(e => e.id === note.id), edited);
  await writeFile(path.join(results, "method-note.edited.excalidraw"), saved);
  await page.screenshot({ path: path.join(results, "method-note.edited.png") });
  return { corpus: corpus.length, textEdits: textEdits + 1, maxWidthDrift, forcedWrongDimensionsRepaired: true, composedFrameMovement: true };
}
