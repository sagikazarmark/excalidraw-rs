import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyLinks(page, results) {
  const json = await readFile(process.env.LINKED_FILE || path.join(import.meta.dirname, "../output/linked_chart.excalidraw"), "utf8");
  const raw = JSON.parse(json).elements;
  const label = raw.find(e => e.text === "Source report");
  assert.ok(label?.link);
  assert.equal(raw.filter(e => e.link).length, 1);
  const scene = () => page.evaluate(() => window.editor.getSceneElements());
  const restored = await page.evaluate(json => window.checks.load(json), json);
  assert.equal(restored.length, raw.length);
  for (const [i, e] of restored.entries()) {
    assert.equal(e.link ?? null, raw[i].link ?? null);
    assert.deepEqual(e.customData, raw[i].customData);
  }
  await page.evaluate(() => window.editor.updateScene({ appState: { openSidebar: null, selectedElementIds: {} } }));
  await page.mouse.click(100 + label.x + label.width / 2, 100 + label.y + label.height / 2);
  await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], label.id);
  // Inspect the actual native hyperlink popup, then click its destination while
  // intercepting navigation. The placeholder report needs no external network.
  const anchor = page.locator(`a[href="${label.link}"]`).filter({ visible: true });
  await anchor.first().waitFor();
  assert.equal(await anchor.first().getAttribute("href"), label.link);
  const requests = [];
  await page.context().route("https://example.org/**", route => {
    requests.push(route.request().url());
    return route.fulfill({ contentType: "text/html", body: "Source report fixture" });
  });
  const popupPromise = page.waitForEvent("popup");
  await anchor.first().click();
  const popup = await popupPromise;
  await popup.waitForLoadState();
  assert.equal(popup.url(), label.link);
  assert.deepEqual(requests, [label.link.split("#")[0]]);
  await popup.close();
  await page.context().unroute("https://example.org/**");
  await page.bringToFront();
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(100 + label.x + label.width / 2, 100 + label.y + label.height / 2);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.fill("Edited source report");
  await textarea.press("Escape");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id)?.text === "Edited source report", label.id);
  const saved = await page.evaluate(() => window.checks.save());
  const reopened = await page.evaluate(json => window.checks.load(json), saved);
  const editedLabel = reopened.find(e => e.id === label.id);
  assert.equal(editedLabel.text, "Edited source report");
  assert.equal(editedLabel.link, label.link);
  assert.deepEqual(editedLabel.customData, label.customData);
  await writeFile(path.join(results, "linked.edited.excalidraw"), saved);

  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.mouse.click(950, 700);
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+c");
  await page.waitForFunction(async id => {
    try { return JSON.parse(await navigator.clipboard.readText()).elements.some(e => e.id === id); }
    catch { return false; }
  }, label.id);
  for (let i = 1; i <= 2; i++) {
    await page.mouse.move(800, 600);
    await page.keyboard.press("Control+v");
    await page.waitForFunction(n => window.editor.getSceneElements().length === n, raw.length * (i + 1));
  }
  const duplicates = await scene();
  assert.equal(new Set(duplicates.map(e => e.id)).size, raw.length * 3);
  const groups = reopened.flatMap(e => e.groupIds);
  const originals = new Set(groups);
  const insertedGroups = duplicates.slice(raw.length).flatMap(e => e.groupIds);
  assert.ok(insertedGroups.every(id => !originals.has(id)));
  assert.equal(new Set(insertedGroups).size, originals.size * 2);
  const linked = duplicates.filter(e => e.link);
  assert.equal(linked.length, 3);
  for (const e of linked) {
    assert.equal(e.text, "Edited source report");
    assert.equal(e.link, label.link);
    assert.deepEqual(e.customData, label.customData);
  }
  const duplicateSave = await page.evaluate(() => window.checks.save());
  const duplicateReopen = await page.evaluate(json => window.checks.load(json), duplicateSave);
  for (const e of duplicates) {
    const actual = duplicateReopen.find(a => a.id === e.id);
    for (const field of ["link", "customData", "groupIds", "frameId", "text", "points", "x", "y"]) assert.deepEqual(actual[field], e[field]);
  }
  await writeFile(path.join(results, "linked.duplicates.excalidraw"), duplicateSave);
  await page.screenshot({ path: path.join(results, "linked.png") });
  return { nativeLinkDestination: label.link, editedLinkedLabel: true, saveReopen: true, duplicateInsertions: 2, repeatedSourceKey: label.customData.excaliplot.sourceKey };
}
