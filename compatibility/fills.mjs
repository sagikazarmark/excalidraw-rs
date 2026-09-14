import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyFills(page, results, compareScenes, names = ["area", "pie", "donut"]) {
  const report = {};
  for (const name of names) {
    const area = name.startsWith("area");
    const donut = name.startsWith("donut");
    const single = name === "pie-single";
    const directory = name.includes("-")
      ? process.env.FILLED_GALLERY_DIR || path.join(import.meta.dirname,"../output/filled-gallery")
      : process.env.FILLS_DIR || path.join(import.meta.dirname,"../output");
    const json = await readFile(path.join(directory, `${name}.excalidraw`), "utf8");
    const raw = JSON.parse(json).elements;
    compareScenes(raw, await page.evaluate(json => window.checks.load(json), json));
    await page.waitForFunction(id => window.editor.getSceneElements()[0]?.id === id, raw[0].id);
    const savedChart = await page.evaluate(() => window.checks.save());
    compareScenes(raw, JSON.parse(savedChart).elements);
    compareScenes(raw, await page.evaluate(json => window.checks.load(json), savedChart));
    await writeFile(path.join(results, `${name}.saved.excalidraw`), savedChart);
    await writeFile(path.join(results, `${name}.raw.excalidraw`), json);
    await page.screenshot({ path: path.join(results, `${name}.png`) });
    const fills = raw.filter(e => e.type === "line" && e.backgroundColor !== "transparent");
    assert.equal(fills.length, area || single ? 1 : 3);
    // Verify actual legend-group selection before isolating wedges for edits.
    if (!area) {
      await page.mouse.click(950,700);
      await page.keyboard.press("Control+a");
      await page.keyboard.press("Control+Shift+G");
      await page.waitForFunction(() => window.editor.getSceneElements().every(e => e.groupIds.length <= 1));
      await page.keyboard.press("Escape");
      const label = raw.find(e => e.text === (single ? "All" : "Major"));
      await page.mouse.click(100+label.x+label.width/2,100+label.y+label.height/2);
      await page.waitForFunction(() => Object.keys(window.editor.getAppState().selectedElementIds).length === 3);
      const selected = await page.evaluate(() => Object.keys(window.editor.getAppState().selectedElementIds));
      assert.ok(selected.includes(fills[0].id));
      assert.ok(selected.includes(label.id));
    }
    for (const fill of fills) {
      assert.deepEqual(fill.points[0], fill.points.at(-1));
      // Isolate the filled object so the separate area border cannot steal a hit.
      // Exercise the release's real Edit line action, including the closure vertex.
      // A middle outer-arc point avoids ambiguous hits on the narrow wedge's
      // neighboring handles. Also edit a middle inner-arc point for donuts.
      const outerMid = area ? 2 : Math.floor((fill.points.length-1)/(donut ? 4 : 2));
      const indices = donut ? [outerMid, fill.points.length-2-outerMid, 0] : [outerMid, 0];
      for (const index of indices) {
        const [dx,dy] = fill.points[index];
        await page.evaluate(async ({fill,dx,dy}) => {
          await window.checks.load(JSON.stringify({type:"excalidraw",version:2,elements:[{...fill, groupIds:[]}],appState:{},files:{}}));
          window.editor.updateScene({appState:{zoom:{value:8},scrollX:75-fill.x-dx,scrollY:50-fill.y-dy}});
        }, {fill,dx,dy});
        await page.waitForFunction(id => window.editor.getSceneElements().length === 1 && window.editor.getSceneElements()[0].id === id, fill.id);
        await page.mouse.click(950, 700);
        await page.keyboard.press("Control+a");
        await page.getByRole("button", {name:"Edit line",exact:true}).click();
        await page.waitForFunction(() => window.editor.getAppState().editingLinearElement);
        await page.mouse.move(600,400);
        await page.mouse.down();
        await page.mouse.move(696,304,{steps:8});
        await page.mouse.up();
        if (index === 0) {
          const opened = await page.evaluate(() => window.editor.getSceneElements()[0]);
          assert.notDeepEqual(opened.points[0], opened.points.at(-1), "0.18.0 endpoints are independent");
          // Re-close through native point editing, without modifying JSON.
          await page.mouse.move(696,304);
          await page.mouse.down();
          await page.mouse.move(600,400,{steps:8});
          await page.mouse.up();
        }
        await page.keyboard.press("Escape");
        const saved = await page.evaluate(() => window.checks.save());
        const edited = JSON.parse(saved).elements;
        if (index !== 0) assert.notDeepEqual(edited[0].points, fill.points, `${name}: vertex ${index} did not move`);
        assert.deepEqual(edited[0].points[0], edited[0].points.at(-1), `${name}: editing vertex ${index} broke closure`);
        assert.equal(edited[0].backgroundColor, fill.backgroundColor);
        compareScenes(edited, await page.evaluate(json => window.checks.load(json), saved));
        await page.waitForFunction(points => JSON.stringify(window.editor.getSceneElements()[0].points) === JSON.stringify(points), edited[0].points);
        const svg = await page.evaluate(() => window.checks.svg());
        assert.ok(svg.includes(`fill="${fill.backgroundColor}"`), `${name}: native renderer lost the fill`);
        await writeFile(path.join(results, `${name}-${fills.indexOf(fill)}-vertex-${index}.edited.excalidraw`), saved);
      }
    }
    report[name] = {elements:raw.length, fills:fills.length, vertices:fills.reduce((n,e)=>n+e.points.length,0), wholeChartSaveReopen:true, interiorEdits:true, independentEndpointsReclosed:true, nativeSvgFill:true, saveReopen:true};
  }
  return report;
}
