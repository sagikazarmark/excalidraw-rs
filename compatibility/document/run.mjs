// Uses public editor exports in a real browser. Snapshot sources are isolated
// from the registry profile and checked against an immutable commit.
import { createServer } from "vite";
import { chromium } from "playwright-core";
import { execFileSync } from "node:child_process";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import assert from "node:assert/strict";
import { corpus, relationshipCorpus } from "./corpus.mjs";
import { extended } from "./extended.mjs";
import { authored } from "./authored.mjs";
import { authoredGallery } from "./authored-gallery.mjs";
import { runRust } from "./rust.mjs";

const root = import.meta.dirname;
const repo = path.resolve(root, "../..");
const source = process.env.EXCALIDRAW_SOURCE;
const snapshot = !!source;
const sha = "afa3a653fc5d2b742adcbd5a6063187b056d2419";
const releaseVersion=JSON.parse(await readFile(path.join(repo,"compatibility/node_modules/@excalidraw/excalidraw/package.json"),"utf8")).version;
if(!snapshot) assert.equal(releaseVersion,"0.18.1");
const aliases = [];
if (snapshot) {
  assert.equal(execFileSync("git", ["rev-parse", "HEAD"], {cwd:source,encoding:"utf8"}).trim(), sha);
  aliases.push({find:"@excalidraw/excalidraw/index.css",replacement:path.join(source,"packages/excalidraw/fonts/fonts.css")});
  for (const name of ["common","element","math","utils","fractional-indexing","laser-pointer","excalidraw"]) {
    const base = path.join(source,"packages",name,name === "excalidraw" ? "" : "src");
    aliases.push({find:new RegExp(`^@excalidraw/${name}$`),replacement:path.join(base,name === "excalidraw" ? "index.tsx" : "index.ts")});
    aliases.push({find:new RegExp(`^@excalidraw/${name}/`),replacement:`${base}/`});
  }
  for (const name of ["react", "react-dom"]) aliases.push({find:new RegExp(`^${name}(?=/|$)`),replacement:path.join(source,"node_modules",name)});
}
const server = await createServer({root,configFile:false,server:{host:"127.0.0.1",port:5189,strictPort:true,fs:{allow:[repo,...(source?[source]:[])]}},
  resolve:{alias:aliases},define:{"import.meta.env.VITE_DOCUMENT_SNAPSHOT":JSON.stringify(String(snapshot))},
  esbuild:{jsx:"automatic"}, optimizeDeps:{exclude:snapshot?["@excalidraw/excalidraw"]:[]}});
let browser;
try {
  await server.listen();
  browser = await chromium.launch({executablePath:process.env.CHROMIUM_PATH || execFileSync("which",["chromium"],{encoding:"utf8"}).trim(),headless:true});
  const page = await browser.newPage({viewport:{width:1100,height:800}});
  page.on("pageerror", error=>console.error(error));
  // Release EXPORT_SOURCE is captured at import, not at call time.
  await page.addInitScript(()=>{window.EXCALIDRAW_EXPORT_SOURCE="document-oracle";});
  await page.goto("http://127.0.0.1:5189");
  await page.waitForFunction(()=>window.oracle,{},{timeout:120000});
  const input = JSON.parse(await readFile(path.join(repo,"crates/excalidraw-document/tests/fixtures/projection.json"),"utf8"));
  const report = {profile:snapshot?sha:releaseVersion,browser:browser.version(),node:process.version,
    lockHash:createHash("sha256").update(await readFile(snapshot?path.join(source,"yarn.lock"):path.join(repo,"compatibility/package-lock.json"))).digest("hex"), projections:{}};
  for (const mode of ["local","database"]) {
    const actual = await page.evaluate(({input,mode})=>window.oracle.serialize(input,mode),{input,mode});
    const rust = runRust(repo, "project", [snapshot ? "snapshot" : "release", mode], input);
    assert.deepEqual(rust,actual);
    if(mode==="local") assert.deepEqual(actual,JSON.parse(await readFile(path.join(repo,`crates/excalidraw-document/tests/fixtures/${snapshot?"snapshot":"release"}-local.json`),"utf8")));
    report.projections[mode] = actual;
  }
  const scene = {type:"excalidraw",version:2,source:"fixture",appState:{viewBackgroundColor:"#ffffff"},files:{},elements:[
    {id:"box",type:"rectangle",x:200,y:150,width:120,height:80,strokeColor:"#123456",backgroundColor:"#aabbcc",fillStyle:"solid",roughness:0,version:1,versionNonce:0,index:"a0",isDeleted:false,updated:1},
  ]};
  const loaded = await page.evaluate(scene=>window.oracle.load(scene),scene);
  assert.equal(loaded.elements.length,1); assert.equal(loaded.elements[0].id,"box");
  await page.evaluate(scene=>window.oracle.mount(scene),scene);
  await page.waitForFunction(()=>window.editor);
  const saved = await page.evaluate(()=>window.oracle.save());
  const reopened = await page.evaluate(scene=>window.oracle.load(scene),saved);
  assert.equal(reopened.elements[0].strokeColor,"#123456");
  report.loadSaveReopen = true;
  // Each known persisted kind must survive file load and preserving Rust encode.
  const full = corpus(snapshot);
  const roundtrip = runRust(repo, "roundtrip", [], full);
  assert.deepEqual(roundtrip,full);
  const restored = await page.evaluate(scene=>window.oracle.load(scene),roundtrip);
  assert.deepEqual(restored.elements.map(e=>e.type),full.elements.map(e=>e.type));
  assert.deepEqual(restored.files,full.files);
  // Mount only ordinary interactive kinds; AI/embed elements receive data-model
  // and public-loader coverage without external DOM/plugin activation.
  const interactive = {...full,elements:full.elements.filter(e=>!["iframe","embeddable","magicframe"].includes(e.type))};
  await page.evaluate(scene=>window.oracle.mount(scene),interactive);
  await page.waitForFunction(()=>window.editor?.getSceneElements().length>1);
  await page.waitForFunction(()=>Object.keys(window.editor.getFiles()).includes("pixel"));
  const imageDecoded = await page.evaluate(async url=>{ const image=new Image(); image.src=url; await image.decode(); return image.naturalWidth; },full.files.pixel.dataURL);
  assert.equal(imageDecoded,1);
  await page.evaluate(()=>window.editor.updateScene({appState:{scrollX:200,scrollY:150,zoom:{value:1}}}));
  await page.waitForFunction(()=>window.editor.getAppState().scrollX===200);
  // Native pointer movement of a rectangle, using actual restored viewport state.
  const location = await page.evaluate(()=>{ const e=window.editor.getSceneElements().find(e=>e.type==="rectangle"), s=window.editor.getAppState(); return {id:e.id,x:e.x,y:e.y,sx:(e.x+e.width/2+s.scrollX)*s.zoom.value+s.offsetLeft,sy:(e.y+e.height/2+s.scrollY)*s.zoom.value+s.offsetTop}; });
  await page.mouse.click(location.sx,location.sy);
  await page.mouse.move(location.sx,location.sy);
  await page.mouse.down(); await page.mouse.move(location.sx+30,location.sy+20,{steps:5}); await page.mouse.up();
  await page.waitForFunction(({id,x})=>window.editor.getSceneElements().find(e=>e.id===id).x!==x,location);
  const edited = await page.evaluate(()=>window.oracle.save());
  const reread = await page.evaluate(scene=>window.oracle.load(scene),edited);
  assert.equal(reread.elements.find(e=>e.id===location.id).x,edited.elements.find(e=>e.id===location.id).x);
  assert.deepEqual(reread.files.pixel,full.files.pixel);
  const items=[{id:"fixture-library",status:"unpublished",created:1,elements:restored.elements.filter(e=>["rectangle","text","arrow"].includes(e.type))}];
  const library=await page.evaluate(items=>window.oracle.library(items),items);
  assert.deepEqual(library.libraryItems,items);
  report.persistedKinds=restored.elements.map(e=>e.type);
  report.imageDecode=true; report.pointerEditSaveReopen=true; report.librarySerialization=true;
  const relationships=relationshipCorpus(snapshot);
  const relationshipRoundtrip=runRust(repo, "roundtrip", [], relationships);
  assert.deepEqual(relationshipRoundtrip,relationships);
  await page.evaluate(scene=>window.oracle.mount(scene),relationshipRoundtrip);
  await page.waitForFunction(()=>window.editor?.getSceneElements().some(e=>e.id==="elbow"));
  await page.evaluate(()=>window.editor.updateScene({appState:{scrollX:0,scrollY:0,zoom:{value:1},selectedElementIds:{box:true}}}));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.box);
  await page.mouse.click(370,200);
  const beforeRelationshipEdit=await page.evaluate(()=>window.oracle.save());
  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(x=>window.editor.getSceneElements().find(e=>e.id==="box").x>x,beforeRelationshipEdit.elements.find(e=>e.id==="box").x);
  const movedRelationships=await page.evaluate(()=>window.oracle.save());
  assert.ok(movedRelationships.elements.find(e=>e.id==="label").x>beforeRelationshipEdit.elements.find(e=>e.id==="label").x);
  assert.equal(movedRelationships.elements.find(e=>e.id==="leader").endBinding.elementId,"box");
  await page.keyboard.press("Escape");
  const labelPosition=await page.evaluate(()=>{const e=window.editor.getSceneElements().find(e=>e.id==="label"),s=window.editor.getAppState();return [(e.x+e.width/2+s.scrollX)*s.zoom.value+s.offsetLeft,(e.y+e.height/2+s.scrollY)*s.zoom.value+s.offsetTop];});
  await page.mouse.dblclick(...labelPosition);
  const textarea=page.locator("textarea.excalidraw-wysiwyg");
  await textarea.waitFor(); await textarea.fill("Edited bound label"); await textarea.press("Escape");
  await page.waitForFunction(()=>window.editor.getSceneElements().find(e=>e.id==="label").originalText==="Edited bound label");
  const finalRelationships=await page.evaluate(()=>window.oracle.save());
  const reopenedRelationships=await page.evaluate(scene=>window.oracle.load(scene),finalRelationships);
  for(const id of ["label","leader","elbow"]) {
    const expected=finalRelationships.elements.find(e=>e.id===id),actual=reopenedRelationships.elements.find(e=>e.id===id);
    for(const field of ["containerId","startBinding","endBinding","fixedSegments","startIsSpecial","endIsSpecial","originalText"]) assert.deepEqual(actual[field],expected[field]);
  }
  if(snapshot) {
    const sticky=reopenedRelationships.elements.find(e=>e.id==="sticky");
    const text=reopenedRelationships.elements.find(e=>e.id==="sticky-label");
    assert.equal(sticky.baseHeight,120); assert.equal(text.baseFontSize,20); assert.equal(text.containerId,"sticky");
  }
  report.relationships={boundContainerMove:true,nativeBoundTextEdit:true,elbowBindingSaveReopen:true,stickyPairLoadSave:snapshot};
  const output = process.env.DOCUMENT_RESULTS || path.join(repo,"compatibility/results",`document-${snapshot?"snapshot":"release"}-${Date.now()}`);
  await mkdir(output,{recursive:true});
  if(process.env.DOCUMENT_EXTENDED==="1") {
    let stage=0;
    try {
      report.extended=await extended(page,repo,snapshot,async(name,value)=>
        writeFile(path.join(output,`extended-${String(stage++).padStart(2,"0")}-${name}.json`),JSON.stringify(value,null,2)));
      await writeFile(path.join(output,"extended-report.json"),JSON.stringify(report.extended,null,2));
    } catch(error) {
      await writeFile(path.join(output,"extended-failure.txt"),error.stack || String(error));
      // Diagnostics are best effort; retain the original assertion failure.
      const scene=await page.evaluate(()=>window.oracle.save()).catch(()=>null);
      if(scene!==null)await writeFile(path.join(output,"extended-failure-scene.json"),JSON.stringify(scene,null,2));
      await page.screenshot({path:path.join(output,"extended-failure.png"),timeout:5000}).catch(()=>{});
      throw error;
    }
  }
  const authoredResult = await authored(page,repo,snapshot);
  report.authored = authoredResult.checks;
  // Write each gallery stage immediately so a failed native interaction retains
  // its Rust input and the last completed load/edit boundary for diagnosis.
  try {
    report.authoredGallery = await authoredGallery(page,repo,snapshot,async(name,value)=>
      writeFile(path.join(output,`authored-gallery-${name}.json`),JSON.stringify(value,null,2)));
  } catch(error) {
    await writeFile(path.join(output,"authored-gallery-failure.txt"),error.stack || String(error));
    await writeFile(path.join(output,"authored-gallery-failure-scene.json"),JSON.stringify(await page.evaluate(()=>window.oracle.save()),null,2));
    await page.screenshot({path:path.join(output,"authored-gallery-failure.png")});
    console.error(`Authored gallery failed; evidence ${output}`);
    throw error;
  }
  await writeFile(path.join(output,"report.json"),JSON.stringify(report,null,2));
  await writeFile(path.join(output,"corpus.json"),JSON.stringify(full,null,2));
  await writeFile(path.join(output,"edited.json"),JSON.stringify(edited,null,2));
  await writeFile(path.join(output,"relationships.json"),JSON.stringify(finalRelationships,null,2));
  await writeFile(path.join(output,"authored.json"),JSON.stringify(authoredResult.input,null,2));
  await writeFile(path.join(output,"authored-edited.json"),JSON.stringify(authoredResult.saved,null,2));
  await page.screenshot({path:path.join(output,"scene.png")});
  console.log(`Document oracle passed: ${report.profile}; evidence ${output}`);
} finally { await browser?.close(); await server.close(); }
