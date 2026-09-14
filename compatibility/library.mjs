import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

// Extend the generated example and replay this native route as new kinds/metadata
// land. Compare all source fields except identities and insertion translation.
function insertedItem(source, inserted) {
  assert.equal(inserted.length, source.length, "library insertion must not drop content");
  const ids = new Map(source.map((e, i) => [e.id, inserted[i].id]));
  const groups = new Map();
  const dx = inserted[0].x - source[0].x, dy = inserted[0].y - source[0].y;
  for (const [i, raw] of source.entries()) {
    const actual = inserted[i];
    assert.notEqual(actual.id, raw.id);
    assert.equal(actual.frameId, raw.frameId ? ids.get(raw.frameId) : null);
    assert.equal(actual.groupIds.length, raw.groupIds.length);
    for (const [index, group] of raw.groupIds.entries()) {
      if (!groups.has(group)) groups.set(group, actual.groupIds[index]);
      assert.equal(actual.groupIds[index], groups.get(group));
      assert.notEqual(actual.groupIds[index], group);
    }
    for (const field of Object.keys(raw)) {
      if (["id", "groupIds", "frameId", "x", "y", "seed", "version", "versionNonce", "updated"].includes(field)) continue;
      if (field === "index" && raw[field] === null) {
        // Native insertion retains assigned keys for the source's unassigned indices.
        assert.equal(typeof actual.index, "string", "insertion assigns an index");
        if (i > 0) assert.ok(inserted[i - 1].index < actual.index, "insertion indices follow painter order");
        continue;
      }
      assert.deepEqual(actual[field], raw[field], `${raw.type}/${field}`);
    }
    assert.ok(Math.abs(actual.x - raw.x - dx) < 1e-8);
    assert.ok(Math.abs(actual.y - raw.y - dy) < 1e-8);
  }
  assert.equal(new Set(groups.values()).size, groups.size);
}

function independent(instances) {
  const ids = new Set(), groups = new Set();
  for (const elements of instances) {
    const localIds = new Set(elements.map(e => e.id));
    assert.equal(localIds.size, elements.length);
    for (const id of localIds) { assert.ok(!ids.has(id)); ids.add(id); }
    for (const group of new Set(elements.flatMap(e => e.groupIds))) {
      assert.ok(!groups.has(group), "groups must be instance-local"); groups.add(group);
    }
    for (const element of elements) {
      if (element.frameId) assert.equal(elements.find(e => e.id === element.frameId)?.type, "frame");
    }
  }
}

export async function verifyLibrary(page, results) {
  const folder = process.env.LIBRARY_DIR || path.join(import.meta.dirname, "../output/library");
  const scene = () => page.evaluate(() => window.editor.getSceneElements());
  const jsons = await Promise.all(["diagram", "other"].map(name => readFile(path.join(folder, `${name}.excalidrawlib`), "utf8")));
  const items = jsons.map(json => JSON.parse(json).libraryItems[0]);
  await page.evaluate(() => {
    window.editor.resetScene();
    window.editor.updateScene({ appState: { viewBackgroundColor: "#d3f9d8", scrollX: 0, scrollY: 0, zoom: { value: 1 } } });
  });
  for (const [i, json] of jsons.entries()) {
    const imported = await page.evaluate(json => window.checks.importLibrary(json), json);
    const item = imported.find(item => item.id === items[i].id);
    assert.ok(item);
    assert.equal(item.status, "unpublished");
    assert.equal(item.created, items[i].created);
    assert.equal(item.elements.length, items[i].elements.length);
    for (const [index, raw] of items[i].elements.entries()) {
      for (const field of Object.keys(raw)) {
        if (["version", "versionNonce", "updated"].includes(field)) continue;
        if (field === "index" && raw[field] === null) {
          // Shared constructors emit the explicit unassigned-index state. Native
          // library restoration assigns ordered keys without changing array order.
          const assigned = item.elements[index].index;
          assert.equal(typeof assigned, "string", "import assigns an index");
          if (index > 0) assert.ok(item.elements[index - 1].index < assigned, "import indices follow painter order");
          continue;
        }
        assert.deepEqual(item.elements[index][field], raw[field], `import/${field}`);
      }
    }
  }
  assert.equal((await scene()).length, 0, "importing a library does not open a scene");
  const instances = [];
  for (const itemIndex of [0, 0, 1]) {
    await page.evaluate(index => window.editor.updateScene({ appState: { openSidebar: { name: "default", tab: "library" }, selectedElementIds: {}, scrollX: -1600 * index, scrollY: 0 } }), instances.length);
    // Imported items merge newest-first. Click the actual rendered library tile.
    const tile = page.locator(".library-unit__dragger").filter({ has: page.locator("svg") }).nth(itemIndex === 0 ? 1 : 0);
    const before = (await scene()).length;
    await tile.click();
    await page.waitForFunction(n => window.editor.getSceneElements().length === n, before + items[itemIndex].elements.length);
    const inserted = (await scene()).slice(before);
    insertedItem(items[itemIndex].elements, inserted);
    instances.push(inserted);
    await page.keyboard.press("Escape");
  }
  independent(instances);
  const all = await scene();
  assert.equal(all.filter(e => e.type === "frame").length, 5);
  assert.equal(all.filter(e => e.backgroundColor === "#ffffff").length, 5);
  assert.equal(await page.evaluate(() => window.editor.getAppState().viewBackgroundColor), "#d3f9d8");

  // Isolate a frame's name in the viewport and edit via its real input. This
  // avoids changing source geometry or membership to make UI testing convenient.
  const frame = instances[0].find(e => e.type === "frame");
  await page.evaluate(frame => window.editor.updateScene({ appState: { openSidebar: null, scrollX: 350 - frame.x, scrollY: 180 - frame.y, zoom: { value: 1 }, selectedElementIds: {} } }), frame);
  const name = page.locator(`[id$="-frame-name-${frame.id}"]`);
  await name.dblclick();
  const input = name.locator("input");
  await input.fill("Library edit");
  await input.press("Enter");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id)?.name === "Library edit", frame.id);
  let edited = await scene();
  for (const e of all) {
    const actual = edited.find(a => a.id === e.id);
    if (e.id === frame.id) assert.equal(actual.name, "Library edit");
    else assert.deepEqual(actual, e, "editing one instance must leave other content untouched");
  }
  const method = instances[0].find(e => e.text?.includes("\n"));
  assert.ok(method, "library fixture must exercise multiline notes");
  await page.evaluate(e => window.editor.updateScene({ appState: { scrollX: 500 - e.x, scrollY: 300 - e.y, selectedElementIds: {} } }), method);
  // Enter the nested selection using native ungroup commands, retaining frames.
  for (const group of [...method.groupIds].reverse()) {
    await page.keyboard.press("Escape");
    await page.mouse.click(1000, 750);
    await page.mouse.click(510, 312);
    await page.waitForFunction(id => window.editor.getAppState().selectedElementIds[id], method.id);
    await page.keyboard.press("Control+Shift+G");
    await page.waitForFunction(id => window.editor.getSceneElements().every(e => !e.groupIds.includes(id)), group);
  }
  const beforeNoteEdit = await scene();
  await page.keyboard.press("Escape");
  await page.mouse.dblclick(510, 312);
  const textarea = page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  await textarea.fill("Method: library edit\nFive-minute windows.");
  await textarea.press("Escape");
  await page.waitForFunction(id => window.editor.getSceneElements().find(e => e.id === id)?.text === "Method: library edit\nFive-minute windows.", method.id);
  edited = await scene();
  for (const e of beforeNoteEdit) {
    const actual = edited.find(a => a.id === e.id);
    if (e.id === method.id) {
      assert.equal(actual.originalText, actual.text);
      assert.equal(actual.height, 50);
      assert.notEqual(actual.width, e.width);
      assert.equal(actual.frameId, e.frameId);
      assert.equal(actual.containerId, null);
    } else assert.deepEqual(actual, e, "editing a note must be isolated to one element");
  }
  const saved = await page.evaluate(() => window.checks.save());
  const reopened = await page.evaluate(json => window.checks.load(json), saved);
  for (const e of edited) {
    const actual = reopened.find(a => a.id === e.id);
    for (const field of ["id", "type", "groupIds", "frameId", "name", "x", "y", "width", "height", "points", "text", "originalText", "lineHeight", "containerId", "autoResize", "backgroundColor", "link", "customData"]) assert.deepEqual(actual[field], e[field]);
  }
  independent(instances.map(elements => elements.map(e => reopened.find(a => a.id === e.id))));
  const linked = reopened.filter(e => e.link);
  assert.equal(linked.length, 4);
  assert.ok(linked.every(e => e.link === "https://example.org/reports/latency#summary" && e.customData.excaliplot.sourceKey === "synthetic-latency"));
  await writeFile(path.join(results, "library.edited.excalidraw"), saved);
  const svg = await page.evaluate(() => window.checks.svg());
  assert.match(svg, /stroke-dasharray/);
  const backgrounds = await page.evaluate(svg => {
    const host = document.createElement("div");
    host.style.cssText = "position:absolute;left:-10000px;top:0";
    host.innerHTML = svg;
    document.body.append(host);
    try {
      return [...host.querySelectorAll('[fill="#ffffff"]')].map(node => {
        const b = node.getBBox();
        return [b.width, b.height];
      });
    } finally { host.remove(); }
  }, svg);
  assert.deepEqual(backgrounds, Array(5).fill([500, 340]), "official SVG retains five finite backgrounds");
  await writeFile(path.join(results, "library.svg"), svg);
  const png = Buffer.from(await page.evaluate(() => window.checks.png()));
  const screenshot = await page.screenshot({ path: path.join(results, "library.canvas.png") });
  const pixels = async (bytes, positions) => page.evaluate(async ({ base64, positions }) => {
    const image = new Image();
    image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.width; canvas.height = image.height;
    const ctx = canvas.getContext("2d");
    ctx.drawImage(image, 0, 0);
    return positions.map(([x, y]) => [...ctx.getImageData(x, y, 1, 1).data]);
  }, { base64: bytes.toString("base64"), positions });
  // Known clean fixture: first background begins at (66.5,66.5) in the
  // padded official export; the outer export margin remains transparent.
  assert.deepEqual(await pixels(png, [[80, 80], [5, 5]]), [[255, 255, 255, 255], [0, 0, 0, 0]]);
  const background = reopened.find(e => e.backgroundColor === "#ffffff");
  assert.deepEqual(await pixels(screenshot, [[background.x + 110, background.y + 110], [200, 200]]), [[255, 255, 255, 255], [211, 249, 216, 255]]);
  await writeFile(path.join(results, "library.export.png"), png);
  return { importedItems: 2, insertions: 3, frames: 5, linkedLabels: linked.length, independentIdentitiesAndGroups: true, isolatedNativeEdit: true, saveReopen: true, coloredCanvas: "#d3f9d8", officialExports: ["svg", "png"] };
}
