// Acceptance begins with the Rust public authoring interface, never a JS corpus.
import assert from "node:assert/strict";
import { runRust } from "./rust.mjs";

export async function authored(page, repo, snapshot) {
  const input = runRust(repo, "authored_diagram", [snapshot ? "snapshot" : "release"]);
  const restored = await page.evaluate(scene => window.oracle.load(scene), input);
  assert.deepEqual(restored.elements.map(e=>e.id), input.elements.map(e=>e.id));
  // The loader may add editor defaults; it must not repair authored relationships,
  // path geometry, content or ordering. Text layout is caller-supplied and is
  // independently exercised below through native editing.
  for (const expected of input.elements) {
    const actual = restored.elements.find(e=>e.id===expected.id);
    for (const key of ["index","isDeleted","boundElements","containerId","startBinding","endBinding","points","originalText","text"]) {
      // Both null and [] denote no reverse bindings; restoration canonicalizes
      // null to []. Compare references rather than treating that as a repair.
      if (key==="boundElements") { assert.deepEqual(actual[key]??[],expected[key]??[]); continue; }
      if (key in expected) assert.deepEqual(actual[key],expected[key],`${expected.id}.${key} changed on load`);
    }
    if (expected.type!=="text") {
      for (const key of ["x","y","width","height"]) assert.equal(actual[key],expected[key],`${expected.id}.${key} changed on load`);
    }
  }
  await page.evaluate(scene=>window.oracle.mount(scene),input);
  await page.waitForFunction(()=>window.editor?.getSceneElements().some(e=>e.id==="connector"));
  await page.evaluate(()=>window.editor.updateScene({appState:{scrollX:0,scrollY:0,zoom:{value:1},selectedElementIds:{left:true}}}));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.left);
  // Focus the canvas without hitting the label; then use a native arrow-key move.
  await page.mouse.click(120,170);
  const before=await page.evaluate(()=>window.oracle.save());
  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(x=>window.editor.getSceneElements().find(e=>e.id==="left").x>x,before.elements.find(e=>e.id==="left").x);
  const moved=await page.evaluate(()=>window.oracle.save());
  const find=(scene,id)=>scene.elements.find(e=>e.id===id);
  assert.ok(find(moved,"left-label").x>find(before,"left-label").x,"label did not follow container");
  const start=scene=>{const e=find(scene,"connector");return [e.x+e.points[0][0],e.y+e.points[0][1]];};
  assert.notDeepEqual(start(moved),start(before),"bound arrow endpoint did not follow container");
  assert.equal(find(moved,"connector").startBinding.elementId,"left");
  assert.equal(find(moved,"connector").endBinding.elementId,"right");
  await page.keyboard.press("Escape");
  await page.evaluate(()=>window.editor.updateScene({appState:{selectedElementIds:{left:true}}}));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.left);
  await page.keyboard.press("Enter");
  const textarea=page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor();
  assert.equal(await textarea.inputValue(),"Input","Rust text replacement reverted to stale originalText");
  await textarea.fill("Edited input");
  await textarea.press("Escape");
  await page.waitForFunction(()=>window.editor.getSceneElements().find(e=>e.id==="left-label").originalText==="Edited input");
  const saved=await page.evaluate(()=>window.oracle.save());
  const reopened=await page.evaluate(scene=>window.oracle.load(scene),saved);
  for (const expected of saved.elements) {
    const actual=find(reopened,expected.id);
    for (const key of ["containerId","boundElements","startBinding","endBinding","originalText","text","points","index"]) {
      assert.deepEqual(actual[key],expected[key],`${expected.id}.${key} changed on reopen`);
    }
  }
  assert.equal(find(reopened,"left-label").originalText,"Edited input");
  return {input,saved,checks:{rustConstruction:true,loadInvariants:true,boundContainerMove:true,arrowEndpointMove:true,rustTextReplacement:true,nativeTextEditSaveReopen:true}};
}
