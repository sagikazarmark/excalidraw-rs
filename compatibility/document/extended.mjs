import assert from "node:assert/strict";
import {runRust} from "./rust.mjs";
import {corpus,relationshipCorpus} from "./corpus.mjs";

export async function extended(page,repo,snapshot,record){
  const report={};
  const mount=async doc=>{
    await record("input",doc);
    await page.evaluate(doc=>window.oracle.mount(doc),doc);
    await page.waitForFunction(()=>window.editor?.getSceneElements().length);
    await page.evaluate(()=>window.editor.updateScene({appState:{scrollX:0,scrollY:0,zoom:{value:1}}}));
    await page.mouse.click(1000,700);
  };
  const get=async id=>page.evaluate(id=>window.editor.getSceneElements().find(e=>e.id===id),id);
  const saveReopen=async()=>{
    const saved=await page.evaluate(()=>window.oracle.save());
    await record("saved",saved);
    const read=await page.evaluate(d=>window.oracle.load(d),saved);
    await record("reopened",read);
    assert.deepEqual(read.elements.map(e=>e.id),saved.elements.map(e=>e.id));return read;
  };
  const single=(kind)=>{
    const doc=corpus(snapshot);doc.elements=[{...doc.elements.find(e=>e.type===kind),id:"subject",x:350,y:250,width:160,height:120,index:"a0"}];return doc;
  };
  // Native freehand resizing must alter samples, not replace the stroke with an image.
  const free=single("freedraw");free.elements[0].points=[[0,0],[80,120],[160,0]];
  await mount(free);await page.mouse.click(430,370);
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.subject);
  const before=await get("subject");
  await page.mouse.move(510,370);await page.mouse.down();await page.mouse.move(550,410,{steps:6});await page.mouse.up();
  await page.waitForFunction(p=>JSON.stringify(window.editor.getSceneElements().find(e=>e.id==="subject").points)!==p,JSON.stringify(before.points));
  const freeSaved=(await saveReopen()).elements.find(e=>e.id==="subject");
  assert.equal(freeSaved.type,"freedraw");assert.deepEqual(freeSaved.pressures,before.pressures);report.freehandResize=true;

  // Image flip through native shortcut; crop via native image editor controls.
  const image=single("image");image.elements[0].scale=[1,1];
  await mount(image);await page.mouse.click(430,310);await page.keyboard.press("Shift+H");
  await page.waitForFunction(()=>window.editor.getSceneElements().find(e=>e.id==="subject").scale[0]===-1);
  await page.mouse.dblclick(430,310);
  await page.waitForFunction(()=>window.editor.getAppState().croppingElementId==="subject");
  await page.mouse.move(350,310);await page.mouse.down();await page.mouse.move(390,310,{steps:6});await page.mouse.up();
  await page.keyboard.press("Escape");
  const cropped=await get("subject");assert.ok(cropped.crop);assert.ok(cropped.crop.width<cropped.crop.naturalWidth);
  const imageSaved=await saveReopen();assert.deepEqual(imageSaved.elements[0].crop,cropped.crop);assert.equal(imageSaved.elements[0].scale[0],-1);assert.ok(imageSaved.files.pixel);report.imageCropFlip=true;

  // Sticky-note text grows beyond its base and shrinks after shortening.
  if(snapshot){
    const doc=relationshipCorpus(true);doc.elements=doc.elements.filter(e=>["sticky","sticky-label"].includes(e.id));
    await mount(doc);
    const note=await get("sticky");
    const edit=async text=>{
      const label=await get("sticky-label");await page.mouse.dblclick(label.x+label.width/2,label.y+label.height/2);
      const textarea=page.locator("textarea.excalidraw-wysiwyg");await textarea.waitFor();await textarea.fill(text);await textarea.press("Escape");
      await page.waitForFunction(text=>window.editor.getSceneElements().find(e=>e.id==="sticky-label").originalText===text,text);
    };
    await edit(Array.from({length:20},(_,i)=>`Line ${i}`).join("\n"));
    const grown=await get("sticky");assert.ok(grown.height>note.height);
    await edit("Short note");const shrunk=await get("sticky");assert.ok(shrunk.height<grown.height);assert.ok(shrunk.height>=shrunk.baseHeight);
    await saveReopen();report.stickyGrowShrink=true;
  }

  // Routing triggered by moving an elbow's bound target; segment data survives.
  const routed=relationshipCorpus(snapshot);routed.elements=routed.elements.filter(e=>["target","elbow"].includes(e.id));
  await mount(routed);await page.mouse.click(750,440);await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.target);
  const elbow=await get("elbow");await page.keyboard.press("Shift+ArrowRight");
  await page.waitForFunction(p=>JSON.stringify(window.editor.getSceneElements().find(e=>e.id==="elbow").points)!==p,JSON.stringify(elbow.points));
  const routedSaved=(await saveReopen()).elements.find(e=>e.id==="elbow");assert.equal(routedSaved.elbowed,true);assert.equal(routedSaved.endBinding.elementId,"target");report.elbowTargetRouting=true;

  // Rust -> official native loader, and official image exports -> Rust extraction.
  const doc=single("rectangle");doc.elements[0].backgroundColor="#aabbcc";doc.elements[0].customData={text:"Unicode β é"};doc.files={};
  const bridge=request=>runRust(repo, "embedded", [], request);
  const embedded=bridge({operation:"embed",document:doc,png:"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",svg:'<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>'});
  await record("rust-embedded",embedded);
  const loaded=await page.evaluate(v=>window.oracle.loadEmbedded(v),embedded);
  await record("editor-extracted",loaded);
  for(const key of ["png","svg"])assert.deepEqual(loaded[key].elements[0].customData,doc.elements[0].customData);
  await mount(doc);const exported=await page.evaluate(()=>window.oracle.exportEmbedded());
  await record("editor-embedded",exported);
  const extracted=bridge({operation:"extract",...exported});
  await record("rust-extracted",extracted);
  for(const key of ["png","svg"])assert.deepEqual(extracted[key].elements[0].customData,doc.elements[0].customData);
  report.embeddedBidirectional=true;return report;
}
